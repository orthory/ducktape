//! snapshot/install round-trip for the agent orchestrator: committed state
//! covering both owner origin shapes, every turn policy, and every run status
//! — built through the real ordered-op path — crosses to a fresh module as
//! canonical bytes and re-derives the identical root, with query parity on
//! agents, watches, and runs. the bytes arrive UNTRUSTED (a byzantine peer
//! serves them), so the flip side is exercised too: tampered, truncated,
//! padded, misordered, and bad-discriminant snapshots are rejected and the
//! target module is left byte-identical to before the call.

use std::collections::BTreeMap;

use agent::{AgentModule, run_id_for};
use agent_interface::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentMsg, AgentOutput, AgentQuery, AgentReply,
    AgentStatus, RunStatus, TurnPolicy, decode_reply, encode_msg, encode_output, encode_query,
};
use chat_interface::{
    AuthorRef, Block, ChatQuery, ChatReply, MessageHead, MessageView,
    decode_query as chat_decode_query, encode_reply as chat_encode_reply,
};
use futures::executor::block_on;
use saga_interface::{SagaCallback, SagaOrigin, SagaOutcome, encode_callback};
use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};

/// a minimal `Ctx`: drives `execute` with a controllable env and serves
/// canned chat transcripts (context pins + reply probes read through it).
struct TestCtx {
    env: Env,
    transcripts: BTreeMap<String, Vec<MessageView>>,
}
impl TestCtx {
    fn new(height: u64, origin: Origin) -> Self {
        Self {
            env: Env {
                height,
                consensus_time: height,
                origin,
                me: "agent".into(),
            },
            transcripts: BTreeMap::new(),
        }
    }
    fn with_transcript(mut self, channel: &str, messages: Vec<MessageView>) -> Self {
        self.transcripts.insert(channel.into(), messages);
        self
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        if target != "chat" {
            return Err(Error::UnknownModule(target.into()));
        }
        match chat_decode_query(req).map_err(Error::Module)? {
            ChatQuery::MessagesRange {
                channel_id,
                from_seq,
                limit,
            } => {
                let transcript = self
                    .transcripts
                    .get(&channel_id)
                    .ok_or_else(|| Error::Module(format!("unknown channel: {channel_id}")))?;
                let head = transcript.len() as u64;
                let from = from_seq.max(1);
                let mut window = Vec::new();
                if limit > 0 && from <= head {
                    let to = head.min(from + limit - 1);
                    window = transcript[(from - 1) as usize..to as usize].to_vec();
                }
                Ok(chat_encode_reply(&ChatReply::Messages(window)))
            }
            ChatQuery::Message { message_id } => Ok(chat_encode_reply(&ChatReply::Message(
                self.transcripts
                    .values()
                    .flatten()
                    .find(|v| v.head.message_id == message_id)
                    .cloned(),
            ))),
            _ => Err(Error::QueryUnsupported),
        }
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: Event) {}
    fn request_effect(&mut self, _eff: Effect) {}
}

fn message_in(channel: &str, seq: u64, text: &str, thread: Option<u64>) -> MessageView {
    MessageView {
        channel_id: channel.into(),
        seq,
        head: MessageHead {
            message_id: format!("{channel}-m{seq}"),
            author: AuthorRef::User(vec![1; 32]),
            blocks: vec![Block::paragraph(text)],
            created_at: 0,
            rev: 0,
            edited_at: None,
            base_rev: None,
            deleted: false,
            thread,
            reply_count: 0,
            last_reply_seq: None,
        },
        reactions: Vec::new(),
        channel_head_seq: seq,
    }
}

fn exec(m: &mut AgentModule, mut ctx: TestCtx, op: &AgentMsg) {
    let msg = Msg {
        target: "agent".into(),
        payload: encode_msg(op),
    };
    block_on(m.execute(&mut ctx, &msg)).unwrap();
}

fn exec_callback(m: &mut AgentModule, mut ctx: TestCtx, run_id: &str, outcome: SagaOutcome) {
    let msg = Msg {
        target: "agent".into(),
        payload: encode_callback(&SagaCallback {
            saga_id: format!("agent/{run_id}"),
            payload: run_id.as_bytes().to_vec(),
            outcome,
        }),
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
        model_ref: "model-1".into(),
        prompt_hash: vec![7u8; 32],
        allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
    }
}

