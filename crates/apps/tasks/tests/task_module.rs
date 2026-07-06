use futures::executor::block_on;
use host::{BlockContext, Host};
use package::{
    PackageActionMsg, PackageActionQuery, PackageActionReply, decode_action_reply,
    encode_action_msg, encode_action_query,
};
use sdk::{
    Ctx, Effect, Env, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle,
};
use tasks::Tasks;
use tasks::{
    ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS, TaskMsg, TaskQuery, TaskReply, TaskStatus,
    decode_reply, encode_msg, encode_query,
};

const TASKS: &str = "tasks";

fn msg(task_msg: TaskMsg) -> Msg {
    Msg {
        target: TASKS.into(),
        payload: encode_msg(&task_msg),
    }
}

fn create(task_id: &str, title: &str) -> Msg {
    msg(TaskMsg::CreateTask {
        task_id: task_id.into(),
        title: title.into(),
    })
}

fn update(task_id: &str, status: TaskStatus) -> Msg {
    msg(TaskMsg::UpdateStatus {
        task_id: task_id.into(),
        status,
    })
}

async fn module_tasks(tasks: &Tasks) -> Vec<tasks::Task> {
    match decode_reply(
        &tasks
            .query(&encode_query(&TaskQuery::List))
            .await
            .expect("query tasks"),
    )
    .expect("decode reply")
    {
        TaskReply::Tasks(tasks) => tasks,
    }
}

async fn host_tasks(host: &Host) -> Vec<tasks::Task> {
    match decode_reply(
        &host
            .query(TASKS, &encode_query(&TaskQuery::List))
            .await
            .expect("query tasks"),
    )
    .expect("decode reply")
    {
        TaskReply::Tasks(tasks) => tasks,
    }
}

struct TestCtx {
    env: Env,
    /// the no-fail Apply arm's breadcrumbs land here.
    events: Vec<Event>,
}

impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height: 0,
                consensus_time,
                origin: Origin::System,
                me: TASKS.into(),
            },
            events: Vec::new(),
        }
    }

    fn with_origin(mut self, origin: Origin) -> Self {
        self.env.origin = origin;
        self
    }

    fn breadcrumbs(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect()
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, event: Event) {
        self.events.push(event);
    }
    fn request_effect(&mut self, _effect: Effect) {}
}

#[test]
fn create_list_and_update_status() {
    block_on(async {
        let mut tasks = Tasks::new(TASKS);

        tasks
            .execute(&mut TestCtx::at(11), &create("task-b", "second"))
            .await
            .expect("create b");
        tasks
            .execute(&mut TestCtx::at(11), &create("task-a", "first"))
            .await
            .expect("create a");
        tasks.commit_block().await.expect("commit creates");

        let listed = module_tasks(&tasks).await;
        let ids: Vec<&str> = listed.iter().map(|task| task.id.as_str()).collect();
        assert_eq!(ids, ["task-a", "task-b"], "list order is deterministic");
        assert_eq!(listed[0].title, "first");
        assert_eq!(listed[0].status, TaskStatus::Open);
        assert_eq!(listed[0].created_at, 11);
        assert_eq!(listed[0].updated_at, 11);

        tasks
            .execute(
                &mut TestCtx::at(22),
                &update("task-a", TaskStatus::InProgress),
            )
            .await
            .expect("update status");
        tasks.commit_block().await.expect("commit update");

        let listed = module_tasks(&tasks).await;
        assert_eq!(listed[0].status, TaskStatus::InProgress);
        assert_eq!(listed[0].created_at, 11);
        assert_eq!(listed[0].updated_at, 22);
    });
}

