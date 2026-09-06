//! the adapter-port equivalence proof for the runs cutover — the FINAL
//! portable module: the `runs` guest component (the NATIVE `runs` crate
//! compiled to wasm behind `guest-adapter`) and the native `RunsModule`
//! answer the SAME op sequence with IDENTICAL query replies, emit IDENTICAL
//! event traces (WorkerRequests included), land IDENTICAL follow-ups on every
//! sibling, and their roots move in lockstep. the roots THEMSELVES differ:
//! the port persists the native canonical snapshot as one host-KV value plus
//! the delivered-runs ring under its own key. This proof pins that current v1
//! layout from genesis (runs' empty canonical encoding carries THREE zero
//! counts, the empty host-KV store ONE — different preimages).
//!
//! runs is the collaboration loop's actor, so this proof drives the REAL
//! loop (the `crates/modules/apps/runs/tests/collaboration_loop.rs` shape) through
//! both runtimes side by side: both hosts carry the REAL native siblings
//! under the production ids (`bin/node/src/host_state.rs`) — chat and pages
//! over their own qmdb stores, tagging, saga, DISPATCH (native in production
//! too: its read facade serves COMMITTED-ONLY state by design, and runs'
//! `turn_taken` / `lease_holder` reads depend on exactly that view), agent,
//! tasks, jobs, and files over an on-disk odb. forge alone stays unwired
//! (vendored libgit2 has no place in this proof); the runs module documents
//! that degradation — the PR sink and forge-channel compose degrade to
//! breadcrumbs — and the guest carries the production `.with_sink_forge`
//! wiring regardless, identically to the native twin.
//!
//! surfaces this proof leans on beyond the usual reply/root matrix:
//!
//! * THE LOOP (P2/P4/P6): watch → mention → same-block pending entry +
//!   dispatch + saga trigger (+ the WorkerRequest event the reactor feeds
//!   workers from) → oracle result → next-block delivery: the validated
//!   reply (authored as the AGENT), the task write, and the entry prune all
//!   commit in the one delivery block — byte-identically on both runtimes.
//! * READ-YOUR-WRITES: a watch and the mention post that engages it in ONE
//!   block; a duplicate `RequestRun` no-opping against the SAME-BLOCK staged
//!   turn claim — on the wasm side each later dispatch reloads the prior
//!   dispatch's staged `__state`.
//! * THE SESSION LANE: the real lease path (saga Accept → committed
//!   assignee), the open/act authorizations, the budget counter, and the
//!   prune-with-the-run close-out.
//! * THE JOBS LANE: submit → same-block claim + dispatch, delivery-block
//!   finalize.
//! * THE DELIVERED-RUNS RING: derived per-node state outside the NATIVE
//!   root/snapshot, but persisted by the guest under its own `__history` key
//!   (the app's runs client and the dogfood receipt lane read it) — so
//!   `RunsQuery::RecentRuns` answers IDENTICALLY on both runtimes, record
//!   for record, and it sits in the equality matrix like every other reply.

