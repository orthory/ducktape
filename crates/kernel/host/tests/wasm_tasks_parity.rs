//! the STORE-BACKED cutover-continuity proof for the merged `tasks` work
//! module: the tasks guest component over `WasmModule::with_store(QmdbStore)`
//! and the native `Tasks` over the same store shape are ROOT-CONTINUOUS — the
//! same op sequence commits the IDENTICAL qmdb merkle root after every block
//! across BOTH boards (the assigned-list task board AND the first-claim job
//! board). both roots ARE the store's root; qmdb's batch canonicalizes
//! mutations by hashed key, so the native logical-key commit order and the
//! wasm hashed-key drain order produce the same op log. this executor swap
//! changes not one committed byte — including the byte-identical NO-OP blocks
//! (a same-status task update) that stage nothing on either side.
//!
//! query replies match after every block over the whole read matrix,
//! rejections carry the native reason and leave both roots byte-identical, and
//! multi-dispatch blocks exercise the read-your-writes seam (a claim reading
//! the submit staged one dispatch earlier).

use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Error, Module, Msg, Origin, StateRoot, StateSyncHandle};
use statesync::qmdb::QmdbStore;
use tasks::{
    JobStatus, JobsMsg, JobsQuery, JobsReply, TaskMsg, TaskQuery, TaskReply, TaskStatus, Tasks,
    decode_job_reply, decode_task_reply, encode_job_msg, encode_job_query, encode_task_msg,
    encode_task_query,
};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the module crate's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const TASKS_WASM: &[u8] = include_bytes!("fixtures/tasks.component.wasm");

