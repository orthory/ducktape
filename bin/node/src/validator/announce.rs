use std::collections::BTreeMap;

use host::Host;
use sdk::Msg;

/// the node-local worker that self-emits a validator-origin `SignalReady` op
/// ONCE per pending upgrade this binary can execute. deliberately NOT a
/// `host::worker::Worker`: readiness must survive restart/late-join, so it polls the
/// COMMITTED upgrade state each pump tick and re-derives its decision idempotently
/// rather than reacting to a one-shot block effect. "ready" is a truthful machine
/// statement about the running binary — it signals iff `MAX_PROTOCOL_VERSION >=
/// to_version` (never a version it cannot execute).
pub(crate) struct ReadinessSignaller {
    /// the highest protocol version this binary can execute (`MAX_PROTOCOL_VERSION`).
    max_version: u32,
    /// this node's own validator pubkey bytes — the readiness identity.
    me: Vec<u8>,
    /// the `(name, to_version)` we have already emitted a signal for, latched so a
    /// signal in flight (not yet committed into the module's `ready` set, several
    /// ticks out) is not re-emitted every pump tick (risk R10 — local dedupe atop
    /// module idempotence).
    pub(crate) signaled: Option<(String, u32)>,
}

impl ReadinessSignaller {
    pub(crate) fn new(max_version: u32, me: Vec<u8>) -> Self {
        Self {
            max_version,
            me,
            signaled: None,
        }
    }

    /// the PURE decision core: given the committed status, decide whether to emit a
    /// `SignalReady` and latch it. returns the `(name, to_version, commitment)` to
    /// signal, or `None`. truthful (binary can execute the numeric version and any
    /// named route), member-gated, and idempotent.
    pub(crate) fn decide(
        &mut self,
        status: &lifecycle::UpgradeStatus,
    ) -> Option<(String, u32, Option<Vec<u8>>)> {
        let pending = status.pending.as_ref()?;
        // never lie: a binary that cannot execute the target version stays silent so
        // the boundary cleanly aborts rather than arming onto an under-versioned node.
        if pending.to_version > self.max_version {
            return None;
        }
        let commitment = match lifecycle::required_readiness_commitment(&pending.name) {
            Some(expected) if pending.name == crate::constants::CLIENTS_MODULE_UPGRADE_NAME => {
                Some(expected.to_vec())
            }
            Some(_) => return None,
            None => None,
        };
        // only a CURRENT boundary member is in the readiness denominator (R = n).
        if !status.members.iter().any(|m| m == &self.me) {
            return None;
        }
        // the module already recorded our (committed) signal — nothing to do.
        if status.ready.iter().any(|k| k == &self.me) {
            return None;
        }
        // a signal for this exact upgrade is already in flight (submitted, awaiting
        // finalization) — do not re-submit every tick.
        if self.signaled.as_ref() == Some(&(pending.name.clone(), pending.to_version)) {
            return None;
        }
        self.signaled = Some((pending.name.clone(), pending.to_version));
        Some((pending.name.clone(), pending.to_version, commitment))
    }

    /// query committed upgrade state and, when a signal is due, build the
    /// validator-origin `SignalReady` op. gracefully `None` when the module is
    /// absent (pre-retrofit) or the reply is unreadable — no panic on a baseline net.
    pub(crate) async fn maybe_signal(&mut self, host: &Host) -> Option<(Msg, String, u32)> {
        use lifecycle::{
            LifecycleMsg, LifecycleQuery, LifecycleReply, decode_reply, encode_msg, encode_query,
        };
        let reply = host
            .query(host::LIFECYCLE_MODULE_ID, &encode_query(&LifecycleQuery::UpgradeStatus))
            .await
            .ok()?;
        let LifecycleReply::UpgradeStatus(status) = decode_reply(&reply).ok()? else {
            return None;
        };
        let (name, to_version, commitment) = self.decide(&status)?;
        let msg = Msg {
            target: host::LIFECYCLE_MODULE_ID.into(),
            payload: encode_msg(&LifecycleMsg::UpgradeReady {
                name: name.clone(),
                to_version,
                commitment,
            }),
        };
        Some((msg, name, to_version))
    }
}

