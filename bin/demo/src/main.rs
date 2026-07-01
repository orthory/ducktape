//! a runnable super-app demo: six isolated modules — a qmdb-backed kv, a sync
//! in-memory directory, a stateless greeter, a GIT-backed forge, a qmdb-backed
//! block DOCUMENT module, and an ed25519 permissionless VALSET — dispatched over
//! ONE host, showing the app-hash evolve as typed cross-module ops flow, forge's
//! git HEAD oid compose into that same app-hash as its root, a document's qmdb
//! merkle root do the same, and a new validator JOIN move the valset root off ZERO.
//!
//! run: `cargo run -p demo`

use commonware_runtime::{deterministic, Runner as _, Supervisor as _};
use directory::Directory;
use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery};
use forge::Forge;
use forge_interface::{decode_reply as forge_decode_reply, encode_msg as forge_encode_msg,
    encode_query as forge_encode_query, ForgeMsg, ForgeQuery, ForgeReply};
use document::Document;
use document_interface::{decode_reply as doc_decode_reply, encode_msg as doc_encode_msg,
    encode_query as doc_encode_query, Block, BlockKind, DocMsg, DocQuery, DocReply};
use greeter::Greeter;
use host::Host;
use sdk::Msg;
use valset::Valset;
use valset_interface::{decode_reply as valset_decode_reply, encode_msg as valset_encode_msg,
    encode_query as valset_encode_query, ValsetMsg, ValsetQuery, ValsetReply};
use commonware_cryptography::{ed25519::PrivateKey, Signer as _};
use commonware_codec::DecodeExt as _;

