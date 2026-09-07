use attribution::{AttributionMsg, AttributionQuery, AttributionReply, Reason, Source};
use futures::executor::block_on;
use host::{BlockContext, Host};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sdk_testkit::MemStore;
use tasks::{JobsMsg, Party, TaskMsg, TaskQuery, TaskReply, TaskStatus, Tasks};

struct Executor;
#[async_trait::async_trait(?Send)]
impl Module for Executor {
    fn id(&self) -> ModuleId {
        "executor".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, _: &mut dyn Ctx, _: &Msg) -> Result<(), Error> {
        Ok(())
    }
}

fn context(origin: Origin) -> BlockContext {
    BlockContext {
        height: 1,
        consensus_time: 1,
        origin,
    }
}
fn alice() -> Origin {
    Origin::External(vec![1; 32])
}
fn bob() -> Origin {
    Origin::External(vec![2; 32])
}

async fn arena() -> Host {
    let mut host = Host::genesis(vec![
        Box::new(Tasks::new(
            "tasks",
            "identity",
            "attribution",
            Box::new(MemStore::new()),
        )),
        Box::new(identity::Identity::new(
            "identity",
            Box::new(MemStore::new()),
            "test".into(),
        )),
        Box::new(attribution::AttributionModule::new(
            "attribution",
            Box::new(MemStore::new()),
        )),
        Box::new(Executor),
    ])
    .unwrap();
    for (origin, name) in [(alice(), "Alice"), (bob(), "Bob")] {
        host.submit_at(
            context(origin),
            Msg {
                target: "identity".into(),
                payload: identity::encode_msg(&identity::IdentityMsg::Create {
                    name: name.into(),
                    scheme: identity::KeyScheme::Ed25519,
                }),
            },
        )
        .await
        .unwrap();
    }
    host.submit_at(
        context(Origin::Module("executor".into())),
        Msg {
            target: "identity".into(),
            payload: identity::encode_msg(&identity::IdentityMsg::CreateProgram {
                name: "Program".into(),
                controller: 1,
                request: 0,
            }),
        },
    )
    .await
    .unwrap();
    host
}
async fn task(host: &mut Host, origin: Origin, msg: TaskMsg) {
    host.submit_at(
        context(origin),
        Msg {
            target: "tasks".into(),
            payload: tasks::encode_task_msg(&msg),
        },
    )
    .await
    .unwrap();
}
async fn job(host: &mut Host, origin: Origin, msg: JobsMsg) {
    host.submit_at(
        context(origin),
        Msg {
            target: "tasks".into(),
            payload: tasks::encode_job_msg(&msg),
        },
    )
    .await
    .unwrap();
}
async fn relations(host: &Host, kind: &str, object: &str) -> attribution::ObjectRelations {
    let query = AttributionQuery::Relations {
        source: Source {
            module: "tasks".into(),
            kind: kind.into(),
            object: object.into(),
        },
    };
    let bytes = host
        .query("attribution", &attribution::encode_query(&query))
        .await
        .unwrap();
    let AttributionReply::Relations(Some(relations)) = attribution::decode_reply(&bytes).unwrap()
    else {
        panic!("source relations");
    };
    relations
}
fn create() -> TaskMsg {
    TaskMsg::CreateTask {
        task_id: "task".into(),
        title: "Work".into(),
        owner: None,
    }
}

