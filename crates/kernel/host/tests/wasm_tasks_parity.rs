//! the adapter-port equivalence proof for the tasks cutover: the `tasks-wasm`
//! component (the NATIVE `tasks` crate compiled to wasm behind `guest-adapter`,
//! with `default-features = false` dropping only the node-local derived index)
//! and the native `Tasks` module answer the SAME op sequence with IDENTICAL
//! query replies, and their roots move in lockstep (move on commit, hold on
//! abort AND on an accepted no-op). the roots THEMSELVES differ — the port
//! persists the native canonical snapshot as one host-KV value, a declared
//! state-schema break (revision 2) — and this proof pins that difference so it
//! can never be mistaken for accidental compatibility.

use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Error, Msg, Origin, StateRoot};
use tasks::{
    decode_reply, encode_msg, encode_query, TaskMsg, TaskQuery, TaskReply, TaskStatus, Tasks,
};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/tasks-wasm` by the module
/// build target; committed so this proof is self-contained.
const TASKS_WASM: &[u8] = include_bytes!("fixtures/tasks.component.wasm");

fn wasm_tasks() -> WasmModule {
    WasmModule::from_bytes("tasks", TASKS_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the tasks
        // canonical state.
        .with_state_schema_revision(2)
}

fn native_host() -> Host {
    Host::genesis(vec![Box::new(Tasks::new("tasks"))]).expect("genesis")
}

fn wasm_host_() -> Host {
    Host::genesis(vec![Box::new(wasm_tasks())]).expect("genesis")
}

/// a 32-byte submitter key (tasks does not gate on authorship; the parity
/// claim only needs the env — origin included — identical on both sides).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn op(m: &TaskMsg) -> Msg {
    Msg {
        target: "tasks".into(),
        payload: encode_msg(m),
    }
}

fn create(task_id: &str, title: &str) -> TaskMsg {
    TaskMsg::CreateTask {
        task_id: task_id.into(),
        title: title.into(),
    }
}

fn update(task_id: &str, status: TaskStatus) -> TaskMsg {
    TaskMsg::UpdateStatus {
        task_id: task_id.into(),
        status,
    }
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, who: &[u8]) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin: Origin::External(who.to_vec()),
        protocol_version: 0,
    }
}

/// the read matrix: tasks' whole query surface is the unpaged `List`.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    vec![
        h.query("tasks", &encode_query(&TaskQuery::List))
            .await
            .expect("query"),
    ]
}

async fn listed(h: &Host) -> Vec<tasks::Task> {
    let reply = h
        .query("tasks", &encode_query(&TaskQuery::List))
        .await
        .expect("query");
    let TaskReply::Tasks(tasks) = decode_reply(&reply).expect("decode");
    tasks
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("tasks").expect("tasks registered")
}

#[test]
fn same_ops_same_replies_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = key(0xA1);

    // the schema break is visible from genesis: the native root hashes the
    // canonical (empty) task encoding, the wasm root commits to the (empty)
    // host-KV store.
    // at GENESIS the roots coincide: this module's native encoding of empty
    // state is the same empty canonical map the wasm host store hashes. the
    // declared schema break manifests on the FIRST WRITE (asserted per block
    // below), which is what the revision-2 fence actually guards.
    assert_eq!(
        root_of(&native),
        root_of(&wasm),
        "empty-state roots coincide by construction"
    );

    // every op family, in one deterministic sequence: both creates land, then
    // status transitions walk t1 through every non-open status and move t2 once.
    let ops: Vec<TaskMsg> = vec![
        create("t1", "ship the port"),
        create("t2", "prove the port"),
        update("t1", TaskStatus::InProgress),
        update("t1", TaskStatus::Done),
        update("t2", TaskStatus::InProgress),
    ];

    for (height, msg) in ops.into_iter().enumerate() {
        let height = height as u64 + 1;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, &alice), op(&msg))
            .await
            .expect("native submit");
        wasm.submit_at(block(height, &alice), op(&msg))
            .await
            .expect("wasm submit");

        // replies identical after every block (the whole read matrix).
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "replies diverge after block {height}"
        );
        // roots move in LOCKSTEP: each of these ops changes state, so both
        // commit boundaries must move their module root...
        assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        // ...to values that differ from each other (the pinned schema break).
        assert_ne!(root_of(&native), root_of(&wasm));
    }

    // an ACCEPTED no-op: a same-status update commits fine but stages nothing
    // (the native early-return), so both roots must HOLD — lockstep is a claim
    // about holds as much as moves.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    for h in [&mut native, &mut wasm] {
        h.submit_at(block(6, &alice), op(&update("t2", TaskStatus::InProgress)))
            .await
            .expect("no-op update applies");
    }
    assert_eq!(root_of(&native), n_before, "native root moved on a no-op");
    assert_eq!(root_of(&wasm), w_before, "wasm root moved on a no-op");
    assert_eq!(replies(&native).await, replies(&wasm).await);

    // decoded spot check: deterministic list order, statuses landed, and the
    // timestamps pin creation to its block and updates to the LAST real
    // transition (the no-op did not restamp updated_at).
    let tasks = listed(&wasm).await;
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "t1");
    assert_eq!(tasks[0].title, "ship the port");
    assert_eq!(tasks[0].status, TaskStatus::Done);
    assert_eq!(tasks[0].created_at, 1_001);
    assert_eq!(tasks[0].updated_at, 1_004);
    assert_eq!(tasks[1].id, "t2");
    assert_eq!(tasks[1].status, TaskStatus::InProgress);
    assert_eq!(tasks[1].created_at, 1_002);
    assert_eq!(tasks[1].updated_at, 1_005);
    assert_eq!(listed(&native).await, tasks);

    // queries are read-only on the wasm side too: the root is STABLE across
    // the whole read matrix.
    let settled = root_of(&wasm);
    let _ = replies(&wasm).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = key(0xA1);

    for host in [&mut native, &mut wasm] {
        host.submit_at(block(1, &alice), op(&create("seed", "already here")))
            .await
            .expect("seed create");
    }

    // the rejection matrix: every distinct refusal family the native module
    // implements, plus the decode failure. each rejected block must leave BOTH
    // roots byte-identical (the abort path: staged writes discarded, no trace).
    let rejects: Vec<(Vec<u8>, &str)> = vec![
        (
            encode_msg(&create("", "no id")),
            "task_id must not be empty",
        ),
        (encode_msg(&create("t9", "")), "title must not be empty"),
        (
            encode_msg(&create("seed", "duplicate")),
            "task already exists",
        ),
        (
            encode_msg(&update("ghost", TaskStatus::Done)),
            "task not found",
        ),
        (
            encode_msg(&update("", TaskStatus::Done)),
            "task_id must not be empty",
        ),
        // a payload that is not a TaskMsg at all: both sides reject with the
        // same serde rendering (decode_msg runs inside the guest too).
        (br#"{"no_such_op":{}}"#.to_vec(), "unknown variant"),
    ];

    for (height, (payload, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 2;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        let msg = Msg {
            target: "tasks".into(),
            payload,
        };

        let n_err = native
            .submit_at(block(height, &alice), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, &alice), msg)
            .await
            .expect_err("wasm must reject");

        // both reject DETERMINISTICALLY with the native module's reason. the
        // wasm runtime wraps the reason in its wit-error rendering, so the
        // parity claim is containment, not string equality.
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

        // abort leaves no trace: both roots byte-identical to pre-block.
        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_eq!(replies(&native).await, replies(&wasm).await);
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = key(0xA1);

    // ONE block, two ops: the second op's existence check READS the first op's
    // staged write (the task only exists in this block's overlay). on the wasm
    // side that is the outer staged `__state` being reloaded by the second
    // dispatch — the read-your-writes seam the adapter relies on.
    let batch = vec![
        (
            Origin::External(alice.clone()),
            op(&create("infra", "stand up the rig")),
        ),
        (
            Origin::External(alice.clone()),
            op(&update("infra", TaskStatus::InProgress)),
        ),
    ];
    let n_out = native
        .submit_block(block(1, &alice), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, &alice), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "both members must apply: {:?}",
            out.members
        );
    }
    assert_eq!(replies(&native).await, replies(&wasm).await);
    // the same-block update landed on the same-block create.
    let tasks = listed(&wasm).await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, TaskStatus::InProgress);

    // ONE block where the SECOND member rejects: the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal the
    // accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (
            Origin::External(alice.clone()),
            op(&update("infra", TaskStatus::Done)),
        ),
        (
            Origin::External(alice.clone()),
            op(&update("ghost", TaskStatus::Done)),
        ),
    ];
    let n_out = native
        .submit_block(block(2, &alice), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, &alice), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    // the accepted member landed (roots moved), the rejected one left nothing.
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        let tasks = listed(host).await;
        assert_eq!(tasks.len(), 1, "a rejected member must create no task");
        assert_eq!(tasks[0].id, "infra");
        assert_eq!(tasks[0].status, TaskStatus::Done);
    }
}
