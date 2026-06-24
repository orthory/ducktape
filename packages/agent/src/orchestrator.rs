//! agentic orchestrator / supervisor.
//!
//! advisor-shaped: given a list of [`TaskSpec`]s, fan each one out to a worker
//! (a claude-code driver run, each in its own cwd / worktree), collect the
//! results, and produce a stream of [`op::Op`]s describing what happened.
//!
//! the spawn seam is the [`Worker`] trait so tests can inject a fake instead of
//! shelling out. the real impl, [`ClaudeWorker`], wraps [`crate::driver::run`]
//! and points the subprocess at a per-task cwd.
//!
//! concurrency is bounded by a semaphore: tasks run in parallel up to
//! `max_concurrency` at a time. output order is *not* deterministic — callers
//! must match ops by `task_id`, not position.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::driver::{ClaudeConfig, Error, TaskRun};
use crate::spec::TaskSpec;

/// the spawn seam. one `run` per task, in the task's own working directory.
///
/// the future borrows nothing of the orchestrator — it owns the task and cwd —
/// so it can be `spawn`ed onto a [`JoinSet`] (which requires `'static`).
pub trait Worker: Send + Sync + 'static {
    /// run a single task in `cwd`, yielding its parsed run result.
    fn run(
        &self,
        task: &TaskSpec,
        cwd: &Path,
    ) -> impl std::future::Future<Output = Result<TaskRun, Error>> + Send;
}

/// the real worker: shells out to `claude` via [`crate::driver::run`].
///
/// the prompt fed to the cli is the task's title + body. each run gets a fresh
/// config cloned from the template with `cwd` repointed at the per-task dir.
pub struct ClaudeWorker {
    /// template config; `cwd` is overwritten per task.
    pub cfg_template: ClaudeConfig,
}

impl Worker for ClaudeWorker {
    async fn run(&self, task: &TaskSpec, cwd: &Path) -> Result<TaskRun, Error> {
        let mut cfg = self.cfg_template.clone();
        cfg.cwd = cwd.to_path_buf();
        let prompt = format!("{}\n\n{}", task.title, task.body);
        crate::driver::run(&cfg, &prompt).await
    }
}

/// fans tasks out to a [`Worker`] and maps each outcome to an [`op::Op`].
pub struct Orchestrator<W: Worker> {
    /// the worker tasks are handed to.
    pub worker: Arc<W>,
    /// per-task working dirs are minted as `workspace_root / <task id>`.
    pub workspace_root: PathBuf,
    /// max tasks running at once.
    pub max_concurrency: usize,
}

impl<W: Worker> Orchestrator<W> {
    /// construct an orchestrator over a worker. `max_concurrency` is clamped to
    /// at least 1.
    pub fn new(worker: W, workspace_root: PathBuf, max_concurrency: usize) -> Self {
        Self {
            worker: Arc::new(worker),
            workspace_root,
            max_concurrency: max_concurrency.max(1),
        }
    }

    /// the per-task working directory: `workspace_root / <task id>`.
    ///
    /// for the unit path the fake worker ignores this; the real worktree
    /// creation (git worktree add) lives in a helper exercised by the e2e.
    fn cwd_for(&self, task: &TaskSpec) -> PathBuf {
        self.workspace_root.join(task.id.to_string())
    }

    /// run every task concurrently (bounded by `max_concurrency`) and collect
    /// the resulting ops.
    ///
    /// each task maps to exactly one control op: success ->
    /// [`control::op::Op::TaskResult`] carrying the run's `result` as the
    /// outcome; failure -> [`control::op::Op::TaskFailed`] carrying the error
    /// string. both route to [`op::Lane::Consensus`].
    ///
    /// the returned `Vec` order follows completion order, *not* input order.
    pub async fn develop(&self, tasks: Vec<TaskSpec>) -> Vec<op::Op> {
        let sem = Arc::new(Semaphore::new(self.max_concurrency));
        let mut set: JoinSet<op::Op> = JoinSet::new();

        for task in tasks {
            let worker = Arc::clone(&self.worker);
            let sem = Arc::clone(&sem);
            let cwd = self.cwd_for(&task);

            set.spawn(async move {
                // permit is held for the duration of the run; bounds concurrency.
                let _permit = sem
                    .acquire()
                    .await
                    .expect("semaphore not closed while in use");
                let task_id = task.id;
                match worker.run(&task, &cwd).await {
                    Ok(run) => op::Op::Control(control::op::Op::TaskResult {
                        task_id,
                        outcome: run.result,
                    }),
                    Err(e) => op::Op::Control(control::op::Op::TaskFailed {
                        task_id,
                        error: e.to_string(),
                    }),
                }
            });
        }

        let mut ops = Vec::new();
        while let Some(joined) = set.join_next().await {
            // a panicked task is a bug in the worker; surface it loudly.
            ops.push(joined.expect("worker task panicked"));
        }
        ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// a worker that returns a canned result and tracks observed concurrency.
    ///
    /// on entry it bumps `current` and records `peak = max(peak, current)`, then
    /// yields (sleeps) so overlapping runs can be observed, then decrements on
    /// exit. `peak > 1` proves the orchestrator actually ran tasks in parallel.
    struct FakeWorker {
        current: AtomicUsize,
        peak: AtomicUsize,
    }

    impl FakeWorker {
        fn new() -> Self {
            Self {
                current: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
            }
        }
    }

    impl Worker for FakeWorker {
        async fn run(&self, task: &TaskSpec, _cwd: &Path) -> Result<TaskRun, Error> {
            let cur = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(cur, Ordering::SeqCst);
            // the yield point: without it, a run could finish before the next
            // even starts and peak would never exceed 1.
            tokio::time::sleep(Duration::from_millis(10)).await;
            self.current.fetch_sub(1, Ordering::SeqCst);
            Ok(TaskRun {
                session_id: None,
                result: format!("did: {}", task.title),
                total_cost_usd: None,
                is_error: false,
            })
        }
    }

    fn spec(title: &str) -> TaskSpec {
        TaskSpec {
            id: uid::new(),
            title: title.to_string(),
            body: format!("body of {title}"),
        }
    }

    #[tokio::test]
    async fn develop_fans_tasks_to_results() {
        let tasks = vec![spec("a"), spec("b"), spec("c")];
        let ids: HashSet<uid::Uid> = tasks.iter().map(|t| t.id).collect();

        let worker = FakeWorker::new();
        let orch = Orchestrator::new(worker, PathBuf::from("/tmp/agent-test"), 4);

        let ops = orch.develop(tasks).await;

        // one op per task.
        assert_eq!(ops.len(), 3);

        // every op is a TaskResult routing to consensus, and the set of task_ids
        // matches the input set (order is nondeterministic).
        let mut got_ids = HashSet::new();
        for op in &ops {
            assert_eq!(op.lane(), op::Lane::Consensus);
            match op {
                op::Op::Control(control::op::Op::TaskResult { task_id, outcome }) => {
                    assert!(outcome.starts_with("did: "));
                    got_ids.insert(*task_id);
                }
                other => panic!("expected TaskResult, got {other:?}"),
            }
        }
        assert_eq!(got_ids, ids);

        // proves the runs actually overlapped (peak concurrency > 1).
        assert!(
            orch.worker.peak.load(Ordering::SeqCst) > 1,
            "expected concurrent execution, peak was {}",
            orch.worker.peak.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn empty_task_list_yields_no_ops() {
        let orch = Orchestrator::new(FakeWorker::new(), PathBuf::from("/tmp/agent-test"), 2);
        assert!(orch.develop(Vec::new()).await.is_empty());
    }
}