#[test]
fn program_owns_ordinary_tasks_and_recreation_retains_revision_history() {
    block_on(async {
        let mut host = arena().await;
        task(&mut host, Origin::Program(3), create()).await;
        let source = relations(&host, "task", "task").await;
        assert_eq!(source.revision, 1);
        assert_eq!(source.relations[0].recipient, 3);
        let query = tasks::encode_task_query(&TaskQuery::Get {
            task_id: "task".into(),
        });
        let TaskReply::Task(Some(record)) =
            tasks::decode_task_reply(&host.query("tasks", &query).await.unwrap()).unwrap()
        else {
            panic!("task");
        };
        assert_eq!(record.owner, Party::Account(3));
        let root = host.root_hash();
        let stranger = TaskMsg::UpdateStatus {
            task_id: "task".into(),
            status: TaskStatus::Done,
        };
        assert!(
            host.submit_at(
                context(alice()),
                Msg {
                    target: "tasks".into(),
                    payload: tasks::encode_task_msg(&stranger)
                }
            )
            .await
            .is_err()
        );
        assert_eq!(host.root_hash(), root);
        task(
            &mut host,
            Origin::Program(3),
            TaskMsg::DeleteTask {
                task_id: "task".into(),
            },
        )
        .await;
        let retired = relations(&host, "task", "task").await;
        assert_eq!(retired.revision, 2);
        assert!(retired.relations.is_empty());
        task(&mut host, Origin::Program(3), create()).await;
        assert_eq!(relations(&host, "task", "task").await.revision, 3);
        let query = AttributionQuery::ChangesOf {
            source: source.source,
            after: 0,
            limit: 100,
        };
        let bytes = host
            .query("attribution", &attribution::encode_query(&query))
            .await
            .unwrap();
        let AttributionReply::Changes(changes) = attribution::decode_reply(&bytes).unwrap() else {
            panic!("changes");
        };
        assert_eq!(changes.len(), 3);
        assert!(
            changes
                .iter()
                .all(|entry| entry.change.actor == attribution::Actor::Account(3))
        );
    });
}

#[test]
fn claims_release_and_results_publish_full_relation_sets() {
    block_on(async {
        let mut host = arena().await;
        job(
            &mut host,
            alice(),
            JobsMsg::Submit {
                job_id: "job".into(),
                kind: "review".into(),
                spec: "Check".into(),
            },
        )
        .await;
        job(
            &mut host,
            Origin::Program(3),
            JobsMsg::Claim {
                job_id: "job".into(),
                lease_views: 10,
            },
        )
        .await;
        assert!(
            relations(&host, "job", "job")
                .await
                .relations
                .iter()
                .any(|relation| relation.recipient == 3 && relation.reason == Reason::Assignment)
        );
        job(
            &mut host,
            Origin::Program(3),
            JobsMsg::Release {
                job_id: "job".into(),
            },
        )
        .await;
        assert!(
            !relations(&host, "job", "job")
                .await
                .relations
                .iter()
                .any(|relation| relation.reason == Reason::Assignment)
        );
        job(
            &mut host,
            bob(),
            JobsMsg::Claim {
                job_id: "job".into(),
                lease_views: 10,
            },
        )
        .await;
        job(
            &mut host,
            bob(),
            JobsMsg::Finalize {
                job_id: "job".into(),
                ok: false,
                payload: "Review failed".into(),
            },
        )
        .await;
        let completed = relations(&host, "job", "job").await;
        assert_eq!(completed.revision, 5);
        assert!(
            completed
                .relations
                .iter()
                .any(|relation| relation.recipient == 1 && relation.reason == Reason::Result)
        );
        assert!(
            completed
                .relations
                .iter()
                .any(|relation| relation.recipient == 2 && relation.reason == Reason::Assignment)
        );
        job(
            &mut host,
            alice(),
            JobsMsg::Prune {
                job_id: "job".into(),
            },
        )
        .await;
        assert!(relations(&host, "job", "job").await.relations.is_empty());
        job(
            &mut host,
            alice(),
            JobsMsg::Submit {
                job_id: "job".into(),
                kind: "again".into(),
                spec: "".into(),
            },
        )
        .await;
        assert_eq!(relations(&host, "job", "job").await.revision, 7);
    });
}