fn main() {
    // forge's substrate is a real git repo on disk. wipe any prior run's dir so
    // genesis starts from an unborn repo (root == ZERO) and output is reproducible.
    let forge_repo = std::env::temp_dir().join("ducktape-forge-demo");
    let _ = std::fs::remove_dir_all(&forge_repo);

    deterministic::Runner::default().start(|context| async move {
        // genesis: the module registry (would be consensus state on a real chain).
        let document = Document::init(context.child("document"), "document").await;
        let kv = kv::Kv::init(context, "kv").await;
        let directory = Directory::new("directory");
        let greeter = Greeter::new("greeter");
        let forge = Forge::init("forge", forge_repo.clone()).expect("forge init");
        let valset = Valset::new("valset");
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(directory),
            Box::new(greeter),
            Box::new(forge),
            Box::new(document),
            Box::new(valset),
        ])
        .expect("genesis");

        println!("=== super-app demo — 6 isolated modules over one host ===");
        println!("forge repo       : {}", forge_repo.display());
        println!("genesis app-hash : {:?}", host.app_hash());
        println!("genesis forge root (unborn git repo): {:?}", host.module_root("forge").unwrap());
        println!("genesis valset root (empty set)     : {:?}", host.module_root("valset").unwrap());
        println!("genesis document root (no docs)     : {:?}", host.module_root("document").unwrap());

        // block 1: a typed Set to the in-memory directory module.
        let out = host
            .submit(Msg {
                target: "directory".into(),
                payload: encode_msg(&DirMsg::Set { key: "name".into(), value: "world".into() }),
            })
            .await
            .expect("submit block 1");
        println!("\n[block 1] directory <- Set(name = world)");
        println!("  app-hash       : {:?}", out.app_hash);

        // block 2: trigger greeter. it QUERIES directory (typed, cross-module),
        // then emits typed follow-up writes to directory + kv — all in one block.
        let out = host
            .submit(Msg { target: "greeter".into(), payload: b"name".to_vec() })
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
        println!("  forge root     : {:?}", host.module_root("forge").unwrap());

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
            .query("directory", &encode_query(&DirQuery::Get { key: "greeting:name".into() }))
            .await
            .expect("query directory");
        println!("\ndirectory[greeting:name] = {:?}", decode_reply(&reply).unwrap());

        // and read it back out of the QMDB kv module — a real async cross-module read.
        let kv_reply = host
            .query("kv", &kv_interface::encode_query(&kv_interface::KvQuery::Get { key: b"greeting:name".to_vec() }))
            .await
            .expect("query kv");
        if let kv_interface::KvReply::Value(Some(v)) = kv_interface::decode_reply(&kv_reply).unwrap() {
            println!("kv[greeting:name]        = {:?}", String::from_utf8_lossy(&v));
        }

        // block 4: a NEW validator JOINs the permissionless ed25519 valset. derive
        // the key deterministically from a fixed seed (any 32 bytes is a valid
        // ed25519 seed) so the demo is reproducible. the valset root moves off
        // ZERO and folds a 5th module's commitment into the app-hash.
        let seed = [7u8; 32];
        let new_validator = PrivateKey::decode(&seed[..])
            .expect("32-byte seed is a valid ed25519 private key")
            .public_key()
            .as_ref()
            .to_vec();
        let out = host
            .submit(Msg {
                target: "valset".into(),
                payload: valset_encode_msg(&ValsetMsg::Join { key: new_validator.clone() }),
            })
            .await
            .expect("submit block 4");
        println!("\n[block 4] valset <- Join(ed25519 pubkey) — a new validator joins");
        let reply = host
            .query("valset", &valset_encode_query(&ValsetQuery::Validators))
            .await
            .expect("query valset");
        let ValsetReply::Validators(vs) = valset_decode_reply(&reply).unwrap();
        println!("  validator count: {} (was 0 at genesis)", vs.len());
        println!("  valset root    : {:?}", host.module_root("valset").unwrap());
        println!("  app-hash       : {:?}", out.app_hash);

        // block 5: the DOCUMENT module (ducktape's founding product, reborn as a
        // simple block-based store on qmdb). create a doc, insert two blocks, then
        // update one. each op is its own block (documents emit no follow-ups), and
        // the document's qmdb merkle root moves into the app-hash on every commit.
        println!("\n[block 5] document <- CreateDoc + 2x InsertBlock + UpdateBlock");
        host.submit(Msg {
            target: "document".into(),
            payload: doc_encode_msg(&DocMsg::CreateDoc { doc_id: "readme".into() }),
        }).await.expect("doc create");
        host.submit(Msg {
            target: "document".into(),
            payload: doc_encode_msg(&DocMsg::InsertBlock {
                doc_id: "readme".into(),
                after: None,
                block: Block { id: "title".into(), kind: BlockKind::Heading, text: "ducktape".into() },
            }),
        }).await.expect("doc insert 1");
        host.submit(Msg {
            target: "document".into(),
            payload: doc_encode_msg(&DocMsg::InsertBlock {
                doc_id: "readme".into(),
                after: Some("title".into()),
                block: Block { id: "intro".into(), kind: BlockKind::Paragraph, text: "a block document".into() },
            }),
        }).await.expect("doc insert 2");
        let out = host.submit(Msg {
            target: "document".into(),
            payload: doc_encode_msg(&DocMsg::UpdateBlock {
                doc_id: "readme".into(),
                block_id: "intro".into(),
                text: "a simple, block-based document on qmdb".into(),
            }),
        }).await.expect("doc update");
        println!("  app-hash       : {:?}", out.app_hash);
        println!("  document root  : {:?}", host.module_root("document").unwrap());

        // read the whole doc back out (typed query) — the ordered blocks.
        let reply = host
            .query("document", &doc_encode_query(&DocQuery::GetDoc { doc_id: "readme".into() }))
            .await
            .expect("query document");
        if let DocReply::Doc(Some(blocks)) = doc_decode_reply(&reply).unwrap() {
            println!("  readme blocks  :");
            for b in &blocks {
                println!("    - [{:?}] {} = {:?}", b.kind, b.id, b.text);
            }
        }

        println!("\nmodule roots:");
        for id in ["directory", "document", "forge", "greeter", "kv", "valset"] {
            println!("  {id:>10} : {:?}", host.module_root(id).unwrap());
        }
        println!("\nfinal app-hash   : {:?}", host.app_hash());
    });
}
