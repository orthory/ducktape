use futures::executor::block_on;
use host::{BlockContext, Host};
use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sdk_testkit::{MemStore, TestCtx};
use tasks::Tasks;
use tasks::{
    TaskMsg, TaskQuery, TaskReply, TaskStatus, decode_task_reply as decode_reply,
    encode_task_msg as encode_msg, encode_task_query as encode_query,
};

const TASKS: &str = "tasks";

/// build the module the way a host does: concrete store first, injected as
/// `Box<dyn MerkleStore>`. these tests assert BEHAVIOR, so the in-memory store
/// stands in for qmdb; the real-store round trip lives in `sync_round_trip`.
fn tasks_on_mem() -> Tasks {
    Tasks::new(TASKS, "identity", "attribution", Box::new(MemStore::new()))
}

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
        owner: None,
    })
}

fn update(task_id: &str, status: TaskStatus) -> Msg {
    msg(TaskMsg::UpdateStatus {
        task_id: task_id.into(),
        status,
    })
}

fn delete(task_id: &str) -> Msg {
    msg(TaskMsg::DeleteTask {
        task_id: task_id.into(),
    })
}

// tasks' execute reads env.origin for task-board ownership; this stands in
// behind an External-origin constructor for the origin-gating tests.
fn at_as(consensus_time: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height: 0,
        consensus_time,
        origin,
        me: TASKS.into(),
        cause: sdk::Cause::Direct,
    })
    .on_query("identity", |_| {
        Ok(identity::encode_reply(&identity::IdentityReply::Account(
            None,
        )))
    })
}

fn ext(who: &str) -> Origin {
    Origin::External(who.as_bytes().to_vec())
}

/// the whole board as ONE page — every test board here is far under the clamp.
fn whole_board() -> TaskQuery {
    TaskQuery::List {
        limit: tasks::MAX_LIST_LIMIT,
        after: None,
    }
}

fn page_of(reply: &[u8]) -> Vec<tasks::Task> {
    let TaskReply::Tasks(tasks) = decode_reply(reply).expect("decode reply") else {
        panic!("a list answers a page");
    };
    tasks
}

async fn module_tasks(tasks: &Tasks) -> Vec<tasks::Task> {
    page_of(
        &tasks
            .query(&encode_query(&whole_board()))
            .await
            .expect("query tasks"),
    )
}

async fn host_tasks(host: &Host) -> Vec<tasks::Task> {
    page_of(
        &host
            .query(TASKS, &encode_query(&whole_board()))
            .await
            .expect("query tasks"),
    )
}

// tasks' execute reads only env (consensus_time); me/height are cosmetic, so
// the shared TestCtx stands in behind a thin System-origin constructor.
fn at(consensus_time: u64) -> TestCtx {
    TestCtx::with_env(Env {
        height: 0,
        consensus_time,
        origin: Origin::System,
        me: TASKS.into(),
        cause: sdk::Cause::Direct,
    })
    .on_query("identity", |_| {
        Ok(identity::encode_reply(&identity::IdentityReply::Account(
            None,
        )))
    })
}

#[test]
fn create_list_and_update_status() {
    block_on(async {
        let mut tasks = tasks_on_mem();

        tasks
            .execute(&mut at(11), &create("task-b", "second"))
            .await
            .expect("create b");
        tasks
            .execute(&mut at(11), &create("task-a", "first"))
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
            .execute(&mut at(22), &update("task-a", TaskStatus::InProgress))
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
        let mut tasks = tasks_on_mem();
        let root0 = tasks.root();

        tasks
            .execute(&mut at(7), &create("task-1", "write docs"))
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
            .execute(&mut at(8), &update("task-1", TaskStatus::Done))
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
        let mut host = Host::genesis(vec![
            Box::new(identity::Identity::new(
                "identity",
                Box::new(MemStore::new()),
                "test".into(),
            )),
            Box::new(attribution::AttributionModule::new(
                "attribution",
                Box::new(MemStore::new()),
            )),
            Box::new(tasks_on_mem()),
            Box::new(CreateThenFail),
        ])
        .expect("genesis");

        let root0 = host.module_root(TASKS).expect("tasks root");
        let app0 = host.root_hash();

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
            host.root_hash(),
            app0,
            "failed block must leave root-hash unchanged"
        );
        assert!(
            host_tasks(&host).await.is_empty(),
            "the staged create must be discarded"
        );
    });
}