use agent::{
    ACTION_CHAT_POST, ACTION_CHAT_POST_MESSAGE, ACTION_TASKS_CREATE, AgentAction, AgentModule,
    AgentMsg, AgentResponse, ReplyBlock, ResourceCaps, SkillRef, encode_msg as agent_encode_msg,
    encode_response,
};
use capability::CapabilityRegistry;
use chat::{
    AuthorRef, Block, Chat, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use dispatch::{
    DispatchModule, DispatchQuery, DispatchReply, DispatchStatus,
    decode_reply as dispatch_decode_reply, encode_query as dispatch_encode_query,
};
use files::Files;
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use pages::Pages;
use runs::{
    RunsModule, RunsMsg, RunsQuery, RunsReply, TurnPolicy, decode_reply as runs_decode_reply,
    dispatch_id_for, encode_msg as runs_encode_msg, encode_query as runs_encode_query,
    job_run_id_for, reply_message_id, run_id_for,
};
use saga::{
    SagaMsg, SagaQuery, SagaReply, decode_reply as saga_decode_reply, decode_worker_request,
    encode_msg as saga_encode_msg, encode_query as saga_encode_query,
};
use sdk::{Error, Event, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;
use tagging::TaggingModule;
use tasks::{JobsMsg, encode_job_msg as jobs_encode_msg};
use tasks::{
    TaskQuery, TaskReply, Tasks, decode_task_reply as tasks_decode_reply,
    encode_task_query as tasks_encode_query,
};
use valset::Valset;
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `runs` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const RUNS_WASM: &[u8] = include_bytes!("fixtures/runs.component.wasm");

/// the chain id both hosts run on — the genesis `__config` parameter the
/// composer installs into this Map tenant, and the network every `duck://`
/// link the injector renders names. Both sides take it, or the two disagree
/// on the `?net=` of a rendered page link.
const PARITY_CHAIN_ID: &str = "parity#d0cdf950";

fn wasm_runs() -> WasmModule {
    let mut module = WasmModule::from_bytes("runs", RUNS_WASM).expect("load component");
    // exactly what `noded::compose` seeds a Map-backed network-bound tenant
    // with at genesis; without it the guest refuses every dispatch.
    let config = sdk::genesis_config::encode_config(&[("chain_id", PARITY_CHAIN_ID.as_bytes())]);
    let (bytes, root) =
        wasm_host::initial_state(&[(sdk::genesis_config::CONFIG_KEY, config.as_slice())]);
    module.install(&bytes, root).expect("seed genesis config");
    module
}

/// the production wiring, verbatim (`bin/node/src/host_state.rs`) — the exact
/// constructor chain the guest compiles in.
fn native_runs() -> RunsModule {
    RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("tasks".into()),
    )
    .with_files_module("files")
    .with_sink_forge("forge")
    .with_pages_module("pages")
    .with_chain_id(PARITY_CHAIN_ID)
}

/// the shared native sibling set under the production ids. `chat_label` /
/// `pages_label` / `agent_label` keep the two hosts' qmdb runtime children
/// distinct; `files_dir` is each host's own (empty) odb dir — the files root
/// is content-derived, so the two empty instances agree from genesis.
async fn siblings(
    context: &deterministic::Context,
    chat_label: &'static str,
    pages_label: &'static str,
    agent_label: &'static str,
    files_dir: std::path::PathBuf,
    saga: saga::SagaModule,
    assignment_members: Option<&[Vec<u8>]>,
) -> Vec<Box<dyn sdk::Module>> {
    let chat_store = QmdbStore::init(context.child(chat_label), "chat").await;
    let pages_store = QmdbStore::init(context.child(pages_label), "pages").await;
    let agent_store = QmdbStore::init(context.child(agent_label), "agent").await;
    let mut modules: Vec<Box<dyn sdk::Module>> = vec![
        Box::new(Chat::new("chat", Box::new(chat_store)).with_tagging("tagging")),
        Box::new(Pages::new("pages", Box::new(pages_store)).with_tagging("tagging")),
        Box::new(TaggingModule::new(
            "tagging",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(saga),
        Box::new(DispatchModule::new(
            "dispatch",
            "saga",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(AgentModule::new(
            "agent",
            Box::new(agent_store),
            "saga",
            Some("runs".into()),
        )),
        Box::new(Tasks::new("tasks", Box::new(sdk_testkit::MemStore::new()))),
        Box::new(Files::open("files", files_dir).expect("files open")),
    ];
    if let Some(members) = assignment_members {
        let mut valset = Valset::new(
            "valset",
            Box::new(sdk_testkit::MemStore::new()),
            "governance",
        );
        for member in members {
            valset.seed(member.clone()).await.expect("seed valset");
        }
        valset.finish_seed().await.expect("seed valset");
        modules.push(Box::new(valset));
        modules.push(Box::new(CapabilityRegistry::new(
            "capability",
            Box::new(sdk_testkit::MemStore::new()),
            Some("valset".into()),
        )));
    }
    modules
}

async fn native_host(context: &deterministic::Context, files_dir: std::path::PathBuf) -> Host {
    let mut modules = siblings(
        context,
        "native_chat",
        "native_pages",
        "native_agent",
        files_dir,
        saga::SagaModule::new("saga", Box::new(sdk_testkit::MemStore::new())),
        None,
    )
    .await;
    modules.push(Box::new(native_runs()));
    Host::genesis(modules).expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context, files_dir: std::path::PathBuf) -> Host {
    let mut modules = siblings(
        context,
        "wasm_chat",
        "wasm_pages",
        "wasm_agent",
        files_dir,
        saga::SagaModule::new("saga", Box::new(sdk_testkit::MemStore::new())),
        None,
    )
    .await;
    modules.push(Box::new(wasm_runs()));
    Host::genesis(modules).expect("genesis")
}

/// one block's agreed context. consensus_time == height, as on the real
/// validator network (runs stamps `created_at`/`opened_at` from it).
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin,
    }
}

fn alice() -> Origin {
    Origin::External(vec![0xA1; 32])
}

fn runs_op(m: &RunsMsg) -> Msg {
    Msg {
        target: "runs".into(),
        payload: runs_encode_msg(m),
    }
}

fn quackbot_ref() -> AuthorRef {
    AuthorRef::Agent {
        module: "runs".into(),
        agent_id: "quackbot".into(),
    }
}

fn register_quackbot(actions: Vec<String>) -> Msg {
    register_agent("quackbot", "Quackbot", actions, None, None)
}

fn register_agent(
    agent_id: &str,
    display_name: &str,
    actions: Vec<String>,
    caps: Option<ResourceCaps>,
    skills: Option<Vec<SkillRef>>,
) -> Msg {
    Msg {
        target: "agent".into(),
        payload: agent_encode_msg(&AgentMsg::RegisterAgent {
            agent_id: agent_id.into(),
            display_name: display_name.into(),
            capability: "mock-llm-1".into(),
            allowed_actions: actions,
            recipe_hash: None,
            caps,
            skills,
        }),
    }
}

fn create_channel(channel_id: &str) -> Msg {
    Msg {
        target: "chat".into(),
        payload: chat_encode_msg(&ChatMsg::CreateChannel {
            channel_id: channel_id.into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    }
}

fn watch_channel(channel_id: &str) -> Msg {
    runs_op(&RunsMsg::WatchChannel {
        channel_id: channel_id.into(),
        policy: TurnPolicy::Mention,
    })
}

/// a user post mentioning quackbot — what fires the engagement intake.
fn mention_post(channel_id: &str, message_id: &str) -> Msg {
    Msg {
        target: "chat".into(),
        payload: chat_encode_msg(&ChatMsg::PostMessage {
            channel_id: channel_id.into(),
            message_id: message_id.into(),
            blocks: vec![Block::Paragraph(vec![
                Span::plain("hey "),
                Span {
                    text: "@quackbot".into(),
                    marks: vec![Mark::Mention(quackbot_ref())],
                },
                Span::plain(" can you pick this up?"),
            ])],
            thread: None,
            as_agent: None,
        }),
    }
}

/// a mention-free user post — an anchor message that engages nobody.
fn plain_post(channel_id: &str, message_id: &str) -> Msg {
    Msg {
        target: "chat".into(),
        payload: chat_encode_msg(&ChatMsg::PostMessage {
            channel_id: channel_id.into(),
            message_id: message_id.into(),
            blocks: vec![Block::paragraph("an anchor, nothing more")],
            thread: None,
            as_agent: None,
        }),
    }
}

/// One all-policy trigger whose reference set consumes most of Runs' shared
/// sibling-read ledger on the first compose. Later composes may replay those
/// exact reads for free; only their agent and turn lookups are new.
fn reference_post(channel_id: &str, message_id: &str) -> Msg {
    let mut text = (0..50)
        .map(|index| format!("[page-{index}](duck://page/page-{index})"))
        .collect::<Vec<_>>()
        .join(" ");
    text.push_str(" [notes](duck://files/shared/attachments/u/notes.md)");
    Msg {
        target: "chat".into(),
        payload: chat_encode_msg(&ChatMsg::PostMessage {
            channel_id: channel_id.into(),
            message_id: message_id.into(),
            blocks: vec![Block::paragraph(text)],
            thread: None,
            as_agent: None,
        }),
    }
}

/// a benign op to advance one block — what triggers the hosts' committed
/// delivery injection.
fn noop_block(n: u64) -> Msg {
    create_channel(&format!("noop-{n}"))
}

/// the minimal host-assembled runner-result wrapper the oracle ALWAYS
/// delivers around the model's raw text (the collaboration-loop shape).
fn wrap_runner(prose: Vec<u8>) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": String::from_utf8(prose).expect("utf-8 prose"),
        "workspace_receipt": {
            "source_prefix": "/shared/agent-workspaces/bot",
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": true
        }
    }))
    .expect("wrapper serializes")
}

/// the model's RAW text: a strict AgentResponse JSON the in-consensus
/// normalization accepts as-is — one reply paragraph + one task action.
fn canned_response(run_id: &str) -> Vec<u8> {
    encode_response(&AgentResponse {
        reply_blocks: vec![ReplyBlock {
            kind: "paragraph".into(),
            text: format!("quack: handling {run_id}"),
            lang: None,
        }],
        actions: vec![AgentAction::CreateTask {
            task_id: "task-1".into(),
            title: "follow up on the mention".into(),
        }],
        commit_message: None,
    })
}

/// the read matrix: every runs query — the delivered-runs ring included,
/// because the guest persists it under `__history` and must serve it
/// record-for-record (the loop test additionally decodes it).
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        runs_encode_query(&RunsQuery::PendingRuns),
        runs_encode_query(&RunsQuery::Watches),
        runs_encode_query(&RunsQuery::AgentSessions),
        runs_encode_query(&RunsQuery::RecentRuns),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("runs", q).await.expect("query"));
    }
    out
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("runs").expect("runs registered")
}

