use std::time::{SystemTime, UNIX_EPOCH};

use sha2::Digest;

pub(crate) fn rand() -> [u8; 32] {
    let timestamp: [u8; 16] = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_be_bytes();

    let hash = sha2::Sha256::digest(timestamp);
    hash.into()
}
