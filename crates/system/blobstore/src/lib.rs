//! node-local content-addressed byte store for the daemon's receipt lane:
//! op payloads staged at submit time and served back over
//! `GET /v1/files/blob/{digest}`. never consensus state, never in any root.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sha2::{Digest as _, Sha256};

#[derive(Default)]
pub struct BlobStore {
    chunks: HashMap<[u8; 32], Vec<u8>>,
}

impl BlobStore {
    pub fn put_chunk(&mut self, bytes: Vec<u8>) -> [u8; 32] {
        let digest = sha256(&bytes);
        self.chunks.insert(digest, bytes);
        digest
    }

    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<&[u8]> {
        self.chunks.get(digest).map(Vec::as_slice)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.chunks.contains_key(digest)
    }
}

#[derive(Clone, Default)]
pub struct BlobHandle(Arc<Mutex<BlobStore>>);

impl BlobHandle {
    pub fn put_chunk(&self, bytes: Vec<u8>) -> [u8; 32] {
        self.0.lock().expect("blob store poisoned").put_chunk(bytes)
    }

    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("blob store poisoned")
            .get_chunk(digest)
            .map(<[u8]>::to_vec)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.0
            .lock()
            .expect("blob store poisoned")
            .has_chunk(digest)
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}
