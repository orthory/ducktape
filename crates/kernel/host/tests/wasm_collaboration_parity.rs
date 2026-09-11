//! the STORE-BACKED cutover-continuity proof for collaboration: the guest
//! component over `WasmModule::with_store(QmdbStore)` and the native
//! `Collaboration` over the same store shape are ROOT-CONTINUOUS — the same op
//! sequence commits the IDENTICAL qmdb merkle root after every block (both
//! roots ARE the store's root), and the same refusals leave both roots
//! untouched.
//!
//! The READ half is the part that needs a proof at all here, because this
//! module's whole read surface is authenticated from the query CONTEXT: a
//! protected read resolves its caller from `ctx.env().origin` and from nothing
//! else. So the proof drives reads BOTH ways on both runtimes:
//!
//! * `Host::query` — the node's public lane, which builds `Origin::System`.
//!   Both runtimes must answer `Denied(Unauthenticated)`.
//! * a `Prober` module's in-dispatch `ctx.query`, which carries the submitting
//!   origin. Both runtimes must serve the SAME reply bytes — that pins
//!   `WasmModule::query_with` threading the real env through the guest's query
//!   export, which a `query`-only lane would not.
//!
//! identity, tasks and chat stand as fixed-reply siblings on both hosts: the
//! module reads them (actor resolution, task-attempt fencing, channel access
//! and the message a delivery names) and they must answer identically on both
//! sides or the comparison would be measuring them.

use std::cell::RefCell;
use std::rc::Rc;

use collaboration::{
    BoundPrincipal, Collaboration, CollaborationMsg, CollaborationQuery, DeliverRequest,
    DeliveryState, MessageKind, Party, ProtectedRead, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, SubmitError};
use sdk::{Ctx, Error, MerkleStore as _, Module, ModuleId, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `collaboration` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const COLLABORATION_WASM: &[u8] = include_bytes!("fixtures/collaboration.component.wasm");

const MODULE: &str = "collaboration";
/// the height lane's ceiling — the unit this proof's blocks advance in.
const TTL: u64 = collaboration::max_delivery_ttl(sdk::genesis_config::TimeUnit::Height);
/// the service key alice's binding authorizes. It posts the messages below
/// and asks for their delivery, never signing with the owner key.
const SERVICE: u8 = 0x5e;
/// bob's own bound service. A delivery record is the RECIPIENT's, so only
/// bob's binding may advance it — alice's service key asks and cannot
/// acknowledge.
const SERVICE_B: u8 = 0x5f;
/// the network both runtimes are composed with — every op names it, and a
/// wasm genesis gets it as the `chain_id` config parameter.
const NETWORK: &str = "parity-net";
/// the chat sequences the stub assigns to `m1` and `m2`: the delivery record
/// is keyed by the CHAT sequence of the message, not by the collaboration
/// event that requested it.
const M1_SEQ: u64 = 1;
const M2_SEQ: u64 = 2;
/// posted but never requested: the refusals that must not hit the
/// idempotent-repeat check name this one.
const M3_SEQ: u64 = 3;

/// a fixed-reply sibling: enough of `identity` and `tasks` for the module's
/// two cross-module reads to resolve, identically on both runtimes.
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

/// every external origin resolves to `Party::Key`, the non-account principal —
/// the module's ownership check then compares raw keys on both runtimes.
fn identity_stub() -> Stub {
    Stub {
        id: "identity".into(),
        reply: identity::encode_reply(&identity::IdentityReply::Account(None)),
    }
}

fn tasks_stub() -> Stub {
    Stub {
        id: "tasks".into(),
        reply: tasks::encode_job_reply(&tasks::JobsReply::Job(None)),
    }
}

fn party(byte: u8) -> Party {
    Party::Key(vec![byte; 32])
}

/// a chat sibling holding ONE open channel, `c1`, and three messages in it —
/// `m1`, `m2` and `m3` at sequences 1, 2 and 3, all posted by alice's service
/// key. Every party may read `c1` and nothing else exists. Its writes (the
/// seating follow-ups a bind emits) are accepted and discarded, identically
/// on both runtimes.
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
            chat::ChatQuery::Access { channel_id, .. } => {
                let open = channel_id == "c1";
                chat::ChatReply::Access(chat::ChannelAccess {
                    may_read: open,
                    may_post: open,
                })
            }
            chat::ChatQuery::Message { message_id } => {
                let seq = match message_id.as_str() {
                    "m1" => Some(M1_SEQ),
                    "m2" => Some(M2_SEQ),
                    "m3" => Some(M3_SEQ),
                    _ => None,
                };
                chat::ChatReply::Message(seq.map(|seq| chat::MessageView {
                    channel_id: "c1".into(),
                    seq,
                    head: chat::MessageHead {
                        message_id,
                        author: party(SERVICE),
                        origin: Origin::External(vec![SERVICE; 32]),
                        content_origin: Origin::External(vec![SERVICE; 32]),
                        blocks: vec![chat::Block::paragraph("please review")],
                        created_at: seq,
                        rev: 0,
                        revision: 1,
                        edited_at: None,
                        base_rev: None,
                        deleted: false,
                        thread: None,
                        reply_count: 0,
                        last_reply_seq: None,
                    },
                }))
            }
            other => return Err(Error::Module(format!("unserved {other:?}"))),
        };
        Ok(chat::encode_reply(&reply))
    }
}