#[test]
fn root_hash_changes_when_task_state_changes() {
    block_on(async {
        let mut host = Host::genesis(vec![
            Box::new(identity::Identity::new(
                "identity",
                Box::new(MemStore::new()),
                "test".into(),
            )),
            Box::new(attribution::AttributionModule::new(
                "attribution",
                Box::new(MemStore::new()),
            )),
            Box::new(tasks_on_mem()),
        ])
        .expect("genesis");
        let app0 = host.root_hash();

        let created = host
            .submit_at(
                BlockContext {
                    height: 1,
                    consensus_time: 3,
                    origin: Origin::External(b"tester".to_vec()),
                },
                create("task-1", "ship tasks"),
            )
            .await
            .expect("create task");
        assert_ne!(
            created.root_hash, app0,
            "create must move the global root-hash"
        );
        assert_eq!(created.root_hash, host.root_hash());

        let updated = host
            .submit_at(
                BlockContext {
                    height: 2,
                    consensus_time: 4,
                    origin: Origin::External(b"tester".to_vec()),
                },
                update("task-1", TaskStatus::Done),
            )
            .await
            .expect("update task status");
        assert_ne!(
            updated.root_hash, created.root_hash,
            "status changes must move the global root-hash"
        );

        let tasks = host_tasks(&host).await;
        assert_eq!(tasks[0].status, TaskStatus::Done);
        assert_eq!(tasks[0].updated_at, 4);
    });
}

#[test]
fn state_sync_handle_is_resolver_backed() {
    block_on(async {
        let mut tasks = tasks_on_mem();
        tasks
            .execute(&mut at(5), &create("task-1", "sync me"))
            .await
            .expect("create");
        tasks.commit_block().await.expect("commit");

        // the module is qmdb-backed: sync rides the store's resolver lane, so
        // capture is O(1) and NEVER a re-serialization of the whole board.
        match tasks.state_sync_handle().expect("state-sync handle") {
            StateSyncHandle::ResolverBacked { backend, .. } => assert_eq!(backend, "qmdb"),
            other => panic!("expected ResolverBacked, got {other:?}"),
        }
        assert!(
            tasks.snapshot_bytes().is_none(),
            "a store-backed module ships no byte snapshot"
        );
    });
}

#[test]
fn updated_at_before_created_at_is_execute_reachable() {
    block_on(async {
        // consensus_time has NO cross-block monotonicity guarantee, so a status
        // update in a later block can legitimately stamp updated_at BELOW
        // created_at. the board stores what execute produced -- there is no
        // decode-time invariant sweep to refuse it (the store's merkle root is
        // the integrity check).
        let mut tasks = tasks_on_mem();
        tasks
            .execute(&mut at(10), &create("task-1", "time travels"))
            .await
            .expect("create at t=10");
        tasks.commit_block().await.expect("commit create");
        tasks
            .execute(&mut at(5), &update("task-1", TaskStatus::Done))
            .await
            .expect("update at t=5");
        tasks.commit_block().await.expect("commit update");

        let listed = module_tasks(&tasks).await;
        assert_eq!(listed[0].created_at, 10);
        assert_eq!(
            listed[0].updated_at, 5,
            "the premise: updated_at < created_at is execute-reachable"
        );
    });
}

// the ONE guard the storage swap adds: a record the concrete store's codec
// would panic decoding (over 1 MiB) is refused at WRITE time -- never staged,
// never committed -- instead of poisoning every later read on every validator.
#[test]
fn oversized_task_record_is_refused_before_staging() {
    block_on(async {
        let mut tasks = tasks_on_mem();
        let root0 = tasks.root();

        let err = tasks
            .execute(
                &mut at(1),
                &create("big", &"x".repeat(tasks::MAX_RECORD_BYTES)),
            )
            .await
            .expect_err("an over-cap task record must be refused");
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("store record cap")),
            "unexpected error: {err:?}"
        );

        tasks.commit_block().await.expect("commit");
        assert_eq!(
            tasks.root(),
            root0,
            "a refused write must not move the root"
        );
        assert!(module_tasks(&tasks).await.is_empty());
    });
}