const SIBLING_IDS: [&str; 9] = [
    "chat", "pages", "tagging", "saga", "dispatch", "agent", "tasks", "jobs", "files",
];

fn event_tuples(events: &[Event]) -> Vec<(String, Vec<u8>)> {
    events
        .iter()
        .map(|e| (e.source.clone(), e.payload.clone()))
        .collect()
}

/// the worker-claimable subset of a block's events — the exact surface the
/// host-side reactor feeds executors from.
fn worker_requests(events: &[Event]) -> Vec<saga::WorkerRequest> {
    events
        .iter()
        .filter_map(|e| decode_worker_request(&e.payload).ok())
        .collect()
}

/// submit one ACCEPTED op to both hosts and assert the full parity claim:
/// identical event traces (decoded work orders included), identical replies,
/// per-sibling cross-host agreement (the follow-up lanes — a diverging or
/// missing dispatch/reply/task/claim diverges the sibling roots), and
/// lockstep runs-root movement.
async fn roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    height: u64,
    origin: Origin,
    m: Msg,
    moves: bool,
) -> Vec<saga::WorkerRequest> {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let n_out = native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect("native submit");
    let w_out = wasm
        .submit_at(block(height, origin), m)
        .await
        .expect("wasm submit");
    assert_eq!(
        event_tuples(&n_out.events),
        event_tuples(&w_out.events),
        "event traces diverge at {height}"
    );
    let (n_reqs, w_reqs) = (
        worker_requests(&n_out.events),
        worker_requests(&w_out.events),
    );
    assert_eq!(
        n_reqs, w_reqs,
        "decoded work orders diverge at {height} — the reactor would feed workers differently"
    );
    assert_eq!(
        replies(native).await,
        replies(wasm).await,
        "replies diverge after block {height}"
    );
    for sibling in SIBLING_IDS {
        assert_eq!(
            native.module_root(sibling),
            wasm.module_root(sibling),
            "the {sibling} sibling diverged at {height}"
        );
    }
    if moves {
        assert_ne!(root_of(native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(wasm), w_before, "wasm root stuck at {height}");
    } else {
        assert_eq!(root_of(native), n_before, "native root moved at {height}");
        assert_eq!(root_of(wasm), w_before, "wasm root moved at {height}");
    }
    // the roots themselves always differ (the pinned schema break).
    assert_ne!(root_of(native), root_of(wasm));
    n_reqs
}

/// submit one REJECTED op to both hosts: reasons carry the same needle, and
/// both runs roots (and every sibling) are byte-identical to pre-block — the
/// abort lane leaves no trace.
async fn reject_roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    height: u64,
    origin: Origin,
    m: Msg,
    needle: &str,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
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
    for sibling in SIBLING_IDS {
        assert_eq!(native.module_root(sibling), wasm.module_root(sibling));
    }
    assert_eq!(replies(native).await, replies(wasm).await);
}

