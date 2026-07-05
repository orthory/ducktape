//! the FULL network joiner property: a fresh node rebuilds EVERY module of a
//! running host purely through the statesync wire protocol — one manifest at
//! one finalized boundary, chunked snapshot fetches for the snapshot-lane
//! modules, live proof-carrying qmdb op-range fetches for the resolver-lane
//! modules — and composes the source's exact app-hash.
//!
//! this supersedes the in-process handoffs of `joiner_rebuilds_global_app_hash`
//! (which still pins the per-module sync primitives): here NOTHING crosses the
//! boundary except protocol bytes. the transport is an in-process channel; the
//! bytes, frames, and client code are identical to what a p2p channel carries.

use futures::channel::{mpsc, oneshot};
use futures::{SinkExt as _, StreamExt as _};

use chat::Chat;
use chat_interface::{Block as ChatBlock, ChatMsg, PostPolicy, encode_msg as chat_encode_msg};
use directory::Directory;
use directory_interface::{DirMsg, encode_msg as dir_encode_msg};
use document::Document;
use document_interface::{Block, BlockKind, DocMsg, encode_msg as doc_encode_msg};
use forge::Forge;
use greeter::Greeter;
use host::{FinalizedBlock, Host};
use kv::Kv;
use kv_interface::{KvMsg, encode as kv_encode};
use saga::SagaModule;
use saga_interface::{SagaMsg, encode_msg as saga_encode_msg};
use sdk::{Module, Msg, StateRoot};
use state::global_root;
use valset::Valset;
use valset_interface::{ValsetMsg, encode_msg as valset_encode_msg};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};

use statesync::qmdb::RemoteQmdbResolver;
use statesync::{
    ManifestEntry, PayloadKind, SyncClient, SyncError, SyncRequest, SyncResponse, SyncServer,
    decode_response, encode_request, fetch_manifest, fetch_snapshot,
};

// ---- the in-process transport (same shape as the statesync crate test) -----

type RpcPair = (Vec<u8>, oneshot::Sender<Vec<u8>>);

#[derive(Clone)]
struct ChannelClient {
    tx: mpsc::Sender<RpcPair>,
}

fn pinned_target(entry: &ManifestEntry) -> statesync::qmdb::SyncTarget {
    entry
        .resolver_target
        .as_ref()
        .expect("resolver entry carries pinned target")
        .to_sync_target()
        .expect("pinned target range is non-empty")
}

impl SyncClient for ChannelClient {
    fn request(
        &self,
        req: SyncRequest,
    ) -> impl std::future::Future<Output = Result<SyncResponse, SyncError>> + Send {
        let mut tx = self.tx.clone();
        async move {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send((encode_request(&req), reply_tx))
                .await
                .map_err(|e| SyncError::Transport(format!("request channel closed: {e}")))?;
            let bytes = reply_rx
                .await
                .map_err(|_| SyncError::Transport("server dropped the reply".into()))?;
            Ok(decode_response(&bytes)?)
        }
    }
}

fn validator_key(seed_byte: u8) -> Vec<u8> {
    let seed = [seed_byte; 32];
    PrivateKey::decode(&seed[..])
        .expect("any 32 bytes is a valid ed25519 seed")
        .public_key()
        .as_ref()
        .to_vec()
}

