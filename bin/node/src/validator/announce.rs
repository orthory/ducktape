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
    /// `SignalReady` and latch it. returns the `(name, to_version)` to signal, or
    /// `None`. truthful (binary can execute `to_version`), member-gated (self is a
    /// current boundary member), and idempotent (module already holds our signal, or
    /// one is already in flight).
    pub(crate) fn decide(&mut self, status: &upgrade::UpgradeStatus) -> Option<(String, u32)> {
        let pending = status.pending.as_ref()?;
        // never lie: a binary that cannot execute the target version stays silent so
        // the boundary cleanly aborts rather than arming onto an under-versioned node.
        if pending.to_version > self.max_version {
            return None;
        }
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
        Some((pending.name.clone(), pending.to_version))
    }

    /// query committed upgrade state and, when a signal is due, build the
    /// validator-origin `SignalReady` op. gracefully `None` when the module is
    /// absent (pre-retrofit) or the reply is unreadable — no panic on a baseline net.
    pub(crate) async fn maybe_signal(&mut self, host: &Host) -> Option<(Msg, String, u32)> {
        use upgrade::{
            UpgradeMsg, UpgradeQuery, UpgradeReply, decode_reply, encode_msg, encode_query,
        };
        let reply = host
            .query("upgrade", &encode_query(&UpgradeQuery::Status))
            .await
            .ok()?;
        let UpgradeReply::Status(status) = decode_reply(&reply).ok()?;
        let (name, to_version) = self.decide(&status)?;
        let msg = Msg {
            target: "upgrade".into(),
            payload: encode_msg(&UpgradeMsg::SignalReady {
                name: name.clone(),
                to_version,
                commitment: None,
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
    /// the set we last SUBMITTED (not yet observed committed), latched so an
    /// in-flight announce is not re-sent every tick.
    pub(crate) announced: Option<Vec<String>>,
}

impl CapabilityAnnouncer {
    pub(crate) fn new(me: Vec<u8>, capabilities: Vec<String>) -> Self {
        Self {
            me,
            capabilities,
            announced: None,
        }
    }

    /// the PURE decision core: given this node's committed announced set,
    /// decide whether to (re)announce. `None` when the registry already matches
    /// what we'd announce, or an identical announce is already in flight.
    pub(crate) fn decide(&mut self, committed: &[String]) -> Option<Vec<String>> {
        // nothing to provide and nothing recorded: stay silent (genesis state).
        if self.capabilities.is_empty() && committed.is_empty() {
            return None;
        }
        // the registry already reflects our providers — nothing to do.
        if committed == self.capabilities.as_slice() {
            self.announced = None;
            return None;
        }
        // an announce for this exact set is already in flight.
        if self.announced.as_deref() == Some(self.capabilities.as_slice()) {
            return None;
        }
        self.announced = Some(self.capabilities.clone());
        Some(self.capabilities.clone())
    }

    /// query this node's committed capability set and, when an announce is due,
    /// build the external-origin `Announce` op. gracefully `None` when the
    /// module is absent (pre-retrofit net) or the reply is unreadable.
    pub(crate) async fn maybe_announce(&mut self, host: &Host) -> Option<Msg> {
        use capability::{
            CapabilityMsg, CapabilityQuery, CapabilityReply, decode_reply, encode_msg, encode_query,
        };
        let reply = host
            .query(
                "capability",
                &encode_query(&CapabilityQuery::Node {
                    node: self.me.clone(),
                }),
            )
            .await
            .ok()?;
        let CapabilityReply::Node(committed) = decode_reply(&reply).ok()? else {
            return None;
        };
        let capabilities = self.decide(&committed)?;
        Some(Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::Announce {
                capabilities,
                resources: Default::default(),
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
