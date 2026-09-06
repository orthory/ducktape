//! One large source deletion commits with all withdrawals under real guest limits.
use attribution::{
    Actor, AttributionModule, AttributionMsg, AttributionQuery, AttributionReply,
    AttributionUpdate, ChangeKind, ObjectRef, Reason, Relation, Source,
};
use futures::executor::block_on;
use host::{BlockContext, Host};
use identity::{Identity, IdentityMsg, KeyScheme};
use pages::{Block, BlockKind, InlineMark, PageMsg, PageQuery, PageReply, Pages, Party, SpanMark};
use sdk::{Cause, Env, MerkleStore, Module, Msg, Origin};
use sdk_testkit::{MemStore, TestCtx};
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