fn tmp_repo(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("ducktape-net-joiner-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

#[test]
fn joiner_rebuilds_every_module_over_the_wire_and_matches_the_app_hash() {
    let source_repo = tmp_repo("source");
    let joiner_repo = tmp_repo("joiner");
    let (source_dir, joiner_dir) = (source_repo.clone(), joiner_repo.clone());

    deterministic::Runner::default().start(|context| async move {
        // ---- SOURCE: the full module set behind one Host, real op path ------
        let kv = Kv::init(context.child("source_kv"), "kv").await;
        let document = Document::init(context.child("source_document"), "document").await;
        let chat = Chat::init(context.child("source_chat"), "chat").await;
        let forge = Forge::init("forge", source_dir.clone()).expect("forge init");
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(document),
            Box::new(chat),
            Box::new(forge),
            Box::new(Directory::new("directory")),
            Box::new(Valset::new("valset")),
            Box::new(SagaModule::new("saga")),
            Box::new(Greeter::new("greeter")),
        ])
        .expect("genesis");

        // content through every module, including overwrites (op-log order).
        let ops: Vec<Msg> = vec![
            Msg {
                target: "kv".into(),
                payload: kv_encode(&KvMsg::Set {
                    key: b"motd".to_vec(),
                    value: b"draft".to_vec(),
                }),
            },
            Msg {
                target: "kv".into(),
                payload: kv_encode(&KvMsg::Set {
                    key: b"motd".to_vec(),
                    value: b"final".to_vec(),
                }),
            },
            Msg {
                target: "document".into(),
                payload: doc_encode_msg(&DocMsg::CreateDoc {
                    doc_id: "readme".into(),
                }),
            },
            Msg {
                target: "document".into(),
                payload: doc_encode_msg(&DocMsg::InsertBlock {
                    doc_id: "readme".into(),
                    after: None,
                    block: Block {
                        id: "title".into(),
                        kind: BlockKind::Heading,
                        text: "ducktape".into(),
                    },
                }),
            },
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                }),
            },
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "u1".into(),
                    blocks: vec![ChatBlock::paragraph("hello")],
                    thread: None,
                    as_agent: None,
                }),
            },
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "a1".into(),
                    blocks: vec![ChatBlock::paragraph("synced over the wire")],
                    thread: None,
                    as_agent: None,
                }),
            },
            Msg {
                target: "forge".into(),
                payload: forge_interface::encode_msg(&forge_interface::ForgeMsg::Commit {
                    repo: String::new(),
                    path: "README.md".into(),
                    content: "# ducktape\n".into(),
                    message: "init".into(),
                }),
            },
            Msg {
                target: "directory".into(),
                payload: dir_encode_msg(&DirMsg::Set {
                    key: "name".into(),
                    value: "world".into(),
                }),
            },
            Msg {
                target: "valset".into(),
                payload: valset_encode_msg(&ValsetMsg::Join {
                    key: validator_key(7),
                }),
            },
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::Trigger {
                    pinned_assignee: None,
                    saga_id: "greet".into(),
                    spec: b"reverse hello".to_vec(),
                    reply_to: None,
                    reply_payload: Vec::new(),
                    deadline: None,
                    max_attempts: 1,
                    lease_views: None,
                    capability: None,
                }),
            },
        ];
        let mut height = 0u64;
        for op in ops {
            // membership ops are governance-gated: drive every source op on the
            // SYSTEM origin lane (trusted test orchestration), which valset and
            // every product module accept alike.
            host.submit_at(
                host::BlockContext { protocol_version: 0,
                    height: height + 1,
                    consensus_time: height + 1,
                    origin: sdk::Origin::System,
                },
                op,
            )
            .await
            .expect("source op");
            height += 1;
        }
        let finalized = FinalizedBlock {
            height,
            app_hash: host.app_hash(),
        };

        // ---- the wire ---------------------------------------------------------
        let (tx, rx) = mpsc::channel::<RpcPair>(16);
        let client = ChannelClient { tx };
        let mut server = SyncServer::new();

        let joiner_ctx = context.child("joiner");
        let client_for_join = client.clone();
        let join_side = async move {
            let client = client_for_join;
            let manifest = fetch_manifest(&client).await.expect("manifest");
            assert_eq!(manifest.app_hash, finalized.app_hash);
            assert_eq!(
                manifest.entries.len(),
                8,
                "every registered module is listed"
            );
            let boundary = manifest.boundary_id();

            // --- resolver lane: kv, document, chat -----------------------------
            let kv_entry = manifest.entry("kv").unwrap();
            let kv_root = kv_entry.root;
            let resolver = RemoteQmdbResolver::new(client.clone(), boundary, "kv");
            let target = pinned_target(kv_entry);
            assert_eq!(
                StateRoot(target.root.0),
                kv_root,
                "kv target matches manifest"
            );
            let join_kv = Kv::sync_from(
                joiner_ctx.child("joiner_kv"),
                "kv-rebuilt",
                target,
                resolver,
            )
            .await;
            assert_eq!(join_kv.root(), kv_root);
            assert_eq!(
                join_kv.get(b"motd").await.as_deref(),
                Some(b"final".as_ref())
            );

            let doc_entry = manifest.entry("document").unwrap();
            let doc_root = doc_entry.root;
            let resolver = RemoteQmdbResolver::new(client.clone(), boundary, "document");
            let target = pinned_target(doc_entry);
            assert_eq!(StateRoot(target.root.0), doc_root);
            let join_document = Document::sync_from(
                joiner_ctx.child("joiner_document"),
                "document-rebuilt",
                target,
                resolver,
            )
            .await;
            assert_eq!(join_document.root(), doc_root);

            let chat_entry = manifest.entry("chat").unwrap();
            let chat_root = chat_entry.root;
            let resolver = RemoteQmdbResolver::new(client.clone(), boundary, "chat");
            let target = pinned_target(chat_entry);
            assert_eq!(StateRoot(target.root.0), chat_root);
            let join_chat = Chat::sync_from(
                joiner_ctx.child("joiner_chat"),
                "chat-rebuilt",
                target,
                resolver,
            )
            .await;
            assert_eq!(join_chat.root(), chat_root);

            // --- snapshot lane: directory, valset, saga, forge ----------------
            let entry = manifest.entry("directory").unwrap();
            assert_eq!(entry.kind, PayloadKind::Snapshot);
            let bytes = fetch_snapshot(&client, boundary, "directory")
                .await
                .expect("directory snapshot");
            let mut join_directory = Directory::new("directory");
            join_directory
                .install(&bytes, entry.root)
                .expect("directory install");

            let entry = manifest.entry("valset").unwrap();
            let bytes = fetch_snapshot(&client, boundary, "valset")
                .await
                .expect("valset snapshot");
            let mut join_valset = Valset::new("valset");
            join_valset
                .install(&bytes, entry.root)
                .expect("valset install");

            let entry = manifest.entry("saga").unwrap();
            let bytes = fetch_snapshot(&client, boundary, "saga")
                .await
                .expect("saga snapshot");
            let mut join_saga = SagaModule::new("saga");
            join_saga.install(&bytes, entry.root).expect("saga install");

            let entry = manifest.entry("forge").unwrap();
            let bytes = fetch_snapshot(&client, boundary, "forge")
                .await
                .expect("forge snapshot");
            let mut join_forge =
                Forge::init("forge", joiner_dir.clone()).expect("joiner forge init");
            join_forge
                .install(&bytes, entry.root)
                .expect("forge install");

            // --- stateless lane ------------------------------------------------
            assert_eq!(
                manifest.entry("greeter").unwrap().kind,
                PayloadKind::Stateless
            );
            let join_greeter = Greeter::new("greeter");

            // --- THE property: the composed app-hash equals the manifest's ----
            // the rebuilt qmdb stores live under distinct storage ids inside this
            // ONE deterministic runner (a real joiner has its own disk); compose
            // under the canonical module ids exactly as `global_root` would see
            // them on a real node.
            struct AtId {
                id: &'static str,
                root: StateRoot,
            }
            #[async_trait::async_trait(?Send)]
            impl Module for AtId {
                fn id(&self) -> sdk::ModuleId {
                    self.id.to_string()
                }
                fn root(&self) -> StateRoot {
                    self.root
                }
                async fn execute(
                    &mut self,
                    _c: &mut dyn sdk::Ctx,
                    _m: &Msg,
                ) -> Result<(), sdk::Error> {
                    Ok(())
                }
            }
            let kv_at = AtId {
                id: "kv",
                root: join_kv.root(),
            };
            let doc_at = AtId {
                id: "document",
                root: join_document.root(),
            };
            let chat_at = AtId {
                id: "chat",
                root: join_chat.root(),
            };
            let mods: [&dyn Module; 8] = [
                &kv_at,
                &doc_at,
                &chat_at,
                &join_directory,
                &join_valset,
                &join_saga,
                &join_forge,
                &join_greeter,
            ];
            assert_eq!(
                global_root(&mods),
                manifest.app_hash,
                "the joiner lands on the exact app-hash the source finalized"
            );
        };

        let server_side = async {
            // fixed coordinates: this test exercises the module payload
            // lanes; the epoch fields just ride the manifest.
            let coords = statesync::BoundaryCoords::default();
            let mut rx = rx;
            while let Some((frame, reply)) = rx.next().await {
                let resp = server
                    .handle_frame(&host, Some(finalized), &coords, &frame)
                    .await;
                let _ = reply.send(resp);
            }
        };
        drop(client);
        futures::join!(join_side, server_side);
    });

    let _ = std::fs::remove_dir_all(&source_repo);
    let _ = std::fs::remove_dir_all(&joiner_repo);
}
