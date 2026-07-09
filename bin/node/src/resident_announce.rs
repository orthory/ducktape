//! the RESIDENT-tier capability announce pump.
//!
//! a synced resident (joined → admitted → boundary-following, not yet
//! promoted) provides real executors, but it never runs the validator loop —
//! so the validator-tier [`crate::CapabilityAnnouncer`] pump never fires for
//! it. this wrapper reuses that announcer's state-driven decision core
//! (compare discovery against the COMMITTED registry, announce once, stay
//! quiet when matched) and adapts its delivery to the resident's only write
//! path: the submit-relay lane, where the frame's consensus fate arrives as
//! an asynchronous `RelayMsg::Reply` instead of a local submit result.
//!
//! liveness over the lossy lane is deadline-based: an announce that neither
//! settles in the committed registry nor draws a Reply within
//! [`ANNOUNCE_RETRY`] un-latches, and the next tick re-decides from state —
//! so a dropped frame or a crashed relay target degrades to a retry, never a
//! permanently silent resident. duplicate announces are harmless: the module
//! applies a declarative replace, so a re-send converges on the same state.

use std::time::{Duration, Instant};

use host::Host;
use sdk::Msg;

use crate::CapabilityAnnouncer;
use crate::duckdns_announce::DuckDnsAnnouncer;

/// how long a relayed announce may await its consensus fate before the pump
/// un-latches and re-decides from committed state. comfortably above the
/// relay's own 10s SUBMIT_HOLD, so a swept-but-applied frame usually shows up
/// in the registry before this fires (and the state read stays quiet).
const ANNOUNCE_RETRY: Duration = Duration::from_secs(15);

pub(crate) struct ResidentAnnouncer {
    announcer: CapabilityAnnouncer,
    /// the relayed announce awaiting its fate: the frame's content address
    /// plus the give-up deadline.
    in_flight: Option<(node::FrameId, Instant)>,
}

impl ResidentAnnouncer {
    pub(crate) fn new(me: Vec<u8>, capabilities: Vec<String>) -> Self {
        Self {
            announcer: CapabilityAnnouncer::new(me, capabilities),
            in_flight: None,
        }
    }

    /// the discovered tag set this pump announces — for log lines.
    pub(crate) fn capabilities(&self) -> &[String] {
        &self.announcer.capabilities
    }

    /// query the served boundary's committed registry and decide whether an
    /// announce is due. quiet while a frame is in flight (until its deadline
    /// passes), quiet once the committed set matches discovery.
    pub(crate) async fn maybe_announce(&mut self, host: &Host, now: Instant) -> Option<Msg> {
        self.rearm_if_stale(now);
        if self.in_flight.is_some() {
            return None;
        }
        self.announcer.maybe_announce(host).await
    }

    /// a decided announce left on the relay lane: latch its content address
    /// so the pump stays quiet while the fate is pending.
    pub(crate) fn sent(&mut self, frame: node::FrameId, now: Instant) {
        self.in_flight = Some((frame, now + ANNOUNCE_RETRY));
    }

    /// the relay send itself failed (no validator known / unreachable):
    /// un-latch immediately so the next tick retries.
    pub(crate) fn send_failed(&mut self) {
        self.in_flight = None;
        self.announcer.announced = None;
    }

    /// a validator's relay Reply. `Some(applied)` when the frame was this
    /// pump's announce (the caller logs it), `None` when it belongs to
    /// someone else's hold. a non-applied fate (Rejected / Refused) un-latches
    /// so the next tick retries; an applied one stays latched until the
    /// committed registry confirms (the announcer's own state-driven quiesce).
    pub(crate) fn on_reply(&mut self, frame: &node::FrameId, applied: bool) -> Option<bool> {
        match &self.in_flight {
            Some((id, _)) if id == frame => {}
            _ => return None,
        }
        self.in_flight = None;
        if !applied {
            self.announcer.announced = None;
        }
        Some(applied)
    }

    /// deadline-based liveness: a frame whose fate never arrived (dropped
    /// reply, crashed validator, swept hold) stops blocking after its
    /// deadline — un-latch and let the next `maybe_announce` re-decide from
    /// the committed registry (which is quiet if the announce actually
    /// landed).
    fn rearm_if_stale(&mut self, now: Instant) {
        if let Some((_, deadline)) = &self.in_flight
            && now >= *deadline
        {
            self.in_flight = None;
            self.announcer.announced = None;
        }
    }
}

