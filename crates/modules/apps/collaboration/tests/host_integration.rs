//! collaboration under the REAL host lifecycle: origin-threaded blocks over a
//! real qmdb store, rollback on failure, and — the point of this file — the
//! two read lanes side by side.
//!
//! `Host::query` is the node's public read lane and builds `Origin::System`,
//! so every protected read over it must fail closed. A read issued from
//! INSIDE a dispatch inherits that dispatch's authenticated origin, so the
//! same request served that way is answered. Both are exercised here against
//! the real host, not a test double of it.

use std::cell::RefCell;
use std::rc::Rc;

use collaboration::{
    decode_reply, encode_msg, encode_query, Collaboration, CollaborationMsg, CollaborationQuery,
    CollaborationReply, DenyReason, MessageId, MessageKind, ProtectedRead, Role, SendRequest,
};
use commonware_runtime::{deterministic, Runner as _};
use host::{BlockContext, Host};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;

const MODULE: &str = "collaboration";
const TTL: u64 = collaboration::HEIGHT_LANE_MAX_DELIVERY_TTL;

fn as_user(byte: u8, height: u64) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin: Origin::External(vec![byte; 32]),
    }
}

fn msg(payload: CollaborationMsg) -> Msg {
    Msg {
        target: MODULE.into(),
        payload: encode_msg(&payload),
    }
}

/// a fixed-reply sibling: enough of `identity` and `tasks` for the module's
/// two cross-module reads to resolve under the real host's query routing.
struct Stub {
    id: ModuleId,
    reply: Vec<u8>,
}

#[async_trait::async_trait(?Send)]
impl Module for Stub {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Err(Error::Module("this stub only answers reads".into()))
    }
    async fn query(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(self.reply.clone())
    }
}

fn identity_stub() -> Stub {
    Stub {
        id: "identity".into(),
        // no account for any key: every external origin resolves to
        // `Party::Key`, the non-account principal.
        reply: identity::encode_reply(&identity::IdentityReply::Account(None)),
    }
}

fn tasks_stub() -> Stub {
    Stub {
        id: "tasks".into(),
        reply: tasks::encode_job_reply(&tasks::JobsReply::Job(None)),
    }
}

/// a module that, when dispatched, issues ONE collaboration read and records
/// the reply. Its dispatch carries the submitting origin, so this is the
/// authenticated read lane exercised through the real host.
struct Prober {
    id: ModuleId,
    request: Vec<u8>,
    seen: Rc<RefCell<Vec<CollaborationReply>>>,
}

#[async_trait::async_trait(?Send)]
impl Module for Prober {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        let bytes = ctx.query(MODULE, &self.request).await?;
        self.seen
            .borrow_mut()
            .push(decode_reply(&bytes).map_err(Error::Module)?);
        Ok(())
    }
}

async fn genesis(context: commonware_runtime::deterministic::Context, prober: Prober) -> Host {
    let store = QmdbStore::init(context, MODULE).await;
    Host::genesis(vec![
        Box::new(Collaboration::new(
            MODULE,
            "identity",
            "tasks",
            Box::new(store),
            TTL,
        )),
        Box::new(identity_stub()),
        Box::new(tasks_stub()),
        Box::new(prober),
    ])
    .expect("genesis composes")
}

/// alice registers herself and a conversation, seats herself and bob, and
/// sends bob one notice. Returns the host.
async fn conversation_with_one_message(host: &mut Host) {
    host.submit_at(
        as_user(1, 1),
        msg(CollaborationMsg::RegisterParticipant {
            participant_id: "alice".into(),
            display_name: "alice".into(),
            agent_account: None,
        }),
    )
    .await
    .expect("alice registers");
    host.submit_at(
        as_user(2, 1),
        msg(CollaborationMsg::RegisterParticipant {
            participant_id: "bob".into(),
            display_name: "bob".into(),
            agent_account: None,
        }),
    )
    .await
    .expect("bob registers");
    host.submit_at(
        as_user(1, 2),
        msg(CollaborationMsg::CreateConversation {
            conversation_id: "c1".into(),
            topic: "review".into(),
        }),
    )
    .await
    .expect("the conversation is created");
    for who in ["alice", "bob"] {
        host.submit_at(
            as_user(1, 3),
            msg(CollaborationMsg::SetRoster {
                conversation_id: "c1".into(),
                participant_id: who.into(),
                role: Some(Role::Member),
            }),
        )
        .await
        .expect("the roster is set");
    }
    host.submit_at(
        as_user(1, 4),
        msg(CollaborationMsg::Send(SendRequest {
            conversation_id: "c1".into(),
            sender_participant_id: "alice".into(),
            message_id: MessageId {
                generation: 1,
                sequence: 1,
            },
            recipient_participant_id: "bob".into(),
            kind: MessageKind::Notice,
            reply_to: None,
            task: None,
            body: "please review".into(),
            expires_at: 500,
            references: Vec::new(),
        })),
    )
    .await
    .expect("the message is admitted");
}

