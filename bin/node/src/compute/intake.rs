//! the compute daemon's work intake: the `watch` verb in its cheapest form.
//!
//! A daemon executes no blocks, so no effect ever reaches it. It discovers its
//! node's assigned work the way a synced RESIDENT already does — by asking the
//! saga module's COMMITTED projection (`SagaQuery::AssignedPending`) for the
//! exact [`WorkerRequest`]s the effect lane would have carried — except the
//! query rides `/v1/query` instead of an in-process `Host`.
//!
//! Each pass offers new assignments to the off-loop [`compute_service::DispatchPool`]
//! ONCE and submits whatever follow-up ops are due. A pass NEVER awaits a
//! provider: the gate verdict is inline and immediate, the CLI runs on a spawned
//! task, and its result re-enters through [`WorkPump::completed`].
//!
//! ## exactly once, across a lossy read
//!
//! The chain is the source of truth, so a missed hint DELAYS work but never
//! loses it. The converse — treating a failed read as an empty projection —
//! would be far worse: emptiness retires entries, and retiring a live attempt
//! re-offers it as fresh work (a second child process for the same paid call,
//! and a computed-but-unsent result silently dropped). So:
//!
//! - an unreadable projection is not an empty one: the pass does nothing;
//! - plain absence needs a CONFIRMING second read before an entry retires;
//! - authoritative supersession (terminal saga, a higher attempt, a changed
//!   assignee) retires on the first read, and cancels the local run;
//! - the pump's own latch — not the pool's in-flight map, which prunes at
//!   delivery — is what keeps a still-pending attempt from being re-offered.
//!
//! A restart re-runs at worst once; the saga module's result singularity
//! collapses the duplicate.

use std::collections::{HashMap, HashSet};

use compute_service::AttemptControl;
use host::worker::{WorkOutcome, Worker};
use noded::node_link::NodeLink;
use saga::{SagaMsg, SagaQuery, SagaReply, WorkerRequest};
use sdk::{Event, Msg};

/// one attempt's idempotency key — what the worker echoes into its result.
type AttemptKey = (String, u32);

/// why an attempt vanished from this daemon's assigned subset.
enum AttemptProjection {
    Active,
    Retired,
}

/// where one assigned attempt sits in the execute-then-submit lifecycle.
enum Stage {
    /// offered to the pool; the provider runs (or queues) on a spawned task.
    /// Nothing to submit yet, and the attempt must NOT be offered again.
    Executing,
    /// the follow-up op is computed and needs a (re)submit.
    Due(Msg),
    /// nothing further to send — the op committed, or the request was a
    /// deliberate skip. Held until committed state retires the attempt, so it
    /// is never re-offered.
    Settled,
}

/// one tracked attempt: its stage plus the retire confirmation. `missed` marks
/// a read that failed to name the attempt; only a SECOND consecutive miss
/// retires the entry.
struct Entry {
    stage: Stage,
    missed: bool,
}

pub(crate) struct WorkPump {
    /// the off-loop execution pool behind the shared `Worker` seam: gate
    /// inline, spawn the provider, return immediately.
    pool: Box<dyn Worker>,
    /// cancellation for attempts already handed to the pool.
    control: AttemptControl,
    /// the node's external submit key — the lease identity queried for, and
    /// the identity `/v1/submit` stamps on every op this pump sends.
    me: Vec<u8>,
    work: HashMap<AttemptKey, Entry>,
}

impl WorkPump {
    pub(crate) fn new(pool: Box<dyn Worker>, control: AttemptControl, me: Vec<u8>) -> Self {
        Self {
            pool,
            control,
            me,
            work: HashMap::new(),
        }
    }

