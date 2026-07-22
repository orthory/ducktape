//! the shared duckfs test harness: the deterministic `sdk::Ctx` (the shared
//! `sdk_testkit::TestCtx`), a module constructor over a tempdir, and hash
//! helpers. every later test file mounts this module and uses a subset, so
//! unused helpers are expected here.
#![allow(dead_code)]

use sdk::{Env, Origin};
use sha2::{Digest as _, Sha256};

pub use sdk_testkit::TestCtx;

/// a `files`-scoped [`TestCtx`] at block `height` (`consensus_time == height`)
/// with `origin`; the module id is fixed to "files", the harness's only tenant.
/// captured follow-up msgs are read back via [`TestCtx::msgs`].
pub fn test_ctx(origin: Origin, height: u64) -> TestCtx {
    TestCtx::with_env(Env {
        protocol_version: 0,
        height,
        consensus_time: height,
        origin,
        me: "files".into(),
    })
}

pub fn open_files(dir: &tempfile::TempDir) -> files::Files {
    files::Files::open("files", dir.path().to_path_buf()).expect("open")
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

pub fn to_hex(bytes: &[u8]) -> String {
    files::to_hex(bytes)
}
