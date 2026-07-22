//! the job board under test (via the merged `tasks` work module): the full
//! lifecycle, every race/guard rejection, caps, lease clamping, queries,
//! origin-derived identity, snapshot/install, and commit/abort staging — plus
//! real-`Host` proofs that first-claim-wins under the host's ordered dispatch.
//!
//! the board lives inside the `tasks` module now, so ops ride the `WorkMsg`
//! envelope (`encode_job_*`) and the combined snapshot carries an empty
//! task-board prefix ahead of the job-board bytes.

use futures::executor::block_on;
use host::{BlockContext, Host, SubmitError};
use tasks::{
    Tasks as Jobs, MAX_ATTEMPTS, MAX_JOBS, MAX_KIND, MAX_LIST_LIMIT, MAX_PAYLOAD, MAX_SPEC,
    MAX_WORKERS,
};
use tasks::{
    BoardCounts, Job, JobStatus, JobsEvent, JobsMsg, JobsQuery, JobsReply,
    decode_job_event as decode_jobs_event, decode_job_reply as decode_reply,
    encode_job_event as encode_jobs_event, encode_job_msg as encode_msg,
    encode_job_query as encode_query,
};
use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;

// the merged work module's genesis id -- the job board now lives here.
const JOBS: &str = "tasks";

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
        protocol_version: 0,
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
    match decode_reply(
        &jobs
            .query(&encode_query(&JobsQuery::Get {
                job_id: job_id.into(),
            }))
            .await
            .expect("query get"),
    )
    .expect("decode")
    {
        JobsReply::Job(job) => job,
        other => panic!("expected Job, got {other:?}"),
    }
}

async fn list(jobs: &Jobs, status: Option<JobStatus>, kind_prefix: &str, limit: u64) -> Vec<Job> {
    match decode_reply(
        &jobs
            .query(&encode_query(&JobsQuery::List {
                status,
                kind_prefix: kind_prefix.into(),
                limit,
            }))
            .await
            .expect("query list"),
    )
    .expect("decode")
    {
        JobsReply::Jobs(jobs) => jobs,
        other => panic!("expected Jobs, got {other:?}"),
    }
}

async fn counts(jobs: &Jobs) -> BoardCounts {
    match decode_reply(
        &jobs
            .query(&encode_query(&JobsQuery::Counts {}))
            .await
            .expect("query counts"),
    )
    .expect("decode")
    {
        JobsReply::Counts(counts) => counts,
        other => panic!("expected Counts, got {other:?}"),
    }
}

// ============================================================================
// lifecycle
// ============================================================================

#[test]
fn full_lifecycle_happy_path() {
    block_on(async {
        let mut jobs = Jobs::new(JOBS);

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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
        assert!(list(&jobs, None, "", 256).await.is_empty(), "board empty");
        assert_ne!(jobs.root(), before, "prune moves the committed root");
    });
}

// ============================================================================
// caps enforced at execute time (poison-value lesson)
// ============================================================================

