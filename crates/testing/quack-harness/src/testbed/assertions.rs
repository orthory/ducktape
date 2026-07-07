//! the panicking, test-style assertion kit the ADR harness checklist scripts
//! against: jobs board, pending runs, action-owner routing, and the
//! most-recently-committed block's failure breadcrumbs.

use jobs::{JobStatus, JobsQuery, JobsReply};
use runs::{RunsQuery, RunsReply, encode_query as runs_encode_query};

use super::PackageTestBed;

impl PackageTestBed {
    /// assert the jobs board holds exactly `expected` jobs of `kind_prefix`
    /// (any status) — "an engagement event mints exactly one job".
    pub async fn assert_job_count(&self, kind_prefix: &str, expected: usize) {
        let jobs = self.jobs_matching(kind_prefix).await;
        assert_eq!(
            jobs.len(),
            expected,
            "expected {expected} jobs with kind prefix {kind_prefix:?}, found {}: {:?}",
            jobs.len(),
            jobs.iter().map(|j| j.job_id.as_str()).collect::<Vec<_>>()
        );
    }

    /// assert one job's status.
    pub async fn assert_job_status(&self, job_id: &str, expected: JobStatus) {
        let reply = self
            .query(
                "jobs",
                &jobs::encode_query(&JobsQuery::Get {
                    job_id: job_id.into(),
                }),
            )
            .await
            .expect("jobs query");
        match jobs::decode_reply(&reply).expect("jobs reply") {
            JobsReply::Job(Some(job)) => assert_eq!(
                job.status, expected,
                "job {job_id} is {:?}, expected {expected:?}",
                job.status
            ),
            JobsReply::Job(None) => panic!("job {job_id} does not exist"),
            other => panic!("unexpected jobs reply: {other:?}"),
        }
    }

    /// assert whether a pending (not-yet-delivered) run exists for `agent_id`.
    pub async fn assert_pending_run_for_agent(&self, agent_id: &str, exists: bool) {
        let runs = self.pending_runs().await;
        let found = runs.iter().any(|r| r.agent_id == agent_id);
        assert_eq!(
            found,
            exists,
            "pending runs for agent {agent_id:?}: {:?}",
            runs.iter().map(|r| r.run_id.as_str()).collect::<Vec<_>>()
        );
    }

    /// assert the package registry resolves `tag` to `owner`.
    pub async fn assert_action_owner(&self, tag: &str, owner: Option<&str>) {
        let reply = self
            .query(
                "package",
                &package::encode_query(&package::PackageQuery::ActionOwner { tag: tag.into() }),
            )
            .await
            .expect("package query");
        match package::decode_reply(&reply).expect("package reply") {
            package::PackageReply::Owner(actual) => {
                assert_eq!(actual.as_deref(), owner, "action {tag:?} owner mismatch")
            }
            other => panic!("unexpected package reply: {other:?}"),
        }
    }

    /// assert the MOST RECENTLY COMMITTED block left a breadcrumb event from
    /// `source` containing `contains` — how the no-fail arms record failure
    /// ("mutate nothing, record failure").
    pub fn assert_failure_breadcrumb(&self, source: &str, contains: &str) {
        assert!(
            self.has_failure_breadcrumb(source, contains),
            "no event from {source:?} containing {contains:?} in the last block; events: {:?}",
            self.last_events
        );
    }

    pub(crate) fn has_failure_breadcrumb(&self, source: &str, contains: &str) -> bool {
        self.last_events
            .iter()
            .any(|e| e.source == source && e.text.contains(contains))
    }

    pub(crate) async fn jobs_matching(&self, kind_prefix: &str) -> Vec<jobs::Job> {
        let reply = self
            .query(
                "jobs",
                &jobs::encode_query(&JobsQuery::List {
                    status: None,
                    kind_prefix: kind_prefix.into(),
                    limit: 10_000,
                }),
            )
            .await
            .expect("jobs query");
        match jobs::decode_reply(&reply).expect("jobs reply") {
            JobsReply::Jobs(jobs) => jobs,
            other => panic!("unexpected jobs reply: {other:?}"),
        }
    }

    pub(crate) async fn pending_runs(&self) -> Vec<runs::PendingRun> {
        let reply = self
            .query("runs", &runs_encode_query(&RunsQuery::PendingRuns))
            .await
            .expect("runs query");
        match runs::decode_reply(&reply).expect("runs reply") {
            RunsReply::PendingRuns(runs) => runs,
            other => panic!("unexpected runs reply: {other:?}"),
        }
    }
}
