//! a runnable super-app demo: ten registered modules — a qmdb-backed kv, a sync
//! in-memory directory, a stateless greeter, a GIT-backed forge, a qmdb-backed
//! block DOCUMENT module, a qmdb-backed block-based CHAT module, an ed25519
//! permissionless VALSET, the SAGA async-RPC ledger, the AGENT orchestrator,
//! and a TASKS ledger — dispatched over ONE host, showing the app-hash evolve
//! as typed cross-module ops flow, ending on the agent-collaboration beat: a
//! mention becomes a run and a pending saga in one block.
//!
//! run: `cargo run -p demo`

use agent::AgentModule;
use agent_interface::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentMsg, AgentQuery, AgentReply, TurnPolicy,
    decode_reply as agent_decode_reply, encode_msg as agent_encode_msg,
    encode_query as agent_encode_query,
};
use chat::Chat;
use chat_interface::{
    Block as ChatBlock, ChatMsg, ChatQuery, ChatReply, PostPolicy,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use directory::Directory;
use directory_interface::{DirMsg, DirQuery, decode_reply, encode_msg, encode_query};
use document::Document;
use document_interface::{
    Block, BlockKind, DocMsg, DocQuery, DocReply, decode_reply as doc_decode_reply,
    encode_msg as doc_encode_msg, encode_query as doc_encode_query,
};
use forge::Forge;
use forge_interface::{
    ForgeMsg, ForgeQuery, ForgeReply, decode_reply as forge_decode_reply,
    encode_msg as forge_encode_msg, encode_query as forge_encode_query,
};
use greeter::Greeter;
use host::{BlockContext, Host};
use saga::SagaModule;
use saga_interface::{
    SagaQuery, SagaReply, decode_reply as saga_decode_reply, encode_query as saga_encode_query,
};
use sdk::{Msg, Origin};
use tasks::Tasks;
use valset::Valset;
use valset_interface::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_msg as valset_encode_msg, encode_query as valset_encode_query,
};

