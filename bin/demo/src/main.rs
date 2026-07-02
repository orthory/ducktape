//! a runnable super-app demo: seven registered modules — a qmdb-backed kv, a sync
//! in-memory directory, a stateless greeter, a GIT-backed forge, a qmdb-backed
//! block DOCUMENT module, a queryable agent-session module backed by messaging
//! storage, and an ed25519 permissionless VALSET — dispatched over ONE host,
//! showing the app-hash evolve as typed cross-module ops flow.
//!
//! run: `cargo run -p demo`

use agent::Agent;
use agent_interface::{
    AgentMsg, AgentQuery, AgentReply, decode_reply as agent_decode_reply,
    encode_msg as agent_encode_msg, encode_query as agent_encode_query,
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
use host::Host;
use sdk::Msg;
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
        let agent =
            Agent::init_with_messaging_id(context.child("agent"), "agent", "agent-messaging").await;
        let valset = Valset::new("valset");
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(directory),
            Box::new(greeter),
            Box::new(forge),
            Box::new(document),
            Box::new(agent),
            Box::new(valset),
        ])
        .expect("genesis");

        println!("=== super-app demo — 7 registered modules over one host ===");
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
            "genesis agent root (no sessions)    : {:?}",
            host.module_root("agent").unwrap()
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

        // block 4: open an agent session. agent is a queryable module with a
        // real messaging-backed storage root.
        let out = host
            .submit(Msg {
                target: "agent".into(),
                payload: agent_encode_msg(&AgentMsg::OpenSession {
                    session_id: "general".into(),
                    title: "General".into(),
                }),
            })
            .await
            .expect("submit block 4");
        println!("\n[block 4] agent <- OpenSession(general)");
        println!(
            "  agent root     : {:?}",
            host.module_root("agent").unwrap()
        );
        println!("  app-hash       : {:?}", out.app_hash);

        let out = host
            .submit(Msg {
                target: "agent".into(),
                payload: agent_encode_msg(&AgentMsg::AppendTurn {
                    session_id: "general".into(),
                    user_message_id: "u1".into(),
                    assistant_message_id: "a1".into(),
                    user: "demo-user".into(),
                    assistant: "demo-agent".into(),
                    user_body: "what changed?".into(),
                    assistant_body: "agent is now queryable and root-backed".into(),
                }),
            })
            .await
            .expect("submit block 5");
        let reply = host
            .query(
                "agent",
                &agent_encode_query(&AgentQuery::Messages {
                    session_id: "general".into(),
                }),
            )
            .await
            .expect("query agent");
        if let AgentReply::Messages(entries) = agent_decode_reply(&reply).unwrap() {
            println!("\n[block 5] agent <- AppendTurn(general, u1, a1)");
            println!("  entry count    : {}", entries.len());
            println!(
                "  agent root     : {:?}",
                host.module_root("agent").unwrap()
            );
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
        // membership ops are governance-gated (external origins are refused);
        // the demo's direct join rides a SYSTEM-origin block — the same trusted
        // orchestration lane genesis seeding uses.
        let out = host
            .submit_at(
                host::BlockContext {
                    height: 0,
                    consensus_time: 0,
                    origin: sdk::Origin::System,
                },
                Msg {
                    target: "valset".into(),
                    payload: valset_encode_msg(&ValsetMsg::Join {
                        key: new_validator.clone(),
                    }),
                },
            )
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

        println!("\nmodule roots:");
        for id in [
            "directory",
            "document",
            "forge",
            "greeter",
            "agent",
            "kv",
            "valset",
        ] {
            println!("  {id:>10} : {:?}", host.module_root(id).unwrap());
        }
        println!("\nfinal app-hash   : {:?}", host.app_hash());
    });
}
