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
//! identity and tasks stand as fixed-reply siblings on both hosts: the module
//! reads them (actor resolution, task-attempt fencing) and they must answer
//! identically on both sides or the comparison would be measuring them.

use std::cell::RefCell;
use std::rc::Rc;

use collaboration::{
    BoundPrincipal, Collaboration, CollaborationMsg, CollaborationQuery, DeliveryState, MessageId,
    MessageKind, ProtectedRead, Role, SendRequest, encode_msg, encode_query,
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
/// the service key alice's binding authorizes. sends and reads below sign with
/// THIS, never with the owner key.
const SERVICE: u8 = 0x5e;
/// bob's own bound service. A receipt is the RECIPIENT's record, so only bob's
/// binding may advance it — alice's service key sends and cannot acknowledge.
const SERVICE_B: u8 = 0x5f;
/// the network both runtimes are composed with — every op names it, and a
/// wasm genesis gets it as the `chain_id` config parameter.
const NETWORK: &str = "parity-net";
/// the conversation's ONE event sequence covers roster edits and binding
/// changes too, so the first message lands after them: seating alice took 1,
/// seating bob 2, alice's binding 3, bob's 4, and the first send 5.
const FIRST_MESSAGE_SEQ: u64 = 5;

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

fn send(sequence: u64, body: &str, expires_at: u64) -> Msg {
    msg(CollaborationMsg::Send(SendRequest {
        conversation_id: "c1".into(),
        sender_participant_id: "alice".into(),
        // the credential alice's ONE binding drew — a participant's first is 2
        // (1 is its owner credential), and a send must name the credential its
        // origin actually authenticates with.
        message_id: MessageId {
            generation: 2,
            sequence,
        },
        recipient_participant_id: "bob".into(),
        kind: MessageKind::Notice,
        reply_to: None,
        task: None,
        body: body.into(),
        expires_at,
        references: Vec::new(),
    }))
}

/// the accepted op matrix, each entry the signer and the op: registration,
/// a conversation, a roster, a scoped binding, two sends under the binding's
/// credential, and the delivery walk to a terminal state.
fn accepted() -> Vec<(u8, Msg)> {
    vec![
        (
            1,
            msg(CollaborationMsg::RegisterParticipant {
                participant_id: "alice".into(),
                display_name: "alice".into(),
                agent_account: None,
            }),
        ),
        (
            2,
            msg(CollaborationMsg::RegisterParticipant {
                participant_id: "bob".into(),
                display_name: "bob".into(),
                agent_account: None,
            }),
        ),
        (
            1,
            msg(CollaborationMsg::CreateConversation {
                conversation_id: "c1".into(),
                topic: "review".into(),
            }),
        ),
        (
            1,
            msg(CollaborationMsg::SetRoster {
                conversation_id: "c1".into(),
                participant_id: "alice".into(),
                role: Some(Role::Member),
            }),
        ),
        (
            1,
            msg(CollaborationMsg::SetRoster {
                conversation_id: "c1".into(),
                participant_id: "bob".into(),
                role: Some(Role::Member),
            }),
        ),
        (
            1,
            msg(CollaborationMsg::Bind {
                conversation_id: "c1".into(),
                participant_id: "alice".into(),
                device: "laptop".into(),
                principal: BoundPrincipal::ServiceKey(vec![SERVICE; 32]),
                expected_credential: 0,
            }),
        ),
        (
            2,
            msg(CollaborationMsg::Bind {
                conversation_id: "c1".into(),
                participant_id: "bob".into(),
                device: "laptop".into(),
                principal: BoundPrincipal::ServiceKey(vec![SERVICE_B; 32]),
                expected_credential: 0,
            }),
        ),
        (SERVICE, send(1, "please review", 400)),
        (SERVICE, send(2, "and this one too", 400)),
        (
            SERVICE_B,
            msg(CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq: FIRST_MESSAGE_SEQ,
                binding_credential: 2,
                state: DeliveryState::Queued,
                reason: None,
            }),
        ),
        (
            SERVICE_B,
            msg(CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq: FIRST_MESSAGE_SEQ,
                binding_credential: 2,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            }),
        ),
    ]
}

