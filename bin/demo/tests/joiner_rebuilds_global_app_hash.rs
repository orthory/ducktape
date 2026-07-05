//! the joiner property, end-to-end: a fresh node reconstructs EVERY stateful
//! module from its per-module state-sync surface and lands on the EXACT global
//! app-hash the source composed — the acceptance check a new validator must
//! pass before its votes can count at the boundary height.
//!
//! the source drives real content through each module's own execute +
//! commit_block path (payloads built via the *-interface crates, exactly as the
//! demo binary does), including OVERWRITES in kv and document: a qmdb root is
//! op-log ordered, so a naive "export current pairs and re-apply" could never
//! reproduce it — only the real sync path can. the joiner rebuilds kv, document,
//! and chat through the qmdb sync engine (target + resolver), forge / valset /
//! directory / saga / agent through snapshot + install gated on the source
//! root, and greeter fresh (stateless). every reconstructed module is then
//! read back — content, not just digests — and one tampered snapshot must be
//! refused without disturbing the already-installed state.

use agent::AgentModule;
use agent_interface::{
    ACTION_CHAT_POST, AgentMsg, AgentQuery, AgentReply, AgentStatus, TurnPolicy,
    decode_reply as agent_decode_reply, encode_msg as agent_encode_msg,
    encode_query as agent_encode_query,
};
use chat::Chat;
use chat_interface::{
    Block as ChatBlock, ChatMsg, ChatQuery, ChatReply, MessageView, PostPolicy,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use demo::state_sync::{
    LoopbackStateSyncResolver, MeshParticipant, MeshRole, StateSyncKind, StateSyncPayload,
    StateSyncPeerId, StateSyncRequest, decode_qmdb_target, decode_response, encode_qmdb_target,
    encode_request,
};
use directory::Directory;
use directory_interface::{DirMsg, encode_msg as dir_encode_msg};
use document::Document;
use document_interface::{
    Block, BlockKind, DocMsg, DocQuery, DocReply, decode_reply as doc_decode_reply,
    encode_msg as doc_encode_msg, encode_query as doc_encode_query,
};
use forge::Forge;
use forge_interface::{ForgeMsg, encode_msg as forge_encode_msg};
use greeter::Greeter;
use kv::Kv;
use kv_interface::{KvMsg, encode as kv_encode};
use saga::SagaModule;
use saga_interface::{
    SagaMsg, SagaQuery, SagaReply, SagaStatus, decode_reply as saga_decode_reply,
    encode_msg as saga_encode_msg, encode_query as saga_encode_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use state::global_root;
use valset::Valset;
use valset_interface::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_msg as valset_encode_msg, encode_query as valset_encode_query,
};

// a minimal Ctx so each module's execute can be driven without a full host.
// forge reads consensus_time from it; saga drops a WorkerRequest effect into it
// (the worker half is not under test); nothing else touches it.
struct TestCtx {
    env: sdk::Env,
}
impl TestCtx {
    fn at(consensus_time: u64, me: &str) -> Self {
        Self {
            env: sdk::Env { protocol_version: 0,
                height: 0,
                consensus_time,
                origin: sdk::Origin::System,
                me: me.to_string(),
            },
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

/// drive one op through the REAL module path as its own block: execute stages,
/// commit_block publishes — so committed state (and, for the qmdb stores, the
/// op log) is exactly what a validator would have produced.
async fn commit_op(module: &mut dyn Module, at: u64, payload: Vec<u8>) {
    let target = module.id();
    let msg = Msg {
        target: target.clone(),
        payload,
    };
    module
        .execute(&mut TestCtx::at(at, &target), &msg)
        .await
        .unwrap();
    module.commit_block().await.unwrap();
}

/// like [`commit_op`] but with an explicit dispatch origin — the agent
/// module's admin ops are owner-gated and reject the system origin.
async fn commit_op_as(module: &mut dyn Module, at: u64, origin: sdk::Origin, payload: Vec<u8>) {
    let target = module.id();
    let msg = Msg {
        target: target.clone(),
        payload,
    };
    let mut ctx = TestCtx::at(at, &target);
    ctx.env.origin = origin;
    module.execute(&mut ctx, &msg).await.unwrap();
    module.commit_block().await.unwrap();
}

/// the (id, root) pair a node's registry reports for a module — the exact input
/// `state::global_root` consumes. needed because ONE deterministic runner
/// shares ONE storage-partition namespace: the joiner's qmdb stores must
/// rebuild under distinct storage ids or they would open the source's live
/// partitions, while the app-hash both nodes agree on is composed over the
/// canonical module id. a real joiner has its own disk, so its stores open the
/// canonical id directly and this adapter does not exist.
struct RegistryEntry {
    id: ModuleId,
    root: StateRoot,
}
impl RegistryEntry {
    fn of(id: &str, module: &dyn Module) -> Self {
        Self {
            id: id.to_string(),
            root: module.root(),
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Module for RegistryEntry {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn root(&self) -> StateRoot {
        self.root
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), Error> {
        Ok(())
    }
}

/// compose the joiner's app-hash: rebuilt qmdb stores that use non-canonical
/// storage ids enter under their canonical module ids via [`RegistryEntry`];
/// the rest carry their canonical ids natively. a free function (not a
/// borrowing closure) so the caller can still mutate a module between
/// compositions.
#[allow(clippy::too_many_arguments)]
fn joiner_app_hash(
    kv: &dyn Module,
    document: &dyn Module,
    chat: &dyn Module,
    directory: &dyn Module,
    greeter: &dyn Module,
    forge: &dyn Module,
    valset: &dyn Module,
    saga: &dyn Module,
    agent: &dyn Module,
) -> StateRoot {
    let kv_entry = RegistryEntry::of("kv", kv);
    let document_entry = RegistryEntry::of("document", document);
    let chat_entry = RegistryEntry::of("chat", chat);
    let mods: [&dyn Module; 9] = [
        &kv_entry,
        directory,
        greeter,
        forge,
        &document_entry,
        &chat_entry,
        valset,
        saga,
        agent,
    ];
    global_root(&mods)
}

/// a deterministic, VALID 32-byte ed25519 public key: any 32 bytes is a valid
/// seed, and the derived public key is always a valid curve point.
fn validator_key(seed_byte: u8) -> Vec<u8> {
    let seed = [seed_byte; 32];
    PrivateKey::decode(&seed[..])
        .expect("any 32 bytes is a valid ed25519 seed")
        .public_key()
        .as_ref()
        .to_vec()
}

async fn validators(v: &Valset) -> Vec<Vec<u8>> {
    let reply = v
        .query(&valset_encode_query(&ValsetQuery::Validators))
        .await
        .unwrap();
    match valset_decode_reply(&reply).unwrap() {
        ValsetReply::Validators(list) => list,
    }
}

async fn chat_messages<E>(chat: &Chat<E>, channel_id: &str) -> Vec<MessageView>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = chat
        .query(&chat_encode_query(&ChatQuery::MessagesLatest {
            channel_id: channel_id.into(),
            limit: 16,
        }))
        .await
        .unwrap();
    match chat_decode_reply(&reply).unwrap() {
        ChatReply::Messages(messages) => messages,
        other => panic!("unexpected chat reply: {other:?}"),
    }
}

fn tmp_repo(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("ducktape-joiner-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

#[test]
fn joiner_rebuilds_every_module_and_lands_on_the_source_app_hash() {
    let source_repo = tmp_repo("source");
    let joiner_repo = tmp_repo("joiner");
    let (source_dir, joiner_dir) = (source_repo.clone(), joiner_repo.clone());

    deterministic::Runner::default().start(|context| async move {
        // ---- SOURCE: real content through every module's own op path --------
        let mut src_kv = Kv::init(context.child("source_kv"), "kv").await;
        commit_op(
            &mut src_kv,
            0,
            kv_encode(&KvMsg::Set {
                key: b"greeting:name".to_vec(),
                value: b"hello world".to_vec(),
            }),
        )
        .await;
        commit_op(
            &mut src_kv,
            0,
            kv_encode(&KvMsg::Set {
                key: b"motd".to_vec(),
                value: b"draft".to_vec(),
            }),
        )
        .await;
        // overwrite: two committed ops on one key — op-log order matters.
        commit_op(
            &mut src_kv,
            0,
            kv_encode(&KvMsg::Set {
                key: b"motd".to_vec(),
                value: b"final".to_vec(),
            }),
        )
        .await;

        let mut src_document = Document::init(context.child("source_document"), "document").await;
        commit_op(
            &mut src_document,
            0,
            doc_encode_msg(&DocMsg::CreateDoc {
                doc_id: "readme".into(),
            }),
        )
        .await;
        commit_op(
            &mut src_document,
            0,
            doc_encode_msg(&DocMsg::InsertBlock {
                doc_id: "readme".into(),
                after: None,
                block: Block {
                    id: "title".into(),
                    kind: BlockKind::Heading,
                    text: "ducktape".into(),
                },
            }),
        )
        .await;
        commit_op(
            &mut src_document,
            0,
            doc_encode_msg(&DocMsg::InsertBlock {
                doc_id: "readme".into(),
                after: Some("title".into()),
                block: Block {
                    id: "intro".into(),
                    kind: BlockKind::Paragraph,
                    text: "a draft".into(),
                },
            }),
        )
        .await;
        // overwrite of the doc's qmdb key — op-log order matters here too.
        commit_op(
            &mut src_document,
            0,
            doc_encode_msg(&DocMsg::UpdateBlock {
                doc_id: "readme".into(),
                block_id: "intro".into(),
                text: "a block document, rebuilt by a joiner".into(),
            }),
        )
        .await;

        let mut src_directory = Directory::new("directory");
        commit_op(
            &mut src_directory,
            0,
            dir_encode_msg(&DirMsg::Set {
                key: "name".into(),
                value: "world".into(),
            }),
        )
        .await;

        // chat content exercises every record family the sync must carry:
        // channel + index, message heads, a thread reply index, an edit's
        // immutable revision record, and a reaction set. commit_op's TestCtx
        // dispatches with Origin::System, so the stored author is System and
        // the same origin may edit.
        let mut src_chat = Chat::init(context.child("source_chat"), "chat").await;
        commit_op(
            &mut src_chat,
            10,
            chat_encode_msg(&ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await;
        commit_op(
            &mut src_chat,
            20,
            chat_encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "m1".into(),
                blocks: vec![ChatBlock::paragraph("hello")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        commit_op(
            &mut src_chat,
            21,
            chat_encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "r1".into(),
                blocks: vec![ChatBlock::paragraph("threaded")],
                thread: Some(1),
                as_agent: None,
            }),
        )
        .await;
        commit_op(
            &mut src_chat,
            22,
            chat_encode_msg(&ChatMsg::EditMessage {
                channel_id: "general".into(),
                seq: 1,
                blocks: vec![ChatBlock::paragraph("hello, edited")],
                base_rev: Some(0),
            }),
        )
        .await;
        commit_op(
            &mut src_chat,
            23,
            chat_encode_msg(&ChatMsg::AddReaction {
                channel_id: "general".into(),
                seq: 2,
                emoji: "thumbsup".into(),
            }),
        )
        .await;

        let mut src_forge = Forge::init("forge", source_dir.clone()).expect("forge init");
        commit_op(
            &mut src_forge,
            100,
            forge_encode_msg(&ForgeMsg::Commit {
                repo: String::new(),
                path: "README.md".into(),
                content: "# ducktape\n".into(),
                message: "init".into(),
            }),
        )
        .await;
        // a second commit: the snapshot pack must carry real history, not one object.
        commit_op(
            &mut src_forge,
            200,
            forge_encode_msg(&ForgeMsg::Commit {
                repo: String::new(),
                path: "README.md".into(),
                content: "# ducktape\n\nrebuilt from a snapshot\n".into(),
                message: "expand".into(),
            }),
        )
        .await;

        let mut src_valset = Valset::new("valset");
        commit_op(
            &mut src_valset,
            0,
            valset_encode_msg(&ValsetMsg::Join {
                key: validator_key(7),
            }),
        )
        .await;
        commit_op(
            &mut src_valset,
            0,
            valset_encode_msg(&ValsetMsg::Join {
                key: validator_key(9),
            }),
        )
        .await;

        let mut src_saga = SagaModule::new("saga");
        commit_op(
            &mut src_saga,
            0,
            saga_encode_msg(&SagaMsg::Trigger {
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
        )
        .await;
        commit_op(
            &mut src_saga,
            0,
            saga_encode_msg(&SagaMsg::OracleResult {
                saga_id: "greet".into(),
                attempt: 0,
                outcome: Ok(b"olleh".to_vec()),
            }),
        )
        .await;
        // a second saga still in flight — Pending and Done must both survive the trip.
        commit_op(
            &mut src_saga,
            0,
            saga_encode_msg(&SagaMsg::Trigger {
                pinned_assignee: None,
                saga_id: "translate".into(),
                spec: b"hola".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 1,
                lease_views: None,
                capability: None,
            }),
        )
        .await;

        // the agent orchestrator: a registry entry per owner shape (one
        // paused) plus a channel watch — the run machinery is exercised by
        // the agent crate's own snapshot suite; this leg pins the joiner
        // path. admin ops are owner-gated, so they carry an external origin.
        let owner = sdk::Origin::External(b"agent-owner".to_vec());
        let mut src_agent = AgentModule::new(
            "agent",
            "chat",
            "saga",
            Some("tasks".into()),
            Some("jobs".into()),
        );
        commit_op_as(
            &mut src_agent,
            30,
            owner.clone(),
            agent_encode_msg(&AgentMsg::RegisterAgent {
                agent_id: "quackbot".into(),
                display_name: "Quackbot".into(),
                capability: "mock-llm-1".into(),
                prompt_hash: vec![7u8; 32],
                allowed_actions: vec![ACTION_CHAT_POST.into()],
            }),
        )
        .await;
        commit_op_as(
            &mut src_agent,
            31,
            owner.clone(),
            agent_encode_msg(&AgentMsg::RegisterAgent {
                agent_id: "sleepy".into(),
                display_name: "Sleepy".into(),
                capability: "mock-llm-1".into(),
                prompt_hash: vec![8u8; 32],
                allowed_actions: Vec::new(),
            }),
        )
        .await;
        commit_op_as(
            &mut src_agent,
            32,
            owner.clone(),
            agent_encode_msg(&AgentMsg::PauseAgent {
                agent_id: "sleepy".into(),
            }),
        )
        .await;
        commit_op_as(
            &mut src_agent,
            33,
            owner.clone(),
            agent_encode_msg(&AgentMsg::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::Mention,
            }),
        )
        .await;

        let src_greeter = Greeter::new("greeter");

        // ---- the source app-hash: what consensus commits to ------------------
        let src_kv_root = src_kv.root();
        let src_document_root = src_document.root();
        let src_directory_root = src_directory.root();
        let src_forge_root = src_forge.root();
        let src_chat_root = src_chat.root();
        let src_valset_root = src_valset.root();
        let src_saga_root = src_saga.root();
        let src_agent_root = src_agent.root();
        for (id, root) in [
            ("kv", src_kv_root),
            ("document", src_document_root),
            ("directory", src_directory_root),
            ("forge", src_forge_root),
            ("chat", src_chat_root),
            ("valset", src_valset_root),
            ("saga", src_saga_root),
            ("agent", src_agent_root),
        ] {
            assert_ne!(
                root,
                StateRoot::ZERO,
                "{id}: the source must hold real state"
            );
        }

        let src_global = {
            let mods: [&dyn Module; 9] = [
                &src_kv,
                &src_directory,
                &src_greeter,
                &src_forge,
                &src_document,
                &src_chat,
                &src_valset,
                &src_saga,
                &src_agent,
            ];
            global_root(&mods)
        };

        // the per-module surfaces the joiner pulls: snapshot bytes for the
        // in-memory + git modules, sync targets + resolvers for the qmdb stores
        // (into_resolver consumes the source store — capture everything first).
        // directory snapshot bytes and the kv qmdb target cross an explicit
        // request/response boundary keyed by a validator-set mesh participant;
        // the remaining sources stay on the older in-process handoff in this
        // slice.
        let valset_bytes = src_valset.snapshot();
        let saga_bytes = src_saga.snapshot();
        let agent_bytes = src_agent.snapshot();
        let forge_bytes = src_forge.snapshot().expect("forge snapshot");
        let src_validators = validators(&src_valset).await;
        let src_chat_messages = chat_messages(&src_chat, "general").await;

        let source_peer = StateSyncPeerId::ed25519_public_key(src_validators[0].clone())
            .expect("source validator key is a peer id");
        let joiner_peer = StateSyncPeerId::ed25519_public_key(validator_key(11))
            .expect("joiner key is a peer id");
        let mut state_sync = LoopbackStateSyncResolver::default();
        state_sync.insert_participant(MeshParticipant::validator_set_participant(
            source_peer.clone(),
        ));
        state_sync
            .serve_module(
                &source_peer,
                "directory",
                src_directory_root,
                StateSyncPayload::Snapshot(src_directory.snapshot()),
            )
            .expect("serve directory snapshot");

        let raw_kv_target = src_kv.sync_target().await;
        state_sync
            .serve_module(
                &source_peer,
                "kv",
                src_kv_root,
                StateSyncPayload::QmdbTarget(encode_qmdb_target(&raw_kv_target)),
            )
            .expect("serve kv target");
        let directory_request = StateSyncRequest::new(
            joiner_peer.clone(),
            source_peer.clone(),
            "directory",
            src_directory_root,
            StateSyncKind::Snapshot,
        );
        let directory_response = decode_response(
            &state_sync
                .resolve_bytes(&encode_request(&directory_request))
                .expect("directory state-sync response"),
        )
        .expect("decode directory state-sync response");
        assert!(directory_response.source.has_role(MeshRole::Bootnode));
        assert!(directory_response.source.has_role(MeshRole::Relayer));
        let directory_bytes = directory_response
            .payload
            .into_snapshot_bytes()
            .expect("directory snapshot payload");

        let kv_request = StateSyncRequest::new(
            joiner_peer,
            source_peer,
            "kv",
            src_kv_root,
            StateSyncKind::QmdbTarget,
        );
        let kv_response = decode_response(
            &state_sync
                .resolve_bytes(&encode_request(&kv_request))
                .expect("kv state-sync response"),
        )
        .expect("decode kv state-sync response");
        let kv_target = decode_qmdb_target(
            &kv_response
                .payload
                .into_qmdb_target_bytes()
                .expect("kv target payload"),
        )
        .expect("decode kv target");
        let kv_resolver = src_kv.into_resolver();
        let document_target = src_document.sync_target().await;
        let document_resolver = src_document.into_resolver();
        let chat_target = src_chat.sync_target().await;
        let chat_resolver = src_chat.into_resolver();

        // ---- JOINER: reconstruct every stateful module -----------------------
        let join_kv = Kv::sync_from(
            context.child("joiner_kv"),
            "kv-rebuilt",
            kv_target,
            kv_resolver,
        )
        .await;
        let join_document = Document::sync_from(
            context.child("joiner_document"),
            "document-rebuilt",
            document_target,
            document_resolver,
        )
        .await;
        let join_chat = Chat::sync_from(
            context.child("joiner_chat"),
            "chat-rebuilt",
            chat_target,
            chat_resolver,
        )
        .await;

        let mut join_directory = Directory::new("directory");
        join_directory
            .install(&directory_bytes, src_directory_root)
            .expect("directory install");
        let mut join_valset = Valset::new("valset");
        join_valset
            .install(&valset_bytes, src_valset_root)
            .expect("valset install");
        let mut join_saga = SagaModule::new("saga");
        join_saga
            .install(&saga_bytes, src_saga_root)
            .expect("saga install");
        let mut join_agent = AgentModule::new(
            "agent",
            "chat",
            "saga",
            Some("tasks".into()),
            Some("jobs".into()),
        );
        join_agent
            .install(&agent_bytes, src_agent_root)
            .expect("agent install");
        let mut join_forge = Forge::init("forge", joiner_dir.clone()).expect("joiner forge init");
        join_forge
            .install(&forge_bytes, src_forge_root)
            .expect("forge install");
        let join_greeter = Greeter::new("greeter");

        // every reconstructed root equals its source root...
        assert_eq!(
            join_kv.root(),
            src_kv_root,
            "kv: synced root != source root"
        );
        assert_eq!(
            join_document.root(),
            src_document_root,
            "document: synced root != source root"
        );
        assert_eq!(
            join_chat.root(),
            src_chat_root,
            "chat: synced root != source root"
        );
        assert_eq!(
            join_directory.root(),
            src_directory_root,
            "directory: installed root != source root"
        );
        assert_eq!(
            join_valset.root(),
            src_valset_root,
            "valset: installed root != source root"
        );
        assert_eq!(
            join_saga.root(),
            src_saga_root,
            "saga: installed root != source root"
        );
        assert_eq!(
            join_forge.root(),
            src_forge_root,
            "forge: installed root != source root"
        );
        assert_eq!(
            join_agent.root(),
            src_agent_root,
            "agent: installed root != source root"
        );

        // ...and the composed app-hash over the same canonical ids is the exact
        // digest consensus committed on the source — THE joiner property.
        assert_eq!(
            joiner_app_hash(
                &join_kv,
                &join_document,
                &join_chat,
                &join_directory,
                &join_greeter,
                &join_forge,
                &join_valset,
                &join_saga,
                &join_agent,
            ),
            src_global,
            "the joiner must land on the exact source app-hash"
        );

        // ---- the state is real: a content read per reconstructed module ------
        assert_eq!(
            join_kv.get(b"motd").await.as_deref(),
            Some(b"final".as_ref())
        );
        assert_eq!(
            join_kv.get(b"greeting:name").await.as_deref(),
            Some(b"hello world".as_ref())
        );

        let reply = join_document
            .query(&doc_encode_query(&DocQuery::GetDoc {
                doc_id: "readme".into(),
            }))
            .await
            .unwrap();
        let DocReply::Doc(Some(blocks)) = doc_decode_reply(&reply).unwrap() else {
            panic!("readme must exist on the joiner");
        };
        let ids: Vec<&str> = blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["title", "intro"]);
        assert_eq!(blocks[1].text, "a block document, rebuilt by a joiner");

        assert_eq!(
            chat_messages(&join_chat, "general").await,
            src_chat_messages
        );

        assert_eq!(join_directory.get("name"), Some(&"world".to_string()));

        assert_eq!(src_validators.len(), 2);
        assert_eq!(validators(&join_valset).await, src_validators);

        let reply = join_saga
            .query(&saga_encode_query(&SagaQuery::Get {
                saga_id: "greet".into(),
            }))
            .await
            .unwrap();
        let SagaReply::Saga(Some(greet)) = saga_decode_reply(&reply).unwrap() else {
            panic!("the greet saga must exist on the joiner");
        };
        assert_eq!(greet.status, SagaStatus::Done);
        assert_eq!(greet.result, Some(b"olleh".to_vec()));
        let reply = join_saga
            .query(&saga_encode_query(&SagaQuery::Get {
                saga_id: "translate".into(),
            }))
            .await
            .unwrap();
        let SagaReply::Saga(Some(translate)) = saga_decode_reply(&reply).unwrap() else {
            panic!("the translate saga must exist on the joiner");
        };
        assert_eq!(translate.status, SagaStatus::Pending);
        assert_eq!(translate.result, None);

        // agent: the registry (statuses included) and the watch survived.
        let reply = join_agent
            .query(&agent_encode_query(&AgentQuery::Agents))
            .await
            .unwrap();
        let AgentReply::Agents(agents) = agent_decode_reply(&reply).unwrap() else {
            panic!("agents reply expected");
        };
        assert_eq!(
            agents
                .iter()
                .map(|a| (a.agent_id.as_str(), a.status))
                .collect::<Vec<_>>(),
            vec![
                ("quackbot", AgentStatus::Active),
                ("sleepy", AgentStatus::Paused),
            ]
        );
        let reply = join_agent
            .query(&agent_encode_query(&AgentQuery::Watches))
            .await
            .unwrap();
        let AgentReply::Watches(watches) = agent_decode_reply(&reply).unwrap() else {
            panic!("watches reply expected");
        };
        assert_eq!(watches.len(), 1);
        assert_eq!(watches[0].channel_id, "general");
        assert_eq!(watches[0].policy, TurnPolicy::Mention);

        // forge: read the committed FILE back out of the joiner's OWN repo —
        // proof the pack landed real objects (commit, tree, blob), not just a
        // head oid that rehashes to the right root.
        {
            // forge now namespaces repos under base/<name>; the default repo
            // (an empty `repo` field) lives at joiner_dir/default.
            let repo =
                git2::Repository::open(joiner_dir.join("default")).expect("joiner repo opens");
            let head = repo
                .refname_to_id("refs/heads/main")
                .expect("joiner ref is born");
            let commit = repo.find_commit(head).unwrap();
            assert_eq!(
                commit.parent_count(),
                1,
                "the pack must carry the full history, not just the head commit"
            );
            let tree = commit.tree().unwrap();
            let entry = tree
                .get_name("README.md")
                .expect("README.md in the head tree");
            let blob = repo.find_blob(entry.id()).unwrap();
            assert_eq!(
                blob.content(),
                b"# ducktape\n\nrebuilt from a snapshot\n".as_ref()
            );
        }

        // ---- a byzantine snapshot is refused, and refusal leaves no trace ----
        // flip one bit inside the last validator key: count, lengths, and sort
        // order all still decode, so only the recomputed-root gate catches it.
        let mut tampered = valset_bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        let err = join_valset.install(&tampered, src_valset_root).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "a tampered snapshot must err with Module"
        );
        assert_eq!(
            join_valset.root(),
            src_valset_root,
            "a refused install must not move the root"
        );
        assert_eq!(
            validators(&join_valset).await,
            src_validators,
            "a refused install must not change membership"
        );

        // the joiner's composed app-hash still stands after the refusal.
        assert_eq!(
            joiner_app_hash(
                &join_kv,
                &join_document,
                &join_chat,
                &join_directory,
                &join_greeter,
                &join_forge,
                &join_valset,
                &join_saga,
                &join_agent,
            ),
            src_global
        );
    });

    let _ = std::fs::remove_dir_all(&source_repo);
    let _ = std::fs::remove_dir_all(&joiner_repo);
}
