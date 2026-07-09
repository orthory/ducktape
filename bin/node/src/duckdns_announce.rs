//! State-driven local DuckDNS publication announcer. Configuration supplies
//! the desired declarations (without targets); this pump compares them with
//! the committed registry and emits one full declarative replacement only
//! when they differ.

use duckdns::{
    DuckDnsMsg, DuckDnsQuery, DuckDnsReply, ServiceAnnouncement, decode_reply, encode_msg,
    encode_query,
};
use host::Host;
use sdk::Msg;

pub(crate) struct DuckDnsAnnouncer {
    me: Vec<u8>,
    announcements: Vec<ServiceAnnouncement>,
    /// Exact replacement last submitted but not yet observed committed.
    announced: Option<Vec<ServiceAnnouncement>>,
}

impl DuckDnsAnnouncer {
    pub(crate) fn new(me: Vec<u8>, mut announcements: Vec<ServiceAnnouncement>) -> Self {
        announcements.sort();
        announcements.dedup();
        Self {
            me,
            announcements,
            announced: None,
        }
    }

    pub(crate) fn announcements(&self) -> &[ServiceAnnouncement] {
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

    pub(crate) async fn maybe_announce(&mut self, host: &Host) -> Option<Msg> {
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

    pub(crate) fn send_failed(&mut self) {
        self.announced = None;
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
            default_homepage: false,
            allow_cross_site: false,
        }
    }

    #[test]
    fn decision_is_declarative_latched_and_clears_stale_state() {
        let mut with_docs = DuckDnsAnnouncer::new(vec![1; 32], vec![docs()]);
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

        let mut empty = DuckDnsAnnouncer::new(vec![1; 32], vec![]);
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
        let announcer =
            DuckDnsAnnouncer::new(vec![1; 32], vec![status.clone(), docs(), status.clone()]);
        assert_eq!(announcer.announcements(), &[docs(), status]);
    }
}
