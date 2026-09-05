//! End-to-end proofs: full voice engines over the data-plane sim transport
//! under a paused clock. Real Opus encode/decode, real plane admission and
//! demux, virtual network with deterministic jitter and loss.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use data_plane::sim::{LinkModel, SimNet};
use data_plane::{
    AdmissionPolicy, DataPlane, DatagramFlow, DatagramPolicy, FlowId, PeerId, PlaneConfig, Service,
    sim::SimEndpoint,
};
use media_service::voice::{
    FRAME_SAMPLES, MediaHeader, SAMPLE_RATE, VoiceConfig, VoiceEncoder, VoiceEngine, media,
};
use tokio::time::sleep;

fn peer(n: u8) -> PeerId {
    PeerId([n; 32])
}

/// Test stand-in for the node layer's consensus-derived admission view.
#[derive(Default)]
struct TestAdmission {
    allowed: Mutex<HashSet<(PeerId, Service, u64)>>,
}

impl TestAdmission {
    fn allow(&self, peer: PeerId, service: Service, flow: FlowId) {
        self.allowed
            .lock()
            .unwrap()
            .insert((peer, service, flow.as_u64()));
    }
}

impl AdmissionPolicy for TestAdmission {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool {
        self.allowed
            .lock()
            .unwrap()
            .contains(&(peer, service, flow.as_u64()))
    }
}

fn tone(freq_hz: f32, tick: usize) -> [i16; FRAME_SAMPLES] {
    let mut pcm = [0i16; FRAME_SAMPLES];
    for (i, sample) in pcm.iter_mut().enumerate() {
        let t = (tick * FRAME_SAMPLES + i) as f32 / SAMPLE_RATE as f32;
        *sample = ((t * freq_hz * std::f32::consts::TAU).sin() * 8000.0) as i16;
    }
    pcm
}

fn rms(pcm: &[i16]) -> f64 {
    let sum: f64 = pcm.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum / pcm.len() as f64).sqrt()
}

const TICK: Duration = Duration::from_millis(20);

/// Full mesh of `n` engines over one voice flow; every pair linked with
/// `link`, every member admitted.
fn mesh(n: u8, link: LinkModel, config: VoiceConfig) -> Vec<(PeerId, VoiceEngine<SimEndpoint>)> {
    let net = SimNet::new();
    let peers: Vec<PeerId> = (1..=n).map(peer).collect();
    let flow = FlowId::derive(b"voice-channel:e2e");
    let admission = Arc::new(TestAdmission::default());
    for p in &peers {
        admission.allow(*p, Service::Voice, flow);
    }
    for (i, a) in peers.iter().enumerate() {
        for b in &peers[i + 1..] {
            net.set_link(*a, *b, link);
        }
    }
    peers
        .iter()
        .map(|p| {
            let plane = DataPlane::new(
                net.endpoint(*p),
                admission.clone(),
                PlaneConfig {
                    bulk_bytes_per_sec: 600_000,
                    bulk_burst_bytes: 16 * 1024,
                },
            );
            let flow_handle = plane
                .datagram_flow(Service::Voice, flow, DatagramPolicy { max_queued: 64 })
                .expect("register voice flow");
            (*p, VoiceEngine::new(flow_handle, config).expect("engine"))
        })
        .collect()
}

/// A's engine plus a RAW flow handle for B on the same voice flow, so a test
/// can hand A frames at a chosen wire seq — a fresh session starts at 0, a
/// two-minute-old one is near 6000, and only the raw handle can say which.
fn rejoin_rig() -> (
    VoiceEngine<SimEndpoint>,
    DatagramFlow<SimEndpoint>,
    PeerId,
    PeerId,
) {
    let (a, b) = (peer(1), peer(2));
    let net = SimNet::new();
    net.set_link(
        a,
        b,
        LinkModel {
            latency: Duration::from_millis(5),
            bytes_per_sec: 1_000_000,
            drop_every: None,
            delay_every: None,
        },
    );
    let flow = FlowId::derive(b"voice-channel:rejoin");
    let admission = Arc::new(TestAdmission::default());
    admission.allow(a, Service::Voice, flow);
    admission.allow(b, Service::Voice, flow);
    let plane = |end| {
        DataPlane::new(
            end,
            admission.clone(),
            PlaneConfig {
                bulk_bytes_per_sec: 600_000,
                bulk_burst_bytes: 16 * 1024,
            },
        )
    };
    let flow_a = plane(net.endpoint(a))
        .datagram_flow(Service::Voice, flow, DatagramPolicy { max_queued: 64 })
        .expect("A flow");
    let flow_b = plane(net.endpoint(b))
        .datagram_flow(Service::Voice, flow, DatagramPolicy { max_queued: 64 })
        .expect("B flow");
    (
        VoiceEngine::new(flow_a, test_config()).expect("engine"),
        flow_b,
        a,
        b,
    )
}