/// issues ONE collaboration read per dispatch and records the RAW reply bytes.
/// Its dispatch carries the submitting origin, so this is the authenticated
/// read lane — the only one this module serves.
struct Prober {
    request: Rc<RefCell<Vec<u8>>>,
    seen: Rc<RefCell<Vec<Vec<u8>>>>,
}

#[async_trait::async_trait(?Send)]
impl Module for Prober {
    fn id(&self) -> ModuleId {
        "prober".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        let request = self.request.borrow().clone();
        let bytes = ctx.query(MODULE, &request).await?;
        self.seen.borrow_mut().push(bytes);
        Ok(())
    }
}

/// one runtime under test: its host, and the two handles the prober reads and
/// writes through.
struct Lane {
    host: Host,
    request: Rc<RefCell<Vec<u8>>>,
    seen: Rc<RefCell<Vec<Vec<u8>>>>,
}

/// a fresh qmdb store carrying the seeded `__config` record — exactly the
/// production genesis seam (`bin/node/src/host_state.rs` `seed_store_config`).
/// BOTH runtimes' stores get the identical one: root-continuity demands it,
/// and the guest reads its network binding and its time unit from it, so a
/// store without it would make the wasm side reject every op the native side
/// admits. `label` doubles as the store id — the deterministic runtime keys
/// storage partitions by id alone, so a shared id would replay the first
/// store's journal into the second.
async fn seeded_store(
    context: &deterministic::Context,
    label: &'static str,
) -> QmdbStore<deterministic::Context> {
    let mut store = QmdbStore::init(context.child(label), label).await;
    // strictly increasing keys, which `encode_config` asserts.
    let config = sdk::genesis_config::encode_config(&[
        (sdk::genesis_config::CHAIN_ID, NETWORK.as_bytes()),
        (
            sdk::genesis_config::TIME_UNIT,
            sdk::genesis_config::TimeUnit::Height.encode(),
        ),
    ]);
    store
        .commit_batch(vec![(
            sdk::store_key(sdk::genesis_config::CONFIG_KEY),
            Some(config),
        )])
        .await
        .expect("seed genesis config");
    store
}

impl Lane {
    async fn new(context: &deterministic::Context, label: &'static str, wasm: bool) -> Self {
        let store = seeded_store(context, label).await;
        let module: Box<dyn Module> = match wasm {
            true => Box::new(
                WasmModule::with_store(MODULE, COLLABORATION_WASM, Box::new(store))
                    .expect("load component"),
            ),
            false => Box::new(Collaboration::new(
                MODULE,
                "identity",
                "tasks",
                "chat",
                Box::new(store),
                TTL,
                NETWORK,
            )),
        };
        let request = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let host = Host::genesis(vec![
            module,
            Box::new(identity_stub()),
            Box::new(tasks_stub()),
            Box::new(ChatStub),
            Box::new(Prober {
                request: request.clone(),
                seen: seen.clone(),
            }),
        ])
        .expect("genesis composes");
        Self {
            host,
            request,
            seen,
        }
    }