#[test]
fn root_changes_only_after_commit() {
    block_on(async {
        let mut tasks = Tasks::new(TASKS);
        let root0 = tasks.root();

        tasks
            .execute(&mut TestCtx::at(7), &create("task-1", "write docs"))
            .await
            .expect("stage create");
        assert_eq!(
            tasks.root(),
            root0,
            "staged writes must not move the committed root"
        );
        assert_eq!(
            module_tasks(&tasks).await.len(),
            1,
            "queries read through the staged overlay"
        );

        tasks.commit_block().await.expect("commit create");
        let root1 = tasks.root();
        assert_ne!(root1, root0, "commit moves the root");

        tasks
            .execute(&mut TestCtx::at(8), &update("task-1", TaskStatus::Done))
            .await
            .expect("stage update");
        assert_eq!(tasks.root(), root1, "root remains committed-state only");
        tasks.abort_block().await.expect("abort update");
        assert_eq!(tasks.root(), root1, "abort keeps the root byte-identical");
        assert_eq!(module_tasks(&tasks).await[0].status, TaskStatus::Open);
    });
}

struct CreateThenFail;

#[async_trait::async_trait(?Send)]
impl Module for CreateThenFail {
    fn id(&self) -> ModuleId {
        "create-then-fail".into()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        ctx.emit_msg(create("task-1", "should roll back"));
        ctx.emit_msg(update("missing", TaskStatus::Done));
        Ok(())
    }
}

#[test]
fn failed_write_rolls_back_task_state() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Tasks::new(TASKS)), Box::new(CreateThenFail)])
            .expect("genesis");

        let root0 = host.module_root(TASKS).expect("tasks root");
        let app0 = host.app_hash();

        let err = host
            .submit(Msg {
                target: "create-then-fail".into(),
                payload: Vec::new(),
            })
            .await
            .expect_err("missing task update must fail the block");
        assert!(
            matches!(
                err,
                host::SubmitError::Rejected(Error::Module(ref message))
                    if message.contains("task not found")
            ),
            "unexpected error: {err:?}"
        );

        assert_eq!(
            host.module_root(TASKS).expect("tasks root"),
            root0,
            "failed block must leave task root unchanged"
        );
        assert_eq!(
            host.app_hash(),
            app0,
            "failed block must leave app-hash unchanged"
        );
        assert!(
            host_tasks(&host).await.is_empty(),
            "the staged create must be discarded"
        );
    });
}

#[test]
fn app_hash_changes_when_task_state_changes() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Tasks::new(TASKS))]).expect("genesis");
        let app0 = host.app_hash();

        let created = host
            .submit_at(
                BlockContext {
                    protocol_version: 0,
                    height: 1,
                    consensus_time: 3,
                    origin: Origin::External(b"tester".to_vec()),
                },
                create("task-1", "ship tasks"),
            )
            .await
            .expect("create task");
        assert_ne!(
            created.app_hash, app0,
            "create must move the global app-hash"
        );
        assert_eq!(created.app_hash, host.app_hash());

        let updated = host
            .submit_at(
                BlockContext {
                    protocol_version: 0,
                    height: 2,
                    consensus_time: 4,
                    origin: Origin::External(b"tester".to_vec()),
                },
                update("task-1", TaskStatus::Done),
            )
            .await
            .expect("update task status");
        assert_ne!(
            updated.app_hash, created.app_hash,
            "status changes must move the global app-hash"
        );

        let tasks = host_tasks(&host).await;
        assert_eq!(tasks[0].status, TaskStatus::Done);
        assert_eq!(tasks[0].updated_at, 4);
    });
}

#[test]
fn state_sync_handle_returns_installable_snapshot_bytes() {
    block_on(async {
        let mut source = Tasks::new(TASKS);
        source
            .execute(&mut TestCtx::at(5), &create("task-1", "sync me"))
            .await
            .expect("create");
        source
            .execute(&mut TestCtx::at(5), &create("task-2", "me too"))
            .await
            .expect("create");
        source.commit_block().await.expect("commit");

        // the module advertises self-contained snapshot bytes...
        let handle = source.state_sync_handle().expect("state-sync handle");
        let bytes = match handle {
            StateSyncHandle::SnapshotBytes(bytes) => bytes,
            other => panic!("expected SnapshotBytes, got {other:?}"),
        };

        // ...that install verbatim on a joiner against the source root.
        let mut target = Tasks::new(TASKS);
        target
            .install(&bytes, source.root())
            .expect("install handle bytes");
        assert_eq!(target.root(), source.root());
        assert_eq!(module_tasks(&target).await, module_tasks(&source).await);
    });
}

