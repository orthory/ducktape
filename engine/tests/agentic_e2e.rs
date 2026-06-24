//! full-loop agentic dev e2e: spec document -> tasks -> mock claude -> ops.
//!
//! proves the whole p2 supervisor path end-to-end with NO real `claude`:
//! - a spec [`Document`] carrying 3 `/task` directives + the `agentic: true`
//!   frontmatter trigger,
//! - [`agent::spec::parse_spec`] turning it into a `Vec<TaskSpec>`,
//! - a *mock* claude binary (a shell script) injected via
//!   [`ClaudeConfig::claude_bin`] that both prints a json envelope AND writes a
//!   side-effect file in its cwd (so a run leaves an observable trace),
//! - [`agent::orchestrator::Orchestrator::develop`] fanning the tasks out to a
//!   real [`ClaudeWorker`] (which actually shells out to the mock) and mapping
//!   each outcome to a [`control::op::Op`],
//! - the produced ops flowing through [`engine::Engine::apply`] (the same seam a
//!   live node routes control ops through).
//!
//! one task carries a `DUCKFAIL` sentinel in its title; the mock exits non-zero
//! when it sees that, so the worker's `NonZeroExit` maps to `TaskFailed`. that
//! makes the failure branch reachable — the assertions are non-tautological:
//! N is derived from the parsed spec (not hardcoded), ops are matched by
//! `task_id` (develop returns completion order, not input order), success tasks
//! must have left their side-effect file and the failing one must NOT.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use agent::driver::ClaudeConfig;
use agent::orchestrator::{ClaudeWorker, Orchestrator};
use agent::spec::{has_agentic_trigger, parse_spec};
use document::Document;

/// a spec with the agentic trigger + 3 task directives. the second task's title
/// carries the `DUCKFAIL` sentinel the mock branches on.
const SPEC: &str = "\
---
title: ship the thing
author: @orthory
created_at: 1
updated_at: 1
agentic: true
---

intro prose, not a task

/task.v1{@orthory;wire the parser;Backlog}
make the parser handle frontmatter
/task.v1

/task.v1{@orthory;DUCKFAIL on purpose;Backlog}
this one is rigged to fail
/task.v1

/task.v1{@orthory;polish the docs;Backlog}
tidy the module docstrings
/task.v1
";

/// the side-effect a successful mock run leaves in its cwd.
const SIDE_EFFECT: &str = "out.txt";

/// write a mock `claude` at `path`, chmod 0o755.
///
/// it inspects the whole argv (`$*`) for the `DUCKFAIL` sentinel — cwd is set
/// via `current_dir`, not passed as an arg, so the prompt (which carries the
/// task title) is the only per-task signal the script sees. on the sentinel it
/// exits 1 (no side-effect, no stdout) so the driver reports `NonZeroExit`.
/// otherwise it writes the side-effect file to its cwd and prints ONLY the json
/// envelope on stdout (the file write is redirected; stray stdout would break
/// the envelope parse).
fn write_mock_claude(path: &Path) {
    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in *DUCKFAIL*) exit 1 ;; esac\n\
         echo \"done by mock\" > {SIDE_EFFECT}\n\
         cat <<'EOF'\n\
         {{\"result\":\"ok\",\"session_id\":\"s\",\"total_cost_usd\":0.0,\"is_error\":false}}\n\
         EOF\n"
    );
    std::fs::write(path, script).expect("write mock claude");
    let mut perms = std::fs::metadata(path).expect("stat mock").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod mock");
}

