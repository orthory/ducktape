//! The binary framing of `/v1/call/ws` — the webview leg of a huddle (audio +
//! camera video + call control on one websocket). This file is the ONLY
//! definition of that wire: `noded`'s call-socket handler and the app's
//! TypeScript leg (`app/src/domain/call-frames.ts`) both port their
//! encode/decode from the layouts here — no second definition site, no
//! independent byte-twiddling on either end.
//!
//! **D1 (endianness):** every structural header field (`ts_ms`, and any
//! future length/count field) is big-endian, matching the mesh leg
//! ([`crate::voice::media`], [`crate::video::frame`]) and the rest of the wire
//! standardization. PCM audio samples are the payload, not a header field,
//! and stay little-endian `i16` — this leg is browser↔node loopback only
//! (never relayed to another node), and the browser's `Int16Array` is
//! platform little-endian, so flipping them would just add a conversion with
//! no interop benefit. Opus/VP8 payload bytes are opaque either way.
//!
//! Tag byte layouts (first byte selects the frame kind):
//! ```text
//! audio    [0x01][pcm i16 LE …]                        — both directions
//! captured [0x02][flags u8][ts_ms u32 BE][vp8 …]        — client → server
//! peer     [0x03][flags u8][ts_ms u32 BE][peer 32][vp8 …] — server → client
//! ```
//! `flags` bit 0 ([`WS_FLAG_KEYFRAME`]) marks a decoder sync point.
//!
//! Every decode function returns `None` on a wrong tag or a too-short frame —
//! never panics. These bytes cross the network from an untrusted webview
//! client; malformed input must be a dropped frame, not a crashed session.

/// binary ws frame tags on `/v1/call/ws` (first byte).
pub const WS_TAG_AUDIO: u8 = 0x01;
pub const WS_TAG_VIDEO_CAPTURED: u8 = 0x02; // client → server
pub const WS_TAG_VIDEO_PEER: u8 = 0x03; // server → client
pub const WS_FLAG_KEYFRAME: u8 = 0b0000_0001;
/// tag + flags + ts_ms.
pub const WS_VIDEO_CAPTURED_HEADER: usize = 6;
/// tag + flags + ts_ms + peer key.
pub const WS_VIDEO_PEER_HEADER: usize = 38;

/// one pcm sample is an i16 — two wire bytes, little endian (payload, not a
/// header field — see the module doc's D1 note).
const PCM_FRAME_BYTES: usize = crate::voice::FRAME_SAMPLES * 2;

/// encode a captured mic / mixed playout frame: `[0x01][pcm i16 LE …]`. `pcm`
/// is encoded at whatever length it is handed — the exact-length gate lives
/// on [`decode_audio`], the boundary that parses untrusted network bytes.
pub fn encode_audio(pcm: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + pcm.len() * 2);
    out.push(WS_TAG_AUDIO);
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// decode one audio frame. `None` on the wrong tag or any length other than
/// exactly one [`crate::voice::FRAME_SAMPLES`]-sample frame (`1 +
/// PCM_FRAME_BYTES` bytes) — a partial or oversized frame is never partially
/// decoded.
pub fn decode_audio(frame: &[u8]) -> Option<Vec<i16>> {
    if frame.len() != 1 + PCM_FRAME_BYTES || frame[0] != WS_TAG_AUDIO {
        return None;
    }
    Some(
        frame[1..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect(),
    )
}

/// one captured, encoded camera frame, webview → hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    /// this frame is a decoder sync point (a full keyframe, not a delta).
    pub keyframe: bool,
    /// capture timestamp in ms (opaque to the hub; echoed to the far webview).
    pub ts_ms: u32,
    /// the encoded (VP8) frame bytes.
    pub data: Vec<u8>,
}

/// encode `[0x02][flags][ts_ms u32 BE][vp8]`.
pub fn encode_captured(f: &CapturedFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(WS_VIDEO_CAPTURED_HEADER + f.data.len());
    out.push(WS_TAG_VIDEO_CAPTURED);
    out.push(if f.keyframe { WS_FLAG_KEYFRAME } else { 0 });
    out.extend_from_slice(&f.ts_ms.to_be_bytes());
    out.extend_from_slice(&f.data);
    out
}