#[test]
fn snapshot_with_updated_at_before_created_at_round_trips() {
    block_on(async {
        // consensus_time has NO cross-block monotonicity guarantee, so a status
        // update in a later block can legitimately stamp updated_at BELOW
        // created_at. install must accept this execute-reachable state instead
        // of refusing a snapshot an honest validator committed.
        let mut source = Tasks::new(TASKS);
        source
            .execute(&mut TestCtx::at(10), &create("task-1", "time travels"))
            .await
            .expect("create at t=10");
        source.commit_block().await.expect("commit create");
        source
            .execute(&mut TestCtx::at(5), &update("task-1", TaskStatus::Done))
            .await
            .expect("update at t=5");
        source.commit_block().await.expect("commit update");

        let listed = module_tasks(&source).await;
        assert_eq!(listed[0].created_at, 10);
        assert_eq!(
            listed[0].updated_at, 5,
            "the premise: updated_at < created_at is execute-reachable"
        );

        let mut target = Tasks::new(TASKS);
        target
            .install(&source.snapshot(), source.root())
            .expect("install must accept execute-reachable timestamps");
        assert_eq!(target.root(), source.root());
        assert_eq!(module_tasks(&target).await, module_tasks(&source).await);
    });
}

#[test]
fn snapshot_install_reconstructs_task_state() {
    block_on(async {
        let mut source = Tasks::new(TASKS);
        source
            .execute(&mut TestCtx::at(5), &create("task-1", "sync me"))
            .await
            .expect("create");
        source.commit_block().await.expect("commit");

        let expected = source.root();
        let snapshot = source.snapshot();
        let mut target = Tasks::new(TASKS);
        target
            .install(&snapshot, expected)
            .expect("install verified snapshot");

        assert_eq!(target.root(), expected);
        assert_eq!(module_tasks(&target).await, module_tasks(&source).await);
    });
}

// ---- the package action-owner contract (design D6) --------------------------
// tasks is the first built-in action owner: `Probe` serves the read-only
// verdict via `query_with`, and the `Apply` arm is a NO-FAIL module-origin
// intake — decode-or-drop, re-check cheaply, breadcrumb on late conflict.

fn probe(tag: &str, payload: serde_json::Value) -> Vec<u8> {
    encode_action_query(&PackageActionQuery::Probe {
        action_id: "a1".into(),
        tag: tag.into(),
        payload: serde_json::to_vec(&payload).expect("payload"),
        run_context: br#"{"run_id":"r1","agent_id":"bot"}"#.to_vec(),
    })
}

fn apply(action_id: &str, tag: &str, payload: serde_json::Value) -> Msg {
    Msg {
        target: TASKS.into(),
        payload: encode_action_msg(&PackageActionMsg::Apply {
            action_id: action_id.into(),
            tag: tag.into(),
            payload: serde_json::to_vec(&payload).expect("payload"),
            run_context: br#"{"run_id":"r1","agent_id":"bot"}"#.to_vec(),
        }),
    }
}

async fn probe_verdict(tasks: &Tasks, tag: &str, payload: serde_json::Value) -> PackageActionReply {
    let ctx = TestCtx::at(0);
    let reply = tasks
        .query_with(&ctx, &probe(tag, payload))
        .await
        .expect("probe is servable");
    decode_action_reply(&reply).expect("a probe verdict")
}

fn rejected_with(verdict: &PackageActionReply, fragment: &str) {
    match verdict {
        PackageActionReply::Rejected { reason } => assert!(
            reason.contains(fragment),
            "reason {reason:?} must mention {fragment:?}"
        ),
        PackageActionReply::Accepted => panic!("expected a rejection mentioning {fragment:?}"),
    }
}