    fn root(&self) -> StateRoot {
        self.host.module_root(MODULE).expect("collaboration seated")
    }

    /// read through the node's PUBLIC lane, which carries no caller.
    async fn public_read(&self, read: &[u8]) -> Vec<u8> {
        self.host.query(MODULE, read).await.expect("answers")
    }

    /// read through a dispatch `signer` submitted, so the read inherits an
    /// authenticated origin.
    async fn authenticated_read(&mut self, signer: u8, height: u64, read: Vec<u8>) -> Vec<u8> {
        *self.request.borrow_mut() = read;
        self.host
            .submit_at(
                block(signer, height),
                Msg {
                    target: "prober".into(),
                    payload: Vec::new(),
                },
            )
            .await
            .expect("the prober runs");
        self.seen.borrow().last().cloned().expect("a recorded reply")
    }
}

/// one block's agreed context. `consensus_time` is the height lane's, which is
/// the unit `TTL` and every `expires_at` below are in.
fn block(signer: u8, height: u64) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin: Origin::External(vec![signer; 32]),
    }
}

fn msg(payload: CollaborationMsg) -> Msg {
    Msg {
        target: MODULE.into(),
        payload: encode_msg(&collaboration::Request::new(NETWORK, payload)),
    }
}

fn deliver(message_id: &str, kind: MessageKind, expires_at: u64) -> Msg {
    msg(CollaborationMsg::Deliver(DeliverRequest {
        channel_id: "c1".into(),
        message_id: message_id.into(),
        recipient: party(2),
        kind,
        task: None,
        references: Vec::new(),
        expires_at,
    }))
}

/// the accepted op matrix, each entry the signer and the op: two scoped
/// bindings, two delivery requests from alice's service key for messages it
/// posted, and the delivery walk to a terminal state under bob's.
fn accepted() -> Vec<(u8, Msg)> {
    vec![
        (
            1,
            msg(CollaborationMsg::Bind {
                channel_id: "c1".into(),
                participant: party(1),
                device: "laptop".into(),
                principal: BoundPrincipal::ServiceKey(vec![SERVICE; 32]),
                expected_credential: 0,
            }),
        ),
        (
            2,
            msg(CollaborationMsg::Bind {
                channel_id: "c1".into(),
                participant: party(2),
                device: "laptop".into(),
                principal: BoundPrincipal::ServiceKey(vec![SERVICE_B; 32]),
                expected_credential: 0,
            }),
        ),
        (SERVICE, deliver("m1", MessageKind::Notice, 400)),
        (SERVICE, deliver("m2", MessageKind::Notice, 400)),
        (
            SERVICE_B,
            msg(CollaborationMsg::Acknowledge {
                channel_id: "c1".into(),
                seq: M1_SEQ,
                recipient: party(2),
                binding_credential: 1,
                state: DeliveryState::Queued,
                reason: None,
            }),
        ),
        (
            SERVICE_B,
            msg(CollaborationMsg::Acknowledge {
                channel_id: "c1".into(),
                seq: M1_SEQ,
                recipient: party(2),
                binding_credential: 1,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            }),
        ),
    ]
}

/// the refusal matrix: an op addressed to a DIFFERENT network, a repeated
/// request with different metadata, a deadline past the ceiling, a message
/// chat does not hold, and a key that did not post the message. Each must be
/// refused on both runtimes with the same reason and stage nothing.
///
/// the first and third carry the whole weight of the guest's genesis config:
/// the network refusal proves it read `chain_id` as THIS network rather than
/// accepting anything, and the ceiling refusal proves it read `time_unit` as
/// the height lane (a millisecond reading would admit that deadline).
fn refused() -> Vec<(u8, Msg, &'static str)> {
    vec![
        (
            SERVICE,
            Msg {
                target: MODULE.into(),
                payload: encode_msg(&collaboration::Request::new(
                    "another-net",
                    CollaborationMsg::Deliver(DeliverRequest {
                        channel_id: "c1".into(),
                        message_id: "m2".into(),
                        recipient: party(2),
                        kind: MessageKind::Notice,
                        task: None,
                        references: Vec::new(),
                        expires_at: 400,
                    }),
                )),
            },
            "not this network",
        ),
        (
            SERVICE,
            deliver("m1", MessageKind::Question, 400),
            "different metadata",
        ),
        (
            SERVICE,
            deliver("m3", MessageKind::Notice, 400 + TTL),
            "more than",
        ),
        (
            SERVICE,
            deliver("m9", MessageKind::Notice, 400),
            "no chat message",
        ),
        (
            9,
            deliver("m3", MessageKind::Notice, 400),
            "not posted by this origin",
        ),
    ]
}