/// the capability self-announcer: the state-driven twin of
/// [`ReadinessSignaller`] for the capability registry. it polls the committed
/// registry each pump tick and, when this node's announced set differs from
/// what discovery found locally, self-submits ONE declarative
/// [`CapabilityMsg::Announce`]. state-driven (survives restart/late-join) and
/// idempotent: once the committed set matches, it stays quiet. a node with no
/// providers announces nothing.
pub(crate) struct CapabilityAnnouncer {
    /// this node's own validator pubkey bytes — the registry identity.
    me: Vec<u8>,
    /// the capability tags discovery found on this host, sorted — the truthful
    /// set to announce. empty means this node provides nothing.
    pub(crate) capabilities: Vec<String>,
    /// the numeric capacity announced ALONGSIDE the tags: probed host totals
    /// for a Podman node, EMPTY for a direct-spawn one (a direct node makes no
    /// capacity promise). Forced empty whenever `capabilities` is empty —
    /// resources-without-tags is a consensus-level reject, never emitted.
    pub(crate) resources: BTreeMap<String, u64>,
    /// the (tags, resources) pair we last SUBMITTED (not yet observed
    /// committed), latched so an in-flight announce is not re-sent every tick.
    pub(crate) announced: Option<(Vec<String>, BTreeMap<String, u64>)>,
}

impl CapabilityAnnouncer {
    pub(crate) fn new(
        me: Vec<u8>,
        capabilities: Vec<String>,
        resources: BTreeMap<String, u64>,
    ) -> Self {
        Self {
            me,
            capabilities,
            resources,
            announced: None,
        }
    }

    /// this announce's resources, with the invariant applied: empty tags carry
    /// no resources (resources-without-tags is a module-level reject).
    fn effective_resources(&self) -> BTreeMap<String, u64> {
        if self.capabilities.is_empty() {
            BTreeMap::new()
        } else {
            self.resources.clone()
        }
    }

    /// the PURE decision core: given this node's committed tags AND resources,
    /// decide whether to (re)announce. re-announces when EITHER the committed
    /// tags or the committed resources differ from what this node would emit.
    /// `None` when the registry already matches, an identical announce is
    /// already in flight, or nothing is provided and nothing is recorded.
    pub(crate) fn decide(
        &mut self,
        committed_tags: &[String],
        committed_resources: &BTreeMap<String, u64>,
    ) -> Option<(Vec<String>, BTreeMap<String, u64>)> {
        let resources = self.effective_resources();
        // nothing to provide and nothing recorded: stay silent (genesis state).
        if self.capabilities.is_empty()
            && committed_tags.is_empty()
            && committed_resources.is_empty()
        {
            return None;
        }
        // the registry already reflects our providers AND resources.
        if committed_tags == self.capabilities.as_slice() && committed_resources == &resources {
            self.announced = None;
            return None;
        }
        // an announce for this exact pair is already in flight.
        if self.announced.as_ref() == Some(&(self.capabilities.clone(), resources.clone())) {
            return None;
        }
        self.announced = Some((self.capabilities.clone(), resources.clone()));
        Some((self.capabilities.clone(), resources))
    }

    /// query this node's committed capability set AND resources and, when an
    /// announce is due, build the external-origin `Announce` op. gracefully
    /// `None` when the module is absent (pre-retrofit net) or a reply is
    /// unreadable.
    pub(crate) async fn maybe_announce(&mut self, host: &Host) -> Option<Msg> {
        use capability::{
            CapabilityMsg, CapabilityQuery, CapabilityReply, decode_reply, encode_msg, encode_query,
        };
        let node_reply = host
            .query(
                "capability",
                &encode_query(&CapabilityQuery::Node {
                    node: self.me.clone(),
                }),
            )
            .await
            .ok()?;
        let CapabilityReply::Node(committed_tags) = decode_reply(&node_reply).ok()? else {
            return None;
        };
        let res_reply = host
            .query(
                "capability",
                &encode_query(&CapabilityQuery::Resources {
                    node: self.me.clone(),
                }),
            )
            .await
            .ok()?;
        let CapabilityReply::Resources(committed_resources) = decode_reply(&res_reply).ok()? else {
            return None;
        };
        let (capabilities, resources) = self.decide(&committed_tags, &committed_resources)?;
        Some(Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::Announce {
                capabilities,
                resources,
            }),
        })
    }
}

