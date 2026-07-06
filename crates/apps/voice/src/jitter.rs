//! The receive-side jitter buffer: absorbs network jitter and reordering,
//! decides per 20 ms playout tick what the decoder should do.
//!
//! [`JitterBuffer`] is the seam — the engine only speaks this interface, so
//! a NetEQ-grade implementation (e.g. the `neteq` crate) can replace
//! [`MinimalJitter`] without touching the engine. The minimal
//! implementation is deliberately simple: fixed prefill that grows on
//! underrun (bounded), seq-driven, no time-stretching.

use std::collections::BTreeMap;

/// What the playout tick should do for one speaker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayoutStep {
    /// Still prefilling (or refilling after an underrun): play silence,
    /// not concealment — there is no decoder state worth extrapolating.
    Buffering,
    /// The expected frame arrived: decode it.
    Frame(Vec<u8>),
    /// The expected frame is lost but its successor is here: reconstruct
    /// from the successor's in-band FEC. The successor stays buffered and
    /// plays in its own slot.
    ConcealWithNext(Vec<u8>),
    /// Lost with no lookahead: packet-loss concealment.
    Conceal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JitterStats {
    pub played: u64,
    pub concealed_fec: u64,
    pub concealed_plc: u64,
    /// Late arrivals (slot already played) plus overflow evictions.
    pub late_dropped: u64,
    pub underruns: u64,
    /// Current prefill target, frames — grows on underrun up to the max.
    pub depth_frames: usize,
}

pub trait JitterBuffer: Send {
    fn insert(&mut self, seq: u16, payload: Vec<u8>);
    /// Called exactly once per 20 ms playout tick.
    fn tick(&mut self) -> PlayoutStep;
    fn stats(&self) -> JitterStats;
}

/// Hard cap on buffered frames — a stalled playout must not accumulate
/// unbounded audio (128 frames ≈ 2.5 s).
const MAX_BUFFERED: usize = 128;

pub struct MinimalJitter {
    /// Buffered frames keyed by unwrapped stream position.
    packets: BTreeMap<u64, Vec<u8>>,
    /// Highest (seq, position) seen — the wrap-unwrapping anchor.
    highest: Option<(u16, u64)>,
    /// Next position to play; `None` until the first fill completes.
    floor: Option<u64>,
    prefill: usize,
    max_depth: usize,
    filling: bool,
    stats: JitterStats,
}

impl MinimalJitter {
    pub fn new(prefill_frames: usize, max_depth_frames: usize) -> Self {
        let prefill = prefill_frames.max(1);
        MinimalJitter {
            packets: BTreeMap::new(),
            highest: None,
            floor: None,
            prefill,
            max_depth: max_depth_frames.max(prefill),
            filling: true,
            stats: JitterStats {
                depth_frames: prefill,
                ..JitterStats::default()
            },
        }
    }

    /// Unwrap a wire seq to a monotonically comparable stream position.
    fn unwrap_seq(&mut self, seq: u16) -> u64 {
        match self.highest {
            None => {
                // Anchor far from zero so early reordering can't underflow.
                let pos = u64::from(u16::MAX) + 1;
                self.highest = Some((seq, pos));
                pos
            }
            Some((high_seq, high_pos)) => {
                let delta = i64::from(seq.wrapping_sub(high_seq) as i16);
                let pos = high_pos.saturating_add_signed(delta);
                if delta > 0 {
                    self.highest = Some((seq, pos));
                }
                pos
            }
        }
    }
}

impl JitterBuffer for MinimalJitter {
    fn insert(&mut self, seq: u16, payload: Vec<u8>) {
        let pos = self.unwrap_seq(seq);
        if let Some(floor) = self.floor
            && pos < floor
        {
            self.stats.late_dropped += 1;
            return;
        }
        self.packets.insert(pos, payload);
        if self.packets.len() > MAX_BUFFERED {
            self.packets.pop_first();
            self.stats.late_dropped += 1;
        }
    }

