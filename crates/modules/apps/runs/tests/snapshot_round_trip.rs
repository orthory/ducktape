//! snapshot/install round-trip for the runs module: committed state covering
//! every turn policy and pending entries in both keyspaces (chat + job,
//! threaded and plain) — built through the real ordered-op path against a
//! canned agent registry — crosses to a fresh module as canonical bytes and
//! re-derives the identical root, with query parity on watches and pending
//! runs. the bytes arrive UNTRUSTED (a byzantine peer serves them), so the
//! flip side is exercised too: tampered, truncated, padded, misordered, and
//! bad-discriminant snapshots are rejected and the target module is left
//! byte-identical to before the call.

use std::collections::BTreeMap;

use agent::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentQuery, AgentRecord, AgentReply, AgentStatus,
    decode_query as agent_decode_query, encode_reply as agent_encode_reply,
};
use chat::{
    AuthorRef, Block, ChatQuery, ChatReply, MessageHead, MessageView,
    decode_query as chat_decode_query, encode_reply as chat_encode_reply,
};
use dispatch::{
    DispatchQuery, DispatchReply, decode_query as dispatch_decode_query,
    encode_reply as dispatch_encode_reply, encode_result_event as dispatch_encode_result_event,
};
use futures::executor::block_on;
use runs::{
    PendingRun, RunsMsg, RunsQuery, RunsReply, TurnPolicy, decode_reply, encode_msg, encode_query,
};
use runs::{RunsModule, dispatch_id_for, job_run_id_for, job_spec_hash, run_id_for};
use saga::SagaOrigin;
use sdk::{Ctx, Env, Error, Event, Module, Msg, Origin, StateRoot};
use tasks::{JobsEvent, encode_job_event as jobs_encode_event};

/// a minimal `Ctx`: drives `execute` with a controllable env, serves a canned
/// agent registry and chat transcripts (context pins), and answers the
/// dispatch module's turn-claim probe with "not taken".
struct TestCtx {
    env: Env,
    agents: BTreeMap<String, AgentRecord>,
    transcripts: BTreeMap<String, Vec<MessageView>>,
}
impl TestCtx {
    fn new(height: u64, origin: Origin) -> Self {
        Self {
            env: Env {
                height,
                consensus_time: height,
                origin,
                me: "runs".into(),
            },
            agents: BTreeMap::new(),
            transcripts: BTreeMap::new(),
        }
    }
    fn with_agent(mut self, agent_id: &str, actions: &[&str]) -> Self {
        self.agents.insert(
            agent_id.into(),
            AgentRecord {
                agent_id: agent_id.into(),
                owner: SagaOrigin::External(b"alice".to_vec()),
                display_name: agent_id.to_uppercase(),
                capability: "model-1".into(),
                allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
                status: AgentStatus::Active,
                role: agent::AgentRole::General,
                created_at: 0,
                updated_at: 0,
                recipe_hash: Vec::new(),
                caps: agent::ResourceCaps::default(),
                skills: Vec::new(),
            },
        );
        self
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
        match target {
            "agent" => match agent_decode_query(req).map_err(Error::Module)? {
                AgentQuery::Agent { agent_id } => Ok(agent_encode_reply(&AgentReply::Agent(
                    self.agents.get(&agent_id).cloned(),
                ))),
                AgentQuery::Agents => Ok(agent_encode_reply(&AgentReply::Agents(
                    self.agents.values().cloned().collect(),
                ))),
            },
            "dispatch" => match dispatch_decode_query(req).map_err(Error::Module)? {
                DispatchQuery::Dispatch { .. } => {
                    Ok(dispatch_encode_reply(&DispatchReply::Dispatch(None)))
                }
                _ => Err(Error::QueryUnsupported),
            },
            "chat" => match chat_decode_query(req).map_err(Error::Module)? {
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
                // every channel this fixture serves a transcript for is open
                // standing — this round-trip test carries no membership case.
                ChatQuery::Access { .. } => {
                    Ok(chat_encode_reply(&ChatReply::Access(chat::ChannelAccess {
                        may_read: true,
                        may_post: true,
                    })))
                }
                _ => Err(Error::QueryUnsupported),
            },
            other => Err(Error::UnknownModule(other.into())),
        }
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: Event) {}
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
    }
}

fn module() -> RunsModule {
    RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("jobs".into()),
    )
}

fn exec(m: &mut RunsModule, mut ctx: TestCtx, op: &RunsMsg) {
    let msg = Msg {
        target: "runs".into(),
        payload: encode_msg(op),
    };
    block_on(m.execute(&mut ctx, &msg)).unwrap();
}

