//! the job board under test (via the merged `tasks` work module): the full
//! lifecycle, every race/guard rejection, caps, lease clamping, the `Get`
//! point read, origin-derived identity, snapshot/install, and commit/abort
//! staging — plus real-`Host` proofs that first-claim-wins under the host's
//! ordered dispatch. board ENUMERATION (status/kind listings, the census) is
//! the index tier's job now, covered by the native tests in `src/index.rs`.
//!
//! the board lives inside the `tasks` module now, so ops ride the `WorkMsg`
//! envelope (`encode_job_*`) and the combined snapshot carries an empty
//! task-board prefix ahead of the job-board bytes.

use futures::executor::block_on;
use host::{BlockContext, Host, SubmitError};
use sdk::{Ctx, Env, Error, MerkleStore as _, Module, ModuleId, Msg, Origin, StateRoot};
use sdk_testkit::{MemStore, TestCtx};
use tasks::{
    Job, JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply,
    decode_job_event as decode_jobs_event, decode_job_reply as decode_reply,
    encode_job_event as encode_jobs_event, encode_job_msg as encode_msg,
    encode_job_query as encode_query,
};
use tasks::{MAX_ATTEMPTS, MAX_JOBS, MAX_KIND, MAX_PAYLOAD, MAX_SPEC, MAX_WORKERS, Tasks as Jobs};

// the merged work module's genesis id -- the job board now lives here.
const JOBS: &str = "tasks";

/// build the module the way a host does: concrete store first, injected as
/// `Box<dyn MerkleStore>`. these tests assert BEHAVIOR, so the in-memory store
/// stands in for qmdb; the real-store round trip lives in `sync_round_trip`.
fn jobs_on_mem() -> Jobs {
    Jobs::new(JOBS, Box::new(MemStore::new()))
}

// ---- wire builders ---------------------------------------------------------

fn jobs_msg(m: JobsMsg) -> Msg {
    Msg {
        target: JOBS.into(),
        payload: encode_msg(&m),
    }
}

fn submit(job_id: &str, kind: &str, spec: &str) -> Msg {
    jobs_msg(JobsMsg::Submit {
        job_id: job_id.into(),
        kind: kind.into(),
        spec: spec.into(),
    })
}

fn claim(job_id: &str, lease_views: u64) -> Msg {
    jobs_msg(JobsMsg::Claim {
        job_id: job_id.into(),
        lease_views,
    })
}

fn finalize(job_id: &str, ok: bool, payload: &str) -> Msg {
    jobs_msg(JobsMsg::Finalize {
        job_id: job_id.into(),
        ok,
        payload: payload.into(),
    })
}

fn release(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Release {
        job_id: job_id.into(),
    })
}

fn reclaim(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Reclaim {
        job_id: job_id.into(),
    })
}

fn cancel(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Cancel {
        job_id: job_id.into(),
    })
}

fn prune(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Prune {
        job_id: job_id.into(),
    })
}

fn register_worker() -> Msg {
    jobs_msg(JobsMsg::RegisterWorker {})
}

fn unregister_worker() -> Msg {
    jobs_msg(JobsMsg::UnregisterWorker {})
}

fn ext(id: &str) -> Origin {
    Origin::External(id.as_bytes().to_vec())
}

/// the module's own external-actor derivation, mirrored so tests can name the
/// exact worker/submitter string an external origin produces: `ext:` +
/// lowercase hex (domain-separated from module ids and "system").
fn actor(id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::from("ext:");
    for &b in id.as_bytes() {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

// ---- a configurable dispatch ctx (height + origin) -------------------------
// jobs never reads module_root, so no live-module set is needed here; the
// shared TestCtx stands in behind a thin constructor.
fn ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: 0,
        origin,
        me: JOBS.into(),
    })
}

// ---- test helpers ----------------------------------------------------------

/// execute one op at a height and origin, then commit it as its own block.
async fn apply(jobs: &mut Jobs, height: u64, origin: Origin, msg: Msg) {
    jobs.execute(&mut ctx(height, origin), &msg)
        .await
        .expect("op should apply");
    jobs.commit_block().await.expect("commit");
}

/// execute one op WITHOUT committing (leaves it staged).
async fn stage(jobs: &mut Jobs, height: u64, origin: Origin, msg: Msg) -> Result<(), Error> {
    jobs.execute(&mut ctx(height, origin), &msg).await
}

