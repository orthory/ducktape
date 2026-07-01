//! a runnable super-app demo: four isolated modules — a qmdb-backed kv, a sync
//! in-memory directory, a stateless greeter, and a GIT-backed forge — dispatched
//! over ONE host, showing the app-hash evolve as typed cross-module ops flow, and
//! showing forge's git HEAD oid compose into that same app-hash as its root.
//!
//! run: `cargo run -p demo`

use commonware_runtime::{deterministic, Runner as _};
use directory::Directory;
use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery};
use forge::Forge;
use forge_interface::{decode_reply as forge_decode_reply, encode_msg as forge_encode_msg,
    encode_query as forge_encode_query, ForgeMsg, ForgeQuery, ForgeReply};
use greeter::Greeter;
use host::Host;
use sdk::Msg;

fn main() {
    // forge's substrate is a real git repo on disk. wipe any prior run's dir so
    // genesis starts from an unborn repo (root == ZERO) and output is reproducible.
    let forge_repo = std::env::temp_dir().join("ducktape-forge-demo");
    let _ = std::fs::remove_dir_all(&forge_repo);

    deterministic::Runner::default().start(|context| async move {
        // genesis: the module registry (would be consensus state on a real chain).
        let kv = kv::Kv::init(context, "kv").await;
        let directory = Directory::new("directory");
        let greeter = Greeter::new("greeter");
        let forge = Forge::init("forge", forge_repo.clone()).expect("forge init");
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(directory),
            Box::new(greeter),
            Box::new(forge),
        ])
        .expect("genesis");

        println!("=== super-app demo — 4 isolated modules over one host ===");
        println!("forge repo       : {}", forge_repo.display());
        println!("genesis app-hash : {:?}", host.app_hash());
        println!("genesis forge root (unborn git repo): {:?}", host.module_root("forge").unwrap());

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

        // read forge's HEAD back out (typed query) — the git oid hex. in sha256
        // mode this hex IS the forge root bytes: a git commit addressing the app-hash.
        let reply = host
            .query("forge", &forge_encode_query(&ForgeQuery::Head))
            .await
            .expect("query forge");
        if let ForgeReply::Head(Some(oid)) = forge_decode_reply(&reply).unwrap() {
            println!("  forge git HEAD : {oid}");
            println!("  (^ equals the forge root bytes above — a git HEAD as a module root)");
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

        println!("\nmodule roots:");
        for id in ["directory", "forge", "greeter", "kv"] {
            println!("  {id:>10} : {:?}", host.module_root(id).unwrap());
        }
        println!("\nfinal app-hash   : {:?}", host.app_hash());
    });
}