/// A peer who leaves and rejoins comes back with a fresh engine whose seq
/// restarts at 0. Until their lane is forgotten, our jitter buffer — anchored
/// at their old high seq — counts every one of those frames late.
#[tokio::test(start_paused = true)]
async fn forgetting_a_departed_peer_admits_their_rejoined_stream() {
    let (engine_a, flow_b, a, b) = rejoin_rig();
    let mut encoder = VoiceEncoder::new(32_000).expect("encoder");
    let payload = encoder.encode(&tone(440.0, 0)).expect("encode");
    let voice = |seq: u16| {
        media::encode_frame(
            MediaHeader {
                seq,
                timestamp: u32::from(seq).wrapping_mul(FRAME_SAMPLES as u32),
            },
            &payload,
        )
        .expect("media frame")
    };

    // B has been speaking for two minutes: their seq is near 6000.
    for seq in 6_000..6_003u16 {
        flow_b.send_to(a, &voice(seq)).await.expect("send");
    }
    sleep(TICK).await;
    for _ in 0..3 {
        engine_a.playout();
    }
    assert_eq!(engine_a.speaker_stats()[0].jitter.played, 3);

    // B leaves and rejoins mid-call; their new session's first frame is seq 0.
    flow_b.send_to(a, &voice(0)).await.expect("send");
    sleep(TICK).await;
    assert_eq!(
        engine_a.speaker_stats()[0].jitter.late_dropped,
        1,
        "a retained lane must reject the restarted stream (the bug's mechanism)"
    );

    // The roster departure evicts the lane; the same frames now land.
    assert!(engine_a.forget_peer(b));
    for seq in 0..4u16 {
        flow_b.send_to(a, &voice(seq)).await.expect("send");
    }
    sleep(TICK).await;
    for _ in 0..4 {
        engine_a.playout();
    }
    let stats = engine_a.speaker_stats();
    assert_eq!(stats.len(), 1, "the rejoiner opened exactly one fresh lane");
    let jitter = stats[0].jitter;
    assert_eq!(
        jitter.late_dropped, 0,
        "rejoined stream still dropped as late: {jitter:?}"
    );
    assert_eq!(jitter.played, 4, "rejoiner inaudible: {jitter:?}");
    assert_eq!(stats[0].decode_errors, 0);
}

fn test_config() -> VoiceConfig {
    VoiceConfig {
        bitrate_bits_per_sec: 32_000,
        prefill_frames: 3,
        max_depth_frames: 8,
    }
}

#[tokio::test(start_paused = true)]
async fn three_speakers_stay_continuous_over_jitter() {
    // Every 5th datagram +35 ms: arrives out of order, 1.75 frames late —
    // inside the 60 ms (3-frame) cushion, so it must be absorbed, not lost.
    let link = LinkModel {
        latency: Duration::from_millis(10),
        bytes_per_sec: 1_000_000,
        drop_every: None,
        delay_every: Some((5, Duration::from_millis(35))),
    };
    let mut party = mesh(3, link, test_config());
    let ids: Vec<PeerId> = party.iter().map(|(p, _)| *p).collect();

    const TICKS: usize = 100;
    const WARMUP: usize = 15;
    let mut quiet_ticks = 0usize;
    for tick in 0..TICKS {
        for (i, (_, engine)) in party.iter_mut().enumerate() {
            let recipients: Vec<PeerId> = ids.iter().copied().filter(|p| *p != ids[i]).collect();
            let pcm = tone(200.0 * (i + 1) as f32, tick);
            engine.send_frame(&pcm, &recipients).await.expect("send");
        }
        sleep(TICK).await;
        for (_, engine) in party.iter_mut() {
            let mixed = engine.playout();
            if tick >= WARMUP && rms(&mixed) < 1_000.0 {
                quiet_ticks += 1;
            }
        }
    }
    // Continuity: after warmup no listener ever hears a dropout.
    assert_eq!(quiet_ticks, 0, "audio dropouts under jitter");
    // And the jitter was genuinely absorbed, not concealed away.
    for (p, engine) in &party {
        for stats in engine.speaker_stats() {
            let j = stats.jitter;
            assert!(j.played >= 90, "{p:?} lane {stats:?}: played {}", j.played);
            assert!(
                j.late_dropped <= 2 && j.gaps <= 2 && j.underruns <= 1,
                "{p:?} lane not absorbing jitter: {j:?}"
            );
            assert_eq!(stats.decode_errors, 0);
        }
        assert_eq!(engine.malformed_frames(), 0);
    }
}