#[test]
fn attribution_rejection_rolls_back_the_source_and_assigned_output() {
    block_on(async {
        let mut host = arena().await;
        host.submit_at(
            context(Origin::Module("tasks".into())),
            Msg {
                target: "attribution".into(),
                payload: attribution::encode_msg(&AttributionMsg::Attribute {
                    object: attribution::ObjectRef {
                        kind: "task".into(),
                        object: "task".into(),
                    },
                    revision: 1,
                    actor: attribution::Actor::System,
                    relations: Vec::new(),
                    transfers: Vec::new(),
                }),
            },
        )
        .await
        .unwrap();
        let root = host.root_hash();
        let result = host
            .submit_at(
                context(alice()),
                Msg {
                    target: "tasks".into(),
                    payload: tasks::encode_task_msg(&create()),
                },
            )
            .await;
        assert!(result.is_err());
        assert_eq!(host.root_hash(), root);
        let bytes = host
            .query(
                "tasks",
                &tasks::encode_task_query(&TaskQuery::Get {
                    task_id: "task".into(),
                }),
            )
            .await
            .unwrap();
        assert_eq!(
            tasks::decode_task_reply(&bytes).unwrap(),
            TaskReply::Task(None)
        );
    });
}

#[test]
fn an_authenticated_key_keeps_its_records_after_joining_identity() {
    block_on(async {
        let mut host = arena().await;
        let signer = Origin::External(vec![4; 32]);
        task(&mut host, signer.clone(), create()).await;
        job(
            &mut host,
            signer.clone(),
            JobsMsg::Submit {
                job_id: "owned".into(),
                kind: "work".into(),
                spec: "".into(),
            },
        )
        .await;
        job(
            &mut host,
            alice(),
            JobsMsg::Submit {
                job_id: "claimed".into(),
                kind: "work".into(),
                spec: "".into(),
            },
        )
        .await;
        job(
            &mut host,
            signer.clone(),
            JobsMsg::Claim {
                job_id: "claimed".into(),
                lease_views: 10,
            },
        )
        .await;
        host.submit_at(
            context(signer.clone()),
            Msg {
                target: "identity".into(),
                payload: identity::encode_msg(&identity::IdentityMsg::Create {
                    name: "Joined".into(),
                    scheme: identity::KeyScheme::Ed25519,
                }),
            },
        )
        .await
        .unwrap();
        task(
            &mut host,
            signer.clone(),
            TaskMsg::UpdateStatus {
                task_id: "task".into(),
                status: TaskStatus::Done,
            },
        )
        .await;
        job(
            &mut host,
            signer.clone(),
            JobsMsg::Cancel {
                job_id: "owned".into(),
            },
        )
        .await;
        job(
            &mut host,
            signer.clone(),
            JobsMsg::Prune {
                job_id: "owned".into(),
            },
        )
        .await;
        job(
            &mut host,
            signer.clone(),
            JobsMsg::Finalize {
                job_id: "claimed".into(),
                ok: true,
                payload: "Finished after joining".into(),
            },
        )
        .await;
        let query = tasks::encode_task_query(&TaskQuery::Get {
            task_id: "task".into(),
        });
        let TaskReply::Task(Some(record)) =
            tasks::decode_task_reply(&host.query("tasks", &query).await.unwrap()).unwrap()
        else {
            panic!("task");
        };
        assert_eq!(
            record.owner,
            Party::Key(vec![4; 32]),
            "admission never transfers ownership"
        );
        assert_eq!(record.status, TaskStatus::Done);
        let root = host.root_hash();
        assert!(
            host.submit_at(
                context(alice()),
                Msg {
                    target: "tasks".into(),
                    payload: tasks::encode_task_msg(&TaskMsg::DeleteTask {
                        task_id: "task".into()
                    })
                }
            )
            .await
            .is_err()
        );
        assert_eq!(host.root_hash(), root);
        task(
            &mut host,
            signer,
            TaskMsg::DeleteTask {
                task_id: "task".into(),
            },
        )
        .await;
    });
}