/// the committed dispatch mailbox's undelivered-result count — the nudge
/// pump's read. `0` when the module is absent or the mailbox is empty.
pub(crate) async fn dispatch_pending_deliveries(host: &Host) -> u64 {
    use dispatch::{DispatchQuery, DispatchReply, decode_reply, encode_query};
    let Ok(reply) = host
        .query("dispatch", &encode_query(&DispatchQuery::PendingDeliveries))
        .await
    else {
        return 0;
    };
    match decode_reply(&reply) {
        Ok(DispatchReply::PendingDeliveries(n)) => n,
        _ => 0,
    }
}

/// the committed saga ledger's earliest pending lease-expiry/deadline — the
/// crank pump's read. `None` when the module is absent or nothing pending
/// carries one.
pub(crate) async fn saga_next_expiry(host: &Host) -> Option<u64> {
    use saga::{SagaQuery, SagaReply, decode_reply, encode_query};
    let reply = host
        .query("saga", &encode_query(&SagaQuery::NextExpiry))
        .await
        .ok()?;
    match decode_reply(&reply).ok()? {
        SagaReply::NextExpiry(v) => v,
        _ => None,
    }
}

#[cfg(test)]
mod capability_announcer_tests {
    use super::*;

    fn tags(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn caps(cores: u64) -> BTreeMap<String, u64> {
        BTreeMap::from([("cores".to_string(), cores)])
    }

    #[test]
    fn re_announces_when_only_resources_drift() {
        // a Podman node: tags already committed, but its announced capacity
        // differs from what the registry holds → re-announce the pair.
        let mut a = CapabilityAnnouncer::new(vec![1u8; 32], tags(&["codex"]), caps(8));
        assert_eq!(
            a.decide(&tags(&["codex"]), &caps(4)),
            Some((tags(&["codex"]), caps(8))),
            "matching tags but drifted resources still re-announce"
        );
        // once the registry reflects both, it goes quiet.
        assert_eq!(a.decide(&tags(&["codex"]), &caps(8)), None);
    }

    #[test]
    fn a_direct_backend_announcer_carries_no_resources() {
        // empty capacity (direct spawn): the announce is tags-only, and it
        // never emits resources even when told the registry is bare.
        let mut a = CapabilityAnnouncer::new(vec![2u8; 32], tags(&["codex"]), BTreeMap::new());
        let (announced_tags, announced_res) = a
            .decide(&[], &BTreeMap::new())
            .expect("a fresh direct node announces its tags");
        assert_eq!(announced_tags, tags(&["codex"]));
        assert!(announced_res.is_empty(), "direct: never any resources");
    }

    #[test]
    fn empty_tags_force_empty_resources() {
        // resources-without-tags is a consensus-level reject: even if a
        // Podman node somehow discovered no executors, it never emits the
        // resources-only shape (and with nothing recorded, it stays silent).
        let mut a = CapabilityAnnouncer::new(vec![3u8; 32], Vec::new(), caps(8));
        assert_eq!(
            a.decide(&[], &BTreeMap::new()),
            None,
            "no tags + nothing recorded: genesis silence"
        );
    }

    #[test]
    fn the_in_flight_latch_covers_the_pair() {
        let mut a = CapabilityAnnouncer::new(vec![4u8; 32], tags(&["codex"]), caps(8));
        // first decide latches the pair.
        assert_eq!(
            a.decide(&[], &BTreeMap::new()),
            Some((tags(&["codex"]), caps(8)))
        );
        // an identical decision while it is still in flight stays quiet.
        assert_eq!(
            a.decide(&[], &BTreeMap::new()),
            None,
            "the latch dedups the exact (tags, resources) pair"
        );
    }
}