/// the reads compared across the runtimes, each as (signer, read).
/// `via: Some("c1")` is how a channel-scoped service key names the binding it
/// acts under; the owner key uses `None`.
fn reads() -> Vec<(u8, Vec<u8>)> {
    let scoped = |participant: Party, read: ProtectedRead| {
        encode_query(&CollaborationQuery::Read {
            participant,
            via: Some("c1".into()),
            read,
        })
    };
    let owned = |participant: Party, read: ProtectedRead| {
        encode_query(&CollaborationQuery::Read {
            participant,
            via: None,
            read,
        })
    };
    vec![
        (
            SERVICE,
            scoped(
                party(1),
                ProtectedRead::Events {
                    channel_id: "c1".into(),
                    from_seq: 1,
                    limit: 16,
                },
            ),
        ),
        (
            SERVICE_B,
            scoped(
                party(2),
                ProtectedRead::Delivery {
                    channel_id: "c1".into(),
                    seq: M1_SEQ,
                },
            ),
        ),
        (
            SERVICE_B,
            scoped(
                party(2),
                ProtectedRead::DeliveryEligibility {
                    channel_id: "c1".into(),
                    seq: M2_SEQ,
                },
            ),
        ),
        (
            SERVICE,
            scoped(
                party(1),
                ProtectedRead::Binding {
                    channel_id: "c1".into(),
                },
            ),
        ),
        // the participant-wide projection is the OWNER's: the same request
        // must be answered for key 2 and denied for the service key, on both
        // runtimes.
        (2, owned(party(2), ProtectedRead::Mailbox)),
        (SERVICE_B, scoped(party(2), ProtectedRead::Mailbox)),
        // and a channel the caller may not read.
        (
            1,
            owned(
                party(1),
                ProtectedRead::Events {
                    channel_id: "absent".into(),
                    from_seq: 1,
                    limit: 16,
                },
            ),
        ),
    ]
}

#[test]
fn same_ops_same_roots_and_the_same_authenticated_replies() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = Lane::new(&context, "native_collaboration", false).await;
        let mut wasm = Lane::new(&context, "wasm_collaboration", true).await;

        // ROOT-CONTINUITY from GENESIS: both roots are the (empty) store's
        // merkle root, identical across the runtimes.
        assert_eq!(
            native.root(),
            wasm.root(),
            "genesis roots must be continuous across the runtimes"
        );

        for (index, (signer, op)) in accepted().into_iter().enumerate() {
            let height = index as u64 + 1;
            let (n_before, w_before) = (native.root(), wasm.root());
            native
                .host
                .submit_at(block(signer, height), op.clone())
                .await
                .unwrap_or_else(|e| panic!("native submit at {height}: {e:?}"));
            wasm.host
                .submit_at(block(signer, height), op)
                .await
                .unwrap_or_else(|e| panic!("wasm submit at {height}: {e:?}"));
            assert_ne!(native.root(), n_before, "native root stuck at {height}");
            assert_ne!(wasm.root(), w_before, "wasm root stuck at {height}");
            assert_eq!(
                native.root(),
                wasm.root(),
                "the two runtimes diverged at block {height}"
            );
        }

        // the PUBLIC lane carries no caller on either runtime.
        let denied = collaboration::encode_reply(&collaboration::CollaborationReply::Denied(
            collaboration::DenyReason::Unauthenticated,
        ));
        for (_, read) in reads() {
            assert_eq!(
                native.public_read(&read).await,
                denied,
                "the native module served the unauthenticated lane"
            );
            assert_eq!(
                wasm.public_read(&read).await,
                denied,
                "the wasm port served the unauthenticated lane"
            );
        }

        // the AUTHENTICATED lane: byte-identical replies, and reads never move
        // a root.
        let (n_settled, w_settled) = (native.root(), wasm.root());
        for (index, (signer, read)) in reads().into_iter().enumerate() {
            let height = 100 + index as u64;
            let n = native.authenticated_read(signer, height, read.clone()).await;
            let w = wasm.authenticated_read(signer, height, read).await;
            assert_eq!(
                n, w,
                "the runtimes answered read {index} differently — native {n:?} vs wasm {w:?}"
            );
            assert_ne!(
                n, denied,
                "read {index} must reach the module as an authenticated caller"
            );
        }
        assert_eq!(native.root(), n_settled, "a read moved the native root");
        assert_eq!(wasm.root(), w_settled, "a read moved the wasm root");
    });
}