/// the saga id a still-awaiting run's work rides on — read off the dispatch
/// module's COMMITTED-ONLY read facade (identical on both hosts).
async fn dispatch_saga_id(h: &Host, run_id: &str) -> String {
    let reply = h
        .query(
            "dispatch",
            &dispatch_encode_query(&DispatchQuery::Dispatch {
                receiver: "runs".into(),
                dispatch_id: dispatch_id_for(run_id),
            }),
        )
        .await
        .expect("dispatch query");
    match dispatch_decode_reply(&reply).expect("decode") {
        DispatchReply::Dispatch(Some(view)) => match view.status {
            DispatchStatus::AwaitingResult { saga_id } => saga_id,
            other => panic!("the run's dispatch must await its saga, got {other:?}"),
        },
        other => panic!("expected the run's dispatch, got {other:?}"),
    }
}

/// the run's committed lease-holder — saga's `assignee`, read directly off the
/// saga the dispatch names (identical on both hosts).
async fn dispatch_assignee(h: &Host, run_id: &str) -> Option<Vec<u8>> {
    let saga_id = dispatch_saga_id(h, run_id).await;
    let reply = h
        .query("saga", &saga_encode_query(&SagaQuery::Get { saga_id }))
        .await
        .expect("saga query");
    match saga_decode_reply(&reply).expect("decode") {
        SagaReply::Saga(view) => view.and_then(|v| v.assignee),
        other => panic!("expected the run's saga, got {other:?}"),
    }
}

async fn oracle_op(h: &Host, run_id: &str, raw: Vec<u8>) -> Msg {
    let saga_id = dispatch_saga_id(h, run_id).await;
    Msg {
        target: "saga".into(),
        payload: saga_encode_msg(&SagaMsg::OracleResult {
            saga_id,
            attempt: 0,
            outcome: Ok(raw),
            usage: None,
        }),
    }
}

async fn chat_message(h: &Host, message_id: &str) -> Option<chat::MessageView> {
    let reply = h
        .query(
            "chat",
            &chat_encode_query(&ChatQuery::Message {
                message_id: message_id.into(),
            }),
        )
        .await
        .expect("chat query");
    match chat_decode_reply(&reply).expect("decode") {
        ChatReply::Message(view) => view,
        other => panic!("unexpected chat reply: {other:?}"),
    }
}

async fn task_ids(h: &Host) -> Vec<String> {
    let reply = h
        .query(
            "tasks",
            &tasks_encode_query(&TaskQuery::List {
                limit: tasks::MAX_LIST_LIMIT,
                after: None,
            }),
        )
        .await
        .expect("tasks query");
    let TaskReply::Tasks(tasks) = tasks_decode_reply(&reply).expect("decode") else {
        panic!("a list answers a page");
    };
    tasks.into_iter().map(|t| t.id).collect()
}

async fn recent_runs(h: &Host) -> Vec<runs::RunRecord> {
    let reply = h
        .query("runs", &runs_encode_query(&RunsQuery::RecentRuns))
        .await
        .expect("runs query");
    match runs_decode_reply(&reply).expect("decode") {
        RunsReply::RecentRuns(records) => records,
        other => panic!("unexpected runs reply: {other:?}"),
    }
}

async fn pending_run_ids(h: &Host) -> Vec<String> {
    let reply = h
        .query("runs", &runs_encode_query(&RunsQuery::PendingRuns))
        .await
        .expect("runs query");
    match runs_decode_reply(&reply).expect("decode") {
        RunsReply::PendingRuns(runs) => runs.into_iter().map(|p| p.run_id).collect(),
        other => panic!("unexpected runs reply: {other:?}"),
    }
}

async fn agent_sessions(h: &Host) -> Vec<runs::AgentSession> {
    let reply = h
        .query("runs", &runs_encode_query(&RunsQuery::AgentSessions))
        .await
        .expect("runs query");
    match runs_decode_reply(&reply).expect("decode") {
        RunsReply::AgentSessions(sessions) => sessions,
        other => panic!("unexpected runs reply: {other:?}"),
    }
}

