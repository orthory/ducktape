//! The engine controller: one encoder for the local speaker, one lane
//! (jitter buffer + stateful decoder) per remote speaker, client-side mix.
//!
//! Wiring: construction spawns a pump that drains the `DatagramFlow` into
//! per-speaker lanes (lanes appear on first frame from a peer — admission
//! already vetted the sender at the plane). The audio clock drives the
//! other side: `send_frame` per captured 20 ms frame, `playout` per output
//! tick. Speakers are keyed by transport-authenticated `PeerId`; the mix is
//! a saturating sum of whatever each lane's jitter buffer decided.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use data_plane::{DataPlaneTransport, DatagramFlow, PeerId, SendError};

use crate::FRAME_SAMPLES;
use crate::codec::{CodecError, VoiceDecoder, VoiceEncoder};
use crate::jitter::{JitterBuffer, JitterStats, MinimalJitter, PlayoutStep};
use crate::media::{self, MediaError, MediaHeader};

#[derive(Clone, Copy, Debug)]
pub struct VoiceConfig {
    pub bitrate_bits_per_sec: i32,
    /// Sizes Opus in-band FEC redundancy; match to observed link loss.
    pub expected_loss_perc: i32,
    /// Initial jitter cushion, frames (x20 ms).
    pub prefill_frames: usize,
    /// Cushion ceiling the buffer may grow to under underruns.
    pub max_depth_frames: usize,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        VoiceConfig {
            bitrate_bits_per_sec: 32_000,
            expected_loss_perc: 10,
            prefill_frames: 2,
            max_depth_frames: 6,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error(transparent)]
    Media(#[from] MediaError),
    #[error(transparent)]
    Send(#[from] SendError),
}

#[derive(Clone, Copy, Debug)]
pub struct SpeakerStats {
    pub peer: PeerId,
    pub jitter: JitterStats,
    pub decode_errors: u64,
}

struct Lane {
    jitter: Box<dyn JitterBuffer>,
    decoder: VoiceDecoder,
    decode_errors: u64,
}

type Lanes = Arc<Mutex<HashMap<PeerId, Lane>>>;
type JitterFactory = Arc<dyn Fn() -> Box<dyn JitterBuffer> + Send + Sync>;

/// One voice channel's engine over one data-plane datagram flow.
pub struct VoiceEngine<T: DataPlaneTransport> {
    flow: Arc<DatagramFlow<T>>,
    encoder: VoiceEncoder,
    seq: u16,
    timestamp: u32,
    lanes: Lanes,
    malformed: Arc<AtomicU64>,
}

impl<T: DataPlaneTransport> VoiceEngine<T> {
    pub fn new(flow: DatagramFlow<T>, config: VoiceConfig) -> Result<Self, CodecError> {
        let factory: JitterFactory = Arc::new(move || {
            Box::new(MinimalJitter::new(
                config.prefill_frames,
                config.max_depth_frames,
            ))
        });
        Self::with_jitter(flow, config, factory)
    }

    /// Same engine, custom jitter buffer (the NetEQ drop-in seam).
    pub fn with_jitter(
        flow: DatagramFlow<T>,
        config: VoiceConfig,
        jitter_factory: JitterFactory,
    ) -> Result<Self, CodecError> {
        let encoder = VoiceEncoder::new(config.bitrate_bits_per_sec, config.expected_loss_perc)?;
        let flow = Arc::new(flow);
        let lanes: Lanes = Arc::new(Mutex::new(HashMap::new()));
        let malformed = Arc::new(AtomicU64::new(0));
        tokio::spawn(pump(
            flow.clone(),
            lanes.clone(),
            jitter_factory,
            malformed.clone(),
        ));
        Ok(VoiceEngine {
            flow,
            encoder,
            seq: 0,
            timestamp: 0,
            lanes,
            malformed,
        })
    }

    /// Encode one captured 20 ms frame and fan it out to the channel's
    /// other members. All recipients are attempted; the first failure is
    /// reported after the fan-out completes.
    pub async fn send_frame(
        &mut self,
        pcm: &[i16; FRAME_SAMPLES],
        recipients: &[PeerId],
    ) -> Result<(), EngineError> {
        let payload = self.encoder.encode(pcm)?;
        let frame = media::encode_frame(
            MediaHeader {
                seq: self.seq,
                timestamp: self.timestamp,
            },
            &payload,
        )?;
        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(FRAME_SAMPLES as u32);
        let mut first_error = None;
        for recipient in recipients {
            if let Err(error) = self.flow.send_to(*recipient, &frame).await
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), |error| Err(error.into()))
    }

    /// One 20 ms output tick: step every speaker's jitter buffer, decode or
    /// conceal, and mix. Call at the frame cadence; returns silence while
    /// no speaker has playable audio.
    pub fn playout(&self) -> [i16; FRAME_SAMPLES] {
        let mut mix = [0i32; FRAME_SAMPLES];
        let mut lanes = self.lanes.lock().expect("lanes lock");
        for lane in lanes.values_mut() {
            let decoded = match lane.jitter.tick() {
                PlayoutStep::Buffering => None,
                PlayoutStep::Frame(payload) => Some(lane.decoder.decode(&payload)),
                PlayoutStep::ConcealWithNext(next) => Some(lane.decoder.conceal_with_fec(&next)),
                PlayoutStep::Conceal => Some(lane.decoder.conceal()),
            };
            let pcm = match decoded {
                None => continue,
                Some(Ok(pcm)) => pcm,
                Some(Err(_)) => {
                    // A peer's undecodable payload must not silence the rest
                    // of the mix: count it and keep going.
                    lane.decode_errors += 1;
                    continue;
                }
            };
            for (mixed, sample) in mix.iter_mut().zip(pcm) {
                *mixed += i32::from(sample);
            }
        }
        let mut out = [0i16; FRAME_SAMPLES];
        for (out_sample, mixed) in out.iter_mut().zip(mix) {
            *out_sample = mixed.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
        out
    }

    pub fn speaker_stats(&self) -> Vec<SpeakerStats> {
        self.lanes
            .lock()
            .expect("lanes lock")
            .iter()
            .map(|(peer, lane)| SpeakerStats {
                peer: *peer,
                jitter: lane.jitter.stats(),
                decode_errors: lane.decode_errors,
            })
            .collect()
    }

    /// Datagrams that failed media decoding (bad version/truncated).
    pub fn malformed_frames(&self) -> u64 {
        self.malformed.load(Ordering::Relaxed)
    }
}

/// Drain the flow into per-speaker lanes. Runs for the engine's lifetime;
/// admission and flow demux already happened at the plane.
async fn pump<T: DataPlaneTransport>(
    flow: Arc<DatagramFlow<T>>,
    lanes: Lanes,
    jitter_factory: JitterFactory,
    malformed: Arc<AtomicU64>,
) {
    loop {
        let (peer, bytes) = flow.recv().await;
        let Ok((header, payload)) = media::decode_frame(&bytes) else {
            malformed.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let mut lanes = lanes.lock().expect("lanes lock");
        if !lanes.contains_key(&peer) {
            let Ok(decoder) = VoiceDecoder::new() else {
                malformed.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            lanes.insert(
                peer,
                Lane {
                    jitter: jitter_factory(),
                    decoder,
                    decode_errors: 0,
                },
            );
        }
        let lane = lanes.get_mut(&peer).expect("lane just ensured");
        lane.jitter.insert(header.seq, payload.to_vec());
    }
}