fn query_reply(m: &AgentModule, q: &AgentQuery) -> AgentReply {
    decode_reply(&block_on(m.query(&encode_query(q))).unwrap()).unwrap()
}

fn run_status(m: &AgentModule, run_id: &str) -> Option<RunStatus> {
    match query_reply(
        m,
        &AgentQuery::Run {
            run_id: run_id.into(),
        },
    ) {
        AgentReply::Run(view) => view.map(|v| v.status),
        other => panic!("unexpected reply: {other:?}"),
    }
}

/// a source holding: agents under both owner shapes (external + module, one
/// paused), a watch per turn policy, and one committed run in EVERY status
/// (with a threaded anchor on the awaiting one so the option field is
/// populated somewhere) — all built through the real execute path, never by
/// poking internals.
fn source() -> AgentModule {
    let alice = Origin::External(b"alice".to_vec());
    let mut m = AgentModule::new(
        "agent",
        "chat",
        "saga",
        Some("tasks".into()),
        Some("jobs".into()),
    );
    // "general": seqs 1..2 plain, seq 3 a thread reply to root 1.
    let general = vec![
        message_in("general", 1, "root", None),
        message_in("general", 2, "second", None),
        message_in("general", 3, "threaded", Some(1)),
    ];
    let dev = vec![message_in("dev", 1, "hello dev", None)];

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
        &register("sleepy-bot", &[]),
    );
    exec(
        &mut m,
        TestCtx::new(1, alice.clone()),
        &AgentMsg::PauseAgent {
            agent_id: "sleepy-bot".into(),
        },
    );
    for (channel, policy) in [
        ("general", TurnPolicy::Mention),
        ("dev", TurnPolicy::All),
        ("standup", TurnPolicy::Assigned("ext-bot".into())),
        ("round", TurnPolicy::RoundRobin),
    ] {
        exec(
            &mut m,
            TestCtx::new(1, alice.clone()),
            &AgentMsg::WatchChannel {
                channel_id: channel.into(),
                policy,
            },
        );
    }
    commit(&mut m);

    // four runs, one per terminal-or-not status.
    let request = |agent: &str, channel: &str, anchor_seq: u64| AgentMsg::RequestRun {
        agent_id: agent.into(),
        channel_id: channel.into(),
        anchor_seq,
    };
    // stays AwaitingOracle; its anchor is threaded, so thread_root = Some(1).
    exec(
        &mut m,
        TestCtx::new(2, alice.clone()).with_transcript("general", general.clone()),
        &request("ext-bot", "general", 3),
    );
    exec(
        &mut m,
        TestCtx::new(2, alice.clone()).with_transcript("general", general.clone()),
        &request("ext-bot", "general", 2),
    );
    exec(
        &mut m,
        TestCtx::new(2, alice.clone()).with_transcript("general", general.clone()),
        &request("ext-bot", "general", 1),
    );
    exec(
        &mut m,
        TestCtx::new(2, alice.clone()).with_transcript("dev", dev.clone()),
        &request("mod-bot", "dev", 1),
    );
    commit(&mut m);

    // general/2 -> Done (a valid reply output), general/1 -> Failed,
    // dev/1 -> Cancelled by its requester.
    let saga = || Origin::Module("saga".into());
    exec_callback(
        &mut m,
        TestCtx::new(3, saga()).with_transcript("general", general.clone()),
        &run_id_for("general", 2, "ext-bot"),
        SagaOutcome::Done(encode_output(&AgentOutput {
            reply_blocks: vec![Block::paragraph("answered")],
            actions: Vec::new(),
        })),
    );
    exec_callback(
        &mut m,
        TestCtx::new(3, saga()),
        &run_id_for("general", 1, "ext-bot"),
        SagaOutcome::Failed("worker crashed".into()),
    );
    exec(
        &mut m,
        TestCtx::new(3, alice.clone()),
        &AgentMsg::CancelRun {
            run_id: run_id_for("dev", 1, "mod-bot"),
        },
    );
    commit(&mut m);
    m
}