// jobs never reads module_root, so the module set is inert; this forwards to
// `stage`, retained so the module-argument call sites stay put.
async fn stage_with_modules(
    jobs: &mut Jobs,
    height: u64,
    origin: Origin,
    _modules: &[&str],
    msg: Msg,
) -> Result<(), Error> {
    stage(jobs, height, origin, msg).await
}

async fn get(jobs: &Jobs, job_id: &str) -> Option<Job> {
    let JobsReply::Job(job) = decode_reply(
        &jobs
            .query(&encode_query(&JobsQuery::Get {
                job_id: job_id.into(),
            }))
            .await
            .expect("query get"),
    )
    .expect("decode");
    job
}

// ============================================================================
// lifecycle
// ============================================================================

#[test]
fn full_lifecycle_happy_path() {
    block_on(async {
        let mut jobs = jobs_on_mem();

        apply(
            &mut jobs,
            1,
            ext("submitter"),
            submit("j1", "email", "spec-body"),
        )
        .await;
        let job = get(&jobs, "j1").await.expect("exists");
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.attempt, 0);
        assert_eq!(job.submitter, actor("submitter"));
        assert!(job.claim.is_none());
        assert_eq!(job.created_at_height, 1);
        assert_eq!(job.updated_at_height, 1);

        apply(&mut jobs, 2, ext("worker-a"), claim("j1", 50)).await;
        let job = get(&jobs, "j1").await.expect("exists");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(job.attempt, 1);
        let c = job.claim.as_ref().expect("claim");
        assert_eq!(c.worker, actor("worker-a"));
        assert_eq!(c.claimed_at_height, 2);
        assert_eq!(c.lease_views, 50);
        assert_eq!(job.updated_at_height, 2);

        apply(
            &mut jobs,
            3,
            ext("worker-a"),
            finalize("j1", true, "done-payload"),
        )
        .await;
        let job = get(&jobs, "j1").await.expect("exists");
        assert_eq!(job.status, JobStatus::Done);
        let r = job.result.as_ref().expect("result");
        assert!(r.ok);
        assert_eq!(r.payload, "done-payload");
        // the claim is retained for the record.
        assert_eq!(job.claim.as_ref().unwrap().worker, actor("worker-a"));
        assert_eq!(job.updated_at_height, 3);
    });
}

// ============================================================================
// worker registry + event codec
// ============================================================================

#[test]
fn jobs_event_codec_round_trips_submitted() {
    let event = JobsEvent::Submitted {
        job_id: "j1".into(),
        kind: "agent/duck".into(),
        submitter: actor("submitter"),
        spec: "summarize this".into(),
        spec_hash: vec![7u8; 32],
    };
    assert_eq!(
        decode_jobs_event(&encode_jobs_event(&event)).unwrap(),
        event
    );
    assert!(
        decode_jobs_event(b"not an event").is_err(),
        "bad event payloads must not decode"
    );
}

