//! snapshot/install round-trip for the agent registry: committed state
//! covering both owner origin shapes and both statuses — built through the
//! real ordered-op path — crosses to a fresh module as canonical bytes and
//! re-derives the identical root, with query parity. the bytes arrive
//! UNTRUSTED (a byzantine peer serves them), so the flip side is exercised
//! too: tampered, truncated, padded, misordered, and bad-discriminant
//! snapshots are rejected and the target module is left byte-identical to
//! before the call.

use agent::AgentModule;
use agent::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentMsg, AgentQuery, AgentReply, AgentStatus,
    PROMPT_HASH_LEN, PromptRef, RENDERER_MEMORY_GENERATION, decode_reply, encode_msg, encode_query,
};
use futures::executor::block_on;
use saga::SagaOrigin;
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};

/// a minimal `Ctx`: drives `execute` with a controllable env; the registry
/// queries nothing, and "memory" is the one known sibling module (the
/// PromptRef validation probes source-module existence).
struct TestCtx {
    env: Env,
}
impl TestCtx {
    fn new(height: u64, origin: Origin) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height,
                consensus_time: height,
                origin,
                me: "agent".into(),
            },
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, t: &str) -> Option<StateRoot> {
        (t == "memory").then_some(StateRoot::ZERO)
    }
    async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::UnknownModule(target.into()))
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: Event) {}
    fn request_effect(&mut self, _eff: Effect) {}
}

fn module() -> AgentModule {
    AgentModule::new("agent", "saga", Some("runs".into()))
}

fn exec(m: &mut AgentModule, mut ctx: TestCtx, op: &AgentMsg) {
    let msg = Msg {
        target: "agent".into(),
        payload: encode_msg(op),
    };
    block_on(m.execute(&mut ctx, &msg)).unwrap();
}

fn commit(m: &mut AgentModule) {
    block_on(m.commit_block()).unwrap();
}

fn register(agent_id: &str, actions: &[&str]) -> AgentMsg {
    AgentMsg::RegisterAgent {
        agent_id: agent_id.into(),
        display_name: agent_id.to_uppercase(),
        capability: "model-1".into(),
        prompt: None,
        allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
    }
}

fn prompt_ref() -> PromptRef {
    PromptRef {
        module: "memory".into(),
        target: "/agents/prompts/pinned-bot@3".into(),
        renderer: RENDERER_MEMORY_GENERATION.into(),
        sha256: vec![7u8; PROMPT_HASH_LEN],
    }
}

fn query_reply(m: &AgentModule, q: &AgentQuery) -> AgentReply {
    decode_reply(&block_on(m.query(&encode_query(q))).unwrap()).unwrap()
}