async fn native_host(context: &deterministic::Context, label: &'static str) -> Host {
    let store = QmdbStore::init(context.child(label), "tasks").await;
    Host::genesis(vec![
        Box::new(identity::Identity::new(
            "identity",
            Box::new(sdk_testkit::MemStore::new()),
            "parity".into(),
        )),
        Box::new(attribution::AttributionModule::new(
            "attribution",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(Tasks::new(
            "tasks",
            "identity",
            "attribution",
            Box::new(store),
        )),
    ])
    .expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context, label: &'static str) -> Host {
    let store = QmdbStore::init(context.child(label), "tasks").await;
    Host::genesis(vec![
        Box::new(identity::Identity::new(
            "identity",
            Box::new(sdk_testkit::MemStore::new()),
            "parity".into(),
        )),
        Box::new(attribution::AttributionModule::new(
            "attribution",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(
            WasmModule::with_store("tasks", TASKS_WASM, Box::new(store)).expect("load component"),
        ),
    ])
    .expect("genesis")
}

fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn ext(who: &[u8]) -> Origin {
    Origin::External(who.to_vec())
}

fn op_task(m: &TaskMsg) -> Msg {
    Msg {
        target: "tasks".into(),
        payload: encode_task_msg(m),
    }
}

fn op_job(m: &JobsMsg) -> Msg {
    Msg {
        target: "tasks".into(),
        payload: encode_job_msg(m),
    }
}

fn create(task_id: &str, title: &str) -> TaskMsg {
    TaskMsg::CreateTask {
        task_id: task_id.into(),
        title: title.into(),
        owner: None,
    }
}

fn update(task_id: &str, status: TaskStatus) -> TaskMsg {
    TaskMsg::UpdateStatus {
        task_id: task_id.into(),
        status,
    }
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

/// the read matrix: the task board's whole surface (a `List` page and a `Get`,
/// hit and absent) plus the job board's kept dispatch read — `Get`, hit and
/// absent shapes alike (board enumeration is the index guest's job on the
/// derived tier).
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        encode_task_query(&whole_board()),
        encode_task_query(&TaskQuery::Get {
            task_id: "t1".into(),
        }),
        encode_task_query(&TaskQuery::Get {
            task_id: "absent".into(),
        }),
        encode_job_query(&JobsQuery::Get {
            job_id: "build".into(),
        }),
        encode_job_query(&JobsQuery::Get {
            job_id: "deploy".into(),
        }),
        encode_job_query(&JobsQuery::Get {
            job_id: "absent".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("tasks", q).await.expect("query"));
    }
    out
}

/// the whole board as ONE page — the parity board is far under the clamp.
fn whole_board() -> TaskQuery {
    TaskQuery::List {
        limit: tasks::MAX_LIST_LIMIT,
        after: None,
    }
}

async fn listed(h: &Host) -> Vec<tasks::Task> {
    let reply = h
        .query("tasks", &encode_task_query(&whole_board()))
        .await
        .expect("query");
    let TaskReply::Tasks(tasks) = decode_task_reply(&reply).expect("decode") else {
        panic!("a list answers a page");
    };
    tasks
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("tasks").expect("tasks registered")
}

#[test]
fn same_ops_same_replies_and_roots_stay_continuous() {
    deterministic::Runner::default().start(|context| async move {
        same_ops_inner(&context).await;
    });
}

async fn same_ops_inner(context: &deterministic::Context) {
    let mut native = native_host(context, "same_native").await;
    let mut wasm = wasm_host_(context, "same_wasm").await;
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    // ROOT CONTINUITY starts at genesis: both roots ARE the (empty) qmdb
    // store's merkle root, so the cutover moves nothing.
    assert_eq!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must match — the port is root-continuous"
    );

    // every op family across BOTH boards, in one deterministic sequence, each
    // accepted op a real state change: the full job lifecycle (submit -> claim
    // -> finalize; claim -> release -> re-claim -> lease-expiry reclaim), cancel
    // + prune, worker (un)registration under a module origin, then the task
    // board's create + status walk. heights are the agreed consensus views.
    let ops: Vec<(u64, Origin, Msg)> = vec![
        (
            1,
            ext(&alice),
            op_job(&JobsMsg::Submit {
                job_id: "build".into(),
                kind: "ci".into(),
                spec: "spec-build".into(),
            }),
        ),
        (
            2,
            ext(&alice),
            op_job(&JobsMsg::Submit {
                job_id: "deploy".into(),
                kind: "cd".into(),
                spec: "spec-deploy".into(),
            }),
        ),
        (
            3,
            ext(&bob),
            op_job(&JobsMsg::Claim {
                job_id: "build".into(),
                lease_views: 100,
            }),
        ),
        (
            4,
            ext(&bob),
            op_job(&JobsMsg::Finalize {
                job_id: "build".into(),
                ok: true,
                payload: "artifact-digest".into(),
            }),
        ),
        (
            5,
            ext(&bob),
            op_job(&JobsMsg::Claim {
                job_id: "deploy".into(),
                lease_views: 50,
            }),
        ),
        (
            6,
            ext(&bob),
            op_job(&JobsMsg::Release {
                job_id: "deploy".into(),
            }),
        ),
        (
            7,
            ext(&carol),
            op_job(&JobsMsg::Claim {
                job_id: "deploy".into(),
                // below MIN_LEASE_VIEWS: clamps to 10, so the deadline is 17.
                lease_views: 1,
            }),
        ),
        (
            // PERMISSIONLESS reclaim: bob is not the claimant; only the
            // deterministic deadline (height 18 > 17) authorizes this.
            18,
            ext(&bob),
            op_job(&JobsMsg::Reclaim {
                job_id: "deploy".into(),
            }),
        ),
        (
            19,
            ext(&alice),
            op_job(&JobsMsg::Submit {
                job_id: "temp".into(),
                kind: "tmp".into(),
                spec: "spec-temp".into(),
            }),
        ),
        (
            20,
            ext(&alice),
            op_job(&JobsMsg::Cancel {
                job_id: "temp".into(),
            }),
        ),
        (
            21,
            ext(&alice),
            op_job(&JobsMsg::Prune {
                job_id: "build".into(),
            }),
        ),
        (
            22,
            Origin::Module("agent-fleet".into()),
            op_job(&JobsMsg::RegisterWorker {}),
        ),
        (
            23,
            Origin::Module("agent-fleet".into()),
            op_job(&JobsMsg::UnregisterWorker {}),
        ),
        // now the task board: creates land, then t1 walks every non-open status
        // and t2 moves once.
        (24, ext(&alice), op_task(&create("t1", "ship the port"))),
        (25, ext(&alice), op_task(&create("t2", "prove the port"))),
        (
            26,
            ext(&alice),
            op_task(&update("t1", TaskStatus::InProgress)),
        ),
        (27, ext(&alice), op_task(&update("t1", TaskStatus::Done))),
        (
            28,
            ext(&alice),
            op_task(&update("t2", TaskStatus::InProgress)),
        ),
    ];

    for (height, origin, msg) in ops {
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect("native submit");
        wasm.submit_at(block(height, origin), msg)
            .await
            .expect("wasm submit");

        // replies identical after every block (the whole read matrix).
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "replies diverge after block {height}"
        );
        // roots move in LOCKSTEP: every op above changes state, so both commit
        // boundaries must move their module root...
        assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        // ...to the SAME value: the executor changed, not one committed byte.
        assert_eq!(
            root_of(&native),
            root_of(&wasm),
            "roots diverge after block {height}"
        );
    }

    // an ACCEPTED no-op on the task board: a same-status update commits fine but
    // stages nothing (the native early-return), so both roots must HOLD.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    for h in [&mut native, &mut wasm] {
        h.submit_at(
            block(29, ext(&alice)),
            op_task(&update("t2", TaskStatus::InProgress)),
        )
        .await
        .expect("no-op update applies");
    }
    assert_eq!(root_of(&native), n_before, "native root moved on a no-op");
    assert_eq!(root_of(&wasm), w_before, "wasm root moved on a no-op");
    assert_eq!(replies(&native).await, replies(&wasm).await);

    // decoded job spot check: the reclaim requeued the job (attempt kept, claim
    // cleared), created/updated heights pinned to submit/reclaim.
    let reply = wasm
        .query(
            "tasks",
            &encode_job_query(&JobsQuery::Get {
                job_id: "deploy".into(),
            }),
        )
        .await
        .expect("get query");
    let JobsReply::Job(Some(job)) = decode_job_reply(&reply).expect("decode") else {
        panic!("expected a live job");
    };
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.attempt, 2);
    assert!(job.claim.is_none());
    assert!(job.result.is_none());
    assert_eq!(job.created_at_height, 2);
    assert_eq!(job.updated_at_height, 18);

    // per-id spot checks (the census itself is index-tier now): the pruned
    // job is GONE, the cancelled one terminal-but-retained (the reclaimed one
    // is asserted Pending above).
    let reply = wasm
        .query(
            "tasks",
            &encode_job_query(&JobsQuery::Get {
                job_id: "build".into(),
            }),
        )
        .await
        .expect("get query");
    assert_eq!(
        decode_job_reply(&reply).expect("decode"),
        JobsReply::Job(None),
        "the pruned job answers None"
    );
    let reply = wasm
        .query(
            "tasks",
            &encode_job_query(&JobsQuery::Get {
                job_id: "temp".into(),
            }),
        )
        .await
        .expect("get query");
    let JobsReply::Job(Some(job)) = decode_job_reply(&reply).expect("decode") else {
        panic!("the cancelled job is retained");
    };
    assert_eq!(job.status, JobStatus::Cancelled);

    // decoded task spot check: deterministic list order, statuses landed, and
    // the timestamps pin creation to its block and updates to the LAST real
    // transition (the no-op did not restamp updated_at).
    let tasks = listed(&wasm).await;
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, "t1");
    assert_eq!(tasks[0].title, "ship the port");
    assert_eq!(tasks[0].status, TaskStatus::Done);
    assert_eq!(tasks[0].created_at, 1_024);
    assert_eq!(tasks[0].updated_at, 1_027);
    assert_eq!(tasks[1].id, "t2");
    assert_eq!(tasks[1].status, TaskStatus::InProgress);
    assert_eq!(tasks[1].created_at, 1_025);
    assert_eq!(tasks[1].updated_at, 1_028);
    assert_eq!(listed(&native).await, tasks);

    // queries are read-only on the wasm side too: the root is STABLE across the
    // whole read matrix.
    let settled = root_of(&wasm);
    let _ = replies(&wasm).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        rejections_inner(&context).await;
    });
}