#[test]
fn register_worker_gating_idempotence_unregister_and_cap() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        let empty_root = jobs.root();

        let err = stage_with_modules(&mut jobs, 1, ext("operator"), &[], register_worker())
            .await
            .expect_err("external registration rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("module origin")));
        jobs.abort_block().await.unwrap();

        let err = stage_with_modules(&mut jobs, 1, ext("operator"), &[], unregister_worker())
            .await
            .expect_err("external unregistration rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("module origin")));
        jobs.abort_block().await.unwrap();

        let err = stage_with_modules(&mut jobs, 1, Origin::System, &[], register_worker())
            .await
            .expect_err("system registration rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("module origin")));
        jobs.abort_block().await.unwrap();

        let err = stage_with_modules(&mut jobs, 1, Origin::System, &[], unregister_worker())
            .await
            .expect_err("system unregistration rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("module origin")));
        jobs.abort_block().await.unwrap();

        stage_with_modules(
            &mut jobs,
            2,
            Origin::Module("agent".into()),
            &[],
            register_worker(),
        )
        .await
        .expect("agent module self-registers");
        jobs.commit_block().await.unwrap();
        let registered_root = jobs.root();
        assert_ne!(registered_root, empty_root, "worker set is consensus state");

        stage_with_modules(
            &mut jobs,
            3,
            Origin::Module("agent".into()),
            &[],
            register_worker(),
        )
        .await
        .expect("re-register is idempotent");
        jobs.commit_block().await.unwrap();
        assert_eq!(
            jobs.root(),
            registered_root,
            "idempotent re-register moves no state"
        );

        stage_with_modules(
            &mut jobs,
            4,
            Origin::Module("ghost".into()),
            &[],
            unregister_worker(),
        )
        .await
        .expect("absent unregister is a deterministic no-op");
        jobs.commit_block().await.unwrap();
        assert_eq!(jobs.root(), registered_root);

        stage_with_modules(
            &mut jobs,
            5,
            Origin::Module("agent".into()),
            &[],
            unregister_worker(),
        )
        .await
        .expect("unregister existing worker");
        jobs.commit_block().await.unwrap();
        assert_eq!(
            jobs.root(),
            empty_root,
            "removing the only worker restores root"
        );

        for i in 0..MAX_WORKERS {
            let module = format!("worker-{i:02}");
            stage_with_modules(
                &mut jobs,
                10 + i as u64,
                Origin::Module(module),
                &[],
                register_worker(),
            )
            .await
            .expect("register within cap");
            jobs.commit_block().await.unwrap();
        }
        let err = stage_with_modules(
            &mut jobs,
            99,
            Origin::Module("worker-overflow".into()),
            &[],
            register_worker(),
        )
        .await
        .expect_err("worker cap enforced");
        assert!(matches!(err, Error::Module(m) if m.contains("worker cap reached")));
    });
}

// ============================================================================
// race resolution — first claim wins by consensus order
// ============================================================================

#[test]
fn second_claim_is_rejected_same_and_later_block() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;

        // worker A wins the claim.
        apply(&mut jobs, 2, ext("worker-a"), claim("j1", 100)).await;

        // a second claim in a LATER block loses deterministically.
        let err = stage(&mut jobs, 3, ext("worker-b"), claim("j1", 100))
            .await
            .expect_err("second claim rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("not claimable")));

        let job = get(&jobs, "j1").await.unwrap();
        assert_eq!(
            job.claim.unwrap().worker,
            actor("worker-a"),
            "A still owns it"
        );
        assert_eq!(job.attempt, 1, "the losing claim never bumped attempt");
    });
}

#[test]
fn wrong_worker_finalize_and_release_rejected() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;
        apply(&mut jobs, 2, ext("worker-a"), claim("j1", 100)).await;

        let err = stage(&mut jobs, 3, ext("worker-b"), finalize("j1", true, "x"))
            .await
            .expect_err("wrong worker cannot finalize");
        assert!(
            matches!(err, Error::Module(m) if m.contains("only the current claimant may finalize"))
        );

        let err = stage(&mut jobs, 3, ext("worker-b"), release("j1"))
            .await
            .expect_err("wrong worker cannot release");
        assert!(
            matches!(err, Error::Module(m) if m.contains("only the current claimant may release"))
        );

        // the rightful claimant can both — release returns it to Pending, claim
        // cleared, attempt kept.
        apply(&mut jobs, 4, ext("worker-a"), release("j1")).await;
        let job = get(&jobs, "j1").await.unwrap();
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.claim.is_none());
        assert_eq!(job.attempt, 1, "release keeps the attempt count");
    });
}

#[test]
fn finalize_on_terminal_rejected_result_singularity() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;
        apply(&mut jobs, 2, ext("worker-a"), claim("j1", 100)).await;
        apply(&mut jobs, 3, ext("worker-a"), finalize("j1", true, "first")).await;

        // a second finalize on the now-terminal job is rejected — the result is
        // singular.
        let err = stage(
            &mut jobs,
            4,
            ext("worker-a"),
            finalize("j1", false, "second"),
        )
        .await
        .expect_err("terminal job cannot be re-finalized");
        assert!(matches!(err, Error::Module(m) if m.contains("not in processing")));

        let job = get(&jobs, "j1").await.unwrap();
        assert_eq!(job.status, JobStatus::Done);
        assert_eq!(job.result.unwrap().payload, "first", "result unchanged");
    });
}

// ============================================================================
// permissionless reclaim on lease expiry
// ============================================================================

#[test]
fn premature_reclaim_rejected() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;
        apply(&mut jobs, 5, ext("worker-a"), claim("j1", 10)).await; // deadline = 15

        // reclaim exactly AT the deadline is still premature (needs height > deadline).
        let err = stage(&mut jobs, 15, Origin::System, reclaim("j1"))
            .await
            .expect_err("reclaim at the deadline is premature");
        assert!(matches!(err, Error::Module(m) if m.contains("lease not expired")));

        // and well before it.
        let err = stage(&mut jobs, 10, ext("anyone"), reclaim("j1"))
            .await
            .expect_err("early reclaim rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("lease not expired")));
    });
}

