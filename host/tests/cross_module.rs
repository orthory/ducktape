//! proves typed cross-module composition end-to-end through the host: a greeter
//! module QUERIES a directory module and, from the result, WRITES a greeting to
//! both directory and kv — using only the modules' interface crates.

use commonware_runtime::{deterministic, Runner as _};
use directory::Directory;
use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery, DirReply};
use greeter::Greeter;
use host::Host;
use sdk::Msg;

#[test]
fn greeter_reads_directory_and_writes_a_derived_greeting() {
    deterministic::Runner::default().start(|context| async move {
        let kv = kv::Kv::init(context, "kv").await;
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(Directory::new("directory")),
            Box::new(Greeter::new("greeter")),
        ])
        .expect("genesis");

        // seed the directory, then trigger the greeter.
        host.submit(Msg {
            target: "directory".into(),
            payload: encode_msg(&DirMsg::Set { key: "name".into(), value: "world".into() }),
        })
        .await
        .expect("seed");

        let kv_root_before = host.module_root("kv").unwrap();

        host.submit(Msg { target: "greeter".into(), payload: b"name".to_vec() })
            .await
            .expect("greet");

        // the greeting is DERIVED from the cross-module query (name=world), not hardcoded.
        let reply = host
            .query("directory", &encode_query(&DirQuery::Get { key: "greeting:name".into() }))
            .await
            .unwrap();
        assert_eq!(decode_reply(&reply).unwrap(), DirReply::Value(Some("hello world".into())));

        // the greeting is also readable from the QMDB kv module via a real async query.
        let kvr = host
            .query("kv", &kv_interface::encode_query(&kv_interface::KvQuery::Get { key: b"greeting:name".to_vec() }))
            .await
            .unwrap();
        assert_eq!(kv_interface::decode_reply(&kvr).unwrap(), kv_interface::KvReply::Value(Some(b"hello world".to_vec())));

        // and the typed follow-up reached the qmdb kv module too (its real root moved).
        assert_ne!(host.module_root("kv").unwrap(), kv_root_before, "greeter's kv write must land");
    });
}

#[test]
fn genesis_rejects_duplicate_ids() {
    deterministic::Runner::default().start(|_| async move {
        let a = Directory::new("dup");
        let b = Directory::new("dup");
        let err = Host::genesis(vec![Box::new(a), Box::new(b)]);
        assert!(err.is_err(), "duplicate module id must be rejected at genesis");
    });
}