/// decode one captured-video frame. `None` on the wrong tag or a frame no
/// longer than the header (the header alone, with no vp8 payload, is not a
/// frame worth forwarding).
pub fn decode_captured(frame: &[u8]) -> Option<CapturedFrame> {
    if frame.len() <= WS_VIDEO_CAPTURED_HEADER || frame[0] != WS_TAG_VIDEO_CAPTURED {
        return None;
    }
    Some(CapturedFrame {
        keyframe: frame[1] & WS_FLAG_KEYFRAME != 0,
        ts_ms: u32::from_be_bytes(frame[2..6].try_into().expect("4 bytes")),
        data: frame[WS_VIDEO_CAPTURED_HEADER..].to_vec(),
    })
}

/// one reassembled camera frame, hub → webview, tagged with the mesh-
/// authenticated sending peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerFrame {
    /// the sending peer's raw ed25519 node key.
    pub peer: [u8; 32],
    pub keyframe: bool,
    pub ts_ms: u32,
    /// the reassembled encoded (VP8) frame bytes.
    pub data: Vec<u8>,
}

/// encode `[0x03][flags][ts_ms u32 BE][peer 32][vp8]`.
pub fn encode_peer(f: &PeerFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(WS_VIDEO_PEER_HEADER + f.data.len());
    out.push(WS_TAG_VIDEO_PEER);
    out.push(if f.keyframe { WS_FLAG_KEYFRAME } else { 0 });
    out.extend_from_slice(&f.ts_ms.to_be_bytes());
    out.extend_from_slice(&f.peer);
    out.extend_from_slice(&f.data);
    out
}

/// decode one peer-video frame. `None` on the wrong tag or a frame no longer
/// than the header.
pub fn decode_peer(frame: &[u8]) -> Option<PeerFrame> {
    if frame.len() <= WS_VIDEO_PEER_HEADER || frame[0] != WS_TAG_VIDEO_PEER {
        return None;
    }
    let mut peer = [0u8; 32];
    peer.copy_from_slice(&frame[6..38]);
    Some(PeerFrame {
        peer,
        keyframe: frame[1] & WS_FLAG_KEYFRAME != 0,
        ts_ms: u32::from_be_bytes(frame[2..6].try_into().expect("4 bytes")),
        data: frame[WS_VIDEO_PEER_HEADER..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_captured_video_be() {
        let f = CapturedFrame {
            keyframe: true,
            ts_ms: 0x0102_0304,
            data: vec![0xAA, 0xBB],
        };
        assert_eq!(
            encode_captured(&f),
            vec![0x02, 0x01, 0x01, 0x02, 0x03, 0x04, 0xAA, 0xBB]
        );
        assert_eq!(decode_captured(&encode_captured(&f)), Some(f));
    }

    #[test]
    fn golden_peer_video_be() {
        let f = PeerFrame {
            peer: [0x11; 32],
            keyframe: false,
            ts_ms: 0x0A0B_0C0D,
            data: vec![0xF0],
        };
        let mut want = vec![0x03, 0x00, 0x0A, 0x0B, 0x0C, 0x0D];
        want.extend_from_slice(&[0x11; 32]);
        want.push(0xF0);
        assert_eq!(encode_peer(&f), want);
        assert_eq!(decode_peer(&encode_peer(&f)), Some(f));
    }

    #[test]
    fn golden_audio_pcm_payload_stays_le() {
        // 960 samples; first two are 1 and -2 → LE bytes 01 00 FE FF after the tag.
        let mut pcm = vec![0i16; crate::voice::FRAME_SAMPLES];
        pcm[0] = 1;
        pcm[1] = -2;
        let bytes = encode_audio(&pcm);
        assert_eq!(bytes.len(), 1 + crate::voice::FRAME_SAMPLES * 2);
        assert_eq!(&bytes[..5], &[0x01, 0x01, 0x00, 0xFE, 0xFF]);
        assert_eq!(decode_audio(&bytes).as_deref(), Some(&pcm[..]));
    }

    #[test]
    fn short_and_wrong_tag_frames_decode_to_none() {
        assert_eq!(decode_captured(&[0x02, 0x01, 0x01]), None); // shorter than header
        assert_eq!(decode_peer(&[0x02, 0x00, 0, 0, 0, 0]), None); // wrong tag
        assert_eq!(decode_audio(&[0x01, 0x00]), None); // not 1+1920 bytes
    }
}