#[test]
fn expired_reclaim_requeues_attempt_kept_claim_cleared() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;
        apply(&mut jobs, 5, ext("worker-a"), claim("j1", 10)).await; // deadline = 15

        // PERMISSIONLESS: a totally unrelated origin can crank the expiry.
        apply(&mut jobs, 16, ext("random-cranker"), reclaim("j1")).await;

        let job = get(&jobs, "j1").await.unwrap();
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.claim.is_none(), "claim cleared on requeue");
        assert_eq!(job.attempt, 1, "attempt count survives the requeue");
        assert_eq!(job.updated_at_height, 16);
    });
}

#[test]
fn attempts_exhausted_reclaim_fails_the_job() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;

        // claim + expired-reclaim, over and over. each claim bumps attempt; each
        // expiry requeues — until attempt hits MAX_ATTEMPTS, when the reclaim
        // fails the job instead.
        let mut height = 10u64;
        loop {
            apply(&mut jobs, height, ext("worker-a"), claim("j1", 10)).await;
            height += 20; // safely past the deadline
            apply(&mut jobs, height, Origin::System, reclaim("j1")).await;
            height += 1;
            let job = get(&jobs, "j1").await.unwrap();
            if job.status != JobStatus::Pending {
                break;
            }
        }

        let job = get(&jobs, "j1").await.unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.attempt, MAX_ATTEMPTS);
        let r = job.result.unwrap();
        assert!(!r.ok);
        assert_eq!(r.payload, "attempts exhausted");
    });
}

// ============================================================================
// cancel / prune (submitter authority)
// ============================================================================

#[test]
fn cancel_only_from_pending_and_only_by_submitter() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;

        // a non-submitter cannot cancel.
        let err = stage(&mut jobs, 2, ext("intruder"), cancel("j1"))
            .await
            .expect_err("non-submitter cannot cancel");
        assert!(matches!(err, Error::Module(m) if m.contains("only the submitter may cancel")));

        // the submitter can, while pending.
        apply(&mut jobs, 3, ext("submitter"), cancel("j1")).await;
        assert_eq!(get(&jobs, "j1").await.unwrap().status, JobStatus::Cancelled);

        // once claimed, even the submitter cannot cancel.
        apply(&mut jobs, 4, ext("submitter"), submit("j2", "k", "")).await;
        apply(&mut jobs, 5, ext("worker-a"), claim("j2", 100)).await;
        let err = stage(&mut jobs, 6, ext("submitter"), cancel("j2"))
            .await
            .expect_err("claimed job cannot be cancelled");
        assert!(matches!(err, Error::Module(m) if m.contains("cancel only applies to pending")));
    });
}

#[test]
fn prune_only_terminal_and_by_submitter_removes_record() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(&mut jobs, 1, ext("submitter"), submit("j1", "k", "")).await;

        // a live (Pending) job cannot be pruned.
        let err = stage(&mut jobs, 2, ext("submitter"), prune("j1"))
            .await
            .expect_err("pending job cannot be pruned");
        assert!(matches!(err, Error::Module(m) if m.contains("prune only applies to terminal")));

        apply(&mut jobs, 3, ext("submitter"), cancel("j1")).await; // now terminal

        // a non-submitter cannot prune.
        let err = stage(&mut jobs, 4, ext("intruder"), prune("j1"))
            .await
            .expect_err("non-submitter cannot prune");
        assert!(matches!(err, Error::Module(m) if m.contains("only the submitter may prune")));

        // the submitter prunes the record out of existence.
        let before = jobs.root();
        apply(&mut jobs, 5, ext("submitter"), prune("j1")).await;
        assert!(get(&jobs, "j1").await.is_none(), "record removed");
        assert_ne!(jobs.root(), before, "prune moves the committed root");
    });
}

// ============================================================================
// caps enforced at execute time (poison-value lesson)
// ============================================================================