    fn tick(&mut self) -> PlayoutStep {
        if self.filling {
            if self.packets.len() < self.prefill {
                return PlayoutStep::Buffering;
            }
            self.filling = false;
            // All buffered positions are >= any previous floor (late guard),
            // so resuming from the earliest buffered frame never replays.
            self.floor = self.packets.keys().next().copied();
        }
        let expected = self.floor.expect("floor set when not filling");
        self.floor = Some(expected + 1);
        if let Some(payload) = self.packets.remove(&expected) {
            self.stats.played += 1;
            return PlayoutStep::Frame(payload);
        }
        if let Some(next) = self.packets.get(&(expected + 1)) {
            self.stats.concealed_fec += 1;
            return PlayoutStep::ConcealWithNext(next.clone());
        }
        if self.packets.is_empty() {
            // Underrun: conceal this tick, then rebuild a deeper cushion.
            self.stats.underruns += 1;
            self.prefill = (self.prefill + 1).min(self.max_depth);
            self.stats.depth_frames = self.prefill;
            self.filling = true;
        }
        self.stats.concealed_plc += 1;
        PlayoutStep::Conceal
    }

    fn stats(&self) -> JitterStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(n: u16) -> Vec<u8> {
        vec![n as u8]
    }

    #[test]
    fn plays_in_order_after_prefill() {
        let mut jb = MinimalJitter::new(2, 6);
        jb.insert(0, frame(0));
        assert_eq!(jb.tick(), PlayoutStep::Buffering);
        jb.insert(1, frame(1));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(0)));
        jb.insert(2, frame(2));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(1)));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(2)));
        assert_eq!(jb.stats().played, 3);
    }

    #[test]
    fn reordered_arrival_is_absorbed() {
        let mut jb = MinimalJitter::new(2, 6);
        jb.insert(1, frame(1));
        jb.insert(0, frame(0));
        jb.insert(3, frame(3));
        jb.insert(2, frame(2));
        for n in 0..4 {
            assert_eq!(jb.tick(), PlayoutStep::Frame(frame(n)));
        }
    }

    #[test]
    fn isolated_loss_uses_fec_from_successor() {
        let mut jb = MinimalJitter::new(2, 6);
        jb.insert(0, frame(0));
        jb.insert(1, frame(1));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(0)));
        // seq 2 lost; 3 arrives.
        jb.insert(3, frame(3));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(1)));
        assert_eq!(jb.tick(), PlayoutStep::ConcealWithNext(frame(3)));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(3)));
        let stats = jb.stats();
        assert_eq!((stats.concealed_fec, stats.concealed_plc), (1, 0));
    }

    #[test]
    fn multi_frame_gap_falls_back_to_plc() {
        let mut jb = MinimalJitter::new(1, 6);
        jb.insert(0, frame(0));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(0)));
        // 1 and 2 lost; 3 buffered.
        jb.insert(3, frame(3));
        assert_eq!(jb.tick(), PlayoutStep::Conceal);
        assert_eq!(jb.tick(), PlayoutStep::ConcealWithNext(frame(3)));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(3)));
    }

    #[test]
    fn underrun_grows_depth_and_refills() {
        let mut jb = MinimalJitter::new(1, 3);
        jb.insert(0, frame(0));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(0)));
        // Nothing buffered: underrun → conceal, then deeper prefill.
        assert_eq!(jb.tick(), PlayoutStep::Conceal);
        assert_eq!(jb.stats().underruns, 1);
        assert_eq!(jb.stats().depth_frames, 2);
        jb.insert(5, frame(5));
        assert_eq!(jb.tick(), PlayoutStep::Buffering);
        jb.insert(6, frame(6));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(5)));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(6)));
    }

    #[test]
    fn late_packet_is_dropped_not_replayed() {
        let mut jb = MinimalJitter::new(1, 6);
        jb.insert(0, frame(0));
        jb.insert(1, frame(1));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(0)));
        assert_eq!(jb.tick(), PlayoutStep::Frame(frame(1)));
        jb.insert(0, frame(0));
        assert_eq!(jb.stats().late_dropped, 1);
    }

    #[test]
    fn seq_wraparound_is_seamless() {
        let mut jb = MinimalJitter::new(1, 6);
        let mut expected = Vec::new();
        for i in 0..8u32 {
            let seq = 65_533u16.wrapping_add(i as u16);
            jb.insert(seq, frame(seq));
            expected.push(seq);
        }
        for seq in expected {
            assert_eq!(jb.tick(), PlayoutStep::Frame(frame(seq)));
        }
        assert_eq!(jb.stats().played, 8);
    }
}
