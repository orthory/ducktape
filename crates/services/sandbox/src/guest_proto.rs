//! the vsock wire between the host and one run's guest init.
//!
//! Deliberately tiny: a 1-byte tag, a 4-byte little-endian length, then the
//! payload. There is no handshake and no version byte — there are no live
//! networks and the guest image ships with the host that boots it, so the two
//! ends are always the same build.
//!
//! Every field here is written by an UNTRUSTED guest, so decoding refuses
//! rather than trusts: an unknown tag is an error, and a length header over
//! [`MAX_FRAME_BYTES`] is refused BEFORE anything is allocated.
//!
//! This module has no dependencies and does no I/O on purpose. The guest init
//! includes this same source file directly (`#[path = ...] mod guest_proto;`)
//! rather than depending on this crate, so PID 1 inside the VM does not carry
//! tokio, serde and tracing — while both ends still share one copy of the
//! codec. Keep it dependency-free or that arrangement breaks.

/// the guest-side vsock port the init dials. Host-side is the run's unix
/// socket that Firecracker multiplexes onto it.
pub const VSOCK_PORT: u32 = 1024;

/// the largest single frame. The guest cannot make the host allocate past it.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

const TAG_STDOUT: u8 = 0;
const TAG_STDERR: u8 = 1;
const TAG_EXIT: u8 = 2;
const HEADER_BYTES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Exit(i32),
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let (tag, payload): (u8, &[u8]) = match frame {
        Frame::Stdout(bytes) => (TAG_STDOUT, bytes),
        Frame::Stderr(bytes) => (TAG_STDERR, bytes),
        Frame::Exit(code) => {
            let mut out = Vec::with_capacity(HEADER_BYTES + 4);
            out.push(TAG_EXIT);
            out.extend((4u32).to_le_bytes());
            out.extend(code.to_le_bytes());
            return out;
        }
    };
    let mut out = Vec::with_capacity(HEADER_BYTES + payload.len());
    out.push(tag);
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// drain one frame from `buf`. `Ok(None)` means the buffer holds only part of
/// one — the caller reads more and calls again.
pub fn decode(buf: &mut Vec<u8>) -> Result<Option<Frame>, String> {
    if buf.len() < HEADER_BYTES {
        return Ok(None);
    }
    let tag = buf[0];
    let len = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    // refused on the HEADER, before the payload is read and before anything is
    // allocated — the guest writes this number and must not be able to size a
    // host allocation with it.
    if len > MAX_FRAME_BYTES {
        return Err(format!(
            "guest frame claims {len} bytes, over the {MAX_FRAME_BYTES} cap"
        ));
    }
    if buf.len() < HEADER_BYTES + len {
        return Ok(None);
    }
    let payload: Vec<u8> = buf.drain(..HEADER_BYTES + len).skip(HEADER_BYTES).collect();
    let frame = match tag {
        TAG_STDOUT => Frame::Stdout(payload),
        TAG_STDERR => Frame::Stderr(payload),
        TAG_EXIT => {
            let bytes: [u8; 4] = payload.as_slice().try_into().map_err(|_| {
                format!("guest exit frame carried {} bytes, want 4", payload.len())
            })?;
            Frame::Exit(i32::from_le_bytes(bytes))
        }
        other => return Err(format!("guest frame carried unknown tag {other}")),
    };
    Ok(Some(frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_frame_survives_a_round_trip() {
        for frame in [
            Frame::Stdout(b"hello".to_vec()),
            Frame::Stderr(b"warning: something".to_vec()),
            Frame::Exit(0),
            Frame::Exit(127),
            // a signal death arrives as a negative code; it must not be
            // mangled by the length field's unsignedness.
            Frame::Exit(-9),
            Frame::Stdout(Vec::new()),
        ] {
            let mut buf = encode(&frame);
            let got = decode(&mut buf).expect("decode").expect("a whole frame");
            assert_eq!(got, frame);
            assert!(buf.is_empty(), "the frame must be drained: {buf:?}");
        }
    }

    /// A vsock read returns whatever happens to be there, so a partial frame is
    /// the normal case, not an error. It must leave the buffer untouched for
    /// the next read to complete.
    #[test]
    fn a_partial_frame_yields_none_and_keeps_the_bytes() {
        let whole = encode(&Frame::Stdout(b"abcdefgh".to_vec()));
        for cut in 0..whole.len() {
            let mut buf = whole[..cut].to_vec();
            assert_eq!(decode(&mut buf).expect("partial is not an error"), None);
            assert_eq!(buf.len(), cut, "a partial decode must not consume");
        }
    }

    /// The length header is written by an untrusted guest. An absurd one must
    /// be refused on the header alone — the host never waits for, or allocates
    /// for, a gigabyte the guest merely claims.
    #[test]
    fn an_oversized_length_header_is_refused_before_any_payload() {
        let mut buf = vec![TAG_STDOUT];
        buf.extend(((MAX_FRAME_BYTES + 1) as u32).to_le_bytes());
        let err = decode(&mut buf).expect_err("must refuse");
        assert!(err.contains("over the"), "{err}");
        assert_eq!(buf.len(), HEADER_BYTES, "nothing was consumed");
    }

    /// No `_` fallback on the wire either: a tag we do not know is a guest we
    /// do not understand, and the run fails rather than silently dropping it.
    #[test]
    fn an_unknown_tag_is_an_error_not_a_silent_drop() {
        let mut buf = vec![99u8, 0, 0, 0, 0];
        let err = decode(&mut buf).expect_err("must refuse");
        assert!(err.contains("unknown tag 99"), "{err}");
    }

    /// The guest can claim `Exit` and then send the wrong number of bytes.
    #[test]
    fn an_exit_frame_of_the_wrong_width_is_refused() {
        let mut buf = vec![TAG_EXIT, 2, 0, 0, 0, 1, 2];
        let err = decode(&mut buf).expect_err("must refuse");
        assert!(err.contains("want 4"), "{err}");
    }

    /// Streams arrive coalesced; draining must yield them in order and leave
    /// the remainder intact.
    #[test]
    fn coalesced_frames_drain_in_order() {
        let sent = [
            Frame::Stdout(b"one".to_vec()),
            Frame::Stderr(b"two".to_vec()),
            Frame::Exit(3),
        ];
        let mut buf: Vec<u8> = sent.iter().flat_map(|f| encode(f)).collect();
        let mut got = Vec::new();
        while let Some(frame) = decode(&mut buf).expect("decode") {
            got.push(frame);
        }
        assert_eq!(got, sent);
        assert!(buf.is_empty());
    }
}