#[tokio::test]
async fn agentic_dev_spec_to_tasks_to_ops() {
    // --- a unique workspace root so parallel test runs don't collide --------
    let root: PathBuf = std::env::temp_dir().join(format!(
        "agentic-e2e-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    // start clean even if a prior aborted run left junk behind.
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create workspace root");

    // --- the spec: parse it, confirm the trigger, extract the tasks ---------
    let doc = Document::from_reader(SPEC.as_bytes()).expect("spec parses");
    assert!(
        has_agentic_trigger(&doc),
        "the spec opts into agentic dev via frontmatter"
    );

    let specs = parse_spec(&doc);
    let n = specs.len();
    assert_eq!(n, 3, "the spec carries three task directives");

    // identify the one rigged-to-fail task by its sentinel title, and the rest
    // (which should succeed). these drive the per-outcome assertions below.
    let fail_id = specs
        .iter()
        .find(|s| s.title.contains("DUCKFAIL"))
        .map(|s| s.id)
        .expect("a DUCKFAIL task exists");
    let success_ids: Vec<uid::Uid> = specs
        .iter()
        .filter(|s| !s.title.contains("DUCKFAIL"))
        .map(|s| s.id)
        .collect();
    assert_eq!(success_ids.len(), 2);

    // --- the mock claude binary, outside any task dir so it isn't one -------
    let mock = root.join("claude");
    write_mock_claude(&mock);

    // develop() does NOT mkdir the per-task cwd (= root/<task id>); a missing
    // cwd makes the subprocess spawn fail. pre-create one per task (this is the
    // worktree-creation seam the orchestrator deliberately leaves to the caller)
    // and prove the side-effect file is absent up front, so its later presence
    // can only mean the run actually executed.
    for s in &specs {
        let cwd = root.join(s.id.to_string());
        std::fs::create_dir_all(&cwd).expect("create per-task cwd");
        assert!(
            !cwd.join(SIDE_EFFECT).exists(),
            "side-effect must not exist before the run"
        );
    }

    // --- run the orchestrator over a REAL ClaudeWorker pointed at the mock --
    let worker = ClaudeWorker {
        cfg_template: ClaudeConfig {
            claude_bin: mock,
            ..Default::default()
        },
    };
    let orch = Orchestrator::new(worker, root.clone(), 4);
    let ops = orch.develop(specs).await;

    // --- assert: one op per task, matched by id (completion-order safe) -----
    assert_eq!(ops.len(), n, "exactly one op per task");

    // every produced op routes to the consensus lane.
    for op in &ops {
        assert_eq!(op.lane(), op::Lane::Consensus);
    }

    // index ops by task_id so we can assert per task regardless of order.
    let mut results: HashMap<uid::Uid, String> = HashMap::new();
    let mut failures: HashMap<uid::Uid, String> = HashMap::new();
    for op in &ops {
        match op {
            op::Op::Control(control::op::Op::TaskResult { task_id, outcome }) => {
                results.insert(*task_id, outcome.clone());
            }
            op::Op::Control(control::op::Op::TaskFailed { task_id, error }) => {
                failures.insert(*task_id, error.clone());
            }
            other => panic!("expected a Control TaskResult/TaskFailed op, got {other:?}"),
        }
    }

    // exactly one TaskFailed, and it's the rigged task.
    assert_eq!(failures.len(), 1, "exactly one task failed");
    assert!(
        failures.contains_key(&fail_id),
        "the DUCKFAIL task is the one that failed"
    );

    // the other two are TaskResults carrying the mock's envelope `result`.
    assert_eq!(results.len(), 2, "the two non-failing tasks succeeded");
    for id in &success_ids {
        assert_eq!(
            results.get(id).map(String::as_str),
            Some("ok"),
            "success op carries the mock's result string"
        );
    }

    // --- assert the real side-effects on disk -------------------------------
    // each successful task left out.txt in its cwd; the failed one did not.
    for id in &success_ids {
        let f = root.join(id.to_string()).join(SIDE_EFFECT);
        assert!(f.exists(), "successful task left its side-effect file: {f:?}");
        let body = std::fs::read_to_string(&f).expect("read side-effect");
        assert_eq!(body.trim(), "done by mock");
    }
    let failed_file = root.join(fail_id.to_string()).join(SIDE_EFFECT);
    assert!(
        !failed_file.exists(),
        "the failed task left no side-effect file"
    );

    // --- feed the produced ops through the engine dispatcher ----------------
    // this is what makes it a `test(engine)`: the supervisor's control ops route
    // through the same Engine::apply seam a live node uses. op::Op is !Clone, so
    // we asserted on &ops above, then move them in here. NoopControl accepts and
    // emits no follow-ups.
    let empty_ws = workspace::Workspace::new_from_entry(workspace::Entry::Directory(Vec::new()));
    let mut engine = engine::Engine::new(empty_ws);
    for op in ops {
        let follow_ups = engine.apply(op).expect("engine routes the control op");
        assert!(
            follow_ups.is_empty(),
            "noop control emits no follow-up ops"
        );
    }

    // best-effort cleanup.
    let _ = std::fs::remove_dir_all(&root);
}
