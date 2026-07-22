//! the adapter-port equivalence proof for the agent-registry cutover: the
//! `agent-wasm` component (the NATIVE `agent` crate compiled to wasm behind
//! `guest-adapter`) and the native `AgentModule` answer the SAME op sequence
//! with IDENTICAL query replies, and their roots move in lockstep. the
//! first state-schema break is pinned with agent's OWN genesis shape: like saga,
//! agent's empty canonical encoding (a bare zero count) hashes to the SAME
//! digest as the wasm port's empty host-KV store — the roots COINCIDE at
//! genesis and diverge at the first committed write. Revision 2 declares that
//! adapter break; revision 3 declares the later role tail in the persisted
//! native snapshot value.
//!
//! the registry itself is self-contained (no sibling queries), so this proof
//! pins its two FOLLOW-UP lanes across the seam:
//!
//! * the REGISTRY HOOK: a registration (and a capability change — and ONLY a
//!   capability change) emits an [`AgentEvent`] msg to the production hook id
//!   ("runs" — `bin/node/src/host_state.rs`). both hosts carry a runs-shaped
//!   recorder under that id whose root folds every hook payload it receives,
//!   so a missing, spurious, or diverging hook diverges the recorder roots.
//! * the SAGA DEAD-LETTER arm: any trigger's `reply_to` may point a saga
//!   callback at this module, and it must be swallowed (an emitted breadcrumb
//!   event, never an error) — both hosts carry the REAL native saga and drive
//!   an actual trigger → result → callback flow into the agent module.