/// drive the jobs intake — the job-keyspace pending entry's real op path.
fn exec_jobs_event(m: &mut RunsModule, mut ctx: TestCtx, job_id: &str, kind: &str, spec: &str) {
    ctx.env.origin = Origin::Module("jobs".into());
    let msg = Msg {
        target: "runs".into(),
        payload: jobs_encode_event(&JobsEvent::Submitted {
            job_id: job_id.into(),
            kind: kind.into(),
            submitter: "ext:01".into(),
            spec: spec.into(),
            spec_hash: job_spec_hash(spec.as_bytes()),
        }),
    };
    block_on(m.execute(&mut ctx, &msg)).unwrap();
}

fn commit(m: &mut RunsModule) {
    block_on(m.commit_block()).unwrap();
}

fn query_reply(m: &RunsModule, q: &RunsQuery) -> RunsReply {
    decode_reply(&block_on(m.query(&encode_query(q))).unwrap()).unwrap()
}

fn pending(m: &RunsModule, run_id: &str) -> Option<PendingRun> {
    match query_reply(m, &RunsQuery::PendingRuns) {
        RunsReply::PendingRuns(runs) => runs.into_iter().find(|p| p.run_id == run_id),
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn alice() -> Origin {
    Origin::External(b"alice".to_vec())
}

/// a source holding: a watch per turn policy and pending entries in BOTH
/// keyspaces — two chat runs (one with a threaded anchor so the option field
/// is populated) and one job-backed run — all built through the real execute
/// path against a canned registry, never by poking internals.
fn source() -> RunsModule {
    let mut m = module();
    // "general": seqs 1..2 plain, seq 3 a thread reply to root 1.
    let general = vec![
        message_in("general", 1, "root", None),
        message_in("general", 2, "second", None),
        message_in("general", 3, "threaded", Some(1)),
    ];
    let dev = vec![message_in("dev", 1, "hello dev", None)];

    for (channel, policy) in [
        ("general", TurnPolicy::Mention),
        ("dev", TurnPolicy::All),
        ("standup", TurnPolicy::Assigned("ext-bot".into())),
        ("round", TurnPolicy::RoundRobin),
    ] {
        exec(
            &mut m,
            TestCtx::new(1, alice()).with_agent("ext-bot", &[ACTION_CHAT_POST]),
            &RunsMsg::WatchChannel {
                channel_id: channel.into(),
                policy,
            },
        );
    }
    commit(&mut m);

    // three pending entries: a threaded chat run, a plain chat run, and a
    // job-backed run.
    let request = |agent: &str, channel: &str, anchor_seq: u64| RunsMsg::RequestRun {
        agent_id: agent.into(),
        channel_id: channel.into(),
        anchor_seq,
        demands: Default::default(),
        skills: Vec::new(),
    };
    exec(
        &mut m,
        TestCtx::new(2, alice())
            .with_agent("ext-bot", &[ACTION_CHAT_POST])
            .with_transcript("general", general.clone()),
        &request("ext-bot", "general", 3),
    );
    exec(
        &mut m,
        TestCtx::new(2, alice())
            .with_agent("mod-bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE])
            .with_transcript("dev", dev.clone()),
        &request("mod-bot", "dev", 1),
    );
    exec_jobs_event(
        &mut m,
        TestCtx::new(2, Origin::System)
            .with_agent("mod-bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE]),
        "job-1",
        "agent/mod-bot",
        "summarize",
    );
    commit(&mut m);
    m
}

#[test]
fn installed_snapshot_reconstructs_root_and_reads_across_both_keyspaces() {
    let src = source();
    let src_root = src.root();
    assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
    let snap = src.snapshot();

    // the source really covers the space: four policies, chat + job pending
    // entries, a threaded anchor.
    let RunsReply::Watches(watches) = query_reply(&src, &RunsQuery::Watches) else {
        panic!("watches reply expected");
    };
    assert_eq!(watches.len(), 4);
    let threaded = pending(&src, &run_id_for("general", 3, "ext-bot")).expect("threaded entry");
    assert_eq!(
        threaded.thread_root,
        Some(1),
        "the option field is populated"
    );
    assert!(pending(&src, &run_id_for("dev", 1, "mod-bot")).is_some());
    let job = pending(&src, &job_run_id_for("job-1", "mod-bot", 2)).expect("job entry");
    assert_eq!(job.job_id, Some("job-1".into()));
    assert_eq!(job.job_claim_height, 2);

    // the joiner has UNCOMMITTED staged work of its own: install must drop it
    // — a snapshot describes a block boundary, nothing staged may shadow it.
    let mut dst = module();
    exec(
        &mut dst,
        TestCtx::new(0, Origin::External(b"bob".to_vec())),
        &RunsMsg::WatchChannel {
            channel_id: "staged".into(),
            policy: TurnPolicy::All,
        },
    );

    dst.install(&snap, src_root).unwrap();

    // THE PROPERTY: identical root — the root-hash linkage a joiner needs.
    assert_eq!(dst.root(), src_root, "installed root must equal the source");

    // query parity across every surface.
    for q in [RunsQuery::Watches, RunsQuery::PendingRuns] {
        assert_eq!(query_reply(&dst, &q), query_reply(&src, &q));
    }
    let RunsReply::Watches(watches) = query_reply(&dst, &RunsQuery::Watches) else {
        panic!("watches reply expected");
    };
    assert!(
        watches.iter().all(|w| w.channel_id != "staged"),
        "install must clear the staged overlay"
    );

    // the delivered-runs ring is DERIVED state and must never enter the
    // canonical bytes: deliver a run on the source (populating its ring +
    // pruning the entry), then round-trip the new snapshot — it still
    // installs under its root, and the joiner's ring is EMPTY while every
    // canonical read matches.
    let mut src = src;
    let mut ctx = TestCtx::new(3, Origin::Module("dispatch".into()))
        .with_agent("mod-bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE])
        .with_transcript("dev", vec![message_in("dev", 1, "hello dev", None)]);
    let msg = Msg {
        target: "runs".into(),
        payload: dispatch_encode_result_event(&dispatch::ResultEvent {
            dispatch_id: dispatch_id_for(&run_id_for("dev", 1, "mod-bot")),
            recipe_id: "agent/mod-bot".into(),
            outcome: Ok(b"all done".to_vec()),
        }),
    };
    block_on(src.execute(&mut ctx, &msg)).unwrap();
    commit(&mut src);
    let RunsReply::RecentRuns(recent) = query_reply(&src, &RunsQuery::RecentRuns) else {
        panic!("recent runs reply expected");
    };
    assert_eq!(recent.len(), 1, "the delivery landed in the source's ring");

    let delivered_root = src.root();
    let mut joiner = module();
    joiner.install(&src.snapshot(), delivered_root).unwrap();
    assert_eq!(
        joiner.root(),
        delivered_root,
        "canonical bytes round-trip unchanged"
    );
    for q in [RunsQuery::Watches, RunsQuery::PendingRuns] {
        assert_eq!(query_reply(&joiner, &q), query_reply(&src, &q));
    }
    assert_eq!(
        query_reply(&joiner, &RunsQuery::RecentRuns),
        RunsReply::RecentRuns(Vec::new()),
        "the ring never crosses the snapshot"
    );
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
        &RunsMsg::WatchChannel {
            channel_id: "local".into(),
            policy: TurnPolicy::All,
        },
    );
    commit(&mut dst);
    let before_root = dst.root();
    let before_view = query_reply(&dst, &RunsQuery::Watches);

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
    assert_eq!(query_reply(&dst, &RunsQuery::Watches), before_view);

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
    // before anything is built from it: the watch count is at offset 0;
    // corrupting it must reject.
    let mut inflated = snap.clone();
    inflated[0] = inflated[0].wrapping_add(1);
    assert!(
        dst.install(&inflated, src_root).is_err(),
        "an inflated watch count must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

/// The canonical bytes of a minimal one-watch / one-pending-run state, built
/// through the real op path. Tests below walk its length-prefixed fields rather
/// than pinning offsets that change whenever PendingState gains a field.
fn minimal_snapshot() -> Vec<u8> {
    let owner = Origin::External(vec![5]);
    let mut m = module();
    exec(
        &mut m,
        TestCtx::new(0, owner.clone()),
        &RunsMsg::WatchChannel {
            channel_id: "c".into(),
            policy: TurnPolicy::Mention,
        },
    );
    exec(
        &mut m,
        TestCtx::new(0, owner)
            .with_agent("a", &[])
            .with_transcript("c", vec![message_in("c", 1, "hi", None)]),
        &RunsMsg::RequestRun {
            agent_id: "a".into(),
            channel_id: "c".into(),
            anchor_seq: 1,
            demands: Default::default(),
            skills: Vec::new(),
        },
    );
    commit(&mut m);
    m.snapshot()
}

struct MinimalOffsets {
    watch_policy: usize,
    dispatch_id: usize,
    authority_tag: usize,
    delegation_tag: usize,
    thread_tag: usize,
    job_tag: usize,
    requester: usize,
}

fn minimal_offsets(snapshot: &[u8]) -> MinimalOffsets {
    fn skip_lp(snapshot: &[u8], cursor: &mut usize) -> usize {
        let length = u64::from_le_bytes(snapshot[*cursor..*cursor + 8].try_into().unwrap());
        *cursor += 8;
        let start = *cursor;
        *cursor += usize::try_from(length).unwrap();
        start
    }

    let watch_policy = 8 + 8 + 1;
    let mut cursor = watch_policy + 1 + 8;
    let dispatch_id = skip_lp(snapshot, &mut cursor);
    skip_lp(snapshot, &mut cursor); // run id
    skip_lp(snapshot, &mut cursor); // agent id
    skip_lp(snapshot, &mut cursor); // workspace agent id
    let authority_tag = cursor;
    cursor += 1;
    let delegation_tag = cursor;
    cursor += 1;
    skip_lp(snapshot, &mut cursor); // channel id
    cursor += 8; // anchor seq
    let thread_tag = cursor;
    cursor += 1;
    let job_tag = cursor;
    cursor += 1;
    cursor += 8; // job claim height
    let requester = cursor;
    cursor += 1;
    skip_lp(snapshot, &mut cursor); // external requester bytes
    cursor += 8; // created_at
    cursor += 8; // empty sessions
    cursor += 8; // empty delegations
    assert_eq!(
        cursor,
        snapshot.len(),
        "minimal snapshot walker covers the layout"
    );
    MinimalOffsets {
        watch_policy,
        dispatch_id,
        authority_tag,
        delegation_tag,
        thread_tag,
        job_tag,
        requester,
    }
}

#[test]
fn unknown_discriminants_and_tags_are_rejected() {
    let empty_root = module().root();
    let snap = minimal_snapshot();
    let offsets = minimal_offsets(&snap);

    // Every discriminant and option tag admits only its known values — a state
    // has one valid encoding.
    for (index, what) in [
        (offsets.watch_policy, "watch policy"),
        (offsets.authority_tag, "authority option tag"),
        (offsets.delegation_tag, "delegation option tag"),
        (offsets.thread_tag, "thread-root option tag"),
        (offsets.job_tag, "job-id option tag"),
        (offsets.requester, "requester origin discriminant"),
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

    // the pending key is DERIVED state: an entry whose dispatch id is not
    // the hex sha256 of the run id its fields produce must be rejected, not
    // adopted. the id's first hex char sits right after the section count
    // and the length prefix.
    let mut bad = snap.clone();
    bad[offsets.dispatch_id] = bad[offsets.dispatch_id].wrapping_add(1);
    let mut dst = module();
    let err = dst.install(&bad, StateRoot::ZERO).unwrap_err();
    assert!(
        matches!(err, Error::Module(reason) if reason.contains("dispatch id")),
        "a mismatched dispatch id must be rejected"
    );
    assert_eq!(dst.root(), empty_root);
}

#[test]
fn non_ascending_or_duplicate_keys_are_rejected() {
    // two same-shape watches "a" and "b": their encoded bodies have identical
    // lengths, so swapping the body slices yields a descending-id stream and
    // copying one over the other a duplicate-id stream — both must reject,
    // since sorted-unique keys are what make the encoding canonical.
    let owner = Origin::External(vec![5]);
    let mut m = module();
    for id in ["a", "b"] {
        exec(
            &mut m,
            TestCtx::new(0, owner.clone()),
            &RunsMsg::WatchChannel {
                channel_id: id.into(),
                policy: TurnPolicy::Mention,
            },
        );
    }
    commit(&mut m);
    let snap = m.snapshot();
    let good_root = m.root();
    // watches section: count 8, then two 10-byte bodies; the pending and
    // session and delegation counts trail.
    assert_eq!(snap.len(), 8 + 10 * 2 + 8 + 8 + 8);
    let body_a = snap[8..18].to_vec();
    let body_b = snap[18..28].to_vec();

    for (first, second, what) in [
        (&body_b, &body_a, "descending ids"),
        (&body_a, &body_a, "duplicate ids"),
    ] {
        let mut bytes = snap.clone();
        bytes[8..18].copy_from_slice(first);
        bytes[18..28].copy_from_slice(second);
        let mut dst = module();
        let err = dst.install(&bytes, StateRoot::ZERO).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "{what} must be rejected");
        assert_eq!(dst.root(), module().root());
    }

    // the untouched stream still installs — the rejection above is the
    // ordering check, not an artifact of the splicing.
    let mut dst = module();
    dst.install(&snap, good_root).unwrap();
    assert_eq!(dst.root(), good_root);
}