    /// how many attempts this pump is tracking.
    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.work.len()
    }

    /// one intake pass: read the committed projection, offer new assignments,
    /// submit what is due.
    pub(crate) async fn tick(&mut self, node: &NodeLink) {
        let Some(assigned) = assigned_pending(node, &self.me).await else {
            return;
        };
        let live: HashSet<AttemptKey> = assigned
            .iter()
            .map(|request| (request.saga_id.clone(), request.attempt))
            .collect();
        let missing: Vec<AttemptKey> = self
            .work
            .keys()
            .filter(|key| !live.contains(*key))
            .cloned()
            .collect();
        let mut retired = HashSet::new();
        let mut active = HashSet::new();
        for key in missing {
            match attempt_projection(node, &self.me, &key).await {
                Some(AttemptProjection::Retired) => {
                    retired.insert(key);
                }
                Some(AttemptProjection::Active) => {
                    active.insert(key);
                }
                // inconclusive: preserve the two-read flap tolerance.
                None => {}
            }
        }
        let due = self.plan(assigned, &retired, &active).await;
        for (key, msg) in due {
            self.send(node, &key, msg).await;
        }
    }

    /// a completed off-loop run arrived on the result lane: queue its op.
    ///
    /// An attempt committed state no longer names (pruned while the provider
    /// ran) is dropped; its op could only be a deterministic no-op anyway.
    pub(crate) fn completed(&mut self, saga_id: String, attempt: u32, msg: Msg) {
        let key = (saga_id, attempt);
        match self.work.get_mut(&key) {
            Some(Entry {
                stage: stage @ Stage::Executing,
                ..
            }) => *stage = Stage::Due(msg),
            _ => tracing::debug!(
                target: "ducktape::saga",
                attempt = ?key,
                reason = "retired_attempt",
                "completed run dropped"
            ),
        }
    }

    /// the pump core, separated from the reads so it is unit-testable: retire
    /// what committed state proves obsolete, offer newly assigned attempts to
    /// the pool ONCE, and surface every op that needs sending.
    async fn plan(
        &mut self,
        assigned: Vec<WorkerRequest>,
        retired: &HashSet<AttemptKey>,
        active: &HashSet<AttemptKey>,
    ) -> Vec<(AttemptKey, Msg)> {
        let live: HashSet<AttemptKey> = assigned
            .iter()
            .map(|r| (r.saga_id.clone(), r.attempt))
            .collect();
        let newest = assigned.iter().fold(
            HashMap::<String, u32>::new(),
            |mut newest, request| {
                newest
                    .entry(request.saga_id.clone())
                    .and_modify(|attempt| *attempt = (*attempt).max(request.attempt))
                    .or_insert(request.attempt);
                newest
            },
        );
        let control = self.control.clone();
        let me = &self.me;
        self.work.retain(|key, entry| {
            if live.contains(key) || active.contains(key) {
                entry.missed = false;
                return true;
            }
            // a higher committed attempt of the same saga — including one
            // assigned elsewhere and confirmed by `SagaQuery::Get` — is proof
            // of supersession rather than a flapped projection.
            let superseded =
                retired.contains(key) || newest.get(&key.0).is_some_and(|attempt| *attempt > key.1);
            if !superseded && !entry.missed {
                entry.missed = true;
                if !matches!(entry.stage, Stage::Settled) {
                    tracing::warn!(
                        target: "ducktape::saga",
                        attempt = ?key,
                        reason = "projection_missing_mid_work",
                        "compute attempt will retire on the next absent read"
                    );
                }
                return true;
            }
            if !matches!(entry.stage, Stage::Settled) {
                control.cancel(&key.0, key.1, me);
            }
            false
        });

        let mut due = Vec::new();
        for request in assigned {
            let key = (request.saga_id.clone(), request.attempt);
            if let Some(entry) = self.work.get_mut(&key) {
                // named again: whatever absence was seen did not persist.
                entry.missed = false;
            }
            match self.work.get_mut(&key).map(|entry| &entry.stage) {
                None => {
                    let stage = self.offer(request, &key, &mut due).await;
                    self.work.insert(
                        key,
                        Entry {
                            stage,
                            missed: false,
                        },
                    );
                }
                // the provider is running off-loop; nothing to do.
                Some(Stage::Executing) => {}
                // computed but not successfully submitted yet: offer again.
                Some(Stage::Due(msg)) => due.push((key.clone(), msg.clone())),
                Some(Stage::Settled) => {}
            }
        }
        due
    }

    /// first sighting of an attempt: hand it to the pool exactly once.
    async fn offer(
        &self,
        request: WorkerRequest,
        key: &AttemptKey,
        due: &mut Vec<(AttemptKey, Msg)>,
    ) -> Stage {
        let event = Event {
            source: "saga".into(),
            payload: saga::encode_worker_request(&request),
        };
        match self.pool.run(&event).await {
            // an inline verdict WITH an op (unresolvable capability, non-utf-8
            // payload): due now.
            Ok(WorkOutcome::Handled(Some(msg))) => {
                due.push((key.clone(), msg.clone()));
                Stage::Due(msg)
            }
            // spawned — the result arrives later via `completed`. A deliberate
            // inline skip maps here too; either way the attempt is latched
            // against re-offer and the retire sweep decides what happens next.
            Ok(WorkOutcome::Handled(None)) => Stage::Executing,
            // not a dispatch WorkSpec: never ours to run.
            Ok(WorkOutcome::NotMine) => Stage::Settled,
            Err(error) => {
                // unreachable for DispatchPool (it never errors) — settle
                // rather than spin a broken worker.
                tracing::warn!(
                    target: "ducktape::saga",
                    attempt = ?key,
                    error = %error,
                    reason = "worker_error",
                    "compute attempt settled"
                );
                Stage::Settled
            }
        }
    }

    /// submit one due op. `/v1/submit` re-signs under the NODE's key, which is
    /// the assignee the saga's lease gate checks — so this is the same op, from
    /// the same identity, the in-process pool used to deliver.
    ///
    /// A refusal leaves the entry `Due`: the next pass re-sends while committed
    /// state still names the attempt. Re-sends are duplicate ops at worst — the
    /// saga's result singularity collapses them deterministically.
    async fn send(&mut self, node: &NodeLink, key: &AttemptKey, msg: Msg) {
        match node.submit(&msg.target, &msg.payload).await {
            Ok(_height) => {
                if let Some(entry) = self.work.get_mut(key) {
                    entry.stage = Stage::Settled;
                }
            }
            Err(error) => tracing::debug!(
                target: "ducktape::saga",
                attempt = ?key,
                error = %error,
                reason = "result_submit_failed",
                "compute result will be re-sent"
            ),
        }
    }
}