#[test]
fn caps_rejection_table() {
    block_on(async {
        let mut jobs = Jobs::new(JOBS);

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
        let mut jobs = Jobs::new(JOBS);
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
        let mut jobs = Jobs::new(JOBS);
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
fn queries_get_list_filters_and_counts() {
    block_on(async {
        let mut jobs = Jobs::new(JOBS);
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
            submit("alpha-2", "email", ""),
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

        // Get hit + miss.
        assert!(get(&jobs, "alpha-1").await.is_some());
        assert!(get(&jobs, "nope").await.is_none());

        // status filter.
        let pending = list(&jobs, Some(JobStatus::Pending), "", 256).await;
        let ids: Vec<&str> = pending.iter().map(|j| j.job_id.as_str()).collect();
        assert_eq!(ids, ["alpha-2", "beta-1"], "ascending, pending only");

        // kind-prefix filter.
        let emails = list(&jobs, None, "email", 256).await;
        let ids: Vec<&str> = emails.iter().map(|j| j.job_id.as_str()).collect();
        assert_eq!(ids, ["alpha-1", "alpha-2"]);

        // combined filter (status Processing AND kind starts with "em").
        let processing = list(&jobs, Some(JobStatus::Processing), "em", 256).await;
        assert_eq!(processing.len(), 1);
        assert_eq!(processing[0].job_id, "alpha-1");

        // counts.
        let c = counts(&jobs).await;
        assert_eq!(
            c,
            BoardCounts {
                pending: 2,
                processing: 1,
                done: 0,
                failed: 0,
                cancelled: 0,
            }
        );
    });
}

#[test]
fn list_limit_is_clamped_to_256() {
    block_on(async {
        let mut jobs = Jobs::new(JOBS);
        // stage 300 jobs in one block, then commit — queries only see
        // committed state.
        for i in 0..300u32 {
            stage(
                &mut jobs,
                1,
                ext("submitter"),
                submit(&format!("job-{i:03}"), "k", ""),
            )
            .await
            .expect("stage");
        }
        jobs.commit_block().await.expect("commit");
        let listed = list(&jobs, None, "", MAX_LIST_LIMIT * 100).await;
        assert_eq!(
            listed.len(),
            MAX_LIST_LIMIT as usize,
            "limit clamped to 256"
        );
        // the clamp keeps the first 256 in ascending id order.
        assert_eq!(listed.first().unwrap().job_id, "job-000");
        assert_eq!(listed.last().unwrap().job_id, "job-255");
    });
}

// ============================================================================
// origin-derived identity
// ============================================================================

#[test]
fn identities_are_derived_from_origin() {
    block_on(async {
        let mut jobs = Jobs::new(JOBS);
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
// snapshot / install / root
// ============================================================================

/// build a board exercising every status + both option fields.
async fn varied_board() -> Jobs {
    let mut jobs = Jobs::new(JOBS);
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

#[test]
fn snapshot_install_round_trip_and_root_stability() {
    block_on(async {
        let source = varied_board().await;
        let expected = source.root();
        // root is a pure function of committed state.
        assert_eq!(source.root(), expected, "root is stable across calls");

        let bytes = source.snapshot();
        let mut target = Jobs::new(JOBS);
        target
            .install(&bytes, expected)
            .expect("install verified snapshot");
        assert_eq!(target.root(), expected);
        assert_eq!(
            list(&target, None, "", 256).await,
            list(&source, None, "", 256).await,
            "every job survives the round trip"
        );

        // the advertised state-sync handle carries those exact bytes.
        match source.state_sync_handle().expect("handle") {
            sdk::StateSyncHandle::SnapshotBytes(h) => assert_eq!(h, bytes),
            other => panic!("expected SnapshotBytes, got {other:?}"),
        }
    });
}

#[test]
fn install_rejects_wrong_root_and_corrupt_bytes() {
    block_on(async {
        let source = varied_board().await;
        let bytes = source.snapshot();

        // wrong expected root.
        let mut target = Jobs::new(JOBS);
        assert!(matches!(
            target.install(&bytes, StateRoot::ZERO),
            Err(Error::Module(m)) if m.contains("root mismatch")
        ));

        // trailing bytes.
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            target.install(&trailing, source.root()),
            Err(Error::Module(m)) if m.contains("trailing bytes")
        ));

        // truncated.
        assert!(matches!(
            target.install(&bytes[..bytes.len() - 1], source.root()),
            Err(Error::Module(_))
        ));

        // an empty board round-trips too (root of the empty map).
        let empty = Jobs::new(JOBS);
        let mut fresh = Jobs::new(JOBS);
        fresh
            .install(&empty.snapshot(), empty.root())
            .expect("empty install");
        assert_eq!(fresh.root(), empty.root());
    });
}

// ---- hand-crafted snapshot bytes (the module's canonical encoding) ---------

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

/// encode a one-job snapshot with full control over status/claim/result — the
/// only way to present execute-unreachable shapes to `install`.
fn snapshot_one(
    status: u8,
    attempt: u64,
    claim: Option<(&str, u64, u64)>,
    result: Option<(bool, &str)>,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u64.to_le_bytes()); // empty task board (task count)
    out.extend_from_slice(&1u64.to_le_bytes()); // job count
    push_string(&mut out, "j1"); // job_id
    push_string(&mut out, "k"); // kind
    push_string(&mut out, "spec"); // spec
    push_string(&mut out, "ext:00"); // submitter
    out.push(status);
    out.extend_from_slice(&attempt.to_le_bytes());
    match claim {
        None => out.push(0),
        Some((worker, height, lease)) => {
            out.push(1);
            push_string(&mut out, worker);
            out.extend_from_slice(&height.to_le_bytes());
            out.extend_from_slice(&lease.to_le_bytes());
        }
    }
    match result {
        None => out.push(0),
        Some((ok, payload)) => {
            out.push(1);
            out.push(u8::from(ok));
            push_string(&mut out, payload);
        }
    }
    out.extend_from_slice(&1u64.to_le_bytes()); // created_at_height
    out.extend_from_slice(&1u64.to_le_bytes()); // updated_at_height
    out.extend_from_slice(&0u64.to_le_bytes()); // worker count
    out
}

fn snapshot_workers(workers: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u64.to_le_bytes()); // empty task board (task count)
    out.extend_from_slice(&0u64.to_le_bytes()); // job count
    out.extend_from_slice(&(workers.len() as u64).to_le_bytes());
    for worker in workers {
        push_string(&mut out, worker);
    }
    out
}

fn root_of_bytes(bytes: &[u8]) -> StateRoot {
    use sha2::Digest as _;
    StateRoot(sha2::Sha256::digest(bytes).into())
}

#[test]
fn install_rejects_execute_unreachable_shapes() {
    block_on(async {
        // (status byte, claim, result, expected rejection)
        let cases = vec![
            // Processing without a claim would be permanently wedged: no
            // transition (finalize/release/reclaim) can repair it.
            (1, None, None, "processing job without claim"),
            // Pending never carries a claim (submit/release/reclaim all clear it).
            (0, Some(("ext:00", 1, 10)), None, "pending job with claim"),
            // Done/Failed are only ever produced with a stored result.
            (
                2,
                Some(("ext:00", 1, 10)),
                None,
                "finalized job without result",
            ),
            (
                3,
                Some(("ext:00", 1, 10)),
                None,
                "finalized job without result",
            ),
        ];
        for (status, claim, result, needle) in cases {
            let bytes = snapshot_one(status, 1, claim, result);
            let mut target = Jobs::new(JOBS);
            // decode rejects BEFORE the root comparison, so the expected root
            // is irrelevant here.
            let err = target
                .install(&bytes, root_of_bytes(&bytes))
                .expect_err("execute-unreachable shape must be rejected");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains(needle)),
                "expected `{needle}`, got {err:?}"
            );
        }

        // sanity: shapes satisfying the enforced invariants install fine —
        // install checks exactly those four, nothing stricter.
        let ok_cases = vec![
            (0, None, None),                                    // Pending
            (1, Some(("ext:00", 1, 10)), None),                 // Processing
            (2, Some(("ext:00", 1, 10)), Some((true, "done"))), // Done
            (3, None, Some((false, "attempts exhausted"))),     // Failed, result stored
            (4, None, None),                                    // Cancelled (no result)
        ];
        for (status, claim, result) in ok_cases {
            let bytes = snapshot_one(status, 1, claim, result);
            let mut target = Jobs::new(JOBS);
            target
                .install(&bytes, root_of_bytes(&bytes))
                .expect("execute-reachable shape must install");
        }
    });
}

#[test]
fn worker_set_snapshot_round_trip_and_strict_decode() {
    block_on(async {
        let mut source = Jobs::new(JOBS);
        for module in ["agent", "bot"] {
            stage_with_modules(
                &mut source,
                1,
                Origin::Module(module.into()),
                &[],
                register_worker(),
            )
            .await
            .expect("register worker");
        }
        source.commit_block().await.unwrap();

        let bytes = source.snapshot();
        let expected = source.root();
        let mut target = Jobs::new(JOBS);
        target
            .install(&bytes, expected)
            .expect("worker set round trip");
        assert_eq!(target.root(), expected);

        let unsorted = snapshot_workers(&["bot".into(), "agent".into()]);
        let err = target
            .install(&unsorted, root_of_bytes(&unsorted))
            .expect_err("worker ids must be strictly ascending");
        assert!(matches!(err, Error::Module(m) if m.contains("worker ids not strictly ascending")));

        let too_many: Vec<String> = (0..=MAX_WORKERS)
            .map(|i| format!("worker-{i:02}"))
            .collect();
        let capped = snapshot_workers(&too_many);
        let err = target
            .install(&capped, root_of_bytes(&capped))
            .expect_err("worker cap must apply during decode");
        assert!(matches!(err, Error::Module(m) if m.contains("worker cap")));
    });
}

#[test]
fn claim_attempt_saturates_instead_of_wrapping() {
    block_on(async {
        // attempt counts are only ever produced by claim, so u64::MAX is not
        // execute-reachable organically — install a crafted (but shape-valid)
        // board to prove the increment saturates instead of wrapping.
        let bytes = snapshot_one(0, u64::MAX, None, None); // Pending, attempt MAX
        let mut jobs = Jobs::new(JOBS);
        jobs.install(&bytes, root_of_bytes(&bytes))
            .expect("install crafted board");

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
        let mut jobs = Jobs::new(JOBS);

        // a staged-but-uncommitted submit is invisible to every projection.
        stage(&mut jobs, 1, ext("submitter"), submit("j1", "k", ""))
            .await
            .expect("stage submit");
        assert!(get(&jobs, "j1").await.is_none(), "Get is blind to staging");
        assert!(
            list(&jobs, None, "", 256).await.is_empty(),
            "List is blind to staging"
        );
        assert_eq!(
            counts(&jobs).await,
            BoardCounts::default(),
            "Counts is blind to staging"
        );

        // commit publishes; the same queries now see it.
        jobs.commit_block().await.expect("commit");
        assert!(get(&jobs, "j1").await.is_some());
        assert_eq!(list(&jobs, None, "", 256).await.len(), 1);
        assert_eq!(counts(&jobs).await.pending, 1);

        // a staged transition is equally invisible: claim staged, queries
        // still report the committed Pending.
        stage(&mut jobs, 2, ext("worker-a"), claim("j1", 50))
            .await
            .expect("stage claim");
        assert_eq!(
            get(&jobs, "j1").await.unwrap().status,
            JobStatus::Pending,
            "the staged claim is not served"
        );
        assert_eq!(counts(&jobs).await.pending, 1);
        assert_eq!(counts(&jobs).await.processing, 0);
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
        let mut jobs = Jobs::new(JOBS);
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
    BlockContext { protocol_version: 0,
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
    match decode_reply(&bytes).expect("decode") {
        JobsReply::Job(job) => job,
        other => panic!("expected Job, got {other:?}"),
    }
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
            Box::new(Jobs::new(JOBS)),
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
        let mut host = Host::genesis(vec![Box::new(Jobs::new(JOBS))]).expect("genesis");

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
            Box::new(Jobs::new(JOBS)),
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
