//! the shared duckfs test harness: a deterministic `sdk::Ctx`, a module
//! constructor over a tempdir, and hash helpers. every later test file mounts
//! this module and uses a subset, so unused helpers are expected here.
#![allow(dead_code)]

use std::collections::VecDeque;

use sdk::{Ctx, Effect, Env, Error, Event, Msg, Origin, StateRoot};
use sha2::{Digest as _, Sha256};

#[derive(Debug)]
pub struct TestCtx {
    pub env: Env,
    /// follow-up msgs emitted during execute — watch fan-out assertions.
    pub emitted: VecDeque<Msg>,
}

impl TestCtx {
    pub fn new(origin: Origin, height: u64) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height,
                consensus_time: height,
                origin,
                me: "files".into(),
            },
            emitted: VecDeque::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push_back(msg);
    }
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
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