/// the committed saga projection of this node's leased pending attempts.
/// `None` when the node is unreachable, the module absent, or the reply
/// unreadable — a failed read must NEVER masquerade as an empty projection.
async fn assigned_pending(node: &NodeLink, me: &[u8]) -> Option<Vec<WorkerRequest>> {
    let reply = node
        .query(
            "saga",
            &saga::encode_query(&SagaQuery::AssignedPending {
                assignee: me.to_vec(),
            }),
        )
        .await
        .ok()?;
    match saga::decode_reply(&reply) {
        Ok(SagaReply::AssignedPending(requests)) => Some(requests),
        _ => None,
    }
}

/// Confirm WHY an assigned attempt disappeared. `AssignedPending` cannot show a
/// retry that moved to another node; the per-saga view can. An unreadable or
/// older view is inconclusive and preserves the two-read flap tolerance.
async fn attempt_projection(
    node: &NodeLink,
    me: &[u8],
    key: &AttemptKey,
) -> Option<AttemptProjection> {
    let reply = node
        .query(
            "saga",
            &saga::encode_query(&SagaQuery::Get {
                saga_id: key.0.clone(),
            }),
        )
        .await
        .ok()?;
    match saga::decode_reply(&reply).ok()? {
        SagaReply::Saga(None) => Some(AttemptProjection::Retired),
        SagaReply::Saga(Some(view))
            if view.status.is_terminal()
                || view.attempt > key.1
                || (view.attempt == key.1 && view.assignee.as_deref() != Some(me)) =>
        {
            Some(AttemptProjection::Retired)
        }
        SagaReply::Saga(Some(view))
            if view.attempt == key.1 && view.assignee.as_deref() == Some(me) =>
        {
            Some(AttemptProjection::Active)
        }
        _ => None,
    }
}

