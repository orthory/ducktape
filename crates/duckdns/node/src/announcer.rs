//! State-driven DuckDNS service announcer. Configuration supplies the desired
//! identity-only declarations; this pump compares them with
//! the committed registry and emits one full declarative replacement only
//! when they differ.

use std::time::{Duration, Instant};

use duckdns::{
    DuckDnsMsg, DuckDnsQuery, DuckDnsReply, ServiceAnnouncement, decode_reply, encode_msg,
    encode_query,
};
use host::Host;
use sdk::Msg;

const ANNOUNCE_RETRY: Duration = Duration::from_secs(15);

pub struct Announcer {
    me: Vec<u8>,
    announcements: Vec<ServiceAnnouncement>,
    /// Exact replacement last submitted but not yet observed committed.
    announced: Option<Vec<ServiceAnnouncement>>,
}

impl Announcer {
    pub fn new(me: Vec<u8>, mut announcements: Vec<ServiceAnnouncement>) -> Self {
        announcements.sort();
        announcements.dedup();
        Self {
            me,
            announcements,
            announced: None,
        }
    }

    pub fn announcements(&self) -> &[ServiceAnnouncement] {
        &self.announcements
    }

    fn decide(&mut self, committed: &[ServiceAnnouncement]) -> Option<Vec<ServiceAnnouncement>> {
        if committed == self.announcements.as_slice() {
            self.announced = None;
            return None;
        }
        if self.announced.as_deref() == Some(self.announcements.as_slice()) {
            return None;
        }
        self.announced = Some(self.announcements.clone());
        Some(self.announcements.clone())
    }

    pub async fn maybe_announce(&mut self, host: &Host) -> Option<Msg> {
        let reply = host
            .query(
                "duckdns",
                &encode_query(&DuckDnsQuery::NodeAnnouncements {
                    node: self.me.clone(),
                }),
            )
            .await
            .ok()?;
        let DuckDnsReply::NodeAnnouncements(committed) = decode_reply(&reply).ok()? else {
            return None;
        };
        let announcements = self.decide(&committed)?;
        Some(Msg {
            target: "duckdns".into(),
            payload: encode_msg(&DuckDnsMsg::ReplaceAnnouncements { announcements }),
        })
    }

    pub fn send_failed(&mut self) {
        self.announced = None;
    }
}

/// Resident delivery wrapper for the declarative announcer. Residents submit
/// over the relay lane, so an in-flight frame is deadline-latched until its
/// asynchronous consensus fate arrives.
pub struct ResidentAnnouncer {
    announcer: Announcer,
    in_flight: Option<(node::FrameId, Instant)>,
}

impl ResidentAnnouncer {
    pub fn new(me: Vec<u8>, announcements: Vec<ServiceAnnouncement>) -> Self {
        Self {
            announcer: Announcer::new(me, announcements),
            in_flight: None,
        }
    }

    pub fn announcements(&self) -> &[ServiceAnnouncement] {
        self.announcer.announcements()
    }

    pub async fn maybe_announce(&mut self, host: &Host, now: Instant) -> Option<Msg> {
        self.rearm_if_stale(now);
        if self.in_flight.is_some() {
            return None;
        }
        self.announcer.maybe_announce(host).await
    }

    pub fn sent(&mut self, frame: node::FrameId, now: Instant) {
        self.in_flight = Some((frame, now + ANNOUNCE_RETRY));
    }

    pub fn send_failed(&mut self) {
        self.in_flight = None;
        self.announcer.send_failed();
    }

    pub fn on_reply(&mut self, frame: &node::FrameId, applied: bool) -> Option<bool> {
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
    use duckdns::ServiceScope;

    fn docs() -> ServiceAnnouncement {
        ServiceAnnouncement {
            scope: ServiceScope::Network,
            service: "docs".into(),
        }
    }

    #[test]
    fn decision_is_declarative_latched_and_clears_stale_state() {
        let mut with_docs = Announcer::new(vec![1; 32], vec![docs()]);
        assert_eq!(with_docs.decide(&[]), Some(vec![docs()]));
        assert_eq!(
            with_docs.decide(&[]),
            None,
            "identical replacement is in flight"
        );
        assert_eq!(
            with_docs.decide(&[docs()]),
            None,
            "committed match is quiet"
        );

        let mut empty = Announcer::new(vec![1; 32], vec![]);
        assert_eq!(
            empty.decide(&[docs()]),
            Some(vec![]),
            "removing config clears stale replicated declarations"
        );
    }

    #[test]
    fn desired_declarations_are_sorted_and_deduplicated() {
        let mut status = docs();
        status.service = "status".into();
        let announcer = Announcer::new(vec![1; 32], vec![status.clone(), docs(), status.clone()]);
        assert_eq!(announcer.announcements(), &[docs(), status]);
    }
}