#[test]
fn the_collaboration_loop_lands_identically_on_both_runtimes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (native_files, wasm_files) = (dir.path().join("native"), dir.path().join("wasm"));
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context, native_files).await;
        let mut wasm = wasm_host_(&context, wasm_files).await;
        let run_id = run_id_for("general", 1, "quackbot");

        // the SCHEMA-BREAK pin, from genesis: runs' empty canonical encoding
        // is THREE zero counts (watches + pending + sessions), the wasm
        // port's empty host-KV store is ONE — different preimages, different
        // roots (contrast saga/agent, whose single-count empty encodings
        // coincide with the empty store until the first write).
        assert_ne!(
            root_of(&native),
            StateRoot::ZERO,
            "runs has no ZERO sentinel"
        );
        assert_ne!(
            root_of(&native),
            root_of(&wasm),
            "genesis roots must differ — the port is a DECLARED schema break"
        );

        // ---- setup: channel (sibling-only — the runs roots hold), then the
        // watch FIRST (+ the plane subscription, one atomic block): the watch
        // is the port's first guest write, so it initializes the host-KV
        // store (`__state`/`__root` land) in the same block that legitimately
        // moves the native root — every later stage-nothing execute rewrites
        // the identical snapshot and both roots hold. the registry block
        // after it proves exactly that: the hook runs THROUGH runs (the
        // recipe lives in dispatch), and neither root moves.
        roundtrip(
            &mut native,
            &mut wasm,
            1,
            alice(),
            create_channel("general"),
            false,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            2,
            alice(),
            watch_channel("general"),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            3,
            alice(),
            register_quackbot(vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()]),
            false,
        )
        .await;

        // ---- block 4: the user post. THE SAME BLOCK carries the message,
        // the tag report, the engagement delivery, the pending entry, the
        // dispatch, and its saga trigger (P2) — and emits exactly one
        // WorkerRequest for the off-consensus seam, identically.
        let reqs = roundtrip(
            &mut native,
            &mut wasm,
            4,
            alice(),
            mention_post("general", "m1"),
            true,
        )
        .await;
        assert_eq!(reqs.len(), 1, "one WorkerRequest event");
        assert_eq!(pending_run_ids(&wasm).await, vec![run_id.clone()]);

        // ---- block 5: the worker's oracle op. the saga settles and the
        // dispatch module commits the outcome into its mailbox — the runs
        // module sees NOTHING this block (never pop-stack; its root holds).
        let oracle = oracle_op(&native, &run_id, wrap_runner(canned_response(&run_id))).await;
        roundtrip(
            &mut native,
            &mut wasm,
            5,
            Origin::External(b"oracle".to_vec()),
            oracle,
            false,
        )
        .await;
        assert_eq!(
            chat_message(&wasm, &reply_message_id(&run_id)).await,
            None,
            "a result never reaches its receiver in the block that agreed on it"
        );

        // ---- block 6: ANY next block injects the delivery. the ResultEvent,
        // the validated reply (authored as the AGENT), and the task action
        // all commit in this one delivery block — and the entry prunes.
        roundtrip(&mut native, &mut wasm, 6, alice(), noop_block(6), true).await;
        assert_eq!(pending_run_ids(&wasm).await, Vec::<String>::new());
        let reply = chat_message(&wasm, &reply_message_id(&run_id))
            .await
            .expect("the agent's reply landed in chat through the wasm module");
        assert_eq!(reply.head.author, quackbot_ref());
        assert_eq!(
            reply.head.blocks,
            vec![Block::paragraph(format!("quack: handling {run_id}"))]
        );
        assert_eq!(task_ids(&wasm).await, vec!["task-1".to_string()]);

        // ---- the DELIVERED-RUNS-RING pin: the native module recorded the
        // delivery into its per-node ring, and the guest persisted the SAME
        // record through its `__history` key — the receipt lane (run id,
        // outcome, executing-node attribution) survives the whole-state fold
        // record for record.
        let native_ring = recent_runs(&native).await;
        assert_eq!(
            native_ring.len(),
            1,
            "the native ring recorded the delivery"
        );
        assert_eq!(native_ring[0].run_id, run_id);
        assert_eq!(
            native_ring,
            recent_runs(&wasm).await,
            "the ring survives the whole-state fold record for record"
        );

        // queries are read-only on the wasm side too.
        let settled = root_of(&wasm);
        let _ = replies(&wasm).await;
        let _ = recent_runs(&wasm).await;
        assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
    });
}

#[test]
fn multi_compose_exhaustion_degrades_before_the_real_wasm_host_budget() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (native_files, wasm_files) = (dir.path().join("native"), dir.path().join("wasm"));
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context, native_files).await;
        let mut wasm = wasm_host_(&context, wasm_files).await;

        roundtrip(
            &mut native,
            &mut wasm,
            1,
            alice(),
            create_channel("general"),
            false,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            2,
            alice(),
            runs_op(&RunsMsg::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::All,
            }),
            true,
        )
        .await;
        for index in 0..20 {
            let agent_id = format!("bot-{index:02}");
            roundtrip(
                &mut native,
                &mut wasm,
                3 + index,
                alice(),
                register_agent(
                    &agent_id,
                    &agent_id,
                    vec![ACTION_CHAT_POST.into()],
                    None,
                    None,
                ),
                false,
            )
            .await;
        }

        let requests = roundtrip(
            &mut native,
            &mut wasm,
            23,
            alice(),
            reference_post("general", "bounded"),
            true,
        )
        .await;
        assert_eq!(requests.len(), 5, "five complete composes fit the ledger");
        assert_eq!(pending_run_ids(&native).await.len(), 5);
        assert_eq!(pending_run_ids(&wasm).await.len(), 5);
    });
}

