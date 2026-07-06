use futures::executor::block_on;
use host::{BlockContext, Host};
use sdk::{
    Ctx, Effect, Env, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle,
};
use tasks::Tasks;
use tasks::{
    TaskMsg, TaskQuery, TaskReply, TaskStatus, decode_reply, encode_msg, encode_query,
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
}

impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: Env { protocol_version: 0,
                height: 0,
                consensus_time,
                origin: Origin::System,
                me: TASKS.into(),
            },
        }
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
    fn emit_event(&mut self, _event: Event) {}
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
                BlockContext { protocol_version: 0,
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
                BlockContext { protocol_version: 0,
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
