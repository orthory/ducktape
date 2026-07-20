//! the node-local worker behind lifecycle's code-swap byte-receipt gate: the state-driven
//! twin of [`super::announce::ReadinessSignaller`] for CODE swaps.
//!
//! it polls COMMITTED lifecycle module state each pump tick and, per pending swap,
//! drives this validator to a truthful `LifecycleMsg::SwapReady`:
//!
//! - bytes verified-resident in the local store → self-submit ONE signal
//!   (latched locally; the module's committed readiness set keeps it
//!   idempotent across restarts).
//! - bytes absent → report the digest so the pump spawns a ranged fetch
//!   (the custodian's data-plane push normally lands first; the fetch heals
//!   a node the push missed). never signal what is not held: "ready" is a
//!   machine statement that `sha256(local bytes) == committed hash`.
//!
//! deliberately NOT a `host::worker::Worker` (same reasoning as the upgrade
//! signaller): readiness must survive restart/late-join, so every decision
//! re-derives from committed state instead of reacting to one-shot effects.

use std::collections::BTreeSet;

use sdk::Msg;

/// one pending swap's identity: `(module_id, swap name)`.
pub(crate) type SwapKey = (String, String);

pub(crate) struct CodeReadinessSignaller {
    /// this node's own validator pubkey bytes — the readiness identity.
    me: Vec<u8>,
    /// swaps a signal is already in flight for (submitted, awaiting
    /// finalization) — local dedupe atop the module's idempotence.
    pub(crate) signaled: BTreeSet<SwapKey>,
    /// digests a fetch is already running for — cleared by the pump when a
    /// fetch task finishes (either way), so a failed fetch retries next tick.
    pub(crate) fetching: BTreeSet<[u8; 32]>,
}

/// what one pump tick should do: signals to submit, fetches to spawn.
#[derive(Default)]
pub(crate) struct CodeActions {
    pub(crate) signals: Vec<(SwapKey, Msg)>,
    pub(crate) fetches: Vec<[u8; 32]>,
}

impl CodeReadinessSignaller {
    pub(crate) fn new(me: Vec<u8>) -> Self {
        Self {
            me,
            signaled: BTreeSet::new(),
            fetching: BTreeSet::new(),
        }
    }

    /// the PURE decision core: given committed lifecycle module status and a local
    /// residency check, decide this tick's signals and fetches. truthful
    /// (signals only verified-resident bytes), idempotent (committed
    /// readiness, the in-flight latch, and the fetch dedupe all short-
    /// circuit), and quiet once a swap is `ready`.
    pub(crate) fn decide(
        &mut self,
        modules: &[lifecycle::ModuleCode],
        resident: impl Fn(&[u8; 32]) -> bool,
    ) -> CodeActions {
        let mut actions = CodeActions::default();
        for m in modules {
            let Some(pending) = &m.pending else { continue };
            // coverage complete: nothing left for anyone to say.
            if pending.ready {
                continue;
            }
            // the module already recorded our (committed) signal.
            if pending.readiness.iter().any(|k| k == &self.me) {
                continue;
            }
            let key: SwapKey = (m.module_id.clone(), pending.name.clone());
            if self.signaled.contains(&key) {
                continue;
            }
            let Ok(digest) = <[u8; 32]>::try_from(pending.code_hash.as_slice()) else {
                continue; // malformed hash can never verify — stay silent.
            };
            if resident(&digest) {
                self.signaled.insert(key.clone());
                let msg = Msg {
                    target: host::LIFECYCLE_MODULE_ID.into(),
                    payload: lifecycle::encode_msg(&lifecycle::LifecycleMsg::SwapReady {
                        name: key.1.clone(),
                        module_id: key.0.clone(),
                    }),
                };
                actions.signals.push((key, msg));
            } else if self.fetching.insert(digest) {
                actions.fetches.push(digest);
            }
        }
        actions
    }

    /// un-latch a swap whose signal submit failed, so the next tick retries.
    pub(crate) fn unlatch(&mut self, key: &SwapKey) {
        self.signaled.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> Vec<u8> {
        vec![7; 32]
    }

    fn pending(
        module: &str,
        name: &str,
        hash: u8,
        ready: bool,
        signed: &[Vec<u8>],
    ) -> lifecycle::ModuleCode {
        lifecycle::ModuleCode {
            module_id: module.into(),
            active_code_hash: vec![0; 32],
            pending: Some(lifecycle::ScheduledSwap {
                name: name.into(),
                activation_height: 10,
                code_hash: vec![hash; 32],
                readiness: signed.to_vec(),
                ready,
            }),
        }
    }

    #[test]
    fn resident_bytes_signal_once_absent_bytes_fetch_once() {
        let mut s = CodeReadinessSignaller::new(me());
        let modules = vec![
            pending("held", "v2", 1, false, &[]),
            pending("missing", "v2", 2, false, &[]),
        ];
        let acts = s.decide(&modules, |d| d == &[1u8; 32]);
        assert_eq!(acts.signals.len(), 1);
        assert_eq!(acts.signals[0].0, ("held".into(), "v2".into()));
        assert_eq!(acts.fetches, vec![[2u8; 32]]);

        // second tick: the signal is latched, the fetch deduped.
        let acts = s.decide(&modules, |d| d == &[1u8; 32]);
        assert!(acts.signals.is_empty());
        assert!(acts.fetches.is_empty());

        // the fetch completes (pump clears the latch) and the bytes are now
        // resident: the swap gets its signal on the next tick.
        s.fetching.clear();
        let acts = s.decide(&modules, |_| true);
        assert_eq!(acts.signals.len(), 1);
        assert_eq!(acts.signals[0].0, ("missing".into(), "v2".into()));
    }

    #[test]
    fn committed_readiness_and_ready_latch_keep_quiet() {
        let mut s = CodeReadinessSignaller::new(me());
        // our signal already committed: silent.
        let ours = vec![pending("a", "v2", 1, false, &[me()])];
        assert!(s.decide(&ours, |_| true).signals.is_empty());
        // swap already ready: silent, even though we never signed.
        let armed = vec![pending("b", "v2", 1, true, &[])];
        let acts = s.decide(&armed, |_| true);
        assert!(acts.signals.is_empty() && acts.fetches.is_empty());
        // no pending at all: silent.
        let idle = vec![lifecycle::ModuleCode {
            module_id: "c".into(),
            active_code_hash: vec![0; 32],
            pending: None,
        }];
        let acts = s.decide(&idle, |_| true);
        assert!(acts.signals.is_empty() && acts.fetches.is_empty());
    }

    #[test]
    fn unlatch_retries_a_failed_submit() {
        let mut s = CodeReadinessSignaller::new(me());
        let modules = vec![pending("a", "v2", 1, false, &[])];
        assert_eq!(s.decide(&modules, |_| true).signals.len(), 1);
        assert!(s.decide(&modules, |_| true).signals.is_empty(), "latched");
        s.unlatch(&("a".into(), "v2".into()));
        assert_eq!(s.decide(&modules, |_| true).signals.len(), 1, "retries");
    }
}