async fn rejections_inner(context: &deterministic::Context) {
    let mut native = native_host(context, "rej_native").await;
    let mut wasm = wasm_host_(context, "rej_wasm").await;
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    // seed one task and one claimed job so both boards' guards have live state.
    for host in [&mut native, &mut wasm] {
        host.submit_at(block(1, ext(&alice)), op_task(&create("seed", "here")))
            .await
            .expect("seed task");
        host.submit_at(
            block(2, ext(&alice)),
            op_job(&JobsMsg::Submit {
                job_id: "build".into(),
                kind: "ci".into(),
                spec: "spec-build".into(),
            }),
        )
        .await
        .expect("submit");
        host.submit_at(
            block(3, ext(&bob)),
            op_job(&JobsMsg::Claim {
                job_id: "build".into(),
                lease_views: 100,
            }),
        )
        .await
        .expect("claim");
    }

    // the rejection matrix across BOTH boards. each rejected block must leave
    // BOTH roots byte-identical (the abort path: staged writes discarded).
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        (
            ext(&alice),
            op_task(&create("", "no id")),
            "task_id must be non-empty",
        ),
        (
            // the id cap: every id shares the ONE `t#` index record, so an
            // over-cap id must be refused IDENTICALLY on both ports -- a port
            // that accepted it would fill the index and diverge forever.
            ext(&alice),
            op_task(&create(&"x".repeat(tasks::MAX_TASK_ID + 1), "over cap")),
            "task_id is",
        ),
        (
            ext(&alice),
            op_task(&create("seed", "dup")),
            "task already exists",
        ),
        (
            ext(&alice),
            op_task(&update("ghost", TaskStatus::Done)),
            "task not found",
        ),
        (
            // a payload that is not a work op at all: both sides reject with the
            // same serde rendering (decode runs inside the guest too).
            ext(&alice),
            Msg {
                target: "tasks".into(),
                payload: br#"{"no_such_arm":{}}"#.to_vec(),
            },
            "unknown variant",
        ),
        (
            // the race-resolution signal: bob already won, carol's claim fails.
            ext(&carol),
            op_job(&JobsMsg::Claim {
                job_id: "build".into(),
                lease_views: 100,
            }),
            "job not claimable",
        ),
        (
            ext(&carol),
            op_job(&JobsMsg::Finalize {
                job_id: "build".into(),
                ok: true,
                payload: "stolen".into(),
            }),
            "only the current claimant may finalize",
        ),
        (
            ext(&bob),
            op_job(&JobsMsg::Reclaim {
                job_id: "build".into(),
            }),
            "lease not expired",
        ),
        (
            ext(&alice),
            op_job(&JobsMsg::Cancel {
                job_id: "build".into(),
            }),
            "cancel only applies to pending jobs",
        ),
        (
            ext(&alice),
            op_job(&JobsMsg::Submit {
                job_id: "build".into(),
                kind: "ci".into(),
                spec: "duplicate".into(),
            }),
            "job already exists",
        ),
        (
            Origin::External(Vec::new()),
            op_job(&JobsMsg::Submit {
                job_id: "anon".into(),
                kind: "ci".into(),
                spec: "anon".into(),
            }),
            "non-empty submitter id",
        ),
        (
            ext(&alice),
            op_job(&JobsMsg::RegisterWorker {}),
            "worker registration requires a module origin",
        ),
    ];

    for (height, (origin, msg, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 4;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), msg)
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
    deterministic::Runner::default().start(|context| async move {
        multi_dispatch_inner(&context).await;
    });
}