fn main() {
    // forge's substrate is a real git repo on disk. wipe any prior run's dir so
    // genesis starts from an unborn repo (root == ZERO) and output is reproducible.
    let forge_repo = std::env::temp_dir().join("ducktape-forge-demo");
    let _ = std::fs::remove_dir_all(&forge_repo);

    deterministic::Runner::default().start(|context| async move {
        // genesis: the module registry (would be consensus state on a real chain).
        let document = Document::init(context.child("document"), "document").await;
        let kv = kv::Kv::init(context.child("kv"), "kv").await;
        let directory = Directory::new("directory");
        let greeter = Greeter::new("greeter");
        let forge = Forge::init("forge", forge_repo.clone()).expect("forge init");
        let chat = Chat::init(context.child("chat"), "chat").await;
        let valset = Valset::new("valset");
        let saga = SagaModule::new("saga");
        let tasks = Tasks::new("tasks");
        let agent = AgentModule::new("agent", "chat", "saga", Some("tasks".into()));
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(directory),
            Box::new(greeter),
            Box::new(forge),
            Box::new(document),
            Box::new(chat),
            Box::new(valset),
            Box::new(saga),
            Box::new(tasks),
            Box::new(agent),
        ])
        .expect("genesis");

        println!("=== super-app demo — 10 registered modules over one host ===");
        println!("forge repo       : {}", forge_repo.display());
        println!("genesis app-hash : {:?}", host.app_hash());
        println!(
            "genesis forge root (unborn git repo): {:?}",
            host.module_root("forge").unwrap()
        );
        println!(
            "genesis valset root (empty set)     : {:?}",
            host.module_root("valset").unwrap()
        );
        println!(
            "genesis document root (no docs)     : {:?}",
            host.module_root("document").unwrap()
        );
        println!(
            "genesis chat root (no channels)     : {:?}",
            host.module_root("chat").unwrap()
        );

        // block 1: a typed Set to the in-memory directory module.
        let out = host
            .submit(Msg {
                target: "directory".into(),
                payload: encode_msg(&DirMsg::Set {
                    key: "name".into(),
                    value: "world".into(),
                }),
            })
            .await
            .expect("submit block 1");
        println!("\n[block 1] directory <- Set(name = world)");
        println!("  app-hash       : {:?}", out.app_hash);

        // block 2: trigger greeter. it QUERIES directory (typed, cross-module),
        // then emits typed follow-up writes to directory + kv — all in one block.
        let out = host
            .submit(Msg {
                target: "greeter".into(),
                payload: b"name".to_vec(),
            })
            .await
            .expect("submit block 2");
        println!("\n[block 2] greeter(name): query directory -> write greeting to directory + kv");
        println!("  app-hash       : {:?}", out.app_hash);

        // block 3: a typed Commit to the GIT-backed forge module. this writes a
        // file + makes a real (deterministic) git commit; forge's root moves to
        // the new HEAD oid, which folds straight into the app-hash.
        let out = host
            .submit(Msg {
                target: "forge".into(),
                payload: forge_encode_msg(&ForgeMsg::Commit {
                    path: "README.md".into(),
                    content: "# hello from a git-backed module\n".into(),
                    message: "forge: initial commit".into(),
                }),
            })
            .await
            .expect("submit block 3");
        println!("\n[block 3] forge <- Commit(README.md) — a real git commit");
        println!("  app-hash       : {:?}", out.app_hash);
        println!(
            "  forge root     : {:?}",
            host.module_root("forge").unwrap()
        );

        // read forge's HEAD back out (typed query) — the sha1 git oid hex. this hex is
        // the sha256 PREIMAGE of the forge root: a git commit addressing the app-hash.
        let reply = host
            .query("forge", &forge_encode_query(&ForgeQuery::Head))
            .await
            .expect("query forge");
        if let ForgeReply::Head(Some(oid)) = forge_decode_reply(&reply).unwrap() {
            println!("  forge git HEAD : {oid}");
            println!("  (^ the 40-char sha1 oid is the sha256 preimage of the forge root above)");
        }

        // read the derived greeting back out of the directory (sync typed query).
        let reply = host
            .query(
                "directory",
                &encode_query(&DirQuery::Get {
                    key: "greeting:name".into(),
                }),
            )
            .await
            .expect("query directory");
        println!(
            "\ndirectory[greeting:name] = {:?}",
            decode_reply(&reply).unwrap()
        );

        // and read it back out of the QMDB kv module — a real async cross-module read.
        let kv_reply = host
            .query(
                "kv",
                &kv_interface::encode_query(&kv_interface::KvQuery::Get {
                    key: b"greeting:name".to_vec(),
                }),
            )
            .await
            .expect("query kv");
        if let kv_interface::KvReply::Value(Some(v)) =
            kv_interface::decode_reply(&kv_reply).unwrap()
        {
            println!(
                "kv[greeting:name]        = {:?}",
                String::from_utf8_lossy(&v)
            );
        }

        // block 4: create a chat channel. chat derives authorship from the
        // dispatch origin (no author field in any payload), so the demo submits
        // with an explicit external origin — a seed-derived ed25519 pubkey
        // standing in for a real submitter id. the default empty external
        // origin would be rejected.
        let demo_user = PrivateKey::decode(&[5u8; 32][..])
            .expect("32-byte seed is a valid ed25519 private key")
            .public_key()
            .as_ref()
            .to_vec();
        let as_demo_user = || BlockContext {
            height: 0,
            consensus_time: 0,
            origin: Origin::External(demo_user.clone()),
        };
        let out = host
            .submit_at(
                as_demo_user(),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::CreateChannel {
                        channel_id: "general".into(),
                        name: "General".into(),
                        post_policy: PostPolicy::Open,
                    }),
                },
            )
            .await
            .expect("submit block 4");
        println!("\n[block 4] chat <- CreateChannel(general) — authorship from origin");
        println!("  chat root      : {:?}", host.module_root("chat").unwrap());
        println!("  app-hash       : {:?}", out.app_hash);

        // block 5: a root message, a thread reply, and a reaction — three ops,
        // three blocks; sequences come from the channel's head_seq counter.
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    blocks: vec![ChatBlock::paragraph("what changed?")],
                    thread: None,
                    as_agent: None,
                }),
            },
        )
        .await
        .expect("submit block 5 post");
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m2".into(),
                    blocks: vec![ChatBlock::paragraph(
                        "chat is block-based and origin-authored now",
                    )],
                    thread: Some(1),
                    as_agent: None,
                }),
            },
        )
        .await
        .expect("submit block 5 reply");
        let out = host
            .submit_at(
                as_demo_user(),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::AddReaction {
                        channel_id: "general".into(),
                        seq: 2,
                        emoji: "🦆".into(),
                    }),
                },
            )
            .await
            .expect("submit block 5 reaction");
        let reply = host
            .query(
                "chat",
                &chat_encode_query(&ChatQuery::MessagesLatest {
                    channel_id: "general".into(),
                    limit: 16,
                }),
            )
            .await
            .expect("query chat");
        if let ChatReply::Messages(messages) = chat_decode_reply(&reply).unwrap() {
            println!("\n[block 5] chat <- PostMessage(m1) + thread reply(m2) + AddReaction");
            println!("  message count  : {}", messages.len());
            println!(
                "  m1 replies     : {} (last reply seq: {:?})",
                messages[0].head.reply_count, messages[0].head.last_reply_seq
            );
            println!(
                "  m2 reactions   : {:?}",
                messages[1]
                    .reactions
                    .iter()
                    .map(|r| (r.emoji.as_str(), r.reactors.len()))
                    .collect::<Vec<_>>()
            );
            println!("  chat root      : {:?}", host.module_root("chat").unwrap());
            println!("  app-hash       : {:?}", out.app_hash);
        }

        // block 6: a NEW validator JOINs the permissionless ed25519 valset. derive
        // the key deterministically from a fixed seed (any 32 bytes is a valid
        // ed25519 seed) so the demo is reproducible. the valset root moves off
        // ZERO and folds another module's commitment into the app-hash.
        let seed = [7u8; 32];
        let new_validator = PrivateKey::decode(&seed[..])
            .expect("32-byte seed is a valid ed25519 private key")
            .public_key()
            .as_ref()
            .to_vec();
        let out = host
            .submit(Msg {
                target: "valset".into(),
                payload: valset_encode_msg(&ValsetMsg::Join {
                    key: new_validator.clone(),
                }),
            })
            .await
            .expect("submit block 6");
        println!("\n[block 6] valset <- Join(ed25519 pubkey) — a new validator joins");
        let reply = host
            .query("valset", &valset_encode_query(&ValsetQuery::Validators))
            .await
            .expect("query valset");
        let ValsetReply::Validators(vs) = valset_decode_reply(&reply).unwrap();
        println!("  validator count: {} (was 0 at genesis)", vs.len());
        println!(
            "  valset root    : {:?}",
            host.module_root("valset").unwrap()
        );
        println!("  app-hash       : {:?}", out.app_hash);

        // block 7: the DOCUMENT module (ducktape's founding product, reborn as a
        // simple block-based store on qmdb). create a doc, insert two blocks, then
        // update one. each op is its own block (documents emit no follow-ups), and
        // the document's qmdb merkle root moves into the app-hash on every commit.
        println!("\n[block 7] document <- CreateDoc + 2x InsertBlock + UpdateBlock");
        host.submit(Msg {
            target: "document".into(),
            payload: doc_encode_msg(&DocMsg::CreateDoc {
                doc_id: "readme".into(),
            }),
        })
        .await
        .expect("doc create");
        host.submit(Msg {
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
        })
        .await
        .expect("doc insert 1");
        host.submit(Msg {
            target: "document".into(),
            payload: doc_encode_msg(&DocMsg::InsertBlock {
                doc_id: "readme".into(),
                after: Some("title".into()),
                block: Block {
                    id: "intro".into(),
                    kind: BlockKind::Paragraph,
                    text: "a block document".into(),
                },
            }),
        })
        .await
        .expect("doc insert 2");
        let out = host
            .submit(Msg {
                target: "document".into(),
                payload: doc_encode_msg(&DocMsg::UpdateBlock {
                    doc_id: "readme".into(),
                    block_id: "intro".into(),
                    text: "a simple, block-based document on qmdb".into(),
                }),
            })
            .await
            .expect("doc update");
        println!("  app-hash       : {:?}", out.app_hash);
        println!(
            "  document root  : {:?}",
            host.module_root("document").unwrap()
        );

        // read the whole doc back out (typed query) — the ordered blocks.
        let reply = host
            .query(
                "document",
                &doc_encode_query(&DocQuery::GetDoc {
                    doc_id: "readme".into(),
                }),
            )
            .await
            .expect("query document");
        if let DocReply::Doc(Some(blocks)) = doc_decode_reply(&reply).unwrap() {
            println!("  readme blocks  :");
            for b in &blocks {
                println!("    - [{:?}] {} = {:?}", b.kind, b.id, b.text);
            }
        }

        // block 8: the agent-collaboration loop (design §3). register an agent
        // (which model+prompt it runs is committed into the app-hash), watch
        // the chat channel under a Mention policy — the watch and chat's hook
        // registration commit atomically — then post a message MENTIONING the
        // agent: the very same block carries the post, the hook delivery, the
        // run record, and the saga trigger. the emitted WorkerRequest effect
        // is the off-consensus LLM seam a reactor driver answers as an
        // ordinary oracle op in some later block.
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "agent".into(),
                payload: agent_encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: "quackbot".into(),
                    display_name: "Quackbot".into(),
                    model_ref: "mock-llm-1".into(),
                    prompt_hash: vec![7u8; 32],
                    allowed_actions: vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()],
                }),
            },
        )
        .await
        .expect("submit block 8 register");
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "agent".into(),
                payload: agent_encode_msg(&AgentMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Mention,
                }),
            },
        )
        .await
        .expect("submit block 8 watch");
        let out = host
            .submit_at(
                as_demo_user(),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::PostMessage {
                        channel_id: "general".into(),
                        message_id: "m3".into(),
                        blocks: vec![ChatBlock::Paragraph(vec![
                            chat_interface::Span::plain("hey "),
                            chat_interface::Span {
                                text: "@quackbot".into(),
                                marks: vec![chat_interface::Mark::Mention(
                                    chat_interface::AuthorRef::Agent {
                                        module: "agent".into(),
                                        agent_id: "quackbot".into(),
                                    },
                                )],
                            },
                            chat_interface::Span::plain(" can you follow up?"),
                        ])],
                        thread: None,
                        as_agent: None,
                    }),
                },
            )
            .await
            .expect("submit block 8 mention");
        println!("\n[block 8] agent <- Register + Watch(Mention); chat <- PostMessage(@quackbot)");
        println!(
            "  effects        : {} WorkerRequest (the off-consensus LLM seam)",
            out.effects.len()
        );
        let reply = host
            .query(
                "agent",
                &agent_encode_query(&AgentQuery::Run {
                    run_id: "general/3/quackbot".into(),
                }),
            )
            .await
            .expect("query agent run");
        if let AgentReply::Run(Some(run)) = agent_decode_reply(&reply).unwrap() {
            println!(
                "  run            : {} {:?} (context pinned to seq {})",
                run.run_id, run.status, run.anchor_seq
            );
        }
        let reply = host
            .query(
                "saga",
                &saga_encode_query(&SagaQuery::Get {
                    saga_id: "agent/general/3/quackbot".into(),
                }),
            )
            .await
            .expect("query saga");
        let SagaReply::Saga(Some(saga_view)) = saga_decode_reply(&reply).unwrap() else {
            panic!("the run's saga must exist");
        };
        println!(
            "  saga           : agent/general/3/quackbot {:?} (deadline view {:?})",
            saga_view.status, saga_view.deadline
        );
        println!("  (post + hook + run + trigger: ONE block — the P2 atomic cascade)");
        println!("  app-hash       : {:?}", out.app_hash);

        println!("\nmodule roots:");
        for id in [
            "agent",
            "chat",
            "directory",
            "document",
            "forge",
            "greeter",
            "kv",
            "saga",
            "tasks",
            "valset",
        ] {
            println!("  {id:>10} : {:?}", host.module_root(id).unwrap());
        }
        println!("\nfinal app-hash   : {:?}", host.app_hash());
    });
}