/// a committed module holding one Open task "t0".
async fn owner_with_t0() -> Tasks {
    let mut tasks = Tasks::new(TASKS);
    tasks
        .execute(&mut TestCtx::at(1), &create("t0", "existing"))
        .await
        .expect("create t0");
    tasks.commit_block().await.expect("commit");
    tasks
}

#[test]
fn probe_accepts_valid_actions_and_rejects_policy_violations() {
    block_on(async {
        let tasks = owner_with_t0().await;

        // valid create and valid update are Accepted.
        assert_eq!(
            probe_verdict(
                &tasks,
                ACTION_TASKS_CREATE,
                serde_json::json!({"task_id": "t1", "title": "fresh"})
            )
            .await,
            PackageActionReply::Accepted
        );
        assert_eq!(
            probe_verdict(
                &tasks,
                ACTION_TASKS_UPDATE_STATUS,
                serde_json::json!({"task_id": "t0", "status": "done"})
            )
            .await,
            PackageActionReply::Accepted
        );

        // a duplicate id rejects the create.
        rejected_with(
            &probe_verdict(
                &tasks,
                ACTION_TASKS_CREATE,
                serde_json::json!({"task_id": "t0", "title": "dup"}),
            )
            .await,
            "already exists",
        );
        // empty fields reject the create.
        rejected_with(
            &probe_verdict(
                &tasks,
                ACTION_TASKS_CREATE,
                serde_json::json!({"task_id": "", "title": "x"}),
            )
            .await,
            "non-empty",
        );
        // a missing task rejects the update.
        rejected_with(
            &probe_verdict(
                &tasks,
                ACTION_TASKS_UPDATE_STATUS,
                serde_json::json!({"task_id": "ghost", "status": "done"}),
            )
            .await,
            "unknown task",
        );
        // a bad status value rejects the update.
        rejected_with(
            &probe_verdict(
                &tasks,
                ACTION_TASKS_UPDATE_STATUS,
                serde_json::json!({"task_id": "t0", "status": "shipped"}),
            )
            .await,
            "unknown task status",
        );
        // a tag this module does not own rejects.
        rejected_with(
            &probe_verdict(&tasks, "pages.comment.add", serde_json::json!({})).await,
            "does not own",
        );
        // a malformed payload rejects instead of erroring.
        rejected_with(
            &probe_verdict(
                &tasks,
                ACTION_TASKS_CREATE,
                serde_json::json!("not an object"),
            )
            .await,
            "malformed",
        );

        // TaskQuery still rides the same lane untouched.
        assert_eq!(module_tasks(&tasks).await.len(), 1);
    });
}

#[test]
fn probe_sees_staged_state() {
    block_on(async {
        let mut tasks = owner_with_t0().await;
        // stage (do not commit) t1: a same-block earlier op.
        tasks
            .execute(&mut TestCtx::at(2), &create("t1", "staged"))
            .await
            .expect("stage t1");
        rejected_with(
            &probe_verdict(
                &tasks,
                ACTION_TASKS_CREATE,
                serde_json::json!({"task_id": "t1", "title": "dup of staged"}),
            )
            .await,
            "already exists",
        );
        assert_eq!(
            probe_verdict(
                &tasks,
                ACTION_TASKS_UPDATE_STATUS,
                serde_json::json!({"task_id": "t1", "status": "done"})
            )
            .await,
            PackageActionReply::Accepted,
            "an update may target a task staged earlier in the block"
        );
    });
}

