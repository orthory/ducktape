//! the store-injection seam (PR4): `Files::in_mem()` runs a real manifest op and
//! serves the matching query with ZERO disk. there is deliberately no `TempDir`
//! anywhere in this file — if the mem arm ever reaches for the filesystem, this
//! test stops compiling (no tempfile import) or the odb/refs land nowhere.

mod harness;
use harness::test_ctx;

use std::collections::BTreeMap;
use std::future::Future;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sdk::{Module as _, Origin};

use files::{
    Change, Content, FilesMsg, FilesQuery, FilesReply, decode_reply, encode_msg, encode_query,
};

fn block_on<F: Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

#[test]
fn in_mem_commit_then_read_with_zero_disk() {
    // the whole point: constructed with no path, no tempdir — pure memory.
    let mut f = files::Files::in_mem();

    // one manifest op: commit a single inline file at /hello.
    let op = sdk::Msg {
        target: "files".into(),
        payload: encode_msg(&FilesMsg::Commit {
            base_snapshot: None,
            message: "hi".into(),
            changes: vec![Change::Put {
                path: "/hello".into(),
                exec: false,
                meta: BTreeMap::new(),
                content: Content::Inline {
                    b64: STANDARD.encode(b"world"),
                },
            }],
        }),
    };
    block_on(f.execute(&mut test_ctx(Origin::System, 1), &op)).expect("commit ok");
    block_on(f.commit_block()).expect("commit_block ok");

    // the matching query serves the committed bytes back — the full op → commit
    // → query loop over the injected mem store, no filesystem touched.
    let reply = block_on(f.query(&encode_query(&FilesQuery::Read {
        path: "/hello".into(),
        snapshot: None,
        offset: 0,
        len: 64,
    })))
    .expect("read query ok");
    match decode_reply(&reply).unwrap() {
        FilesReply::Read { b64, eof } => {
            assert_eq!(STANDARD.decode(b64.as_bytes()).unwrap(), b"world");
            assert!(eof, "the whole file fit in the read window");
        }
        other => panic!("expected a Read reply, got {other:?}"),
    }
}