/// Resident delivery wrapper for the DuckDNS declarative announcer. It shares
/// the same relay fate/deadline discipline as capability announcements.
pub(crate) struct ResidentDuckDnsAnnouncer {
    announcer: DuckDnsAnnouncer,
    in_flight: Option<(node::FrameId, Instant)>,
}

impl ResidentDuckDnsAnnouncer {
    pub(crate) fn new(me: Vec<u8>, announcements: Vec<duckdns::ServiceAnnouncement>) -> Self {
        Self {
            announcer: DuckDnsAnnouncer::new(me, announcements),
            in_flight: None,
        }
    }

    pub(crate) fn announcements(&self) -> &[duckdns::ServiceAnnouncement] {
        self.announcer.announcements()
    }

    pub(crate) async fn maybe_announce(&mut self, host: &Host, now: Instant) -> Option<Msg> {
        self.rearm_if_stale(now);
        if self.in_flight.is_some() {
            return None;
        }
        self.announcer.maybe_announce(host).await
    }

    pub(crate) fn sent(&mut self, frame: node::FrameId, now: Instant) {
        self.in_flight = Some((frame, now + ANNOUNCE_RETRY));
    }

    pub(crate) fn send_failed(&mut self) {
        self.in_flight = None;
        self.announcer.send_failed();
    }

    pub(crate) fn on_reply(&mut self, frame: &node::FrameId, applied: bool) -> Option<bool> {
        match &self.in_flight {
            Some((id, _)) if id == frame => {}
            _ => return None,
        }
        self.in_flight = None;
        if !applied {
            self.announcer.send_failed();
        }
        Some(applied)
    }

    fn rearm_if_stale(&mut self, now: Instant) {
        if let Some((_, deadline)) = &self.in_flight
            && now >= *deadline
        {
            self.send_failed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(byte: u8) -> node::FrameId {
        node::frame_id(&[byte])
    }

    fn pump() -> ResidentAnnouncer {
        let mut p = ResidentAnnouncer::new(vec![1u8; 32], vec!["codex".into()]);
        // simulate a decided announce: the decision core latched the set.
        assert_eq!(
            p.announcer.decide(&[]),
            Some(vec!["codex".to_string()]),
            "an empty committed registry decides an announce"
        );
        p
    }

    #[test]
    fn a_foreign_reply_is_not_ours_and_changes_nothing() {
        let mut p = pump();
        let now = Instant::now();
        p.sent(frame(1), now);
        assert_eq!(p.on_reply(&frame(2), true), None, "not our frame");
        assert!(p.in_flight.is_some(), "the in-flight latch is untouched");
        assert!(p.announcer.announced.is_some(), "the decide latch holds");
    }

    #[test]
    fn an_applied_reply_clears_flight_but_keeps_the_decide_latch() {
        let mut p = pump();
        let now = Instant::now();
        p.sent(frame(1), now);
        assert_eq!(p.on_reply(&frame(1), true), Some(true));
        assert!(p.in_flight.is_none(), "flight settled");
        assert!(
            p.announcer.announced.is_some(),
            "applied: stay latched until the committed registry confirms"
        );
    }

    #[test]
    fn a_rejected_reply_unlatches_for_a_retry() {
        let mut p = pump();
        let now = Instant::now();
        p.sent(frame(1), now);
        assert_eq!(p.on_reply(&frame(1), false), Some(false));
        assert!(p.in_flight.is_none());
        assert!(
            p.announcer.announced.is_none(),
            "rejected: un-latched so the next tick re-decides"
        );
        assert_eq!(
            p.announcer.decide(&[]),
            Some(vec!["codex".to_string()]),
            "and the re-decision announces again"
        );
    }

    #[test]
    fn a_silent_lane_rearms_only_after_the_deadline() {
        let mut p = pump();
        let now = Instant::now();
        p.sent(frame(1), now);

        p.rearm_if_stale(now + ANNOUNCE_RETRY - Duration::from_secs(1));
        assert!(p.in_flight.is_some(), "before the deadline: still waiting");
        assert!(p.announcer.announced.is_some());

        p.rearm_if_stale(now + ANNOUNCE_RETRY);
        assert!(p.in_flight.is_none(), "at the deadline: gave up");
        assert!(
            p.announcer.announced.is_none(),
            "un-latched so the next tick re-decides from committed state"
        );
    }

    #[test]
    fn a_send_failure_unlatches_immediately() {
        let mut p = pump();
        p.sent(frame(1), Instant::now());
        p.send_failed();
        assert!(p.in_flight.is_none());
        assert!(p.announcer.announced.is_none());
    }
}