/// a source holding agents under both owner shapes (external + module), one
/// paused, one tombstoned, and one carrying a PromptRef — all built through
/// the real execute path, never by poking internals.
fn source() -> AgentModule {
    let alice = Origin::External(b"alice".to_vec());
    let mut m = module();
    exec(
        &mut m,
        TestCtx::new(1, alice.clone()),
        &register("ext-bot", &[ACTION_CHAT_POST]),
    );
    exec(
        &mut m,
        TestCtx::new(1, Origin::Module("orchestrator".into())),
        &register("mod-bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE]),
    );
    exec(
        &mut m,
        TestCtx::new(1, alice.clone()),
        &AgentMsg::RegisterAgent {
            agent_id: "pinned-bot".into(),
            display_name: "PINNED-BOT".into(),
            capability: "model-1".into(),
            prompt: Some(prompt_ref()),
            allowed_actions: Vec::new(),
        },
    );
    exec(
        &mut m,
        TestCtx::new(1, alice.clone()),
        &register("sleepy-bot", &[]),
    );
    exec(
        &mut m,
        TestCtx::new(1, alice.clone()),
        &AgentMsg::PauseAgent {
            agent_id: "sleepy-bot".into(),
        },
    );
    exec(
        &mut m,
        TestCtx::new(1, alice),
        &AgentMsg::TombstoneAgent {
            agent_id: "pinned-bot".into(),
        },
    );
    commit(&mut m);
    m
}

#[test]
fn installed_snapshot_reconstructs_root_and_reads() {
    let src = source();
    let src_root = src.root();
    assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
    let snap = src.snapshot();

    // the source really covers the space: four agents, one paused, one
    // tombstoned with a PromptRef, both owner shapes. sorted ids: ext-bot,
    // mod-bot, pinned-bot, sleepy-bot.
    let AgentReply::Agents(agents) = query_reply(&src, &AgentQuery::Agents) else {
        panic!("agents reply expected");
    };
    assert_eq!(agents.len(), 4);
    assert_eq!(agents[0].owner, SagaOrigin::External(b"alice".to_vec()));
    assert_eq!(agents[0].prompt, None);
    assert_eq!(agents[1].owner, SagaOrigin::Module("orchestrator".into()));
    assert_eq!(agents[2].status, AgentStatus::Tombstoned);
    assert_eq!(agents[2].prompt, Some(prompt_ref()));
    assert_eq!(agents[3].status, AgentStatus::Paused);

    // the joiner has UNCOMMITTED staged work of its own: install must drop it
    // — a snapshot describes a block boundary, nothing staged may shadow it.
    let mut dst = module();
    exec(
        &mut dst,
        TestCtx::new(0, Origin::External(b"bob".to_vec())),
        &register("staged-bot", &[]),
    );

    dst.install(&snap, src_root).unwrap();

    // THE PROPERTY: identical root — the app-hash linkage a joiner needs.
    assert_eq!(dst.root(), src_root, "installed root must equal the source");

    // query parity.
    assert_eq!(
        query_reply(&dst, &AgentQuery::Agents),
        query_reply(&src, &AgentQuery::Agents)
    );
    let AgentReply::Agent(staged) = query_reply(
        &dst,
        &AgentQuery::Agent {
            agent_id: "staged-bot".into(),
        },
    ) else {
        panic!("agent reply expected");
    };
    assert_eq!(staged, None, "install must clear the staged overlay");
}

#[test]
fn tampered_snapshot_is_rejected_and_leaves_state_untouched() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();

    // the target already has COMMITTED state of its own, so "untouched" is
    // observable through both root and query.
    let mut dst = module();
    exec(
        &mut dst,
        TestCtx::new(0, Origin::External(b"bob".to_vec())),
        &register("local-bot", &[]),
    );
    commit(&mut dst);
    let before_root = dst.root();
    let before_view = query_reply(&dst, &AgentQuery::Agents);

    // flip one byte in a trailing field: the bytes still DECODE, but the
    // re-derived root cannot match the agreed one.
    let mut forged = snap.clone();
    let last = forged.len() - 1;
    forged[last] ^= 0xff;
    assert!(
        dst.install(&forged, src_root).is_err(),
        "a forged payload must be rejected"
    );
    assert_eq!(dst.root(), before_root, "failed install must not move root");
    assert_eq!(query_reply(&dst, &AgentQuery::Agents), before_view);

    // honest bytes against the WRONG agreed root are equally rejected.
    assert!(dst.install(&snap, StateRoot::ZERO).is_err());
    assert_eq!(dst.root(), before_root);

    // and the failures left the module fully usable: the honest snapshot
    // under the honest root still lands.
    dst.install(&snap, src_root).unwrap();
    assert_eq!(dst.root(), src_root);
}

