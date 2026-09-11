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
    Collaboration, CollaborationMsg, CollaborationQuery, CollaborationReply, DenyReason, Party,
    ProtectedRead, decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, deterministic};
use host::{BlockContext, Host};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;

const MODULE: &str = "collaboration";
const TTL: u64 = collaboration::max_delivery_ttl(sdk::genesis_config::TimeUnit::Height);
const NETWORK: &str = "test-net";

fn as_user(byte: u8, height: u64) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin: Origin::External(vec![byte; 32]),
    }
}

fn party(byte: u8) -> Party {
    Party::Key(vec![byte; 32])
}

fn msg(payload: CollaborationMsg) -> Msg {
    Msg {
        target: MODULE.into(),
        payload: encode_msg(&collaboration::Request::new(NETWORK, payload)),
    }
}

/// a fixed-reply sibling: enough of `identity` and `tasks` for the module's
/// cross-module reads to resolve under the real host's query routing.
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

/// a chat sibling holding ONE channel, `c1`, whose members are keys 1 and 2,
/// and ONE message, `m1`, posted by key 1 at sequence 1. its writes (the
/// seating follow-ups a bind emits) are accepted and discarded.
struct ChatStub;

#[async_trait::async_trait(?Send)]
impl Module for ChatStub {
    fn id(&self) -> ModuleId {
        "chat".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Ok(())
    }
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let reply = match chat::decode_query(req).map_err(Error::Module)? {
            chat::ChatQuery::Access { channel_id, party } => {
                let member = channel_id == "c1" && (party == party_of(1) || party == party_of(2));
                chat::ChatReply::Access(chat::ChannelAccess {
                    may_read: member,
                    may_post: member,
                })
            }
            chat::ChatQuery::Message { message_id } => chat::ChatReply::Message(
                (message_id == "m1").then(|| chat::MessageView {
                    channel_id: "c1".into(),
                    seq: 1,
                    head: chat::MessageHead {
                        message_id: "m1".into(),
                        author: party_of(1),
                        origin: Origin::External(vec![1; 32]),
                        content_origin: Origin::External(vec![1; 32]),
                        blocks: vec![chat::Block::paragraph("please review")],
                        created_at: 1,
                        rev: 0,
                        revision: 1,
                        edited_at: None,
                        base_rev: None,
                        deleted: false,
                        thread: None,
                        reply_count: 0,
                        last_reply_seq: None,
                    },
                }),
            ),
            other => return Err(Error::Module(format!("unserved {other:?}"))),
        };
        Ok(chat::encode_reply(&reply))
    }
}

fn party_of(byte: u8) -> Party {
    party(byte)
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
            "chat",
            Box::new(store),
            TTL,
            NETWORK,
        )),
        Box::new(identity_stub()),
        Box::new(tasks_stub()),
        Box::new(ChatStub),
        Box::new(prober),
    ])
    .expect("genesis composes")
}

/// bob binds a service key on `c1`, and alice asks that her chat message `m1`
/// be delivered to him.
async fn one_delivery(host: &mut Host) {
    host.submit_at(
        as_user(2, 1),
        msg(CollaborationMsg::Bind {
            channel_id: "c1".into(),
            participant: party(2),
            device: "laptop".into(),
            principal: collaboration::BoundPrincipal::ServiceKey(vec![0x5f; 32]),
            expected_credential: 0,
        }),
    )
    .await
    .expect("bob binds");
    host.submit_at(
        as_user(1, 4),
        msg(CollaborationMsg::Deliver(collaboration::DeliverRequest {
            channel_id: "c1".into(),
            message_id: "m1".into(),
            recipient: party(2),
            kind: collaboration::MessageKind::Notice,
            task: None,
            references: Vec::new(),
            expires_at: 500,
        })),
    )
    .await
    .expect("the delivery is requested");
}

fn events_read() -> Vec<u8> {
    encode_query(&CollaborationQuery::Read {
        participant: party(1),
        via: None,
        read: ProtectedRead::Events {
            channel_id: "c1".into(),
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

        one_delivery(&mut host).await;
        assert_ne!(host.module_root(MODULE).unwrap(), root0);
        assert_ne!(host.root_hash(), app0);

        // the PUBLIC lane: `Host::query` builds Origin::System, so the module
        // has no authenticated caller and answers nothing.
        let bytes = host.query(MODULE, &events_read()).await.expect("answers");
        assert_eq!(
            decode_reply(&bytes).unwrap(),
            CollaborationReply::Denied(DenyReason::Unauthenticated),
            "the node's unauthenticated read lane must not serve delivery records"
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
            assert_eq!(page.deliveries.len(), 1);
            assert_eq!(page.deliveries[0].message_id, "m1");
            assert_eq!(page.deliveries[0].sender, party(1));
            assert_eq!(page.deliveries[0].recipient, party(2));
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
            "the dispatch origin decides, not the participant in the payload"
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
        one_delivery(&mut host).await;
        let settled = host.module_root(MODULE).unwrap();
        let app = host.root_hash();

        // bob asking to deliver alice's message: not his words, refused.
        let refusal = host
            .submit_at(
                as_user(2, 7),
                msg(CollaborationMsg::Deliver(collaboration::DeliverRequest {
                    channel_id: "c1".into(),
                    message_id: "m1".into(),
                    recipient: party(1),
                    kind: collaboration::MessageKind::Notice,
                    task: None,
                    references: Vec::new(),
                    expires_at: 500,
                })),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not posted by this origin"),
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
