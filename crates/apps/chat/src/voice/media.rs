//! The media frame carried inside a data-plane datagram: an 8-byte header,
//! then one Opus frame.
//!
//! ```text
//! offset  0        1          2..4          4..8
//!         ver = 1  flags = 0  seq (u16 BE)  timestamp (u32 BE, 48 kHz units)
//! ```
//!
//! `seq` orders frames for the jitter buffer (wrapping, one per 20 ms);
//! `timestamp` rides along for consumers that need media time (DTX gaps,
//! future video sync) — the minimal jitter buffer is seq-driven and ignores
//! it. The sender identity is NOT in the header: it is the datagram's
//! transport-authenticated `PeerId`.

use data_plane::MAX_DATAGRAM_PAYLOAD;

pub const MEDIA_VERSION: u8 = 1;
pub const MEDIA_HEADER_LEN: usize = 8;
/// Largest Opus payload per frame. Voice at 32 kbps is ~80 bytes; the bound
/// exists so a media frame always fits one plane datagram.
pub const MAX_OPUS_PAYLOAD: usize = MAX_DATAGRAM_PAYLOAD - MEDIA_HEADER_LEN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaHeader {
    pub seq: u16,
    pub timestamp: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("opus payload {len} exceeds {MAX_OPUS_PAYLOAD}")]
    PayloadTooLarge { len: usize },
    #[error("media frame truncated")]
    Truncated,
    #[error("unsupported media version {0}")]
    BadVersion(u8),
}

pub fn encode_frame(header: MediaHeader, opus_payload: &[u8]) -> Result<Vec<u8>, MediaError> {
    if opus_payload.len() > MAX_OPUS_PAYLOAD {
        return Err(MediaError::PayloadTooLarge {
            len: opus_payload.len(),
        });
    }
    let mut frame = Vec::with_capacity(MEDIA_HEADER_LEN + opus_payload.len());
    frame.push(MEDIA_VERSION);
    frame.push(0);
    frame.extend_from_slice(&header.seq.to_be_bytes());
    frame.extend_from_slice(&header.timestamp.to_be_bytes());
    frame.extend_from_slice(opus_payload);
    Ok(frame)
}

pub fn decode_frame(frame: &[u8]) -> Result<(MediaHeader, &[u8]), MediaError> {
    if frame.len() < MEDIA_HEADER_LEN {
        return Err(MediaError::Truncated);
    }
    if frame[0] != MEDIA_VERSION {
        return Err(MediaError::BadVersion(frame[0]));
    }
    let seq = u16::from_be_bytes(frame[2..4].try_into().expect("2 bytes"));
    let timestamp = u32::from_be_bytes(frame[4..8].try_into().expect("4 bytes"));
    Ok((MediaHeader { seq, timestamp }, &frame[MEDIA_HEADER_LEN..]))
}

/// Wrapping seq comparison: is `a` newer than `b`? Correct across the u16
/// wrap as long as the true distance is under half the space (~11 minutes
/// of frames).
pub fn seq_newer(a: u16, b: u16) -> bool {
    (a.wrapping_sub(b) as i16) > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// pins the exact wire bytes (D1: header fields big-endian) — locks this
    /// already-BE datagram codec alongside the call-socket codec
    /// (`chat::call_wire`) as part of the wire-standardization sweep.
    #[test]
    fn golden_header_be() {
        let header = MediaHeader {
            seq: 0x0102,
            timestamp: 0x0A0B_0C0D,
        };
        assert_eq!(
            encode_frame(header, &[0x5A]).unwrap(),
            vec![0x01, 0x00, 0x01, 0x02, 0x0A, 0x0B, 0x0C, 0x0D, 0x5A]
        );
    }

    #[test]
    fn frame_round_trip() {
        let header = MediaHeader {
            seq: 65_535,
            timestamp: 4_000_000_000,
        };
        let frame = encode_frame(header, b"opus").unwrap();
        let (decoded, payload) = decode_frame(&frame).unwrap();
        assert_eq!(decoded, header);
        assert_eq!(payload, b"opus");
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            decode_frame(&[1, 0, 0]),
            Err(MediaError::Truncated)
        ));
        let mut frame = encode_frame(
            MediaHeader {
                seq: 0,
                timestamp: 0,
            },
            b"x",
        )
        .unwrap();
        frame[0] = 7;
        assert!(matches!(
            decode_frame(&frame),
            Err(MediaError::BadVersion(7))
        ));
        let big = vec![0u8; MAX_OPUS_PAYLOAD + 1];
        assert!(matches!(
            encode_frame(
                MediaHeader {
                    seq: 0,
                    timestamp: 0
                },
                &big
            ),
            Err(MediaError::PayloadTooLarge { .. })
        ));
    }

    #[test]
    fn seq_comparison_wraps() {
        assert!(seq_newer(1, 0));
        assert!(seq_newer(0, 65_535));
        assert!(seq_newer(5, 65_530));
        assert!(!seq_newer(65_530, 5));
    }
}