async fn multi_dispatch_inner(context: &deterministic::Context) {
    let mut native = native_host(context, "multi_native").await;
    let mut wasm = wasm_host_(context, "multi_wasm").await;
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    // ONE block, two ops: the second op's claimability check READS the first
    // op's staged write (the job only exists in this block's overlay). on the
    // wasm side that read falls through `WitStore::get` to the host's OUTER
    // staged overlay — the read-your-writes seam the adapter relies on, since
    // the guest rebuilds the module (and its inner overlay) per dispatch.
    let batch = vec![
        (
            ext(&alice),
            op_job(&JobsMsg::Submit {
                job_id: "hot".into(),
                kind: "ci".into(),
                spec: "spec-hot".into(),
            }),
        ),
        (
            ext(&bob),
            op_job(&JobsMsg::Claim {
                job_id: "hot".into(),
                lease_views: 100,
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(1, ext(&alice)), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, ext(&alice)), batch)
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

    // ONE block where the SECOND member rejects: the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal the
    // accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (
            ext(&alice),
            op_job(&JobsMsg::Submit {
                job_id: "cold".into(),
                kind: "cd".into(),
                spec: "spec-cold".into(),
            }),
        ),
        (
            // carol's claim lost the race one block ago: "hot" is already
            // Processing under bob's lease.
            ext(&carol),
            op_job(&JobsMsg::Claim {
                job_id: "hot".into(),
                lease_views: 100,
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(2, ext(&alice)), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, ext(&alice)), batch)
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
        let reply = host
            .query(
                "tasks",
                &encode_job_query(&JobsQuery::Get {
                    job_id: "hot".into(),
                }),
            )
            .await
            .expect("query");
        let JobsReply::Job(Some(job)) = decode_job_reply(&reply).expect("decode") else {
            panic!("expected the claimed job");
        };
        // the rejected claim left no trace: still bob's first claim.
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(job.attempt, 1);
        let reply = host
            .query(
                "tasks",
                &encode_job_query(&JobsQuery::Get {
                    job_id: "cold".into(),
                }),
            )
            .await
            .expect("query");
        let JobsReply::Job(Some(job)) = decode_job_reply(&reply).expect("decode") else {
            panic!("expected the accepted member's job");
        };
        assert_eq!(job.status, JobStatus::Pending);
    }
}

/// the store-backed sync surface: the ported guest advertises EXACTLY what the
/// native module does — no byte snapshot, the store's resolver lane.
#[test]
fn sync_handle_matches_native() {
    deterministic::Runner::default().start(|context| async move {
        let native = Tasks::new(
            "tasks",
            "identity",
            "attribution",
            Box::new(QmdbStore::init(context.child("handle_native"), "tasks").await),
        );
        let wasm = WasmModule::with_store(
            "tasks",
            TASKS_WASM,
            Box::new(QmdbStore::init(context.child("handle_wasm"), "tasks").await),
        )
        .expect("load component");

        let n_handle = native.state_sync_handle().expect("native handle");
        let w_handle = wasm.state_sync_handle().expect("wasm handle");
        assert_eq!(n_handle, w_handle, "sync handles diverge");
        assert!(
            matches!(w_handle, StateSyncHandle::ResolverBacked { ref backend, .. } if backend == "qmdb"),
            "store-backed tenant must stay resolver-backed: {w_handle:?}"
        );
        assert!(
            native.snapshot_bytes().is_none(),
            "a store-backed module ships no byte snapshot"
        );
    });
}