#[test]
fn the_same_refusals_reject_identically_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = Lane::new(&context, "native_refusals", false).await;
        let mut wasm = Lane::new(&context, "wasm_refusals", true).await;
        for (index, (signer, op)) in accepted().into_iter().enumerate() {
            let height = index as u64 + 1;
            native
                .host
                .submit_at(block(signer, height), op.clone())
                .await
                .expect("native setup");
            wasm.host
                .submit_at(block(signer, height), op)
                .await
                .expect("wasm setup");
        }

        for (index, (signer, op, needle)) in refused().into_iter().enumerate() {
            let height = 50 + index as u64;
            let (n_before, w_before) = (native.root(), wasm.root());
            let n_err = native
                .host
                .submit_at(block(signer, height), op.clone())
                .await
                .expect_err("native must reject");
            let w_err = wasm
                .host
                .submit_at(block(signer, height), op)
                .await
                .expect_err("wasm must reject");
            let SubmitError::Rejected(Error::Module(n_msg)) = n_err else {
                panic!("native rejection shape: {n_err:?}");
            };
            let SubmitError::Rejected(Error::Module(w_msg)) = w_err else {
                panic!("wasm rejection shape: {w_err:?}");
            };
            assert!(n_msg.contains(needle), "native reason: {n_msg}");
            assert!(
                w_msg.contains(needle),
                "wasm reason must carry the native reason: {w_msg}"
            );
            assert_eq!(native.root(), n_before, "native root moved on reject");
            assert_eq!(wasm.root(), w_before, "wasm root moved on reject");
            assert_eq!(native.root(), wasm.root(), "diverged on refusal {index}");
        }
    });
}

#[test]
fn a_detached_recipients_history_is_readable_but_new_delivery_is_unbound_on_both_runtimes() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = Lane::new(&context, "native_detached", false).await;
        let mut wasm = Lane::new(&context, "wasm_detached", true).await;
        for (index, (signer, op)) in accepted().into_iter().enumerate() {
            let height = index as u64 + 1;
            native
                .host
                .submit_at(block(signer, height), op.clone())
                .await
                .unwrap();
            wasm.host
                .submit_at(block(signer, height), op)
                .await
                .unwrap();
        }
        let op = msg(CollaborationMsg::Unbind {
            channel_id: "c1".into(),
            participant: party(2),
            expected_credential: 1,
        });
        native
            .host
            .submit_at(block(2, 20), op.clone())
            .await
            .unwrap();
        wasm.host.submit_at(block(2, 20), op).await.unwrap();
        assert_eq!(native.root(), wasm.root());
        for (index, read) in [
            ProtectedRead::Delivery {
                channel_id: "c1".into(),
                seq: M2_SEQ,
            },
            ProtectedRead::DeliveryEligibility {
                channel_id: "c1".into(),
                seq: M2_SEQ,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let history = matches!(read, ProtectedRead::Delivery { .. });
            let request = encode_query(&CollaborationQuery::Read {
                participant: party(2),
                via: None,
                read,
            });
            let n = native
                .authenticated_read(2, 21 + index as u64, request.clone())
                .await;
            let w = wasm.authenticated_read(2, 21 + index as u64, request).await;
            assert_eq!(n, w);
            let reply = collaboration::decode_reply(&n).unwrap();
            if history {
                assert!(matches!(
                    reply,
                    collaboration::CollaborationReply::Delivery(Some(_))
                ));
            } else {
                assert_eq!(
                    reply,
                    collaboration::CollaborationReply::Eligibility(
                        collaboration::DeliveryEligibility::Unbound
                    )
                );
            }
        }
    });
}