fn events_read() -> Vec<u8> {
    encode_query(&CollaborationQuery::Read {
        participant_id: "alice".into(),
        via: None,
        read: ProtectedRead::Events {
            conversation_id: "c1".into(),
            from_seq: 1,
            limit: 16,
        },
    })
}

#[test]
fn the_hosts_public_query_lane_reads_nothing_and_a_dispatch_read_is_served() {
    deterministic::Runner::default().start(|context| async move {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let prober = Prober {
            id: "prober".into(),
            request: events_read(),
            seen: seen.clone(),
        };
        let mut host = genesis(context, prober).await;
        let root0 = host.module_root(MODULE).unwrap();
        let app0 = host.root_hash();

        conversation_with_one_message(&mut host).await;
        assert_ne!(host.module_root(MODULE).unwrap(), root0);
        assert_ne!(host.root_hash(), app0);

        // the PUBLIC lane: `Host::query` builds Origin::System, so the module
        // has no authenticated caller and answers nothing.
        let bytes = host.query(MODULE, &events_read()).await.expect("answers");
        assert_eq!(
            decode_reply(&bytes).unwrap(),
            CollaborationReply::Denied(DenyReason::Unauthenticated),
            "the node's unauthenticated read lane must not serve conversation content"
        );

        // the AUTHENTICATED lane: the same request from inside a dispatch that
        // alice's key submitted. The read inherits that origin and is served.
        host.submit_at(
            as_user(1, 5),
            Msg {
                target: "prober".into(),
                payload: Vec::new(),
            },
        )
        .await
        .expect("the prober runs");
        {
            let served = seen.borrow();
            let [CollaborationReply::Events(page)] = served.as_slice() else {
                panic!("the authenticated read is served: {served:?}");
            };
            let collaboration::EventPage::Page { messages, .. } = page else {
                panic!("a page, not a gap");
            };
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].body, "please review");
            assert_eq!(messages[0].sender, "alice");
        }

        // and bob's key gets nothing when it asks to read as alice, through
        // the very same lane.
        host.submit_at(
            as_user(2, 6),
            Msg {
                target: "prober".into(),
                payload: Vec::new(),
            },
        )
        .await
        .expect("the prober runs");
        assert_eq!(
            seen.borrow()[1],
            CollaborationReply::Denied(DenyReason::NotReader),
            "the dispatch origin decides, not the participant_id in the payload"
        );
    });
}

#[test]
fn a_refused_op_rolls_the_block_back_and_leaves_the_root_untouched() {
    deterministic::Runner::default().start(|context| async move {
        let prober = Prober {
            id: "prober".into(),
            request: events_read(),
            seen: Rc::new(RefCell::new(Vec::new())),
        };
        let mut host = genesis(context, prober).await;
        conversation_with_one_message(&mut host).await;
        let settled = host.module_root(MODULE).unwrap();
        let app = host.root_hash();

        // bob's key naming alice as the sender: refused at admission.
        let refusal = host
            .submit_at(
                as_user(2, 7),
                msg(CollaborationMsg::Send(SendRequest {
                    conversation_id: "c1".into(),
                    sender_participant_id: "alice".into(),
                    message_id: MessageId {
                        generation: 1,
                        sequence: 2,
                    },
                    recipient_participant_id: "bob".into(),
                    kind: MessageKind::Notice,
                    reply_to: None,
                    task: None,
                    body: "not from alice".into(),
                    expires_at: 500,
                    references: Vec::new(),
                })),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not authorized to send as"),
            "{refusal:?}"
        );
        assert_eq!(
            host.module_root(MODULE).unwrap(),
            settled,
            "a refused op stages nothing"
        );
        assert_eq!(host.root_hash(), app);
    });
}