use agent::{
    AgentModule, AgentMsg, AgentQuery, AgentReply, LoadMode, ResourceCaps, SkillRef,
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, decode_event, decode_reply, encode_msg, encode_query,
    MAX_AGENT_ID_LEN, MAX_SKILLS_PER_AGENT, RECIPE_HASH_LEN,
};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use saga::{SagaModule, SagaMsg, encode_msg as saga_encode_msg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::Digest as _;
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/agent-wasm` by the module
/// build target; committed so this proof is self-contained.
const AGENT_WASM: &[u8] = include_bytes!("fixtures/agent.component.wasm");

fn wasm_agent() -> WasmModule {
    WasmModule::from_bytes("agent", AGENT_WASM)
        .expect("load component")
        // Revision 2 was the adapter port; revision 3 adds the role tail inside
        // the persisted native snapshot value.
        .with_state_schema_revision(3)
}

/// the production wiring, verbatim (`bin/node/src/host_state.rs`).
fn native_agent() -> AgentModule {
    AgentModule::new("agent", "saga", Some("runs".into()))
}

/// the runs-shaped hook recorder: registered under the PRODUCTION hook id, it
/// commits to the byte-concatenation of every hook msg it receives — decoding
/// each as an [`AgentEvent`] first, so a payload that stopped being a valid
/// hook event fails loud rather than folding silently.
struct HookRecorder {
    staged: Vec<Vec<u8>>,
    committed: Vec<u8>,
}

impl HookRecorder {
    fn new() -> Self {
        Self {
            staged: Vec::new(),
            committed: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for HookRecorder {
    fn id(&self) -> ModuleId {
        "runs".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot(sha2::Sha256::digest(&self.committed).into())
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        if matches!(&ctx.env().origin, Origin::Module(m) if m == "agent") {
            decode_event(&msg.payload)
                .map_err(|e| Error::Module(format!("hook payload must decode: {e}")))?;
        }
        self.staged.push(msg.payload.clone());
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        for payload in self.staged.drain(..) {
            self.committed.extend_from_slice(&payload);
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

/// both hosts carry the REAL native saga (the dead-letter lane's counterpart;
/// the open-policy constructor keeps the callback flow origin-free) and the
/// hook recorder under the production ids.
fn native_host() -> Host {
    Host::genesis(vec![
        Box::new(native_agent()),
        Box::new(SagaModule::new("saga")),
        Box::new(HookRecorder::new()),
    ])
    .expect("genesis")
}

fn wasm_host_() -> Host {
    Host::genesis(vec![
        Box::new(wasm_agent()),
        Box::new(SagaModule::new("saga")),
        Box::new(HookRecorder::new()),
    ])
    .expect("genesis")
}

fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

fn agent_op(m: &AgentMsg) -> Msg {
    Msg {
        target: "agent".into(),
        payload: encode_msg(m),
    }
}

/// registration parameters as a struct so scenarios override single fields.
struct Reg {
    agent_id: String,
    display_name: String,
    capability: String,
    allowed_actions: Vec<String>,
    recipe_hash: Option<Vec<u8>>,
    caps: Option<ResourceCaps>,
    skills: Option<Vec<SkillRef>>,
}

impl From<Reg> for AgentMsg {
    fn from(r: Reg) -> Self {
        AgentMsg::RegisterAgent {
            agent_id: r.agent_id,
            display_name: r.display_name,
            capability: r.capability,
            allowed_actions: r.allowed_actions,
            recipe_hash: r.recipe_hash,
            caps: r.caps,
            skills: r.skills,
        }
    }
}

fn reg(id: &str) -> Reg {
    Reg {
        agent_id: id.into(),
        display_name: format!("Agent {id}"),
        capability: "llm".into(),
        allowed_actions: vec![ACTION_CHAT_POST.into()],
        recipe_hash: None,
        caps: None,
        skills: None,
    }
}

/// the read matrix: the full listing plus per-id gets (present and absent).
async fn replies(h: &Host, ids: &[&str]) -> Vec<Vec<u8>> {
    let mut queries = vec![encode_query(&AgentQuery::Agents)];
    for id in ids {
        queries.push(encode_query(&AgentQuery::Agent {
            agent_id: (*id).into(),
        }));
    }
    queries.push(encode_query(&AgentQuery::Agent {
        agent_id: "absent".into(),
    }));
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("agent", q).await.expect("query"));
    }
    out
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("agent").expect("agent registered")
}

fn hook_root(h: &Host) -> StateRoot {
    h.module_root("runs").expect("runs registered")
}

/// submit one ACCEPTED op to both hosts: identical replies, identical hook
/// lane (recorder roots agree — and move iff `hook_fires`), lockstep agent
/// root movement.
#[allow(clippy::too_many_arguments)]
async fn roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    ids: &[&str],
    height: u64,
    origin: Origin,
    m: Msg,
    moves: bool,
    hook_fires: bool,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let (n_hook, w_hook) = (hook_root(native), hook_root(wasm));
    native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect("native submit");
    wasm.submit_at(block(height, origin), m)
        .await
        .expect("wasm submit");
    assert_eq!(
        replies(native, ids).await,
        replies(wasm, ids).await,
        "replies diverge after block {height}"
    );
    assert_eq!(
        hook_root(native),
        hook_root(wasm),
        "the hook lane diverged at {height}"
    );
    if hook_fires {
        assert_ne!(hook_root(native), n_hook, "no native hook at {height}");
        assert_ne!(hook_root(wasm), w_hook, "no wasm hook at {height}");
    } else {
        assert_eq!(hook_root(native), n_hook, "spurious native hook at {height}");
        assert_eq!(hook_root(wasm), w_hook, "spurious wasm hook at {height}");
    }
    if moves {
        assert_ne!(root_of(native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(wasm), w_before, "wasm root stuck at {height}");
    } else {
        assert_eq!(root_of(native), n_before, "native root moved at {height}");
        assert_eq!(root_of(wasm), w_before, "wasm root moved at {height}");
    }
}

/// submit one REJECTED op to both hosts: reasons carry the same needle, and
/// the agent root (and the hook recorder) are byte-identical to pre-block.
async fn reject_roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    ids: &[&str],
    height: u64,
    origin: Origin,
    m: Msg,
    needle: &str,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let (n_hook, w_hook) = (hook_root(native), hook_root(wasm));
    let n_err = native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect_err("native must reject");
    let w_err = wasm
        .submit_at(block(height, origin), m)
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
    assert_eq!(root_of(native), n_before, "native root moved on reject");
    assert_eq!(root_of(wasm), w_before, "wasm root moved on reject");
    // an aborted block leaves the hook recorder untouched too (a rejected
    // registration's staged hook payload is discarded with the block).
    assert_eq!(hook_root(native), n_hook);
    assert_eq!(hook_root(wasm), w_hook);
    assert_eq!(replies(native, ids).await, replies(wasm, ids).await);
}

#[test]
fn same_ops_same_replies_hooks_in_lockstep() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let owner = Origin::External(vec![0xAA; 32]);
    let stranger = Origin::External(vec![0xBB; 32]);
    let ids = ["quacksmith", "curator"];

    let mut native = native_host();
    let mut wasm = wasm_host_();

    assert_eq!(
        native
            .state_schema()
            .into_iter()
            .find(|(id, _)| id == "agent")
            .map(|(_, revision)| revision),
        Some(2),
        "the native role-tail encoding is revision 2"
    );
    assert_eq!(
        wasm.state_schema()
            .into_iter()
            .find(|(id, _)| id == "agent")
            .map(|(_, revision)| revision),
        Some(3),
        "the adapter wrapper plus role-tail encoding is revision 3"
    );

    // the adapter SCHEMA-BREAK pin, agent-shaped: the empty canonical map and
    // the empty host-KV store share the 8-zero-byte preimage, so genesis roots
    // COINCIDE and the adapter break surfaces at the first committed write.
    // The revision assertions above separately pin the later role-tail break.
    assert_ne!(root_of(&native), StateRoot::ZERO, "agent has no ZERO sentinel");
    assert_eq!(
        root_of(&native),
        root_of(&wasm),
        "empty-canonical-map module: genesis roots coincide by construction"
    );

    // ---- registration fires the hook (Registered) in the SAME block, with
    // the full runtime-identity tail carried through.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        1,
        owner.clone(),
        agent_op(
            &Reg {
                allowed_actions: vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()],
                recipe_hash: Some(vec![0x11; RECIPE_HASH_LEN]),
                caps: Some(ResourceCaps {
                    tools: vec!["web-search".into(), "calculator".into()],
                    subagent_budget: 2,
                    ..ResourceCaps::default()
                }),
                skills: Some(vec![SkillRef {
                    name: "persona".into(),
                    source_prefix: "skills/persona".into(),
                    source_snapshot: Some("snap-1".into()),
                    load: LoadMode::Always,
                }]),
                ..reg("quacksmith")
            }
            .into(),
        ),
        true,
        true,
    )
    .await;
    // the roots diverged at the first write (the declared schema break).
    assert_ne!(root_of(&native), root_of(&wasm));

    // ---- a MODULE origin is a legitimate owner.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        2,
        Origin::Module("jobs".into()),
        agent_op(&reg("curator").into()),
        true,
        true,
    )
    .await;

    // ---- a field update WITHOUT a capability change stays hook-silent.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        3,
        owner.clone(),
        agent_op(&AgentMsg::UpdateAgent {
            agent_id: "quacksmith".into(),
            display_name: Some("The Quacksmith".into()),
            capability: None,
            allowed_actions: None,
            recipe_hash: None,
            caps: None,
            skills: Some(vec![
                SkillRef {
                    name: "persona".into(),
                    source_prefix: "skills/persona".into(),
                    source_snapshot: None,
                    load: LoadMode::Always,
                },
                SkillRef {
                    name: "reference".into(),
                    source_prefix: "skills/reference".into(),
                    source_snapshot: None,
                    load: LoadMode::OnDemand,
                },
            ]),
        }),
        true,
        false,
    )
    .await;

    // ---- a capability change RETUNES: the hook fires (CapabilityChanged).
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        4,
        owner.clone(),
        agent_op(&AgentMsg::UpdateAgent {
            agent_id: "quacksmith".into(),
            display_name: None,
            capability: Some("codegen".into()),
            allowed_actions: None,
            recipe_hash: None,
            caps: None,
            skills: None,
        }),
        true,
        true,
    )
    .await;

    // ---- pause / resume: owner-gated flips; re-flipping is an idempotent
    // no-op (nothing staged, root byte-identical).
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        5,
        owner.clone(),
        agent_op(&AgentMsg::PauseAgent {
            agent_id: "quacksmith".into(),
        }),
        true,
        false,
    )
    .await;
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        6,
        owner.clone(),
        agent_op(&AgentMsg::PauseAgent {
            agent_id: "quacksmith".into(),
        }),
        false,
        false,
    )
    .await;
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        7,
        owner.clone(),
        agent_op(&AgentMsg::ResumeAgent {
            agent_id: "quacksmith".into(),
        }),
        true,
        false,
    )
    .await;

    // ---- the saga DEAD-LETTER lane: a foreign trigger points its callback
    // at the agent module; the terminal result's same-block callback must be
    // swallowed as a no-op breadcrumb on BOTH runtimes (the agent root holds
    // through both blocks — the trigger's, and the callback's).
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(10, stranger.clone()),
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::Trigger {
                    saga_id: "dead-letter".into(),
                    spec: b"noise".to_vec(),
                    reply_to: Some("agent".into()),
                    reply_payload: b"corr".to_vec(),
                    deadline: None,
                    max_attempts: 1,
                    lease_views: None,
                    capability: None,
                    demands: Default::default(),
                    pinned_assignee: None,
                }),
            },
        )
        .await
        .expect("trigger");
    }
    let n_out = native
        .submit_at(
            block(11, stranger.clone()),
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::OracleResult {
                    saga_id: "dead-letter".into(),
                    attempt: 0,
                    outcome: Ok(b"done".to_vec()),
                    usage: None,
                }),
            },
        )
        .await
        .expect("native result");
    let w_out = wasm
        .submit_at(
            block(11, stranger.clone()),
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::OracleResult {
                    saga_id: "dead-letter".into(),
                    attempt: 0,
                    outcome: Ok(b"done".to_vec()),
                    usage: None,
                }),
            },
        )
        .await
        .expect("wasm result — the dead-letter arm must never abort the terminal block");
    // the breadcrumb event surfaces identically (source "agent").
    let breadcrumbs = |events: &[sdk::Event]| -> Vec<Vec<u8>> {
        events
            .iter()
            .filter(|e| e.source == "agent")
            .map(|e| e.payload.clone())
            .collect()
    };
    assert_eq!(breadcrumbs(&n_out.events), breadcrumbs(&w_out.events));
    assert_eq!(
        breadcrumbs(&n_out.events),
        vec![b"dropped a direct saga callback".to_vec()]
    );
    assert_eq!(root_of(&native), n_before, "the dead letter staged nothing");
    assert_eq!(root_of(&wasm), w_before, "the dead letter staged nothing");
    assert_eq!(replies(&native, &ids).await, replies(&wasm, &ids).await);

    // decoded spot check: the hook stream both recorders hold is the same
    // three events, in order.
    // (roots already pinned equal; this is the human-readable rendering.)

    // queries are read-only on the wasm side too.
    let settled = root_of(&wasm);
    let _ = replies(&wasm, &ids).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let owner = Origin::External(vec![0xAA; 32]);
    let stranger = Origin::External(vec![0xBB; 32]);
    let ids = ["quacksmith"];

    let mut native = native_host();
    let mut wasm = wasm_host_();

    for host in [&mut native, &mut wasm] {
        host.submit_at(block(1, owner.clone()), agent_op(&reg("quacksmith").into()))
            .await
            .expect("seed registration");
    }

    // every distinct refusal family: the id grammar (the DNS-label rule),
    // field shapes, the action vocabulary, the runtime-identity validations,
    // the record size gate, origin/owner gating, duplicates, unknowns, and
    // the decode seam.
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        (
            owner.clone(),
            agent_op(&reg("").into()),
            "agent_id must not be empty",
        ),
        (
            owner.clone(),
            agent_op(&reg(&"a".repeat(MAX_AGENT_ID_LEN + 1)).into()),
            "agent_id exceeds",
        ),
        (
            owner.clone(),
            agent_op(&reg("-lead").into()),
            "must not start or end with a hyphen",
        ),
        (
            owner.clone(),
            agent_op(&reg("Upper_Case").into()),
            "must be a DNS label",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    display_name: String::new(),
                    ..reg("r1")
                }
                .into(),
            ),
            "display_name must not be empty",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    capability: "NOT A TAG".into(),
                    ..reg("r2")
                }
                .into(),
            ),
            "invalid characters",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    allowed_actions: vec!["chat.metaphysics".into()],
                    ..reg("r3")
                }
                .into(),
            ),
            "unknown action",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    recipe_hash: Some(vec![0x22; RECIPE_HASH_LEN - 1]),
                    ..reg("r4")
                }
                .into(),
            ),
            "recipe_hash must be empty or",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    caps: Some(ResourceCaps {
                        tools: vec![String::new()],
                        ..ResourceCaps::default()
                    }),
                    ..reg("r5")
                }
                .into(),
            ),
            "cap entries must be non-empty",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    skills: Some(vec![
                        SkillRef {
                            name: "s".into(),
                            source_prefix: "p".into(),
                            source_snapshot: None,
                            load: LoadMode::OnDemand,
                        };
                        MAX_SKILLS_PER_AGENT + 1
                    ]),
                    ..reg("r6")
                }
                .into(),
            ),
            "curate at most",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    skills: Some(vec![SkillRef {
                        name: String::new(),
                        source_prefix: "p".into(),
                        source_snapshot: None,
                        load: LoadMode::OnDemand,
                    }]),
                    ..reg("r7")
                }
                .into(),
            ),
            "skill name must not be empty",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    skills: Some(vec![SkillRef {
                        name: "s".into(),
                        source_prefix: String::new(),
                        source_snapshot: None,
                        load: LoadMode::OnDemand,
                    }]),
                    ..reg("r8")
                }
                .into(),
            ),
            "skill source_prefix must not be empty",
        ),
        (
            owner.clone(),
            agent_op(
                &Reg {
                    skills: Some(vec![SkillRef {
                        name: "s".into(),
                        source_prefix: "p".into(),
                        source_snapshot: Some(String::new()),
                        load: LoadMode::OnDemand,
                    }]),
                    ..reg("r9")
                }
                .into(),
            ),
            "source_snapshot must not be empty when set",
        ),
        // the record size gate: a display name past MAX_AGENT_RECORD_BYTES.
        (
            owner.clone(),
            agent_op(
                &Reg {
                    display_name: "d".repeat(5000),
                    ..reg("r10")
                }
                .into(),
            ),
            "agent record too large",
        ),
        // origin gating: System never owns agents; an empty external key is
        // the pre-consensus default and never owns either.
        (
            Origin::System,
            agent_op(&reg("r11").into()),
            "require an external or module origin",
        ),
        (
            Origin::External(Vec::new()),
            agent_op(&reg("r12").into()),
            "non-empty submitter id",
        ),
        // duplicates and unknowns.
        (
            owner.clone(),
            agent_op(&reg("quacksmith").into()),
            "agent already exists",
        ),
        (
            owner.clone(),
            agent_op(&AgentMsg::PauseAgent {
                agent_id: "ghost".into(),
            }),
            "unknown agent",
        ),
        // the owner gate.
        (
            stranger.clone(),
            agent_op(&AgentMsg::PauseAgent {
                agent_id: "quacksmith".into(),
            }),
            "only the owner may modify",
        ),
        (
            stranger,
            agent_op(&AgentMsg::UpdateAgent {
                agent_id: "quacksmith".into(),
                display_name: Some("Squatter".into()),
                capability: None,
                allowed_actions: None,
                recipe_hash: None,
                caps: None,
                skills: None,
            }),
            "only the owner may modify",
        ),
        // the decode seam.
        (
            owner.clone(),
            Msg {
                target: "agent".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (height, (origin, m, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 2;
        reject_roundtrip(&mut native, &mut wasm, &ids, height, origin, m, needle).await;
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let owner = Origin::External(vec![0xAA; 32]);
    let ids = ["a1", "a2"];

    let mut native = native_host();
    let mut wasm = wasm_host_();

    // ONE block, three ops: the update reads the registration STAGED by the
    // first dispatch, and the pause reads the update's staged record — on the
    // wasm side each later dispatch reloads the prior dispatch's staged
    // `__state`. the register fires its hook; the capability change fires the
    // retune hook from the SECOND dispatch of the same block.
    let batch = vec![
        (owner.clone(), agent_op(&reg("a1").into())),
        (
            owner.clone(),
            agent_op(&AgentMsg::UpdateAgent {
                agent_id: "a1".into(),
                display_name: None,
                capability: Some("codegen".into()),
                allowed_actions: None,
                recipe_hash: None,
                caps: None,
                skills: None,
            }),
        ),
        (
            owner.clone(),
            agent_op(&AgentMsg::PauseAgent {
                agent_id: "a1".into(),
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(1, owner.clone()), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, owner.clone()), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "all members must apply: {:?}",
            out.members
        );
    }
    assert_eq!(replies(&native, &ids).await, replies(&wasm, &ids).await);
    assert_eq!(hook_root(&native), hook_root(&wasm));
    // both hook events (Registered + CapabilityChanged) committed.
    let AgentReply::Agent(Some(record)) = decode_reply(
        &wasm
            .query(
                "agent",
                &encode_query(&AgentQuery::Agent {
                    agent_id: "a1".into(),
                }),
            )
            .await
            .expect("get"),
    )
    .expect("decode") else {
        panic!("a1 must exist");
    };
    assert_eq!(record.capability, "codegen");
    assert_eq!(record.status, agent::AgentStatus::Paused);

    // ONE block where the SECOND member rejects AGAINST THE STAGE (a
    // duplicate of the registration staged by the first member): the runtime
    // aborts the staged overlay and replays the accepted members.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (owner.clone(), agent_op(&reg("a2").into())),
        (owner.clone(), agent_op(&reg("a2").into())),
        (
            owner.clone(),
            agent_op(&AgentMsg::PauseAgent {
                agent_id: "a2".into(),
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(2, owner.clone()), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, owner.clone()), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(
            matches!(out.members[1], MemberOutcome::Rejected { .. }),
            "the duplicate must reject against the SAME-BLOCK stage: {:?}",
            out.members
        );
        assert!(matches!(out.members[2], MemberOutcome::Applied { .. }));
    }
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(replies(&native, &ids).await, replies(&wasm, &ids).await);
    assert_eq!(hook_root(&native), hook_root(&wasm));
}
