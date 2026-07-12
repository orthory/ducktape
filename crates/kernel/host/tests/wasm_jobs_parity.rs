//! the adapter-port equivalence proof for the jobs cutover: the `jobs-wasm`
//! component (the NATIVE `jobs` crate compiled to wasm behind `guest-adapter`)
//! and the native `Jobs` module answer the SAME op sequence with IDENTICAL
//! query replies, and their roots move in lockstep (move on commit, hold on
//! abort). the roots THEMSELVES differ — the port persists the native canonical
//! snapshot as one host-KV value, a declared state-schema break (revision 2) —
//! and this proof pins that difference so it can never be mistaken for
//! accidental compatibility.

use host::{BlockContext, Host, MemberOutcome, SubmitError};
use jobs::{
    decode_reply, encode_msg, encode_query, BoardCounts, JobStatus, Jobs, JobsMsg, JobsQuery,
    JobsReply,
};
use sdk::{Error, Msg, Origin, StateRoot};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/examples/jobs-wasm` by the module
/// build target; committed so this proof is self-contained.
const JOBS_WASM: &[u8] = include_bytes!("fixtures/jobs.component.wasm");

fn wasm_jobs() -> WasmModule {
    WasmModule::from_bytes("jobs", JOBS_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the jobs
        // canonical state — the same declaration bin/node makes.
        .with_state_schema_revision(2)
}

fn native_host() -> Host {
    Host::genesis(vec![Box::new(Jobs::new("jobs"))]).expect("genesis")
}

fn wasm_host_() -> Host {
    Host::genesis(vec![Box::new(wasm_jobs())]).expect("genesis")
}

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn ext(who: &[u8]) -> Origin {
    Origin::External(who.to_vec())
}

fn op(m: &JobsMsg) -> Msg {
    Msg {
        target: "jobs".into(),
        payload: encode_msg(m),
    }
}

/// one block's agreed context: both runtimes must see the identical env.
/// worker (un)registration ops carry a MODULE origin, so the origin is a
/// parameter rather than always-external.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
        protocol_version: 0,
    }
}

/// the read matrix: every query family, including the `None` shapes.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&JobsQuery::Get {
            job_id: "build".into(),
        }),
        encode_query(&JobsQuery::Get {
            job_id: "deploy".into(),
        }),
        encode_query(&JobsQuery::Get {
            job_id: "hot".into(),
        }),
        encode_query(&JobsQuery::Get {
            job_id: "absent".into(),
        }),
        encode_query(&JobsQuery::List {
            status: None,
            kind_prefix: String::new(),
            limit: 64,
        }),
        encode_query(&JobsQuery::List {
            status: Some(JobStatus::Pending),
            kind_prefix: "c".into(),
            limit: 64,
        }),
        encode_query(&JobsQuery::Counts {}),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("jobs", q).await.expect("query"));
    }
    out
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("jobs").expect("jobs registered")
}

#[test]
fn same_ops_same_replies_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    // the schema break is visible from genesis: the native empty root is over
    // the empty canonical encoding, the wasm root commits to the (empty)
    // host-KV store.
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must differ — the port is a DECLARED schema break"
    );

    // every op family, in one deterministic sequence, each accepted op a real
    // state change: the full job lifecycle (submit -> claim -> finalize;
    // claim -> release -> re-claim -> lease-expiry reclaim), cancel + prune,
    // and worker (un)registration under a module origin. heights are the
    // agreed consensus views — they need not be contiguous, and the jump to
    // height 18 is what expires the re-claim's MIN-clamped 10-view lease
    // (claimed at 7, deadline 17).
    let ops: Vec<(u64, Origin, JobsMsg)> = vec![
        (
            1,
            ext(&alice),
            JobsMsg::Submit {
                job_id: "build".into(),
                kind: "ci".into(),
                spec: "spec-build".into(),
            },
        ),
        (
            2,
            ext(&alice),
            JobsMsg::Submit {
                job_id: "deploy".into(),
                kind: "cd".into(),
                spec: "spec-deploy".into(),
            },
        ),
        (
            3,
            ext(&bob),
            JobsMsg::Claim {
                job_id: "build".into(),
                lease_views: 100,
            },
        ),
        (
            4,
            ext(&bob),
            JobsMsg::Finalize {
                job_id: "build".into(),
                ok: true,
                payload: "artifact-digest".into(),
            },
        ),
        (
            5,
            ext(&bob),
            JobsMsg::Claim {
                job_id: "deploy".into(),
                lease_views: 50,
            },
        ),
        (
            6,
            ext(&bob),
            JobsMsg::Release {
                job_id: "deploy".into(),
            },
        ),
        (
            7,
            ext(&carol),
            JobsMsg::Claim {
                job_id: "deploy".into(),
                // below MIN_LEASE_VIEWS: clamps to 10, so the deadline is 17.
                lease_views: 1,
            },
        ),
        (
            // PERMISSIONLESS reclaim: bob is not the claimant; only the
            // deterministic deadline (height 18 > 17) authorizes this.
            18,
            ext(&bob),
            JobsMsg::Reclaim {
                job_id: "deploy".into(),
            },
        ),
        (
            19,
            ext(&alice),
            JobsMsg::Submit {
                job_id: "temp".into(),
                kind: "tmp".into(),
                spec: "spec-temp".into(),
            },
        ),
        (
            20,
            ext(&alice),
            JobsMsg::Cancel {
                job_id: "temp".into(),
            },
        ),
        (
            21,
            ext(&alice),
            JobsMsg::Prune {
                job_id: "build".into(),
            },
        ),
        (
            22,
            Origin::Module("agent-fleet".into()),
            JobsMsg::RegisterWorker {},
        ),
        (
            23,
            Origin::Module("agent-fleet".into()),
            JobsMsg::UnregisterWorker {},
        ),
    ];

    for (height, origin, msg) in ops {
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, origin.clone()), op(&msg))
            .await
            .expect("native submit");
        wasm.submit_at(block(height, origin), op(&msg))
            .await
            .expect("wasm submit");

        // replies identical after every block (the whole read matrix).
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "replies diverge after block {height}"
        );
        // roots move in LOCKSTEP: every accepted jobs op changes state, so
        // both commit boundaries must move their module root...
        assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        // ...to values that differ from each other (the pinned schema break).
        assert_ne!(root_of(&native), root_of(&wasm));
    }

    // decoded spot check: the reclaim requeued the job (attempt count kept for
    // the record, claim cleared), created_at pinned to the submit's block and
    // updated_at to the reclaim's.
    let reply = wasm
        .query(
            "jobs",
            &encode_query(&JobsQuery::Get {
                job_id: "deploy".into(),
            }),
        )
        .await
        .expect("get query");
    let JobsReply::Job(Some(job)) = decode_reply(&reply).expect("decode") else {
        panic!("expected a live job");
    };
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.attempt, 2);
    assert!(job.claim.is_none());
    assert!(job.result.is_none());
    assert_eq!(job.created_at_height, 2);
    assert_eq!(job.updated_at_height, 18);

    // decoded census: the pruned job is GONE (a queue, not an archive), the
    // reclaimed one pending, the cancelled one terminal-but-retained.
    let reply = wasm
        .query("jobs", &encode_query(&JobsQuery::Counts {}))
        .await
        .expect("counts query");
    assert_eq!(
        decode_reply(&reply).expect("decode"),
        JobsReply::Counts(BoardCounts {
            pending: 1,
            processing: 0,
            done: 0,
            failed: 0,
            cancelled: 1,
        })
    );

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
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, ext(&alice)),
            op(&JobsMsg::Submit {
                job_id: "build".into(),
                kind: "ci".into(),
                spec: "spec-build".into(),
            }),
        )
        .await
        .expect("submit");
        host.submit_at(
            block(2, ext(&bob)),
            op(&JobsMsg::Claim {
                job_id: "build".into(),
                lease_views: 100,
            }),
        )
        .await
        .expect("claim");
    }

    // the rejection matrix: every distinct refusal family the native module
    // implements. each rejected block must leave BOTH roots byte-identical
    // (the abort path: staged writes discarded, no trace).
    let rejects: Vec<(Origin, JobsMsg, &str)> = vec![
        (
            // the race-resolution signal: the consensus order already picked
            // bob, so carol's claim fails deterministically on every node.
            ext(&carol),
            JobsMsg::Claim {
                job_id: "build".into(),
                lease_views: 100,
            },
            "job not claimable",
        ),
        (
            ext(&carol),
            JobsMsg::Finalize {
                job_id: "build".into(),
                ok: true,
                payload: "stolen".into(),
            },
            "only the current claimant may finalize",
        ),
        (
            ext(&carol),
            JobsMsg::Release {
                job_id: "build".into(),
            },
            "only the current claimant may release",
        ),
        (
            ext(&bob),
            JobsMsg::Reclaim {
                job_id: "build".into(),
            },
            "lease not expired",
        ),
        (
            ext(&alice),
            JobsMsg::Cancel {
                job_id: "build".into(),
            },
            "cancel only applies to pending jobs",
        ),
        (
            ext(&alice),
            JobsMsg::Prune {
                job_id: "build".into(),
            },
            "prune only applies to terminal jobs",
        ),
        (
            ext(&alice),
            JobsMsg::Submit {
                job_id: "build".into(),
                kind: "ci".into(),
                spec: "duplicate".into(),
            },
            "job already exists",
        ),
        (
            ext(&alice),
            JobsMsg::Cancel {
                job_id: "ghost".into(),
            },
            "job not found",
        ),
        (
            Origin::External(Vec::new()),
            JobsMsg::Submit {
                job_id: "anon".into(),
                kind: "ci".into(),
                spec: "anon".into(),
            },
            "non-empty submitter id",
        ),
        (
            // worker registration is a MODULE-origin op; an external submitter
            // cannot join the fan-out set.
            ext(&alice),
            JobsMsg::RegisterWorker {},
            "worker registration requires a module origin",
        ),
    ];

    for (height, (origin, msg, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 3;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, origin.clone()), op(&msg))
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), op(&msg))
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
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    // ONE block, two ops: the second op's claimability check READS the first
    // op's staged write (the job only exists in this block's overlay). on the
    // wasm side that is the outer staged `__state` being reloaded by the
    // second dispatch — the read-your-writes seam the adapter relies on.
    let batch = vec![
        (
            ext(&alice),
            op(&JobsMsg::Submit {
                job_id: "hot".into(),
                kind: "ci".into(),
                spec: "spec-hot".into(),
            }),
        ),
        (
            ext(&bob),
            op(&JobsMsg::Claim {
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
            op(&JobsMsg::Submit {
                job_id: "cold".into(),
                kind: "cd".into(),
                spec: "spec-cold".into(),
            }),
        ),
        (
            // carol's claim lost the race one block ago: "hot" is already
            // Processing under bob's lease.
            ext(&carol),
            op(&JobsMsg::Claim {
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
                "jobs",
                &encode_query(&JobsQuery::Get {
                    job_id: "hot".into(),
                }),
            )
            .await
            .expect("query");
        let JobsReply::Job(Some(job)) = decode_reply(&reply).expect("decode") else {
            panic!("expected the claimed job");
        };
        // the rejected claim left no trace: still bob's first claim.
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(job.attempt, 1);
        let reply = host
            .query(
                "jobs",
                &encode_query(&JobsQuery::Get {
                    job_id: "cold".into(),
                }),
            )
            .await
            .expect("query");
        let JobsReply::Job(Some(job)) = decode_reply(&reply).expect("decode") else {
            panic!("expected the accepted member's job");
        };
        assert_eq!(job.status, JobStatus::Pending);
    }
}