// every task id shares the ONE `t#` index record, so an uncapped task_id is a
// board-wide weapon: a handful of ~1 MiB ids (each fits in one op frame) packs
// that record to the store cap and every later create -- anyone's -- fails
// FOREVER, with no delete op to free the bytes. MAX_TASK_ID is what keeps the
// index bounded, so the refusal has to land on the ID, before the index grows.
#[test]
fn oversized_task_id_cannot_brick_the_board() {
    block_on(async {
        let mut tasks = tasks_on_mem();

        for attempt in 0..5 {
            let huge_id = format!("{}{attempt}", "x".repeat(256 * 1024));
            let err = tasks
                .execute(&mut at(1), &create(&huge_id, "brick the board"))
                .await
                .expect_err("an over-cap task_id must be refused");
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("task_id is")),
                "unexpected error: {err:?}"
            );
        }
        tasks.commit_block().await.expect("commit the refusals");

        // the premise: after the attack the board is still writable by anyone.
        let at_cap = "y".repeat(tasks::MAX_TASK_ID);
        tasks
            .execute(&mut at(2), &create(&at_cap, "an id exactly at the cap"))
            .await
            .expect("an id of exactly MAX_TASK_ID bytes is accepted");
        tasks
            .execute(&mut at(2), &create("normal", "an ordinary task"))
            .await
            .expect("an ordinary create still works");
        tasks.commit_block().await.expect("commit creates");

        let ids: Vec<String> = module_tasks(&tasks)
            .await
            .into_iter()
            .map(|task| task.id)
            .collect();
        assert_eq!(ids, ["normal".to_owned(), at_cap]);
    });
}

// wired end-to-end through `Tasks::execute` (not the board's own unit tests):
// a stranger's restatus is refused, the owner's is accepted, and a delete
// frees the task's slot.
#[test]
fn a_strangers_update_is_refused_the_owners_is_accepted() {
    block_on(async {
        let mut tasks = tasks_on_mem();
        tasks
            .execute(&mut at_as(1, ext("alice")), &create("t1", "alice's task"))
            .await
            .expect("alice creates");
        tasks.commit_block().await.expect("commit create");

        let refused = tasks
            .execute(
                &mut at_as(2, ext("mallory")),
                &update("t1", TaskStatus::Done),
            )
            .await
            .expect_err("mallory cannot restatus alice's task");
        assert!(
            matches!(refused, Error::Module(ref m) if m.contains("only the owner")),
            "unexpected error: {refused:?}"
        );

        tasks
            .execute(&mut at_as(3, ext("alice")), &update("t1", TaskStatus::Done))
            .await
            .expect("alice may update her own task");
        tasks.commit_block().await.expect("commit update");
        assert_eq!(module_tasks(&tasks).await[0].status, TaskStatus::Done);
    });
}

#[test]
fn delete_frees_a_slot_at_the_cap_and_a_per_owner_cap_admits_another_owner() {
    block_on(async {
        let mut tasks = tasks_on_mem();
        for n in 0..tasks::MAX_OPEN_TASKS_PER_OWNER {
            tasks
                .execute(
                    &mut at_as(1, ext("alice")),
                    &create(&format!("a{n}"), "alice's task"),
                )
                .await
                .expect("under alice's per-owner cap");
        }
        tasks.commit_block().await.expect("commit alice's tasks");

        let refused = tasks
            .execute(
                &mut at_as(2, ext("alice")),
                &create("a-over", "one too many"),
            )
            .await
            .expect_err("alice is at her per-owner cap");
        assert!(
            matches!(refused, Error::Module(ref m) if m.contains("task owner at cap")),
            "unexpected error: {refused:?}"
        );

        // a different owner is unaffected by alice's cap.
        tasks
            .execute(
                &mut at_as(3, ext("mallory")),
                &create("m1", "not alice's problem"),
            )
            .await
            .expect("another owner is still admitted");
        tasks.commit_block().await.expect("commit mallory's task");

        // a stranger cannot free alice's slot.
        let refused = tasks
            .execute(&mut at_as(4, ext("mallory")), &delete("a0"))
            .await
            .expect_err("mallory cannot delete alice's task");
        assert!(
            matches!(refused, Error::Module(ref m) if m.contains("only the owner")),
            "unexpected error: {refused:?}"
        );

        // alice deletes her own task, freeing the slot she was at the cap on.
        tasks
            .execute(&mut at_as(5, ext("alice")), &delete("a0"))
            .await
            .expect("alice may delete her own task");
        tasks.commit_block().await.expect("commit delete");
        tasks
            .execute(
                &mut at_as(6, ext("alice")),
                &create("a-again", "the freed slot is usable"),
            )
            .await
            .expect("the freed slot readmits alice");
    });
}