/// What the pool handed to the delivery lane. ONE discriminant, because the two
/// have different fates: a result belongs to a tracked attempt and is RETRIED
/// until it commits, while a lease heartbeat is fire-and-forget (the next beat
/// is 10s away and supersedes it).
pub(crate) enum Delivered {
    Result {
        saga_id: String,
        attempt: u32,
        msg: Msg,
    },
    Heartbeat(Msg),
}

impl Delivered {
    /// classify one delivered op.
    ///
    /// The pool sends exactly two shapes down this lane. Anything else is wire
    /// drift, and treating it as a heartbeat (submit once, best effort) is the
    /// honest fallback: it still reaches consensus, it just is not retried.
    pub(crate) fn classify(msg: Msg) -> Self {
        match saga::decode_msg(&msg.payload) {
            Ok(SagaMsg::OracleResult {
                saga_id, attempt, ..
            }) => Delivered::Result {
                saga_id,
                attempt,
                msg,
            },
            _ => Delivered::Heartbeat(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// a worker that counts offers and answers with a fixed outcome — the
    /// observable stand-in for the real pool, with no provider and no node.
    struct CountingWorker {
        offers: Arc<AtomicUsize>,
        spawned: bool,
    }

    #[async_trait::async_trait(?Send)]
    impl Worker for CountingWorker {
        async fn run(&self, _event: &Event) -> Result<WorkOutcome, host::worker::Error> {
            self.offers.fetch_add(1, Ordering::SeqCst);
            match self.spawned {
                true => Ok(WorkOutcome::Handled(None)),
                false => Ok(WorkOutcome::Handled(Some(result_msg("s", 0)))),
            }
        }
    }

    const ME: &[u8] = b"compute-node-key";

    fn result_msg(saga_id: &str, attempt: u32) -> Msg {
        Msg {
            target: "saga".into(),
            payload: saga::encode_msg(&SagaMsg::OracleResult {
                saga_id: saga_id.into(),
                attempt,
                outcome: Ok(b"done".to_vec()),
                usage: None,
            }),
        }
    }

    fn request(saga_id: &str, attempt: u32) -> WorkerRequest {
        WorkerRequest {
            saga_id: saga_id.into(),
            attempt,
            spec: b"{}".to_vec(),
            deadline: None,
            assignee: Some(ME.to_vec()),
        }
    }

    fn new_pump(spawned: bool) -> (WorkPump, Arc<AtomicUsize>) {
        let offers = Arc::new(AtomicUsize::new(0));
        let worker = CountingWorker {
            offers: offers.clone(),
            spawned,
        };
        // a control whose node key matches, so cancellation is exercised
        // against the same identity the pump queries under.
        let pool = compute_service::DispatchPool::with_limit(
            Arc::new(provider_host::ProviderSet::empty()),
            ME.to_vec(),
            Arc::new(|_, _| {}),
            Arc::new(|_| Box::pin(async {})),
            1,
            Default::default(),
            Arc::new(NoProvisioner),
        );
        let control = pool.attempt_control();
        (
            WorkPump::new(Box::new(worker), control, ME.to_vec()),
            offers,
        )
    }

    struct NoProvisioner;
    #[async_trait::async_trait]
    impl compute_service::WorkspaceProvisioner for NoProvisioner {
        async fn provision(
            &self,
            _spec: &compute_service::WorkspaceSpec,
        ) -> Result<Box<dyn compute_service::ProvisionedWorkspace>, String> {
            Err("no provisioner in this test".into())
        }
    }

    #[tokio::test]
    async fn an_attempt_is_offered_exactly_once_across_repeated_reads() {
        let (mut pump, offers) = new_pump(true);
        let empty = HashSet::new();
        for _ in 0..5 {
            let due = pump.plan(vec![request("s", 0)], &empty, &empty).await;
            assert!(due.is_empty(), "a spawned attempt has nothing to send yet");
        }
        assert_eq!(offers.load(Ordering::SeqCst), 1, "offered exactly once");
        assert_eq!(pump.tracked(), 1);
    }

    #[tokio::test]
    async fn an_inline_verdict_is_due_and_stays_due_until_it_is_sent() {
        let (mut pump, _) = new_pump(false);
        let empty = HashSet::new();
        let due = pump.plan(vec![request("s", 0)], &empty, &empty).await;
        assert_eq!(due.len(), 1, "an inline op is due immediately");
        // not sent yet: the next pass re-offers the SAME op rather than
        // re-running the attempt.
        let again = pump.plan(vec![request("s", 0)], &empty, &empty).await;
        assert_eq!(again.len(), 1);
    }

    #[tokio::test]
    async fn plain_absence_needs_two_reads_but_supersession_retires_at_once() {
        let (mut pump, offers) = new_pump(true);
        let empty = HashSet::new();
        pump.plan(vec![request("s", 0)], &empty, &empty).await;
        assert_eq!(pump.tracked(), 1);

        // one absent read: retained (a flapped read must not re-run a live
        // attempt as fresh work).
        pump.plan(Vec::new(), &empty, &empty).await;
        assert_eq!(pump.tracked(), 1, "one absence is not proof");
        // a second: retired.
        pump.plan(Vec::new(), &empty, &empty).await;
        assert_eq!(pump.tracked(), 0, "a confirmed absence retires");

        // a higher attempt of the same saga is authoritative supersession —
        // one read is enough, and the newcomer is offered.
        let (mut pump, offers2) = new_pump(true);
        pump.plan(vec![request("s", 0)], &empty, &empty).await;
        pump.plan(vec![request("s", 1)], &empty, &empty).await;
        assert_eq!(pump.tracked(), 1, "only the newest attempt is tracked");
        assert_eq!(offers2.load(Ordering::SeqCst), 2);
        // the first pump never re-offered after retirement.
        assert_eq!(offers.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_completed_result_becomes_due_and_a_retired_one_is_dropped() {
        let (mut pump, _) = new_pump(true);
        let empty = HashSet::new();
        pump.plan(vec![request("s", 0)], &empty, &empty).await;

        pump.completed("s".into(), 0, result_msg("s", 0));
        let due = pump.plan(vec![request("s", 0)], &empty, &empty).await;
        assert_eq!(due.len(), 1, "a completed run's op is due");

        // an attempt nothing tracks any more drops its late result.
        pump.completed("gone".into(), 7, result_msg("gone", 7));
        let due = pump.plan(vec![request("s", 0)], &empty, &empty).await;
        assert_eq!(due.len(), 1, "the unknown result added nothing");
    }

    #[test]
    fn delivery_classification_splits_results_from_lease_heartbeats() {
        let result = Delivered::classify(result_msg("s", 3));
        match result {
            Delivered::Result {
                saga_id, attempt, ..
            } => {
                assert_eq!(saga_id, "s");
                assert_eq!(attempt, 3);
            }
            Delivered::Heartbeat(_) => panic!("an oracle result is not a heartbeat"),
        }

        // the lease heartbeat the pool pushes down the SAME lane: submitted
        // once, never tracked as an attempt result.
        let renew = Msg {
            target: "saga".into(),
            payload: saga::encode_msg(&SagaMsg::RenewLease {
                saga_id: "s".into(),
                attempt: 3,
            }),
        };
        assert!(matches!(
            Delivered::classify(renew),
            Delivered::Heartbeat(_)
        ));
    }
}