#[test]
fn rejections_match_and_leave_no_trace() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (native_files, wasm_files) = (dir.path().join("native"), dir.path().join("wasm"));
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context, native_files).await;
        let mut wasm = wasm_host_(&context, wasm_files).await;

        // an open "general" channel so alice's own chat standing admits the
        // RequestRun rejection case below past the #1630 admission gate —
        // it must fail on the UNKNOWN AGENT it names, not on access.
        roundtrip(
            &mut native,
            &mut wasm,
            1,
            alice(),
            create_channel("general"),
            false,
        )
        .await;

        // every distinct refusal family the runs module implements on its
        // root-op surface: admin-origin gates, field validation, the reserved
        // separator, unknown agents/runs, the session lane's key/lease/ACL
        // doors, and the decode seam.
        let rejects: Vec<(Origin, Msg, &str)> = vec![
            (
                alice(),
                runs_op(&RunsMsg::WatchChannel {
                    channel_id: String::new(),
                    policy: TurnPolicy::Mention,
                }),
                "channel_id must not be empty",
            ),
            (
                alice(),
                runs_op(&RunsMsg::WatchChannel {
                    channel_id: "bad\u{1f}channel".into(),
                    policy: TurnPolicy::Mention,
                }),
                "must not contain the reserved unit separator",
            ),
            (
                alice(),
                runs_op(&RunsMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Assigned("ghost".into()),
                }),
                "assigned agent is not registered: ghost",
            ),
            (
                Origin::System,
                runs_op(&RunsMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Mention,
                }),
                "runs admin ops require an external or module origin",
            ),
            (
                Origin::External(Vec::new()),
                runs_op(&RunsMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Mention,
                }),
                "runs admin ops require a non-empty submitter id",
            ),
            (
                alice(),
                runs_op(&RunsMsg::RequestRun {
                    agent_id: "ghost".into(),
                    channel_id: "general".into(),
                    anchor_seq: 1,
                    demands: Default::default(),
                    skills: Vec::new(),
                }),
                "unknown agent: ghost",
            ),
            (
                Origin::External(Vec::new()),
                runs_op(&RunsMsg::RequestRun {
                    agent_id: "ghost".into(),
                    channel_id: "general".into(),
                    anchor_seq: 1,
                    demands: Default::default(),
                    skills: Vec::new(),
                }),
                "run requests require a non-empty submitter id",
            ),
            (
                alice(),
                runs_op(&RunsMsg::CancelRun {
                    run_id: "nope".into(),
                }),
                "unknown run: nope",
            ),
            (
                alice(),
                runs_op(&RunsMsg::OpenAgentSession {
                    run_id: "nope".into(),
                    session_key: vec![7; 8],
                }),
                "a session key must be 32 bytes",
            ),
            (
                Origin::System,
                runs_op(&RunsMsg::OpenAgentSession {
                    run_id: "nope".into(),
                    session_key: vec![7; 32],
                }),
                "only the node executing a run may open its agent session",
            ),
            (
                alice(),
                runs_op(&RunsMsg::OpenAgentSession {
                    run_id: "nope".into(),
                    session_key: vec![7; 32],
                }),
                "run is not in flight: nope",
            ),
            (
                alice(),
                runs_op(&RunsMsg::AgentAction {
                    run_id: "nope".into(),
                    action: AgentAction::CreateTask {
                        task_id: "t".into(),
                        title: "t".into(),
                    },
                }),
                "run has no open agent session: nope",
            ),
            (
                alice(),
                Msg {
                    target: "runs".into(),
                    payload: b"definitely-not-json".to_vec(),
                },
                "expected value",
            ),
        ];

        for (height, (origin, m, needle)) in rejects.into_iter().enumerate() {
            let height = height as u64 + 2;
            reject_roundtrip(&mut native, &mut wasm, height, origin, m, needle).await;
        }
    });
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (native_files, wasm_files) = (dir.path().join("native"), dir.path().join("wasm"));
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context, native_files).await;
        let mut wasm = wasm_host_(&context, wasm_files).await;

        roundtrip(
            &mut native,
            &mut wasm,
            1,
            alice(),
            create_channel("general"),
            false,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            2,
            alice(),
            create_channel("side"),
            false,
        )
        .await;
        // the first guest write initializes the wasm store in a block that
        // legitimately moves the native root too (see the loop test).
        roundtrip(
            &mut native,
            &mut wasm,
            3,
            alice(),
            watch_channel("warmup"),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            4,
            alice(),
            register_quackbot(vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()]),
            false,
        )
        .await;

        // ---- ONE block, two ops: the watch, then the mention post whose
        // engagement must read the watch STAGED by the first dispatch — on
        // the wasm side the second dispatch reloads the first's staged
        // `__state` (the read-your-writes seam the adapter relies on). the
        // pending entry lands in this same block.
        let batch = vec![
            (alice(), watch_channel("general")),
            (alice(), mention_post("general", "m1")),
        ];
        let n_out = native
            .submit_block(block(5, alice()), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(5, alice()), batch)
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
        assert_eq!(event_tuples(&n_out.events), event_tuples(&w_out.events));
        assert_eq!(replies(&native).await, replies(&wasm).await);
        let run_id = run_id_for("general", 1, "quackbot");
        assert_eq!(
            pending_run_ids(&wasm).await,
            vec![run_id.clone()],
            "the same-block staged watch engaged the post"
        );

        // ---- ONE block, two IDENTICAL explicit requests for an anchor in
        // the UNWATCHED "side" channel: the second must no-op against the
        // turn claim the first STAGED in this very block (staged pending
        // entry, read-your-writes) — an accepted no-op, not a rejection, on
        // both runtimes.
        roundtrip(
            &mut native,
            &mut wasm,
            6,
            alice(),
            plain_post("side", "m2"),
            false,
        )
        .await;
        let request = runs_op(&RunsMsg::RequestRun {
            agent_id: "quackbot".into(),
            channel_id: "side".into(),
            anchor_seq: 1,
            demands: Default::default(),
            skills: Vec::new(),
        });
        let batch = vec![(alice(), request.clone()), (alice(), request)];
        let n_out = native
            .submit_block(block(7, alice()), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(7, alice()), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(
                out.members
                    .iter()
                    .all(|m| matches!(m, MemberOutcome::Applied { .. })),
                "the duplicate turn claim is a silent no-op: {:?}",
                out.members
            );
        }
        assert_eq!(event_tuples(&n_out.events), event_tuples(&w_out.events));
        assert_eq!(replies(&native).await, replies(&wasm).await);
        assert_eq!(
            pending_run_ids(&wasm).await.len(),
            2,
            "two anchors, two runs — the duplicate claimed nothing"
        );

        // ---- ONE block where the MIDDLE member rejects: the runtime aborts
        // its staged overlay and replays the accepted members — committed
        // state equals the accepted subset alone, on both runtimes.
        let batch = vec![
            (alice(), watch_channel("iso-a")),
            (
                alice(),
                runs_op(&RunsMsg::WatchChannel {
                    channel_id: String::new(),
                    policy: TurnPolicy::Mention,
                }),
            ),
            (alice(), watch_channel("iso-b")),
        ];
        let n_out = native
            .submit_block(block(8, alice()), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(8, alice()), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
            assert!(
                matches!(out.members[1], MemberOutcome::Rejected { .. }),
                "the empty channel id must reject mid-block: {:?}",
                out.members
            );
            assert!(matches!(out.members[2], MemberOutcome::Applied { .. }));
        }
        assert_eq!(replies(&native).await, replies(&wasm).await);
        for sibling in SIBLING_IDS {
            assert_eq!(native.module_root(sibling), wasm.module_root(sibling));
        }
        assert_ne!(root_of(&native), root_of(&wasm));
    });
}

