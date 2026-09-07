//! One large source deletion commits with all withdrawals under real guest limits.
use attribution::{
    Actor, AttributionModule, AttributionMsg, AttributionQuery, AttributionReply,
    AttributionUpdate, ChangeKind, ObjectRef, Reason, Relation, Source,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use futures::executor::block_on;
use host::{BlockContext, Host};
use identity::{Identity, IdentityMsg, KeyScheme};
use pages::{
    Block, BlockKind, InlineMark, NewBlock, PageMsg, PageQuery, PageReply, Pages, Party, SpanMark,
};
use sdk::{Cause, Env, MerkleStore, Module, Msg, Origin};
use sdk_testkit::{MemStore, TestCtx};
use statesync::qmdb::QmdbStore;
use wasm_host::WasmModule;

const PAGES: &[u8] = include_bytes!("fixtures/pages.component.wasm");
const ATTRIBUTION: &[u8] = include_bytes!("fixtures/attribution.component.wasm");
const CHILDREN: u64 = 1_100;

fn key(number: u64) -> Vec<u8> {
    number.to_le_bytes().repeat(4)
}
fn block(
    id: &str,
    parent: Option<&str>,
    kind: BlockKind,
    author: u64,
    children: Vec<String>,
) -> Block {
    Block {
        author: Party::Account(author),
        id: id.into(),
        parent: parent.map(str::to_owned),
        page: "home".into(),
        kind,
        text: "tag".into(),
        marks: Vec::new(),
        checked: false,
        children,
    }
}
fn fixture() -> Vec<Block> {
    let mut blocks = vec![
        block("home", None, BlockKind::Page, 1, vec!["branch".into()]),
        block(
            "branch",
            Some("home"),
            BlockKind::Paragraph,
            1,
            (0..CHILDREN).map(|index| format!("leaf-{index}")).collect(),
        ),
    ];
    for index in 0..CHILDREN {
        let mut leaf = block(
            &format!("leaf-{index}"),
            Some("branch"),
            BlockKind::Paragraph,
            index * 2 + 1,
            Vec::new(),
        );
        leaf.marks.push(SpanMark {
            start: 0,
            end: 3,
            kind: InlineMark::Mention(index * 2 + 2),
        });
        blocks.push(leaf);
    }
    blocks
}
async fn page_store(blocks: &[Block]) -> MemStore {
    let mut store = MemStore::new();
    let mut writes: Vec<_> = blocks
        .iter()
        .map(|block| {
            (
                sdk::store_key(block.id.as_bytes()),
                Some(serde_json::to_vec(block).unwrap()),
            )
        })
        .collect();
    writes.push((
        sdk::store_key(b"\0page-index"),
        Some(serde_json::to_vec(&serde_json::json!({"home": null})).unwrap()),
    ));
    writes.push((
        sdk::store_key(b"\0attribution-revision"),
        Some(b"1".to_vec()),
    ));
    store.commit_batch(writes).await.unwrap();
    store
}
async fn identity() -> Identity {
    let mut identity = Identity::new("identity", Box::new(MemStore::new()), "capacity".into());
    for number in 1..=CHILDREN * 2 {
        let mut ctx = TestCtx::with_env(Env {
            height: 1,
            consensus_time: 1,
            origin: Origin::External(key(number)),
            me: "identity".into(),
            cause: Cause::Direct,
        });
        identity
            .execute(
                &mut ctx,
                &Msg {
                    target: "identity".into(),
                    payload: identity::encode_msg(&IdentityMsg::Create {
                        name: format!("person-{number}"),
                        scheme: KeyScheme::Ed25519,
                    }),
                },
            )
            .await
            .unwrap();
    }
    identity.commit_block().await.unwrap();
    identity
}
async fn hosts(blocks: &[Block]) -> (Host, Host) {
    let mut native = Host::new();
    native.register(Box::new(
        Pages::new("pages", Box::new(page_store(blocks).await))
            .with_identity("identity")
            .with_attribution("attribution"),
    ));
    native.register(Box::new(
        AttributionModule::new("attribution", Box::new(MemStore::new()))
            .with_subscribers(["agent", "inbox"]),
    ));
    native.register(Box::new(identity().await));
    let mut wasm = Host::new();
    wasm.register(Box::new(
        WasmModule::with_store("pages", PAGES, Box::new(page_store(blocks).await)).unwrap(),
    ));
    wasm.register(Box::new(
        WasmModule::with_store("attribution", ATTRIBUTION, Box::new(MemStore::new())).unwrap(),
    ));
    wasm.register(Box::new(identity().await));
    (native, wasm)
}
fn initial(block: &Block) -> AttributionUpdate {
    let Party::Account(author) = block.author else {
        panic!("fixture author")
    };
    let mut relations = vec![Relation {
        recipient: author,
        reason: Reason::Authorship,
        detail: Vec::new(),
    }];
    if block.kind == BlockKind::Page {
        relations.push(Relation {
            recipient: author,
            reason: Reason::Ownership,
            detail: Vec::new(),
        });
    }
    for mark in &block.marks {
        if let InlineMark::Mention(recipient) = mark.kind {
            relations.push(Relation {
                recipient,
                reason: Reason::Mention,
                detail: Vec::new(),
            });
        }
    }
    AttributionUpdate {
        object: ObjectRef {
            kind: "block".into(),
            object: block.id.clone(),
        },
        revision: 1,
        actor: Actor::Account(1),
        relations,
        transfers: Vec::new(),
    }
}
fn roots(native: &Host, wasm: &Host) {
    for module in ["pages", "attribution", "identity"] {
        assert_eq!(
            native.module_root(module),
            wasm.module_root(module),
            "{module} root"
        );
    }
}
async fn submit(host: &mut Host, height: u64, origin: Origin, target: &str, payload: Vec<u8>) {
    host.submit_at(
        BlockContext {
            height,
            consensus_time: height,
            origin,
        },
        Msg {
            target: target.into(),
            payload,
        },
    )
    .await
    .unwrap();
}

#[test]
fn one_compiled_page_subtree_deletion_withdraws_2200_recipients_atomically() {
    block_on(async {
        let blocks = fixture();
        let (mut native, mut wasm) = hosts(&blocks).await;
        roots(&native, &wasm);
        // Seed the already-authored source histories through both real central
        // executors. The operation under test is the one subtree deletion below.
        for (index, group) in blocks.chunks(42).enumerate() {
            let payload = attribution::encode_msg(&AttributionMsg::AttributeBatch {
                updates: group.iter().map(initial).collect(),
            });
            for host in [&mut native, &mut wasm] {
                submit(
                    host,
                    index as u64 + 2,
                    Origin::Module("pages".into()),
                    "attribution",
                    payload.clone(),
                )
                .await;
            }
        }
        roots(&native, &wasm);
        let removal = pages::encode_msg(&PageMsg::RemoveBlock {
            block_id: "branch".into(),
        });
        for host in [&mut native, &mut wasm] {
            submit(
                host,
                100,
                Origin::External(key(1)),
                "pages",
                removal.clone(),
            )
            .await;
        }
        roots(&native, &wasm);
        for host in [&native, &wasm] {
            let bytes = host
                .query(
                    "pages",
                    &pages::encode_query(&PageQuery::GetBlock {
                        block_id: "home".into(),
                    }),
                )
                .await
                .unwrap();
            let PageReply::Block(Some(home)) = pages::decode_reply(&bytes).unwrap() else {
                panic!("home")
            };
            assert!(home.children.is_empty());
        }
        // Every distinct source survives in central history. Full root equality
        // proves the guest's entire result; sample queries also pin its decoder.
        for index in 0..CHILDREN {
            let query = attribution::encode_query(&AttributionQuery::ChangesOf {
                source: Source {
                    module: "pages".into(),
                    kind: "block".into(),
                    object: format!("leaf-{index}"),
                },
                after: 0,
                limit: 10,
            });
            let bytes = native.query("attribution", &query).await.unwrap();
            let AttributionReply::Changes(changes) = attribution::decode_reply(&bytes).unwrap()
            else {
                panic!("source history")
            };
            assert_eq!(changes.len(), 4);
            for change in &changes[2..] {
                assert_eq!(change.change.revision, 2);
                assert_eq!(change.change.kind, ChangeKind::Withdrawn);
                assert_eq!(change.change.height, 100);
            }
            if [0, CHILDREN / 2, CHILDREN - 1].contains(&index) {
                assert_eq!(wasm.query("attribution", &query).await.unwrap(), bytes);
                let query = pages::encode_query(&PageQuery::GetBlock {
                    block_id: format!("leaf-{index}"),
                });
                let PageReply::Block(None) =
                    pages::decode_reply(&wasm.query("pages", &query).await.unwrap()).unwrap()
                else {
                    panic!("retired leaf")
                };
            }
        }
    });
}

const MENTION_COUNT: u64 = 900;

async fn mention_identity_store(
    context: &deterministic::Context,
    label: &'static str,
) -> QmdbStore<deterministic::Context> {
    let mut store = QmdbStore::init(context.child(label), label).await;
    store
        .commit_batch(vec![(
            sdk::store_key(sdk::genesis_config::CONFIG_KEY),
            Some(sdk::genesis_config::encode_config(&[(
                "chain_id",
                b"capacity",
            )])),
        )])
        .await
        .unwrap();
    let mut identity = Identity::new("identity", Box::new(store), "capacity".into());
    for number in 1..=MENTION_COUNT + 1 {
        let mut ctx = TestCtx::with_env(Env {
            height: 0,
            consensus_time: 0,
            origin: Origin::External(key(number)),
            me: "identity".into(),
            cause: Cause::Direct,
        });
        identity
            .execute(
                &mut ctx,
                &Msg {
                    target: "identity".into(),
                    payload: identity::encode_msg(&IdentityMsg::Create {
                        name: format!("person-{number}"),
                        scheme: KeyScheme::Ed25519,
                    }),
                },
            )
            .await
            .unwrap();
    }
    identity.commit_block().await.unwrap();
    drop(identity);
    QmdbStore::init(context.child(label), label).await
}

async fn native_mention_host(context: &deterministic::Context) -> Host {
    Host::genesis(vec![
        Box::new(
            Pages::new("pages", Box::new(MemStore::new()))
                .with_identity("identity")
                .with_attribution("attribution"),
        ),
        Box::new(
            AttributionModule::new("attribution", Box::new(MemStore::new()))
                .with_subscribers(["agent", "inbox"]),
        ),
        Box::new(Identity::new(
            "identity",
            Box::new(mention_identity_store(context, "native_mentions").await),
            "capacity".into(),
        )),
    ])
    .unwrap()
}

async fn wasm_mention_host(context: &deterministic::Context) -> Host {
    Host::genesis(vec![
        Box::new(WasmModule::with_store("pages", PAGES, Box::new(MemStore::new())).unwrap()),
        Box::new(
            WasmModule::with_store("attribution", ATTRIBUTION, Box::new(MemStore::new())).unwrap(),
        ),
        Box::new(
            WasmModule::with_store(
                "identity",
                include_bytes!("fixtures/identity.component.wasm"),
                Box::new(mention_identity_store(context, "wasm_mentions").await),
            )
            .unwrap(),
        ),
    ])
    .unwrap()
}

async fn exercise_many_mentions(host: &mut Host) {
    let mentions: Vec<_> = (2..=MENTION_COUNT + 1).collect();
    let marks: Vec<_> = mentions
        .iter()
        .map(|number| SpanMark {
            start: 0,
            end: 3,
            kind: InlineMark::Mention(*number),
        })
        .collect();
    let operations = [
        PageMsg::CreatePage {
            page_id: "home".into(),
            title: "Home".into(),
        },
        PageMsg::InsertBlock {
            parent: "home".into(),
            after: None,
            block: NewBlock {
                id: "many".into(),
                kind: BlockKind::Paragraph,
                text: "tag".into(),
                marks: marks.clone(),
            },
        },
        PageMsg::AddComment {
            thread_id: "thread".into(),
            comment_id: "comment".into(),
            target: "many".into(),
            text: "many mentions".into(),
            anchor: None,
            mentions: mentions.clone(),
        },
    ];
    for (index, operation) in operations.iter().enumerate() {
        submit(
            host,
            index as u64 + 1,
            Origin::External(key(1)),
            "pages",
            pages::encode_msg(operation),
        )
        .await;
    }
    let bytes = host
        .query(
            "pages",
            &pages::encode_query(&PageQuery::GetBlock {
                block_id: "many".into(),
            }),
        )
        .await
        .unwrap();
    let PageReply::Block(Some(block)) = pages::decode_reply(&bytes).unwrap() else {
        panic!("block")
    };
    assert_eq!(block.marks, marks);
    let bytes = host
        .query(
            "pages",
            &pages::encode_query(&PageQuery::GetComment {
                comment_id: "comment".into(),
            }),
        )
        .await
        .unwrap();
    let PageReply::Comment(Some(comment)) = pages::decode_reply(&bytes).unwrap() else {
        panic!("comment")
    };
    assert_eq!(comment.mentions, mentions);
    for (kind, object) in [("block", "many"), ("comment", "comment")] {
        let bytes = host
            .query(
                "attribution",
                &attribution::encode_query(&AttributionQuery::Relations {
                    source: Source {
                        module: "pages".into(),
                        kind: kind.into(),
                        object: object.into(),
                    },
                }),
            )
            .await
            .unwrap();
        let AttributionReply::Relations(Some(relations)) =
            attribution::decode_reply(&bytes).unwrap()
        else {
            panic!("relations")
        };
        let attributed: Vec<_> = relations
            .relations
            .iter()
            .filter(|relation| relation.reason == Reason::Mention)
            .map(|relation| relation.recipient)
            .collect();
        assert_eq!(attributed, mentions);
    }

    // A missing account beyond the first batch rejects the whole replacement.
    let settled = host.module_roots();
    let mut invalid = mentions;
    invalid.push(9_999);
    let error = host
        .submit_at(
            BlockContext {
                height: 4,
                consensus_time: 4,
                origin: Origin::External(key(1)),
            },
            Msg {
                target: "pages".into(),
                payload: pages::encode_msg(&PageMsg::EditComment {
                    comment_id: "comment".into(),
                    text: "must not commit".into(),
                    mentions: invalid,
                }),
            },
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("mention names no account: 9999"),
        "{error}"
    );
    assert_eq!(host.module_roots(), settled);
}

#[test]
fn native_900_recipient_block_and_comment_writes_keep_every_relation() {
    deterministic::Runner::default().start(|context| async move {
        exercise_many_mentions(&mut native_mention_host(&context).await).await;
    });
}

#[test]
fn compiled_900_recipient_block_and_comment_writes_keep_every_relation() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_mention_host(&context).await;
        let mut wasm = wasm_mention_host(&context).await;
        exercise_many_mentions(&mut native).await;
        exercise_many_mentions(&mut wasm).await;
        roots(&native, &wasm);
    });
}