#[test]
fn caps_rejection_table() {
    block_on(async {
        let mut jobs = jobs_on_mem();

        let too_long_id = "x".repeat(257);
        let too_long_kind = "k".repeat(MAX_KIND + 1);
        let over_spec = "s".repeat(MAX_SPEC + 1);
        let max_spec = "s".repeat(MAX_SPEC);

        let cases: Vec<(Msg, &str)> = vec![
            (submit("", "k", ""), "job_id must not be empty"),
            (submit(&too_long_id, "k", ""), "job_id exceeds"),
            (submit("ok", "", ""), "kind must not be empty"),
            (submit("ok", &too_long_kind, ""), "kind exceeds"),
            (submit("ok", "k", &over_spec), "spec exceeds"),
        ];
        for (msg, needle) in cases {
            let err = stage(&mut jobs, 1, ext("submitter"), msg)
                .await
                .expect_err("cap violation must reject");
            assert!(
                matches!(err, Error::Module(m) if m.contains(needle)),
                "expected `{needle}`"
            );
        }

        // spec exactly at the cap is accepted.
        apply(
            &mut jobs,
            1,
            ext("submitter"),
            submit("at-cap", "k", &max_spec),
        )
        .await;
        assert!(get(&jobs, "at-cap").await.is_some());

        // duplicate id is rejected.
        apply(&mut jobs, 2, ext("submitter"), submit("dup", "k", "")).await;
        let err = stage(&mut jobs, 3, ext("submitter"), submit("dup", "k", ""))
            .await
            .expect_err("duplicate rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("already exists")));

        // payload cap on finalize.
        apply(&mut jobs, 4, ext("worker-a"), claim("dup", 100)).await;
        let over_payload = "p".repeat(MAX_PAYLOAD + 1);
        let err = stage(
            &mut jobs,
            5,
            ext("worker-a"),
            finalize("dup", true, &over_payload),
        )
        .await
        .expect_err("oversized payload rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("payload exceeds")));
        // exactly at the cap is accepted.
        let max_payload = "p".repeat(MAX_PAYLOAD);
        apply(
            &mut jobs,
            6,
            ext("worker-a"),
            finalize("dup", true, &max_payload),
        )
        .await;
        assert_eq!(get(&jobs, "dup").await.unwrap().status, JobStatus::Done);
    });
}

#[test]
fn max_jobs_cap_is_overlay_aware() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        // fill the board to exactly MAX_JOBS distinct live ids, committing each
        // so the live-count stays O(1).
        for i in 0..MAX_JOBS {
            apply(
                &mut jobs,
                1,
                ext("submitter"),
                submit(&format!("job-{i:05}"), "k", ""),
            )
            .await;
        }
        // the next distinct id is refused.
        let err = stage(
            &mut jobs,
            1,
            ext("submitter"),
            submit("job-overflow", "k", ""),
        )
        .await
        .expect_err("board full");
        assert!(matches!(err, Error::Module(m) if m.contains("job board full")));
    });
}

#[test]
fn lease_views_are_clamped() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        for id in ["lo", "hi", "mid"] {
            apply(&mut jobs, 1, ext("submitter"), submit(id, "k", "")).await;
        }
        apply(&mut jobs, 2, ext("worker-a"), claim("lo", 0)).await;
        apply(&mut jobs, 2, ext("worker-a"), claim("hi", 1_000_000)).await;
        apply(&mut jobs, 2, ext("worker-a"), claim("mid", 500)).await;

        assert_eq!(
            get(&jobs, "lo").await.unwrap().claim.unwrap().lease_views,
            10
        );
        assert_eq!(
            get(&jobs, "hi").await.unwrap().claim.unwrap().lease_views,
            10_000
        );
        assert_eq!(
            get(&jobs, "mid").await.unwrap().claim.unwrap().lease_views,
            500
        );
    });
}

// ============================================================================
// queries
// ============================================================================

#[test]
fn queries_get_hit_and_miss() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(
            &mut jobs,
            1,
            ext("submitter"),
            submit("alpha-1", "email", ""),
        )
        .await;
        apply(
            &mut jobs,
            1,
            ext("submitter"),
            submit("beta-1", "report", ""),
        )
        .await;
        apply(&mut jobs, 2, ext("worker-a"), claim("alpha-1", 100)).await;

        // Get is the whole kept dispatch surface: a hit answers the live
        // record per id, a miss answers None. board enumeration (status/kind
        // listings, the census) is index-tier — the index test
        // `job_lifecycle_moves_partitions_and_census`.
        assert_eq!(
            get(&jobs, "alpha-1").await.unwrap().status,
            JobStatus::Processing
        );
        assert_eq!(
            get(&jobs, "beta-1").await.unwrap().status,
            JobStatus::Pending
        );
        assert!(get(&jobs, "nope").await.is_none());
    });
}