/// the ephemeral session keypair's public half (its private half never enters
/// consensus) and the node that will hold the run's execution lease.
const SESSION_KEY: [u8; 32] = [0x11; 32];
const WORKER_NODE: &[u8] = b"worker-node";

#[test]
fn the_session_lane_matches_lease_acl_budget_and_close_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (native_files, wasm_files) = (dir.path().join("native"), dir.path().join("wasm"));
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context, native_files).await;
        let mut wasm = wasm_host_(&context, wasm_files).await;
        let run_id = run_id_for("general", 1, "quackbot");

        // an in-flight run, driven through both runtimes. the watch precedes
        // the registry block so the port's first guest write rides a block
        // that legitimately moves the native root (see the loop test).
        roundtrip(
            &mut native,
            &mut wasm,
            1,
            alice(),
            create_channel("general"),
            false,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            2,
            alice(),
            watch_channel("general"),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            3,
            alice(),
            register_quackbot(vec![
                ACTION_CHAT_POST.into(),
                ACTION_CHAT_POST_MESSAGE.into(),
                ACTION_TASKS_CREATE.into(),
            ]),
            false,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            4,
            alice(),
            mention_post("general", "m1"),
            true,
        )
        .await;

        // the REAL lease path: a capable node claims the announced attempt
        // (saga Accept), and the dispatch facade resolves the committed
        // assignee identically on both hosts.
        let saga_id = dispatch_saga_id(&native, &run_id).await;
        let accept = Msg {
            target: "saga".into(),
            payload: saga_encode_msg(&SagaMsg::Accept {
                saga_id,
                attempt: 0,
            }),
        };
        roundtrip(
            &mut native,
            &mut wasm,
            5,
            Origin::External(WORKER_NODE.to_vec()),
            accept,
            false,
        )
        .await;
        assert_eq!(
            dispatch_assignee(&wasm, &run_id).await.as_deref(),
            Some(WORKER_NODE),
            "the accepting node holds the run's committed lease"
        );

        // a non-assignee cannot open the run's session — not even the owner.
        reject_roundtrip(
            &mut native,
            &mut wasm,
            6,
            alice(),
            runs_op(&RunsMsg::OpenAgentSession {
                run_id: run_id.clone(),
                session_key: SESSION_KEY.to_vec(),
            }),
            "lease",
        )
        .await;

        // the lease-holder binds; the session is committed runs state (in
        // the replies matrix and the root) on both runtimes.
        roundtrip(
            &mut native,
            &mut wasm,
            7,
            Origin::External(WORKER_NODE.to_vec()),
            runs_op(&RunsMsg::OpenAgentSession {
                run_id: run_id.clone(),
                session_key: SESSION_KEY.to_vec(),
            }),
            true,
        )
        .await;
        // re-opening is REFUSED, not overwritten — the live key is the
        // authority the agent is acting under.
        reject_roundtrip(
            &mut native,
            &mut wasm,
            8,
            Origin::External(WORKER_NODE.to_vec()),
            runs_op(&RunsMsg::OpenAgentSession {
                run_id: run_id.clone(),
                session_key: vec![0x22; 32],
            }),
            "already has an open agent session",
        )
        .await;

        // THE ACL: only the bound session key may act — the executing node's
        // own key does not pass.
        reject_roundtrip(
            &mut native,
            &mut wasm,
            9,
            Origin::External(WORKER_NODE.to_vec()),
            runs_op(&RunsMsg::AgentAction {
                run_id: run_id.clone(),
                action: AgentAction::PostMessage {
                    channel_id: "general".into(),
                    text: "not from the agent".into(),
                    thread: None,
                },
            }),
            "only the bound session key may act",
        )
        .await;
        // an action outside the committed grant is refused (the SAME
        // validator the settle path runs).
        reject_roundtrip(
            &mut native,
            &mut wasm,
            10,
            Origin::External(SESSION_KEY.to_vec()),
            runs_op(&RunsMsg::AgentAction {
                run_id: run_id.clone(),
                action: AgentAction::UpdateTaskStatus {
                    task_id: "task-1".into(),
                    status: "done".into(),
                },
            }),
            "not allowed to tasks.update_status",
        )
        .await;

        // a GRANTED mid-run post lands as the AGENT, spends one action of the
        // budget (the counter is committed state — the runs root moves), and
        // the chat sibling roots agree.
        roundtrip(
            &mut native,
            &mut wasm,
            11,
            Origin::External(SESSION_KEY.to_vec()),
            runs_op(&RunsMsg::AgentAction {
                run_id: run_id.clone(),
                action: AgentAction::PostMessage {
                    channel_id: "general".into(),
                    text: "still working — halfway there".into(),
                    thread: None,
                },
            }),
            true,
        )
        .await;
        let sessions = agent_sessions(&wasm).await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].actions, 1, "one action spent");
        let posted = chat_message(&wasm, &runs::post_message_id(&run_id, "s0"))
            .await
            .expect("the mid-run post landed through the wasm module");
        assert_eq!(posted.head.author, quackbot_ref());

        // the run settles through the ordinary path; the session dies with it.
        let oracle = oracle_op(&native, &run_id, wrap_runner(canned_response(&run_id))).await;
        roundtrip(
            &mut native,
            &mut wasm,
            12,
            Origin::External(b"oracle".to_vec()),
            oracle,
            false,
        )
        .await;
        roundtrip(&mut native, &mut wasm, 13, alice(), noop_block(13), true).await;
        assert_eq!(
            agent_sessions(&wasm).await,
            Vec::<runs::AgentSession>::new(),
            "a session never outlives its run"
        );
        reject_roundtrip(
            &mut native,
            &mut wasm,
            14,
            Origin::External(SESSION_KEY.to_vec()),
            runs_op(&RunsMsg::AgentAction {
                run_id: run_id.clone(),
                action: AgentAction::CreateTask {
                    task_id: "after-the-fact".into(),
                    title: "too late".into(),
                },
            }),
            "no open agent session",
        )
        .await;
    });
}