/// the refusal matrix: a replayed sequence with different bytes, a deadline
/// past the ceiling, an over-cap body, a key that is not the sender's binding,
/// and an op addressed to a DIFFERENT network. Each must be refused on both
/// runtimes with the same reason and stage nothing.
///
/// the last two carry the whole weight of the guest's genesis config: the
/// ceiling refusal proves it read `time_unit` as the height lane (a millisecond
/// reading would admit that deadline), and the network refusal proves it read
/// `chain_id` as THIS network rather than accepting anything.
fn refused() -> Vec<(u8, Msg, &'static str)> {
    vec![
        (
            SERVICE,
            Msg {
                target: MODULE.into(),
                payload: encode_msg(&collaboration::Request::new(
                    "another-net",
                    CollaborationMsg::Send(SendRequest {
                        conversation_id: "c1".into(),
                        sender_participant_id: "alice".into(),
                        message_id: MessageId {
                            generation: 2,
                            sequence: 3,
                        },
                        recipient_participant_id: "bob".into(),
                        kind: MessageKind::Notice,
                        reply_to: None,
                        task: None,
                        body: "for somebody else's chain".into(),
                        expires_at: 400,
                        references: Vec::new(),
                    }),
                )),
            },
            "not this network",
        ),
        (
            SERVICE,
            send(1, "different bytes, same id", 400),
            "was admitted with different bytes",
        ),
        (
            SERVICE,
            send(3, "too far out", 400 + TTL),
            "time units out",
        ),
        (
            SERVICE,
            send(3, &"x".repeat(collaboration::MAX_BODY_BYTES + 1), 400),
            "over the",
        ),
        (
            9,
            send(3, "not the bound key", 400),
            "not authorized to send as",
        ),
    ]
}

/// the reads compared across the runtimes, each as (signer, `via`, read).
/// `via: Some("c1")` is how a conversation-scoped service key names the
/// binding it acts under; the owner key uses `None`.
fn reads() -> Vec<(u8, Vec<u8>)> {
    let scoped = |read: ProtectedRead| {
        encode_query(&CollaborationQuery::Read {
            participant_id: "alice".into(),
            via: Some("c1".into()),
            read,
        })
    };
    let owned = |read: ProtectedRead| {
        encode_query(&CollaborationQuery::Read {
            participant_id: "alice".into(),
            via: None,
            read,
        })
    };
    vec![
        (
            SERVICE,
            scoped(ProtectedRead::Events {
                conversation_id: "c1".into(),
                from_seq: 1,
                limit: 16,
            }),
        ),
        (
            SERVICE,
            scoped(ProtectedRead::Receipt {
                conversation_id: "c1".into(),
                seq: FIRST_MESSAGE_SEQ,
            }),
        ),
        (
            SERVICE,
            scoped(ProtectedRead::Access {
                conversation_id: "c1".into(),
            }),
        ),
        (
            SERVICE,
            scoped(ProtectedRead::Binding {
                conversation_id: "c1".into(),
            }),
        ),
        (
            SERVICE,
            scoped(ProtectedRead::SendState {
                generation: 2,
                sequence: 1,
            }),
        ),
        // the participant-wide projections are the OWNER's: the same request
        // must be answered for key 1 and denied for the service key, on both
        // runtimes.
        (1, owned(ProtectedRead::Participant)),
        (SERVICE, scoped(ProtectedRead::Participant)),
        (1, owned(ProtectedRead::Mailbox)),
        // and a conversation the caller is not on.
        (
            1,
            owned(ProtectedRead::Conversation {
                conversation_id: "absent".into(),
            }),
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
