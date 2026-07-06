//! The voice media engine: Opus over the data plane's datagram class.
//!
//! Wire surface (this crate root + [`media`]): the 8-byte media header
//! (version, flags, seq, timestamp) carried inside a data-plane datagram on
//! `Service::Voice`, payload = one 20 ms Opus frame. Speakers are identified
//! by the transport-authenticated `PeerId` — the plane's WireGuard identity
//! binding — so there is no SSRC and no media-level identity to spoof.
//!
//! Boundary: this crate is the media pipeline only —
//!
//! ```text
//! pcm 20ms frame → Opus encode (VOIP, in-band FEC) → media header → fan-out
//! per speaker:  datagrams → jitter buffer → Opus decode (FEC/PLC conceal)
//! playout tick: sum decoded speakers → one mixed pcm frame
//! ```
//!
//! No audio hardware (capture/playback/AEC arrive with the hardware edge),
//! no consensus wiring (channel membership → `AdmissionPolicy` is the chat
//! module's job). Everything here is provable on the data-plane sim
//! transport under a paused clock.

pub mod codec;
pub mod engine;
pub mod jitter;
pub mod media;

pub use codec::{CodecError, VoiceDecoder, VoiceEncoder};
pub use engine::{SpeakerStats, VoiceConfig, VoiceEngine};
pub use jitter::{JitterBuffer, JitterStats, MinimalJitter, PlayoutStep};
pub use media::{MediaError, MediaHeader};

/// Voice runs at Opus's native rate, mono.
pub const SAMPLE_RATE: u32 = 48_000;
/// One media frame = 20 ms — Opus's sweet spot and the packet cadence.
pub const FRAME_MILLIS: u64 = 20;
/// Samples per frame: 48 kHz × 20 ms.
pub const FRAME_SAMPLES: usize = 960;
