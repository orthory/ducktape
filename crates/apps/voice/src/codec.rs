//! Opus wrappers pinned to the engine's frame contract: 48 kHz mono VOIP
//! mode, 20 ms frames, in-band FEC enabled.
//!
//! Loss handling comes in two flavors, both through the stateful decoder
//! (state continuity across frames is what makes concealment sound right):
//! - **FEC** ([`VoiceDecoder::conceal_with_fec`]): the NEXT packet carries a
//!   low-bitrate redundant copy of the lost frame (Opus LBRR) — decode it
//!   with `fec = true` to reconstruct the loss almost transparently.
//! - **PLC** ([`VoiceDecoder::conceal`]): no lookahead available — the
//!   decoder extrapolates from its own state.

use crate::{FRAME_SAMPLES, SAMPLE_RATE};

/// Room for any 20 ms mono Opus frame at sane bitrates.
const MAX_ENCODED: usize = 1275;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("opus: {0}")]
    Opus(#[from] opus::Error),
}

pub struct VoiceEncoder {
    inner: opus::Encoder,
}

impl VoiceEncoder {
    /// `expected_loss_perc` sizes the in-band FEC redundancy — match it to
    /// the loss the links actually show; higher trades bitrate for
    /// concealment quality.
    pub fn new(bitrate_bits_per_sec: i32, expected_loss_perc: i32) -> Result<Self, CodecError> {
        let mut inner =
            opus::Encoder::new(SAMPLE_RATE, opus::Channels::Mono, opus::Application::Voip)?;
        inner.set_bitrate(opus::Bitrate::Bits(bitrate_bits_per_sec))?;
        inner.set_inband_fec(true)?;
        inner.set_packet_loss_perc(expected_loss_perc)?;
        Ok(VoiceEncoder { inner })
    }

    /// Encode exactly one 20 ms frame.
    pub fn encode(&mut self, pcm: &[i16; FRAME_SAMPLES]) -> Result<Vec<u8>, CodecError> {
        let mut out = vec![0u8; MAX_ENCODED];
        let len = self.inner.encode(pcm, &mut out)?;
        out.truncate(len);
        Ok(out)
    }
}

pub struct VoiceDecoder {
    inner: opus::Decoder,
}

impl VoiceDecoder {
    pub fn new() -> Result<Self, CodecError> {
        Ok(VoiceDecoder {
            inner: opus::Decoder::new(SAMPLE_RATE, opus::Channels::Mono)?,
        })
    }

    /// Decode a received frame.
    pub fn decode(&mut self, payload: &[u8]) -> Result<[i16; FRAME_SAMPLES], CodecError> {
        let mut pcm = [0i16; FRAME_SAMPLES];
        self.inner.decode(payload, &mut pcm, false)?;
        Ok(pcm)
    }

    /// Reconstruct a LOST frame from the following packet's in-band FEC
    /// data. Call in the lost frame's playout slot; the packet itself still
    /// plays normally in its own slot.
    pub fn conceal_with_fec(
        &mut self,
        next_payload: &[u8],
    ) -> Result<[i16; FRAME_SAMPLES], CodecError> {
        let mut pcm = [0i16; FRAME_SAMPLES];
        self.inner.decode(next_payload, &mut pcm, true)?;
        Ok(pcm)
    }

    /// Conceal a lost frame with no lookahead: decoder-state extrapolation.
    pub fn conceal(&mut self) -> Result<[i16; FRAME_SAMPLES], CodecError> {
        let mut pcm = [0i16; FRAME_SAMPLES];
        self.inner.decode(&[], &mut pcm, false)?;
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
        let mut enc = VoiceEncoder::new(32_000, 10).unwrap();
        let mut dec = VoiceDecoder::new().unwrap();
        // A few frames so codec state settles past the initial transient.
        let mut last = [0i16; FRAME_SAMPLES];
        for tick in 0..10 {
            let payload = enc.encode(&tone(tick)).unwrap();
            assert!(payload.len() <= crate::media::MAX_OPUS_PAYLOAD);
            last = dec.decode(&payload).unwrap();
        }
        let (input_rms, output_rms) = (rms(&tone(9)), rms(&last));
        assert!(
            (output_rms - input_rms).abs() / input_rms < 0.25,
            "energy drifted: in {input_rms}, out {output_rms}"
        );
    }

    #[test]
    fn fec_and_plc_produce_signal_not_silence() {
        let mut enc = VoiceEncoder::new(32_000, 30).unwrap();
        let mut dec = VoiceDecoder::new().unwrap();
        let mut frames = Vec::new();
        for tick in 0..10 {
            frames.push(enc.encode(&tone(tick)).unwrap());
        }
        for frame in &frames[..5] {
            dec.decode(frame).unwrap();
        }
        // Frame 5 "lost": reconstruct from frame 6's FEC, then play 6.
        let fec = dec.conceal_with_fec(&frames[6]).unwrap();
        assert!(
            rms(&fec) > 1000.0,
            "FEC reconstruction is near-silent: {}",
            rms(&fec)
        );
        dec.decode(&frames[6]).unwrap();
        // Frame 7 "lost" with no lookahead: PLC extrapolates.
        let plc = dec.conceal().unwrap();
        assert!(
            rms(&plc) > 1000.0,
            "PLC output is near-silent: {}",
            rms(&plc)
        );
    }
}
