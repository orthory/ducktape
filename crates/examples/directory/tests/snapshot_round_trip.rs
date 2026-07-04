//! snapshot/install round-trip: a fresh `Directory` reconstructs a source's
//! committed state — and therefore its exact root — from the source's canonical
//! snapshot bytes. the snapshot is exactly the byte stream `root()` hashes, so
//! the expected root verifies the WHOLE payload: any flipped byte and any
//! truncation must be rejected, and a rejected install must leave the target
//! byte-identical (state-sync serves untrusted bytes from a byzantine peer;
//! only the root, learned from consensus, is trusted).

use directory::Directory;
use directory_interface::{DirMsg, encode_msg};
use sdk::{Ctx, Error, Event, Module, Msg, StateRoot};
use sha2::{Digest, Sha256};

// a minimal Ctx — directory's execute never touches ctx, but the trait needs one.
struct TestCtx {
    env: sdk::Env,
}
impl TestCtx {
    fn new() -> Self {
        Self {
            env: sdk::Env { protocol_version: 0,
                height: 0,
                consensus_time: 0,
                origin: sdk::Origin::System,
                me: "directory".into(),
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
    fn emit_event(&mut self, _e: Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

fn run<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}

/// a source with real content driven through the execute path and committed.
fn committed_source() -> Directory {
    let mut src = Directory::new("directory");
    let mut ctx = TestCtx::new();
    for (k, v) in [("b", "2"), ("a", "1"), ("a", "3"), ("c", "4")] {
        run(src.execute(&mut ctx, &set(k, v))).unwrap();
    }
    run(src.commit_block()).unwrap();
    src
}

#[test]
fn install_reconstructs_source_root_and_reads() {
    let src = committed_source();
    let src_root = src.root();
    let bytes = src.snapshot();

    // the snapshot IS the byte stream root() hashes — the root verifies the
    // exact payload, not merely a projection of it.
    let mut h = Sha256::new();
    h.update(&bytes);
    assert_eq!(
        StateRoot(h.finalize().into()),
        src_root,
        "sha256(snapshot) == root"
    );

    // JOINER: a fresh instance with an unrelated staged write — a successful
    // install must make the snapshot the whole truth, overlay included.
    let mut dst = Directory::new("directory");
    dst.stage("stale".into(), "overlay".into());
    dst.install(&bytes, src_root).unwrap();

    assert_eq!(
        dst.root(),
        src_root,
        "installed root must equal the source root"
    );
    assert_eq!(
        dst.get("a").map(String::as_str),
        Some("3"),
        "overwrite survives the trip"
    );
    assert_eq!(dst.get("b").map(String::as_str), Some("2"));
    assert_eq!(dst.get("c").map(String::as_str), Some("4"));
    assert_eq!(dst.get("stale"), None, "install drops the staged overlay");
}

#[test]
fn any_flipped_byte_is_rejected_and_leaves_the_target_untouched() {
    let src = committed_source();
    let src_root = src.root();
    let bytes = src.snapshot();

    // the target already holds DIFFERENT committed content plus a staged write:
    // a failed install must leave every bit of it alone.
    for i in 0..bytes.len() {
        let mut tampered = bytes.clone();
        tampered[i] ^= 0x01;

        let mut dst = Directory::new("directory");
        dst.set("x".into(), "9".into());
        dst.stage("y".into(), "8".into());
        let before = dst.root();

        let err = dst.install(&tampered, src_root).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "byte {i}: tamper errs with Module"
        );
        assert_eq!(
            dst.root(),
            before,
            "byte {i}: root unchanged after rejected install"
        );
        assert_eq!(
            dst.get("x").map(String::as_str),
            Some("9"),
            "byte {i}: committed state intact"
        );
        assert_eq!(
            dst.get("y").map(String::as_str),
            Some("8"),
            "byte {i}: staged overlay intact"
        );
    }
}

#[test]
fn any_truncated_snapshot_is_rejected() {
    let src = committed_source();
    let src_root = src.root();
    let bytes = src.snapshot();

    for cut in 0..bytes.len() {
        let mut dst = Directory::new("directory");
        let before = dst.root();
        let err = dst.install(&bytes[..cut], src_root).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "cut {cut}: truncation errs with Module"
        );
        assert_eq!(
            dst.root(),
            before,
            "cut {cut}: root unchanged after rejected install"
        );
    }
}
