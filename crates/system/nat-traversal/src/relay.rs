use std::net::SocketAddr;

use crate::Side;

/// One relay session's opaque UDP splice. The coordinator relay learns each
/// side's source address on that side's first datagram (learn-on-first), then
/// forwards every subsequent OPAQUE datagram to the other side verbatim. It
/// holds only the two learned `SocketAddr`s and the two egress addresses —
/// never a key, never plaintext, never the datagram's meaning. Bounded by an
/// idle timeout via `last_activity`.
pub struct RelaySplice {
    a_egress: SocketAddr,
    b_egress: SocketAddr,
    a_src: Option<SocketAddr>,
    b_src: Option<SocketAddr>,
    last_activity: u64,
}

/// A datagram the splice wants to emit. `from` is the relay egress port the
/// datagram leaves from (the other side's port); `to` is the learned
/// destination; `payload` is forwarded byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forward {
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub payload: Vec<u8>,
}

impl RelaySplice {
    pub fn new(a_egress: SocketAddr, b_egress: SocketAddr, now: u64) -> Self {
        Self {
            a_egress,
            b_egress,
            a_src: None,
            b_src: None,
            last_activity: now,
        }
    }

    /// A datagram arrived on `side`'s relay socket from `src` carrying opaque
    /// `payload`. Learn the source on the first datagram, then PIN it: once a
    /// side's source is known, a datagram from any other source on that side is
    /// an injection/hijack attempt (a third party spoofing into an established
    /// session) and is dropped — without overwriting the learned source or even
    /// touching `last_activity`, so a spoofer can neither redirect the far
    /// side's traffic to itself nor keep a torn-down session alive. If the
    /// datagram is accepted and the other side's source is known, return the
    /// `Forward` to emit toward it; until the other side has sent at least once
    /// there is nowhere to forward, so returns `None` (real WireGuard
    /// retransmits).
    pub fn ingress(
        &mut self,
        side: Side,
        src: SocketAddr,
        now: u64,
        payload: Vec<u8>,
    ) -> Option<Forward> {
        match side {
            Side::A => {
                if matches!(self.a_src, Some(pinned) if pinned != src) {
                    return None;
                }
                self.a_src = Some(src);
                self.last_activity = now;
                self.b_src.map(|to| Forward {
                    from: self.b_egress,
                    to,
                    payload,
                })
            }
            Side::B => {
                if matches!(self.b_src, Some(pinned) if pinned != src) {
                    return None;
                }
                self.b_src = Some(src);
                self.last_activity = now;
                self.a_src.map(|to| Forward {
                    from: self.a_egress,
                    to,
                    payload,
                })
            }
        }
    }

    pub fn is_idle(&self, now: u64, idle_ticks: u64) -> bool {
        now.saturating_sub(self.last_activity) > idle_ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, o)), p)
    }

    fn relay(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), port)
    }

    #[test]
    fn buffers_until_both_sides_have_sent_then_forwards_verbatim() {
        let a_egress = relay(4000);
        let b_egress = relay(4001);
        let mut s = RelaySplice::new(a_egress, b_egress, 0);
        let a_src = addr(1, 5000);
        let b_src = addr(2, 6000);

        // A's first datagram: B's source not yet known -> nowhere to forward.
        assert_eq!(s.ingress(Side::A, a_src, 1, b"opaque-A".to_vec()), None);

        // B sends: now A's source is known -> forward B's payload to A via a_egress.
        let to_a = s.ingress(Side::B, b_src, 2, b"opaque-B".to_vec()).expect("forward");
        assert_eq!(to_a, Forward { from: a_egress, to: a_src, payload: b"opaque-B".to_vec() });

        // A sends again: B's source now known -> forward A's payload to B via b_egress.
        let to_b = s.ingress(Side::A, a_src, 3, b"opaque-A".to_vec()).expect("forward");
        assert_eq!(to_b, Forward { from: b_egress, to: b_src, payload: b"opaque-A".to_vec() });
    }

    #[test]
    fn payload_is_forwarded_byte_for_byte_never_interpreted() {
        let mut s = RelaySplice::new(relay(4000), relay(4001), 0);
        // A control-message-looking byte sequence must be forwarded verbatim,
        // never decoded: the relay is opaque.
        let looks_like_control = vec![3u8, 0, 0, 0]; // TAG_REGISTER prefix + junk
        s.ingress(Side::A, addr(1, 5000), 1, looks_like_control.clone());
        let f = s.ingress(Side::B, addr(2, 6000), 2, vec![7, 7, 7]).expect("forward");
        // B->A carried [7,7,7] untouched; the A payload is likewise untouched
        // when re-driven.
        assert_eq!(f.payload, vec![7, 7, 7]);
        let f2 = s.ingress(Side::A, addr(1, 5000), 3, looks_like_control.clone()).expect("forward");
        assert_eq!(f2.payload, looks_like_control);
    }

    #[test]
    fn is_idle_after_timeout() {
        let mut s = RelaySplice::new(relay(4000), relay(4001), 0);
        s.ingress(Side::A, addr(1, 5000), 10, b"x".to_vec());
        assert!(!s.is_idle(15, 10));
        assert!(s.is_idle(30, 10)); // 30 - 10 > 10
    }

    #[test]
    fn wrong_source_after_learn_is_dropped_and_does_not_hijack_routing() {
        let a_egress = relay(4000);
        let b_egress = relay(4001);
        let mut s = RelaySplice::new(a_egress, b_egress, 0);
        let a_src = addr(1, 5000);
        let b_src = addr(2, 6000);
        let attacker = addr(9, 9999);

        // Establish the session: A's source learned, then B's — B->A forwards.
        assert_eq!(s.ingress(Side::A, a_src, 1, b"a".to_vec()), None);
        let _ = s
            .ingress(Side::B, b_src, 2, b"b".to_vec())
            .expect("forward to a once both sides learned");

        // A third party sprays A's relay socket from a DIFFERENT source. With
        // source pinning it is dropped: no forward is emitted, and — the crux
        // of the hijack fix — the learned `a_src` is NOT overwritten.
        assert_eq!(
            s.ingress(Side::A, attacker, 3, b"inject".to_vec()),
            None,
            "a datagram from an unlearned source must be dropped, never forwarded"
        );

        // The next legitimate B->A datagram still targets the REAL A, proving
        // the injector did not redirect the session.
        let to_a = s
            .ingress(Side::B, b_src, 4, b"b2".to_vec())
            .expect("forward");
        assert_eq!(
            to_a,
            Forward { from: a_egress, to: a_src, payload: b"b2".to_vec() },
            "B->A must still reach the originally-learned A, not the injector"
        );
    }

    #[test]
    fn wrong_source_datagram_does_not_refresh_activity() {
        let mut s = RelaySplice::new(relay(4000), relay(4001), 0);
        s.ingress(Side::A, addr(1, 5000), 10, b"a".to_vec());
        // A spoofer sprays at t=100 from a wrong source. The datagram is
        // dropped and must NOT refresh `last_activity`, else a spoofer could
        // hold a torn-down session open indefinitely.
        assert_eq!(s.ingress(Side::A, addr(9, 9999), 100, b"x".to_vec()), None);
        assert!(
            s.is_idle(30, 10),
            "wrong-source traffic must not keep the session alive"
        );
    }
}
