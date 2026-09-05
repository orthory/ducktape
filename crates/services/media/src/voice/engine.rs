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
use std::collections::hash_map::Entry;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use data_plane::{DataPlaneTransport, DatagramFlow, PeerId, SendError};

use super::FRAME_SAMPLES;
use super::codec::{CodecError, VoiceDecoder, VoiceEncoder};
use super::jitter::{JitterStats, MinimalJitter, PlayoutStep};
use super::media::{self, MediaError, MediaHeader};

#[derive(Clone, Copy, Debug)]
pub struct VoiceConfig {
    pub bitrate_bits_per_sec: i32,
    /// Initial jitter cushion, frames (x20 ms).
    pub prefill_frames: usize,
    /// Cushion ceiling the buffer may grow to under underruns.
    pub max_depth_frames: usize,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        VoiceConfig {
            bitrate_bits_per_sec: 32_000,
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
    /// Packets this peer sent that the codec boundary refused outright
    /// (off-contract TOC, too short, or an unwind out of the library).
    pub bad_packets: u64,
}

/// A refused packet logs on the first and every hundredth occurrence per
/// lane: a peer can send one per datagram, and a warn per datagram is a log
/// bomb that evicts the ring the operator is reading.
const BAD_PACKET_EVERY: u64 = 100;

struct Lane {
    jitter: MinimalJitter,
    decoder: VoiceDecoder,
    decode_errors: u64,
    bad_packets: u64,
}

impl Lane {
    fn note_bad_packet(&mut self, peer: &PeerId, reason: &'static str) {
        self.bad_packets += 1;
        let occurrences = self.bad_packets;
        if occurrences == 1 || occurrences.is_multiple_of(BAD_PACKET_EVERY) {
            tracing::warn!(
                target: "ducktape::voice",
                reason,
                peer = %peer_label(peer),
                occurrences,
                "refused a peer's opus packet"
            );
        }
    }
}

type Lanes = Arc<Mutex<HashMap<PeerId, Lane>>>;

/// One voice channel's engine over one data-plane datagram flow.
pub struct VoiceEngine<T: DataPlaneTransport> {
    flow: Arc<DatagramFlow<T>>,
    encoder: VoiceEncoder,
    seq: u16,
    timestamp: u32,
    lanes: Lanes,
    malformed: Arc<AtomicU64>,
    /// the pump task; aborted on drop so the pump's flow handle is released
    /// and the (service, flow) slot can be registered again by a later engine.
    pump: tokio::task::JoinHandle<()>,
}

impl<T: DataPlaneTransport> Drop for VoiceEngine<T> {
    fn drop(&mut self) {
        self.pump.abort();
    }
}

impl<T: DataPlaneTransport> VoiceEngine<T> {
    pub fn new(flow: DatagramFlow<T>, config: VoiceConfig) -> Result<Self, CodecError> {
        let encoder = VoiceEncoder::new(config.bitrate_bits_per_sec)?;
        let flow = Arc::new(flow);
        let lanes: Lanes = Arc::new(Mutex::new(HashMap::new()));
        let malformed = Arc::new(AtomicU64::new(0));
        let pump = tokio::spawn(pump(flow.clone(), lanes.clone(), config, malformed.clone()));
        Ok(VoiceEngine {
            flow,
            encoder,
            seq: 0,
            timestamp: 0,
            lanes,
            malformed,
            pump,
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

    /// One 20 ms output tick: step every speaker's jitter buffer, decode
    /// present frames, and mix. A gap contributes silence (the codec offers
    /// no concealment). Call at the frame cadence; returns silence while no
    /// speaker has playable audio.
    pub fn playout(&self) -> [i16; FRAME_SAMPLES] {
        let mut mix = [0i32; FRAME_SAMPLES];
        let mut lanes = self.lanes.lock().expect("lanes lock");
        for (peer, lane) in lanes.iter_mut() {
            let decoded = match lane.jitter.tick() {
                // Buffering and Gap both render as silence: nothing to add.
                PlayoutStep::Buffering | PlayoutStep::Gap => None,
                PlayoutStep::Frame(payload) => Some(lane.decoder.decode(&payload)),
            };
            let pcm = match decoded {
                None => continue,
                Some(Ok(pcm)) => pcm,
                // A peer's undecodable payload must not silence the rest of
                // the mix: count it and keep going. A refused packet is the
                // hostile-or-broken case and says so by reason.
                Some(Err(CodecError::BadPacket(reason))) => {
                    lane.note_bad_packet(peer, reason);
                    continue;
                }
                Some(Err(CodecError::Opus(_))) => {
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

    /// Drop a speaker's lane. Call it when the peer leaves the roster: a
    /// rejoiner builds a FRESH engine whose seq restarts at 0, and a retained
    /// jitter buffer anchored at their old high seq counts every one of those
    /// frames late and discards it — for as many frames as they previously
    /// sent. Their next frame after this opens a clean lane instead.
    /// Returns whether a lane was there to drop.
    pub fn forget_peer(&self, peer: PeerId) -> bool {
        self.lanes
            .lock()
            .expect("lanes lock")
            .remove(&peer)
            .is_some()
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
                bad_packets: lane.bad_packets,
            })
            .collect()
    }

    /// Datagrams that failed media decoding (bad version/truncated).
    pub fn malformed_frames(&self) -> u64 {
        self.malformed.load(Ordering::Relaxed)
    }
}

/// the first 8 hex chars of a peer's key: enough to match a roster entry in
/// a log, never the key.
fn peer_label(peer: &PeerId) -> String {
    peer.0[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// Drain the flow into per-speaker lanes. Runs for the engine's lifetime;
/// admission and flow demux already happened at the plane.
async fn pump<T: DataPlaneTransport>(
    flow: Arc<DatagramFlow<T>>,
    lanes: Lanes,
    config: VoiceConfig,
    malformed: Arc<AtomicU64>,
) {
    loop {
        let (peer, bytes) = flow.recv().await;
        let Ok((header, payload)) = media::decode_frame(&bytes) else {
            malformed.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let mut lanes = lanes.lock().expect("lanes lock");
        let lane = match lanes.entry(peer) {
            Entry::Occupied(existing) => existing.into_mut(),
            Entry::Vacant(vacant) => {
                let Ok(decoder) = VoiceDecoder::new() else {
                    malformed.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                // the peer's first admitted frame: the plane authenticated
                // it, the roster admitted it, and its speaker lane exists
                // from here — once per peer per engine, never per frame.
                tracing::info!(
                    target: "ducktape::voice",
                    event = "voice_peer_handshake_complete",
                    peer = %peer_label(&peer),
                    "first media frame from peer — speaker lane opened"
                );
                vacant.insert(Lane {
                    jitter: MinimalJitter::new(config.prefill_frames, config.max_depth_frames),
                    decoder,
                    decode_errors: 0,
                    bad_packets: 0,
                })
            }
        };
        lane.jitter.insert(header.seq, payload.to_vec());
    }
}
