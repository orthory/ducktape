//! Opus wrappers (pure-Rust `opus-rs`) pinned to the engine's frame contract:
//! 48 kHz mono VOIP, 20 ms / 960-sample frames.
//!
//! `i16` PCM is the engine's surface; `opus-rs` is `f32` I/O, so the
//! conversion lives here at the boundary and nowhere else.
//!
//! `opus-rs` exposes neither FEC-decode nor PLC — its decoder errors on empty
//! input and has no flag to reconstruct a frame from a successor packet. So
//! this wrapper only encodes present frames and decodes received ones;
//! concealing a gap is the engine's job (silence today). Encoding LBRR would
//! be dead weight — nothing can decode it — so in-band FEC is left off.

use opus_rs::{Application, OpusDecoder, OpusEncoder};

use super::{FRAME_SAMPLES, SAMPLE_RATE};

/// Room for any 20 ms mono Opus frame at sane bitrates.
const MAX_ENCODED: usize = 1275;
/// Full-scale for i16 ↔ normalized-f32 conversion.
const SCALE: f32 = 32_768.0;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    // opus-rs reports failures as &'static str.
    #[error("opus: {0}")]
    Opus(&'static str),
}

pub struct VoiceEncoder {
    inner: OpusEncoder,
    scratch: Vec<f32>,
}

impl VoiceEncoder {
    pub fn new(bitrate_bits_per_sec: i32) -> Result<Self, CodecError> {
        let mut inner =
            OpusEncoder::new(SAMPLE_RATE as i32, 1, Application::Voip).map_err(CodecError::Opus)?;
        inner.bitrate_bps = bitrate_bits_per_sec;
        inner.use_inband_fec = false;
        inner.packet_loss_perc = 0;
        Ok(VoiceEncoder {
            inner,
            scratch: vec![0.0; FRAME_SAMPLES],
        })
    }

    /// Encode exactly one 20 ms frame.
    pub fn encode(&mut self, pcm: &[i16; FRAME_SAMPLES]) -> Result<Vec<u8>, CodecError> {
        for (dst, &src) in self.scratch.iter_mut().zip(pcm.iter()) {
            *dst = f32::from(src) / SCALE;
        }
        let mut out = vec![0u8; MAX_ENCODED];
        let len = self
            .inner
            .encode(&self.scratch, FRAME_SAMPLES, &mut out)
            .map_err(CodecError::Opus)?;
        out.truncate(len);
        Ok(out)
    }
}

pub struct VoiceDecoder {
    inner: OpusDecoder,
    scratch: Vec<f32>,
}

impl VoiceDecoder {
    pub fn new() -> Result<Self, CodecError> {
        let inner = OpusDecoder::new(SAMPLE_RATE as i32, 1).map_err(CodecError::Opus)?;
        Ok(VoiceDecoder {
            inner,
            scratch: vec![0.0; FRAME_SAMPLES],
        })
    }

    /// Decode one received frame to `i16` PCM. A short decode (fewer samples
    /// than a full frame) leaves the tail silent rather than stale.
    pub fn decode(&mut self, payload: &[u8]) -> Result<[i16; FRAME_SAMPLES], CodecError> {
        let decoded = self
            .inner
            .decode(payload, FRAME_SAMPLES, &mut self.scratch)
            .map_err(CodecError::Opus)?;
        let mut pcm = [0i16; FRAME_SAMPLES];
        for (dst, &src) in pcm.iter_mut().zip(self.scratch.iter()).take(decoded) {
            *dst = (src * SCALE).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        }
        Ok(pcm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(tick: usize) -> [i16; FRAME_SAMPLES] {
        let mut pcm = [0i16; FRAME_SAMPLES];
        for (i, sample) in pcm.iter_mut().enumerate() {
            let t = (tick * FRAME_SAMPLES + i) as f32 / SAMPLE_RATE as f32;
            *sample = ((t * 440.0 * std::f32::consts::TAU).sin() * 8000.0) as i16;
        }
        pcm
    }

    fn rms(pcm: &[i16]) -> f64 {
        let sum: f64 = pcm.iter().map(|&s| (s as f64) * (s as f64)).sum();
        (sum / pcm.len() as f64).sqrt()
    }

    #[test]
    fn round_trip_preserves_energy() {
        let mut enc = VoiceEncoder::new(32_000).unwrap();
        let mut dec = VoiceDecoder::new().unwrap();
        // A few frames so codec state settles past the initial transient.
        let mut last = [0i16; FRAME_SAMPLES];
        for tick in 0..10 {
            let payload = enc.encode(&tone(tick)).unwrap();
            assert!(payload.len() <= super::super::media::MAX_OPUS_PAYLOAD);
            last = dec.decode(&payload).unwrap();
        }
        let (input_rms, output_rms) = (rms(&tone(9)), rms(&last));
        assert!(
            (output_rms - input_rms).abs() / input_rms < 0.25,
            "energy drifted: in {input_rms}, out {output_rms}"
        );
    }
}
