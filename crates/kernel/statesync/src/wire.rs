//! strict little-endian wire primitives shared by every state-sync frame.
//!
//! every read is bounds-checked BEFORE any allocation (a forged length can
//! never drive memory), and top-level decoders require the buffer to be fully
//! consumed (`expect_empty`) so a given frame has exactly one valid encoding.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("frame truncated")]
    Truncated,
    #[error("frame carries trailing bytes")]
    Trailing,
    #[error("bad {0} tag: {1}")]
    BadTag(&'static str, u8),
    #[error("invalid utf-8 string")]
    BadUtf8,
    #[error("codec: {0}")]
    Codec(String),
}

pub fn take_u8(buf: &mut &[u8]) -> Result<u8, WireError> {
    let Some((head, rest)) = buf.split_first() else {
        return Err(WireError::Truncated);
    };
    let v = *head;
    *buf = rest;
    Ok(v)
}

pub fn take_u64(buf: &mut &[u8]) -> Result<u64, WireError> {
    let Some((head, rest)) = buf.split_first_chunk::<8>() else {
        return Err(WireError::Truncated);
    };
    let v = u64::from_le_bytes(*head);
    *buf = rest;
    Ok(v)
}

pub fn take_u32(buf: &mut &[u8]) -> Result<u32, WireError> {
    let Some((head, rest)) = buf.split_first_chunk::<4>() else {
        return Err(WireError::Truncated);
    };
    let v = u32::from_le_bytes(*head);
    *buf = rest;
    Ok(v)
}

pub fn take_array<const N: usize>(buf: &mut &[u8]) -> Result<[u8; N], WireError> {
    let Some((head, rest)) = buf.split_first_chunk::<N>() else {
        return Err(WireError::Truncated);
    };
    let v = *head;
    *buf = rest;
    Ok(v)
}

/// take a u64-length-prefixed byte slice. the length is checked against the
/// remaining buffer BEFORE any slicing, so a forged length cannot allocate.
pub fn take_bytes<'a>(buf: &mut &'a [u8]) -> Result<&'a [u8], WireError> {
    let len = take_u64(buf)?;
    if len > buf.len() as u64 {
        return Err(WireError::Truncated);
    }
    let (head, rest) = buf.split_at(len as usize);
    *buf = rest;
    Ok(head)
}

pub fn take_str(buf: &mut &[u8]) -> Result<String, WireError> {
    let bytes = take_bytes(buf)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| WireError::BadUtf8)
}

// the WRITE side IS the shared `sdk::codec` primitive verbatim (`u64`-LE length
// prefix + bytes). re-export it rather than keep a second copy of the exact same
// byte-producing code — this is the encoded-bytes contract, so byte-identity is
// not merely preserved, it is the same function. the READ side below stays
// statesync's own: it carries a typed [`WireError`] woven through ~40 sites and
// >100 call sites, and every decoder already applies its own count cap
// (`MAX_OPS_PER_BATCH`, `MAX_PROOF_DIGESTS`) — stricter than a generic cursor
// bound — plus `expect_empty` trailing rejection, so converting the readers to
// `sdk::codec::Cursor` would trade the typed error model for a stringly one with
// zero byte benefit.
pub use sdk::codec::{push_bytes as put_bytes, push_str as put_str};

pub fn expect_empty(buf: &[u8]) -> Result<(), WireError> {
    if buf.is_empty() {
        Ok(())
    } else {
        Err(WireError::Trailing)
    }
}