#[tokio::test(start_paused = true)]
async fn loss_produces_bounded_silence_and_stays_aligned() {
    // Deterministic 10% loss, isolated (every 10th datagram). opus-rs has no
    // FEC-decode and no PLC, so a lost frame becomes a silence tick — not a
    // concealed one. The property to prove is that the damage stays bounded
    // to the loss rate and the stream never desyncs: playout keeps tracking
    // the frames that arrived, with no underrun cascade into permanent
    // buffering.
    let link = LinkModel {
        latency: Duration::from_millis(10),
        bytes_per_sec: 1_000_000,
        drop_every: Some(10),
        delay_every: None,
    };
    let mut party = mesh(2, link, test_config());
    let (b_id, _) = party[1];

    const TICKS: usize = 150;
    const WARMUP: usize = 15;
    let mut silence_ticks = 0usize;
    for tick in 0..TICKS {
        let pcm = tone(330.0, tick);
        party[0].1.send_frame(&pcm, &[b_id]).await.expect("send");
        sleep(TICK).await;
        let mixed = party[1].1.playout();
        if tick >= WARMUP && rms(&mixed) < 800.0 {
            silence_ticks += 1;
        }
    }

    let stats = party[1].1.speaker_stats();
    assert_eq!(stats.len(), 1, "exactly one speaker lane");
    let j = stats[0].jitter;
    // ~15 losses over 150 frames: gaps and audible silence track the loss
    // rate, bounded — not amplified into a cascade.
    assert!(
        (10u64..=24).contains(&j.gaps),
        "gaps should track ~10% loss: {j:?}"
    );
    assert!(
        (8..=26).contains(&silence_ticks),
        "silence not bounded to loss: {silence_ticks}"
    );
    // Stream stayed aligned: nearly every arrived frame played, no collapse
    // into permanent buffering.
    assert!(j.played >= 125, "stream desynced, only played {j:?}");
    assert!(j.played + j.gaps >= 140, "playout stalled: {j:?}");
    assert_eq!(stats[0].decode_errors, 0);
}

#[tokio::test(start_paused = true)]
async fn second_speaker_raises_mix_energy() {
    let link = LinkModel {
        latency: Duration::from_millis(10),
        bytes_per_sec: 1_000_000,
        drop_every: None,
        delay_every: None,
    };
    // C only listens; A speaks throughout, B joins halfway.
    let mut party = mesh(3, link, test_config());
    let ids: Vec<PeerId> = party.iter().map(|(p, _)| *p).collect();
    let (a_targets, b_targets) = (vec![ids[1], ids[2]], vec![ids[0], ids[2]]);

    const TICKS: usize = 100;
    let mut listener_rms = Vec::with_capacity(TICKS);
    for tick in 0..TICKS {
        let pcm_a = tone(220.0, tick);
        party[0]
            .1
            .send_frame(&pcm_a, &a_targets)
            .await
            .expect("A send");
        if tick >= 50 {
            let pcm_b = tone(523.0, tick);
            party[1]
                .1
                .send_frame(&pcm_b, &b_targets)
                .await
                .expect("B send");
        }
        sleep(TICK).await;
        listener_rms.push(rms(&party[2].1.playout()));
    }

    let solo: f64 = listener_rms[20..45].iter().sum::<f64>() / 25.0;
    let duet: f64 = listener_rms[70..95].iter().sum::<f64>() / 25.0;
    // Two uncorrelated speakers ≈ double power (~1.41x rms); well above
    // 1.2x proves B's audio is genuinely in C's mix, not just present.
    assert!(
        duet > solo * 1.2,
        "second speaker missing from mix: solo {solo:.0}, duet {duet:.0}"
    );
    assert_eq!(
        party[2].1.speaker_stats().len(),
        2,
        "listener tracks both speakers"
    );
}