#[test]
fn truncated_or_padded_snapshot_is_rejected() {
    let src = source();
    let src_root = src.root();
    let snap = src.snapshot();
    let empty_root = module().root();

    // EVERY strict prefix must fail — no cut point leaves a decodable
    // snapshot, and none of the failures may move the fresh module's root.
    for cut in 0..snap.len() {
        let mut dst = module();
        assert!(
            dst.install(&snap[..cut], src_root).is_err(),
            "a {cut}-byte prefix of a {}-byte snapshot must be rejected",
            snap.len()
        );
        assert_eq!(
            dst.root(),
            empty_root,
            "rejected prefix ({cut} bytes) must not move the root"
        );
    }

    // trailing bytes after a complete snapshot are equally malformed.
    let mut padded = snap.clone();
    padded.push(0);
    let mut dst = module();
    assert!(dst.install(&padded, src_root).is_err());
    assert_eq!(dst.root(), empty_root);

    // a count field claiming more entries than the bytes carry is caught
    // before anything is built from it: the agent count is at offset 0;
    // corrupting it must reject.
    let mut inflated = snap.clone();
    inflated[0] = inflated[0].wrapping_add(1);
    assert!(
        dst.install(&inflated, src_root).is_err(),
        "an inflated agent count must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

/// the canonical bytes of a minimal one-agent state, built through the real
/// op path. the layout is pinned by the asserted length so the
/// discriminant-tampering test can index into it:
/// agents: count 8 | id 8+1 | owner disc 1 + key 8+1 | display 8+1
///         | model 8+1 | prompt option tag 1 | action count 8
///         | status 1 | times 16
fn minimal_snapshot() -> Vec<u8> {
    let owner = Origin::External(vec![5]);
    let mut m = module();
    exec(
        &mut m,
        TestCtx::new(0, owner),
        &AgentMsg::RegisterAgent {
            agent_id: "a".into(),
            display_name: "A".into(),
            capability: "m".into(),
            prompt: None,
            allowed_actions: Vec::new(),
        },
    );
    commit(&mut m);
    let snap = m.snapshot();
    assert_eq!(snap.len(), 71, "the minimal layout this test indexes into");
    snap
}

#[test]
fn unknown_discriminants_and_tags_are_rejected() {
    let empty_root = module().root();
    let snap = minimal_snapshot();

    // the owner origin disc (17), the prompt option tag (45), and the agent
    // status (54) each admit exactly their known values — a state has ONE
    // valid encoding.
    for (index, what) in [
        (17usize, "owner origin discriminant"),
        (45, "prompt option tag"),
        (54, "agent status"),
    ] {
        let mut bad = snap.clone();
        bad[index] = 9;
        let mut dst = module();
        let err = dst.install(&bad, StateRoot::ZERO).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "unknown {what} must be rejected"
        );
        assert_eq!(
            dst.root(),
            empty_root,
            "rejected {what} must not move the root"
        );
    }
}

#[test]
fn non_ascending_or_duplicate_keys_are_rejected() {
    // two same-shape agents "a" and "b": their encoded bodies have identical
    // lengths, so swapping the body slices yields a descending-id stream and
    // copying one over the other a duplicate-id stream — both must reject,
    // since sorted-unique keys are what make the encoding canonical.
    let owner = Origin::External(vec![5]);
    let mut m = module();
    for id in ["a", "b"] {
        exec(
            &mut m,
            TestCtx::new(0, owner.clone()),
            &AgentMsg::RegisterAgent {
                agent_id: id.into(),
                display_name: id.to_uppercase(),
                capability: "m".into(),
                prompt: None,
                allowed_actions: Vec::new(),
            },
        );
    }
    commit(&mut m);
    let snap = m.snapshot();
    let good_root = m.root();
    // agents section: count 8, then two 63-byte bodies.
    assert_eq!(snap.len(), 8 + 63 * 2);
    let body_a = snap[8..71].to_vec();
    let body_b = snap[71..134].to_vec();

    for (first, second, what) in [
        (&body_b, &body_a, "descending ids"),
        (&body_a, &body_a, "duplicate ids"),
    ] {
        let mut bytes = snap.clone();
        bytes[8..71].copy_from_slice(first);
        bytes[71..134].copy_from_slice(second);
        let mut dst = module();
        let err = dst.install(&bytes, StateRoot::ZERO).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(dst.root(), module().root());
    }

    // the untouched stream still installs — the rejection above is the
    // ordering check, not an artifact of the splicing.
    let mut dst = module();
    dst.install(&snap, good_root.clone()).unwrap();
    assert_eq!(dst.root(), good_root);
}
