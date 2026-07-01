//! a runnable super-app demo: three isolated modules (a qmdb-backed kv, a sync
//! in-memory directory, a stateless greeter) dispatched over ONE host, showing
//! the app-hash evolve as typed cross-module ops flow.
//!
//! run: `cargo run -p demo`

use commonware_runtime::{deterministic, Runner as _};
use directory::Directory;
use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery};
use greeter::Greeter;
use host::Host;
use sdk::Msg;

fn main() {
    deterministic::Runner::default().start(|context| async move {
        // genesis: the module registry (would be consensus state on a real chain).
        let kv = kv::Kv::init(context, "kv").await;
        let directory = Directory::new("directory");
        let greeter = Greeter::new("greeter");
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(directory),
            Box::new(greeter),
        ])
        .expect("genesis");

        println!("=== super-app demo — 3 isolated modules over one host ===");
        println!("genesis app-hash : {:?}", host.app_hash());

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

        // read the derived greeting back out of the directory (sync typed query).
        let reply = host
            .query("directory", &encode_query(&DirQuery::Get { key: "greeting:name".into() }))
            .await
            .expect("query directory");
        println!("\ndirectory[greeting:name] = {:?}", decode_reply(&reply).unwrap());

        // and read it back out of the QMDB kv module — a real async cross-module read,
        // which was impossible before query went async.
        let kv_reply = host
            .query("kv", &kv_interface::encode_query(&kv_interface::KvQuery::Get { key: b"greeting:name".to_vec() }))
            .await
            .expect("query kv");
        if let kv_interface::KvReply::Value(Some(v)) = kv_interface::decode_reply(&kv_reply).unwrap() {
            println!("kv[greeting:name]        = {:?}", String::from_utf8_lossy(&v));
        }

        println!("\nmodule roots:");
        for id in ["directory", "greeter", "kv"] {
            println!("  {id:>10} : {:?}", host.module_root(id).unwrap());
        }
        println!("\nfinal app-hash   : {:?}", host.app_hash());
    });
}
