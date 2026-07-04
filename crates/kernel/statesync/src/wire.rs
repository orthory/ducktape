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

pub fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

pub fn put_str(out: &mut Vec<u8>, s: &str) {
    put_bytes(out, s.as_bytes());
}

pub fn expect_empty(buf: &[u8]) -> Result<(), WireError> {
    if buf.is_empty() {
        Ok(())
    } else {
        Err(WireError::Trailing)
    }
}