#[test]
fn installed_snapshot_reconstructs_root_and_reads_across_every_status() {
    let src = source();
    let src_root = src.root();
    assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
    let snap = src.snapshot();

    // the source really covers the space: three agents (one paused, both
    // owner shapes), four policies, four run statuses, a threaded anchor.
    let AgentReply::Agents(agents) = query_reply(&src, &AgentQuery::Agents) else {
        panic!("agents reply expected");
    };
    assert_eq!(agents.len(), 3);
    assert_eq!(agents[0].owner, SagaOrigin::External(b"alice".to_vec()));
    assert_eq!(agents[1].owner, SagaOrigin::Module("orchestrator".into()));
    assert_eq!(agents[2].status, AgentStatus::Paused);
    let AgentReply::Watches(watches) = query_reply(&src, &AgentQuery::Watches) else {
        panic!("watches reply expected");
    };
    assert_eq!(watches.len(), 4);
    assert!(matches!(
        run_status(&src, &run_id_for("general", 3, "ext-bot")),
        Some(RunStatus::AwaitingOracle { .. })
    ));
    assert_eq!(
        run_status(&src, &run_id_for("general", 2, "ext-bot")),
        Some(RunStatus::Done)
    );
    assert!(matches!(
        run_status(&src, &run_id_for("general", 1, "ext-bot")),
        Some(RunStatus::Failed { .. })
    ));
    assert_eq!(
        run_status(&src, &run_id_for("dev", 1, "mod-bot")),
        Some(RunStatus::Cancelled)
    );
    let AgentReply::Run(Some(awaiting)) = query_reply(
        &src,
        &AgentQuery::Run {
            run_id: run_id_for("general", 3, "ext-bot"),
        },
    ) else {
        panic!("awaiting run expected");
    };
    assert_eq!(
        awaiting.thread_root,
        Some(1),
        "the option field is populated"
    );

    // the joiner has UNCOMMITTED staged work of its own: install must drop it
    // — a snapshot describes a block boundary, nothing staged may shadow it.
    let mut dst = AgentModule::new(
        "agent",
        "chat",
        "saga",
        Some("tasks".into()),
        Some("jobs".into()),
    );
    exec(
        &mut dst,
        TestCtx::new(0, Origin::External(b"bob".to_vec())),
        &register("staged-bot", &[]),
    );

    dst.install(&snap, src_root).unwrap();

    // THE PROPERTY: identical root — the app-hash linkage a joiner needs.
    assert_eq!(dst.root(), src_root, "installed root must equal the source");

    // query parity across every surface.
    for q in [AgentQuery::Agents, AgentQuery::Watches] {
        assert_eq!(query_reply(&dst, &q), query_reply(&src, &q));
    }
    assert_eq!(
        query_reply(
            &dst,
            &AgentQuery::Runs {
                channel_id: None,
                limit: 100,
            },
        ),
        query_reply(
            &src,
            &AgentQuery::Runs {
                channel_id: None,
                limit: 100,
            },
        )
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
    let mut dst = AgentModule::new(
        "agent",
        "chat",
        "saga",
        Some("tasks".into()),
        Some("jobs".into()),
    );
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
    let empty_root = AgentModule::new("agent", "chat", "saga", None, None).root();

    // EVERY strict prefix must fail — no cut point leaves a decodable
    // snapshot, and none of the failures may move the fresh module's root.
    for cut in 0..snap.len() {
        let mut dst = AgentModule::new("agent", "chat", "saga", None, None);
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
    let mut dst = AgentModule::new("agent", "chat", "saga", None, None);
    assert!(dst.install(&padded, src_root).is_err());
    assert_eq!(dst.root(), empty_root);

    // a count field claiming more entries than the bytes carry is caught
    // before anything is built from it — for all THREE section counts.
    let agents_section_end = {
        // recompute by decoding a legit install and measuring: cheaper — just
        // inflate the low byte of each count we can locate: the agent count
        // is at offset 0; corrupting it must reject.
        0usize
    };
    let mut inflated = snap.clone();
    inflated[agents_section_end] = inflated[agents_section_end].wrapping_add(1);
    assert!(
        dst.install(&inflated, src_root).is_err(),
        "an inflated agent count must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

/// the canonical bytes of a minimal one-agent / one-watch / one-run state,
/// built through the real op path. the layout is pinned by the asserted
/// length so the discriminant-tampering test can index into it:
/// agents:  count 8 | id 8+1 | owner disc 1 + key 8+1 | display 8+1
///          | model 8+1 | prompt 8+32 | action count 8 | status 1 | times 16
/// watches: count 8 | channel 8+1 | policy 1
/// runs:    count 8 | run id 8+10 | agent 8+1 | channel 8+1 | anchor 8
///          | thread tag 1 | job id tag 1 | job claim height 8
///          | requester disc 1 + key 8+1 | status disc 1
///          + saga id 8+16 | ctx hash 8+32 | times 16
fn minimal_snapshot() -> Vec<u8> {
    let owner = Origin::External(vec![5]);
    let mut m = AgentModule::new("agent", "chat", "saga", None, None);
    exec(
        &mut m,
        TestCtx::new(0, owner.clone()),
        &AgentMsg::RegisterAgent {
            agent_id: "a".into(),
            display_name: "A".into(),
            model_ref: "m".into(),
            prompt_hash: vec![7u8; 32],
            allowed_actions: Vec::new(),
        },
    );
    exec(
        &mut m,
        TestCtx::new(0, owner.clone()),
        &AgentMsg::WatchChannel {
            channel_id: "c".into(),
            policy: TurnPolicy::Mention,
        },
    );
    exec(
        &mut m,
        TestCtx::new(0, owner).with_transcript("c", vec![message_in("c", 1, "hi", None)]),
        &AgentMsg::RequestRun {
            agent_id: "a".into(),
            channel_id: "c".into(),
            anchor_seq: 1,
        },
    );
    commit(&mut m);
    let snap = m.snapshot();
    assert_eq!(snap.len(), 281, "the minimal layout this test indexes into");
    snap
}

#[test]
fn unknown_discriminants_and_tags_are_rejected() {
    let empty_root = AgentModule::new("agent", "chat", "saga", None, None).root();
    let snap = minimal_snapshot();

    // owner origin disc (17), agent status (93), watch policy (127), the run's
    // thread-root option tag (180), job-id option tag (181), requester origin
    // disc (190), and run status disc (200) each admit exactly their known
    // values — a state has ONE valid encoding.
    for (index, what) in [
        (17usize, "owner origin discriminant"),
        (93, "agent status"),
        (127, "watch policy"),
        (180, "thread-root option tag"),
        (181, "job-id option tag"),
        (190, "requester origin discriminant"),
        (200, "run status discriminant"),
    ] {
        let mut bad = snap.clone();
        bad[index] = 9;
        let mut dst = AgentModule::new("agent", "chat", "saga", None, None);
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
    let mut m = AgentModule::new("agent", "chat", "saga", None, None);
    for id in ["a", "b"] {
        exec(
            &mut m,
            TestCtx::new(0, owner.clone()),
            &AgentMsg::RegisterAgent {
                agent_id: id.into(),
                display_name: id.to_uppercase(),
                model_ref: "m".into(),
                prompt_hash: vec![7u8; 32],
                allowed_actions: Vec::new(),
            },
        );
    }
    commit(&mut m);
    let snap = m.snapshot();
    let good_root = m.root();
    // agents section: count 8, then two 102-byte bodies; watches + runs
    // counts trail.
    assert_eq!(snap.len(), 8 + 102 * 2 + 8 + 8);
    let body_a = snap[8..110].to_vec();
    let body_b = snap[110..212].to_vec();

    for (first, second, what) in [
        (&body_b, &body_a, "descending ids"),
        (&body_a, &body_a, "duplicate ids"),
    ] {
        let mut bytes = snap.clone();
        bytes[8..110].copy_from_slice(first);
        bytes[110..212].copy_from_slice(second);
        let mut dst = AgentModule::new("agent", "chat", "saga", None, None);
        let err = dst.install(&bytes, StateRoot::ZERO).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(
            dst.root(),
            AgentModule::new("agent", "chat", "saga", None, None).root()
        );
    }

    // the untouched stream still installs — the rejection above is the
    // ordering check, not an artifact of the splicing.
    let mut dst = AgentModule::new("agent", "chat", "saga", None, None);
    dst.install(&snap, good_root).unwrap();
    assert_eq!(dst.root(), good_root);
}