#[test]
fn apply_creates_and_updates_from_module_origin() {
    block_on(async {
        let mut tasks = owner_with_t0().await;
        let mut ctx = TestCtx::at(9).with_origin(Origin::Module("runs".into()));
        tasks
            .execute(
                &mut ctx,
                &apply(
                    "a1",
                    ACTION_TASKS_CREATE,
                    serde_json::json!({"task_id": "t1", "title": "fresh"}),
                ),
            )
            .await
            .expect("apply is no-fail");
        tasks
            .execute(
                &mut ctx,
                &apply(
                    "a2",
                    ACTION_TASKS_UPDATE_STATUS,
                    serde_json::json!({"task_id": "t1", "status": "in_progress"}),
                ),
            )
            .await
            .expect("apply is no-fail");
        assert!(
            ctx.breadcrumbs().is_empty(),
            "clean applies leave no crumbs"
        );
        tasks.commit_block().await.expect("commit");

        let listed = module_tasks(&tasks).await;
        let t1 = listed.iter().find(|t| t.id == "t1").expect("t1 created");
        assert_eq!(t1.title, "fresh");
        assert_eq!(t1.status, TaskStatus::InProgress);
        assert_eq!(t1.created_at, 9);
    });
}

#[test]
fn apply_on_late_conflict_breadcrumbs_and_never_errs() {
    block_on(async {
        let mut tasks = Tasks::new(TASKS);
        let mut ctx = TestCtx::at(5).with_origin(Origin::Module("runs".into()));
        // two creates of the SAME id in one delivery block: both probed clean
        // before either applied — the second is the in-block late conflict.
        for action_id in ["a1", "a2"] {
            tasks
                .execute(
                    &mut ctx,
                    &apply(
                        action_id,
                        ACTION_TASKS_CREATE,
                        serde_json::json!({"task_id": "dup", "title": "one of two"}),
                    ),
                )
                .await
                .expect("apply must NEVER abort the delivery block");
        }
        let crumbs = ctx.breadcrumbs();
        assert_eq!(crumbs.len(), 1, "exactly the conflicting apply crumbs");
        assert!(
            crumbs[0].contains("a2") && crumbs[0].contains("already exists"),
            "the crumb names the action and the conflict: {crumbs:?}"
        );
        tasks.commit_block().await.expect("commit");
        assert_eq!(module_tasks(&tasks).await.len(), 1, "first create landed");
    });
}

#[test]
fn apply_drops_garbage_with_breadcrumbs_instead_of_erring() {
    block_on(async {
        let mut tasks = owner_with_t0().await;
        let before = tasks.root();
        let mut ctx = TestCtx::at(5).with_origin(Origin::Module("runs".into()));

        // a malformed payload and a foreign tag both drop, never error.
        tasks
            .execute(
                &mut ctx,
                &apply("a1", ACTION_TASKS_CREATE, serde_json::json!(42)),
            )
            .await
            .expect("no-fail");
        tasks
            .execute(
                &mut ctx,
                &apply("a2", "pages.comment.add", serde_json::json!({})),
            )
            .await
            .expect("no-fail");
        assert_eq!(ctx.breadcrumbs().len(), 2);
        tasks.commit_block().await.expect("commit");
        assert_eq!(tasks.root(), before, "nothing staged");
    });
}

#[test]
fn external_origins_never_reach_the_apply_arm() {
    block_on(async {
        let mut tasks = Tasks::new(TASKS);
        // Apply-shaped bytes from an EXTERNAL origin route to the TaskMsg
        // decoder and fail there — an external submitter cannot fake the
        // probed-and-accepted path.
        let mut ctx = TestCtx::at(5).with_origin(Origin::External(vec![1; 32]));
        let err = tasks
            .execute(
                &mut ctx,
                &apply(
                    "a1",
                    ACTION_TASKS_CREATE,
                    serde_json::json!({"task_id": "t1", "title": "sneaky"}),
                ),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(_)));

        // while a module-origin TaskMsg (the automations follow-up path)
        // keeps its registration-posture behavior.
        let mut ctx = TestCtx::at(6).with_origin(Origin::Module("automations".into()));
        tasks
            .execute(&mut ctx, &create("t2", "from automations"))
            .await
            .expect("module-origin TaskMsg still works");
        tasks.commit_block().await.expect("commit");
        assert_eq!(module_tasks(&tasks).await.len(), 1);
    });
}