// ============================================================================
// origin-derived identity
// ============================================================================

#[test]
fn identities_are_derived_from_origin() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        apply(
            &mut jobs,
            1,
            Origin::Module("agent".into()),
            submit("j-mod", "k", ""),
        )
        .await;
        apply(&mut jobs, 1, ext("alice"), submit("j-ext", "k", "")).await;
        apply(&mut jobs, 1, Origin::System, submit("j-sys", "k", "")).await;

        assert_eq!(get(&jobs, "j-mod").await.unwrap().submitter, "agent");
        assert_eq!(get(&jobs, "j-ext").await.unwrap().submitter, actor("alice"));
        assert_eq!(get(&jobs, "j-sys").await.unwrap().submitter, "system");

        // the pre-consensus empty-external default is not an authenticated actor.
        let err = stage(
            &mut jobs,
            1,
            Origin::External(Vec::new()),
            submit("j-bad", "k", ""),
        )
        .await
        .expect_err("empty external origin rejected");
        assert!(matches!(err, Error::Module(m) if m.contains("non-empty submitter id")));
    });
}

// ============================================================================
// records / root
// ============================================================================

/// build a board exercising every status + both option fields.
async fn varied_board() -> Jobs {
    let mut jobs = jobs_on_mem();
    // pending
    apply(
        &mut jobs,
        1,
        ext("submitter"),
        submit("a-pending", "email", "s1"),
    )
    .await;
    // processing (claim retained)
    apply(
        &mut jobs,
        1,
        ext("submitter"),
        submit("b-processing", "report", "s2"),
    )
    .await;
    apply(&mut jobs, 2, ext("worker-a"), claim("b-processing", 42)).await;
    // done (result + claim)
    apply(
        &mut jobs,
        1,
        ext("submitter"),
        submit("c-done", "email", "s3"),
    )
    .await;
    apply(&mut jobs, 2, ext("worker-b"), claim("c-done", 100)).await;
    apply(
        &mut jobs,
        3,
        ext("worker-b"),
        finalize("c-done", true, "ok!"),
    )
    .await;
    // failed
    apply(
        &mut jobs,
        1,
        ext("submitter"),
        submit("d-failed", "email", "s4"),
    )
    .await;
    apply(&mut jobs, 2, ext("worker-b"), claim("d-failed", 100)).await;
    apply(
        &mut jobs,
        3,
        ext("worker-b"),
        finalize("d-failed", false, "nope"),
    )
    .await;
    // cancelled
    apply(
        &mut jobs,
        1,
        ext("submitter"),
        submit("e-cancelled", "report", "s5"),
    )
    .await;
    apply(&mut jobs, 4, ext("submitter"), cancel("e-cancelled")).await;
    jobs
}

/// every status shape survives as its own committed record, and `root()` is a
/// pure function of committed state (it reads the store's cached merkle root,
/// so repeated calls cannot drift). the cross-node round trip of these exact
/// records is `sync_round_trip`.
#[test]
fn every_status_shape_is_a_committed_record_and_root_is_stable() {
    block_on(async {
        let jobs = varied_board().await;
        let root = jobs.root();
        assert_eq!(jobs.root(), root, "root is stable across calls");
        assert_ne!(root, StateRoot::ZERO, "a populated board has a real root");

        for (id, status) in [
            ("a-pending", JobStatus::Pending),
            ("b-processing", JobStatus::Processing),
            ("c-done", JobStatus::Done),
            ("d-failed", JobStatus::Failed),
            ("e-cancelled", JobStatus::Cancelled),
        ] {
            assert_eq!(get(&jobs, id).await.expect("job exists").status, status);
        }

        // the module is qmdb-backed: sync rides the store's resolver lane.
        match jobs.state_sync_handle().expect("handle") {
            sdk::StateSyncHandle::ResolverBacked { backend, .. } => assert_eq!(backend, "qmdb"),
            other => panic!("expected ResolverBacked, got {other:?}"),
        }
    });
}