#[test]
fn the_jobs_lane_claims_dispatches_and_finalizes_identically() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (native_files, wasm_files) = (dir.path().join("native"), dir.path().join("wasm"));
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context, native_files).await;
        let mut wasm = wasm_host_(&context, wasm_files).await;

        // the first guest write initializes the wasm store in a block that
        // legitimately moves the native root (see the loop test); the two
        // stage-nothing admin blocks after it hold both roots.
        roundtrip(
            &mut native,
            &mut wasm,
            1,
            alice(),
            watch_channel("warmup"),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            2,
            alice(),
            runs_op(&RunsMsg::EnableJobWorker { enabled: true }),
            false,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            3,
            alice(),
            register_quackbot(vec![ACTION_TASKS_CREATE.into()]),
            false,
        )
        .await;

        // the submit cascades — through the jobs intake running INSIDE the
        // guest — into a same-block claim + dispatch + saga trigger, and the
        // WorkerRequest carries the composed job payload identically.
        let submit = Msg {
            target: "tasks".into(),
            payload: jobs_encode_msg(&JobsMsg::Submit {
                job_id: "job-1".into(),
                kind: "agent/quackbot".into(),
                spec: "summarize this work item".into(),
            }),
        };
        let reqs = roundtrip(&mut native, &mut wasm, 4, alice(), submit, true).await;
        assert_eq!(reqs.len(), 1, "one WorkerRequest event for the job run");
        let run_id = job_run_id_for("job-1", "quackbot", 4);
        assert_eq!(pending_run_ids(&wasm).await, vec![run_id.clone()]);

        // an actions-only response (job runs carry no reply blocks): the
        // delivery block finalizes the board item and lands the task write.
        let raw = encode_response(&AgentResponse {
            reply_blocks: Vec::new(),
            actions: vec![AgentAction::CreateTask {
                task_id: "job-task".into(),
                title: "complete job".into(),
            }],
            commit_message: None,
        });
        let oracle = oracle_op(&native, &run_id, wrap_runner(raw)).await;
        roundtrip(
            &mut native,
            &mut wasm,
            5,
            Origin::External(b"oracle".to_vec()),
            oracle,
            false,
        )
        .await;
        roundtrip(&mut native, &mut wasm, 6, alice(), noop_block(6), true).await;
        assert_eq!(pending_run_ids(&wasm).await, Vec::<String>::new());
        assert_eq!(task_ids(&wasm).await, vec!["job-task".to_string()]);
        // the jobs sibling roots already agreed (roundtrip); a decoded spot
        // check that the board item really finalized through the wasm module.
        let reply = wasm
            .query(
                "tasks",
                &tasks::encode_job_query(&tasks::JobsQuery::Get {
                    job_id: "job-1".into(),
                }),
            )
            .await
            .expect("jobs query");
        let tasks::JobsReply::Job(Some(job)) = tasks::decode_job_reply(&reply).expect("decode")
        else {
            panic!("job expected");
        };
        assert_eq!(job.status, tasks::JobStatus::Done);
        assert!(job.result.expect("finalize result").ok);
    });
}