#[test]
fn claim_attempt_saturates_instead_of_wrapping() {
    block_on(async {
        // attempt counts are only ever produced by claim, so u64::MAX is not
        // execute-reachable: seed the store record directly (the store is
        // injected, so a test can write one) to prove the increment saturates
        // instead of wrapping.
        let mut store = MemStore::new();
        let job = serde_json::json!({
            "job_id": "j1",
            "kind": "k",
            "spec": "spec",
            "submitter": "ext:00",
            "status": "pending",
            "attempt": u64::MAX,
            "claim": null,
            "result": null,
            "created_at_height": 1,
            "updated_at_height": 1,
        });
        store
            .commit_batch(vec![
                (
                    sdk::store_key(b"j/j1"),
                    Some(serde_json::to_vec(&job).unwrap()),
                ),
                (sdk::store_key(b"j#"), Some(1u64.to_le_bytes().to_vec())),
            ])
            .await
            .expect("seed the store");
        let mut jobs = Jobs::new(JOBS, Box::new(store));

        apply(&mut jobs, 5, ext("worker-a"), claim("j1", 50)).await;
        let job = get(&jobs, "j1").await.expect("exists");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(job.attempt, u64::MAX, "saturated, not wrapped to zero");
    });
}

// ============================================================================
// committed-only query visibility
// ============================================================================

#[test]
fn queries_answer_committed_state_only() {
    block_on(async {
        let mut jobs = jobs_on_mem();

        // a staged-but-uncommitted submit is invisible to the read surface.
        stage(&mut jobs, 1, ext("submitter"), submit("j1", "k", ""))
            .await
            .expect("stage submit");
        assert!(get(&jobs, "j1").await.is_none(), "Get is blind to staging");

        // commit publishes; the same query now sees it.
        jobs.commit_block().await.expect("commit");
        assert_eq!(
            get(&jobs, "j1")
                .await
                .expect("committed, now visible")
                .status,
            JobStatus::Pending
        );

        // a staged transition is equally invisible: claim staged, Get still
        // reports the committed Pending.
        stage(&mut jobs, 2, ext("worker-a"), claim("j1", 50))
            .await
            .expect("stage claim");
        assert_eq!(
            get(&jobs, "j1").await.unwrap().status,
            JobStatus::Pending,
            "the staged claim is not served"
        );
        jobs.commit_block().await.expect("commit claim");
        assert_eq!(
            get(&jobs, "j1").await.unwrap().status,
            JobStatus::Processing
        );
    });
}

// ============================================================================
// staging semantics (commit publishes, abort discards)
// ============================================================================

#[test]
fn commit_and_abort_staging_including_prune_tombstones() {
    block_on(async {
        let mut jobs = jobs_on_mem();
        let root0 = jobs.root();

        // a staged submit moves neither the committed root nor the query view.
        stage(&mut jobs, 1, ext("submitter"), submit("j1", "k", ""))
            .await
            .expect("stage submit");
        assert_eq!(jobs.root(), root0, "staged write must not move the root");
        assert!(
            get(&jobs, "j1").await.is_none(),
            "queries answer committed state only"
        );

        jobs.commit_block().await.unwrap();
        let root1 = jobs.root();
        assert_ne!(root1, root0, "commit moves the root");
        assert!(get(&jobs, "j1").await.is_some(), "committed, now visible");

        // stage a prune tombstone: root unchanged and the committed record is
        // STILL served to queries (the tombstone lives only in the overlay).
        apply(&mut jobs, 2, ext("submitter"), cancel("j1")).await; // terminal
        let root2 = jobs.root();
        stage(&mut jobs, 3, ext("submitter"), prune("j1"))
            .await
            .expect("stage prune");
        assert_eq!(jobs.root(), root2, "staged prune must not move the root");
        assert!(
            get(&jobs, "j1").await.is_some(),
            "queries still serve the committed record"
        );

        // abort discards the tombstone, leaving everything byte-identical.
        jobs.abort_block().await.unwrap();
        assert_eq!(jobs.root(), root2, "abort keeps the root byte-identical");
        assert!(get(&jobs, "j1").await.is_some(), "the record survived");

        // committing the prune actually removes it and moves the root.
        apply(&mut jobs, 4, ext("submitter"), prune("j1")).await;
        assert!(get(&jobs, "j1").await.is_none());
        assert_ne!(jobs.root(), root2, "committed prune moves the root");
    });
}

// ============================================================================
// real-Host: first claim wins under the host's ordered dispatch
// ============================================================================

fn as_origin(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 0,
        origin,
    }
}

async fn host_get(host: &Host, job_id: &str) -> Option<Job> {
    let bytes = host
        .query(
            JOBS,
            &encode_query(&JobsQuery::Get {
                job_id: job_id.into(),
            }),
        )
        .await
        .expect("host query");
    let JobsReply::Job(job) = decode_reply(&bytes).expect("decode");
    job
}

/// a worker module that claims every submitted job it is notified about.
struct ClaimingWorker {
    id: ModuleId,
}

#[async_trait::async_trait(?Send)]
impl Module for ClaimingWorker {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let JobsEvent::Submitted { job_id, .. } =
            decode_jobs_event(&msg.payload).map_err(Error::Module)?;
        ctx.emit_msg(claim(&job_id, 100));
        Ok(())
    }
}

#[test]
fn host_submit_fans_out_to_registered_worker_and_claims_same_block() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(jobs_on_mem()),
            Box::new(ClaimingWorker { id: "agent".into() }),
        ])
        .expect("genesis");

        host.submit_at(
            as_origin(1, Origin::Module("agent".into())),
            register_worker(),
        )
        .await
        .expect("register worker");

        host.submit_at(
            as_origin(2, ext("submitter")),
            submit("j1", "agent/duck", "quack spec"),
        )
        .await
        .expect("submit cascades into claim");

        let job = host_get(&host, "j1").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(
            job.claim.as_ref().map(|claim| claim.worker.as_str()),
            Some("agent"),
            "the worker identity is host-assigned from the module origin"
        );
        assert_eq!(
            job.submitter,
            actor("submitter"),
            "the submitted event carried the origin-derived submitter"
        );
    });
}

#[test]
fn host_first_claim_wins_across_ordered_blocks() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(jobs_on_mem())]).expect("genesis");

        host.submit_at(as_origin(1, ext("submitter")), submit("j1", "k", ""))
            .await
            .expect("submit");

        // block 2: worker A claims — commits.
        host.submit_at(as_origin(2, ext("worker-a")), claim("j1", 100))
            .await
            .expect("A wins the claim");
        let after_a = host.module_root(JOBS).expect("root");

        // block 3: worker B claims the same job — rejected, block aborts.
        let err = host
            .submit_at(as_origin(3, ext("worker-b")), claim("j1", 100))
            .await
            .expect_err("B loses the race");
        assert!(matches!(
            err,
            SubmitError::Rejected(Error::Module(ref m)) if m.contains("not claimable")
        ));

        // the losing claim left no trace: root unchanged, A still the claimant.
        assert_eq!(
            host.module_root(JOBS).unwrap(),
            after_a,
            "loser did not mutate state"
        );
        let job = host_get(&host, "j1").await.unwrap();
        assert_eq!(job.claim.unwrap().worker, actor("worker-a"));
    });
}

/// a helper module that, in ONE block, emits two claims on the same job.
struct DoubleClaim {
    job_id: String,
}

#[async_trait::async_trait(?Send)]
impl Module for DoubleClaim {
    fn id(&self) -> ModuleId {
        "double-claim".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        ctx.emit_msg(claim(&self.job_id, 100));
        ctx.emit_msg(claim(&self.job_id, 100));
        Ok(())
    }
}

#[test]
fn host_two_claims_in_one_block_abort_atomically() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(jobs_on_mem()),
            Box::new(DoubleClaim {
                job_id: "j1".into(),
            }),
        ])
        .expect("genesis");

        host.submit_at(as_origin(1, ext("submitter")), submit("j1", "k", ""))
            .await
            .expect("submit");
        let before = host.module_root(JOBS).expect("root");

        // one block, two claims: the second sees the first's staged Processing
        // and rejects, so the WHOLE block aborts (atomicity).
        let err = host
            .submit_at(
                as_origin(2, ext("trigger")),
                Msg {
                    target: "double-claim".into(),
                    payload: Vec::new(),
                },
            )
            .await
            .expect_err("the second claim aborts the block");
        assert!(matches!(
            err,
            SubmitError::Rejected(Error::Module(ref m)) if m.contains("not claimable")
        ));

        // nothing committed: the job is still exactly Pending, root byte-identical.
        assert_eq!(host.module_root(JOBS).unwrap(), before);
        assert_eq!(
            host_get(&host, "j1").await.unwrap().status,
            JobStatus::Pending
        );
    });
}
