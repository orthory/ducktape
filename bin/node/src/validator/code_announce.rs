//! the node-local worker behind lifecycle's code-swap byte-receipt gate: the state-driven
//! the per-swap byte-receipt readiness signaller for CODE swaps.
//!
//! it polls COMMITTED lifecycle module state each pump tick and, per pending swap,
//! drives this validator to a truthful `LifecycleMsg::SwapReady`:
//!
//! - bytes verified-resident AND loadable on this binary → self-submit ONE
//!   signal (latched locally; the module's committed readiness set keeps it
//!   idempotent across restarts).
//! - bytes absent → report the digest so the pump spawns a ranged fetch
//!   (the custodian's data-plane push normally lands first; the fetch heals
//!   a node the push missed).
//! - bytes held but not loadable on this binary → report the refusal once and
//!   stay silent. Never signal what is not held AND not runnable: "ready" is a
//!   machine statement that `sha256(local bytes) == committed hash` and that
//!   THIS build can instantiate them. Residency alone let a validator on an
//!   older binary arm a swap it then deterministically rejected every op to
//!   while its peers applied them — a silent fork (#1297).
//!
//! deliberately NOT a `host::worker::Worker` (same reasoning as the upgrade
//! signaller): readiness must survive restart/late-join, so every decision
//! re-derives from committed state instead of reacting to one-shot effects.

use std::collections::BTreeSet;

use sdk::Msg;

/// one pending swap's identity: `(module_id, swap name)`.
pub(crate) type SwapKey = (String, String);

/// everything this node can truthfully say about one pending swap's bytes —
/// ONE discriminant, so a new answer has to be routed rather than defaulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodeVerdict {
    /// held, content-verified, AND this binary instantiates them: the only
    /// state that may sign `SwapReady`.
    Loadable,
    /// held, but this binary cannot instantiate them — a host import this
    /// build does not provide, or a component encoding it does not speak.
    Unloadable { detail: String },
    /// not held locally.
    Absent,
}

pub(crate) struct CodeReadinessSignaller {
    /// this node's own validator pubkey bytes — the readiness identity.
    me: Vec<u8>,
    /// swaps a signal is already in flight for (submitted, awaiting
    /// finalization) — local dedupe atop the module's idempotence.
    pub(crate) signaled: BTreeSet<SwapKey>,
    /// digests a fetch is already running for — cleared by the pump when a
    /// fetch task finishes (either way), so a failed fetch retries next tick.
    pub(crate) fetching: BTreeSet<[u8; 32]>,
    /// digests this binary already failed to instantiate. THE LATCH: loading a
    /// component compiles it, and the answer cannot change while this process
    /// lives — so the refusal is decided, reported, and never re-paid.
    unloadable: BTreeSet<[u8; 32]>,
    /// the other half of that latch: digests that already compiled here. The
    /// signal latch is keyed by swap and `unlatch`ed when a submit fails, so
    /// without this a failed submit would re-read and RE-COMPILE the same
    /// bytes on the next tick. A digest names its bytes, so one answer holds.
    loadable: BTreeSet<[u8; 32]>,
}

/// what one pump tick should do: signals to submit, fetches to spawn, refusals
/// to report.
#[derive(Default)]
pub(crate) struct CodeActions {
    pub(crate) signals: Vec<(SwapKey, Msg)>,
    pub(crate) fetches: Vec<[u8; 32]>,
    /// swaps whose bytes this binary cannot run, with the loader's own words —
    /// emitted ONCE per digest by the `unloadable` latch.
    pub(crate) refusals: Vec<(SwapKey, String)>,
}

impl CodeReadinessSignaller {
    pub(crate) fn new(me: Vec<u8>) -> Self {
        Self {
            me,
            signaled: BTreeSet::new(),
            fetching: BTreeSet::new(),
            unloadable: BTreeSet::new(),
            loadable: BTreeSet::new(),
        }
    }

    /// the PURE decision core: given committed lifecycle module status and a
    /// local verdict on each digest, decide this tick's signals, fetches and
    /// refusals. truthful (signals only bytes that are held AND load here),
    /// idempotent (committed readiness, the in-flight latch, the fetch dedupe
    /// and the unloadable latch all short-circuit), and quiet once a swap's
    /// `ready_at` has latched.
    pub(crate) fn decide(
        &mut self,
        modules: &[lifecycle::ModuleCode],
        verdict: impl Fn(&[u8; 32]) -> CodeVerdict,
    ) -> CodeActions {
        let mut actions = CodeActions::default();
        for m in modules {
            let Some(pending) = &m.pending else { continue };
            // coverage complete: nothing left for anyone to say.
            let coverage_complete = pending.ready_at.is_some();
            if coverage_complete {
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
            // already refused: this binary will not start loading bytes it
            // could not load, and re-deciding would recompile the component
            // (and re-report the refusal) on every tick until a restart.
            if self.unloadable.contains(&digest) {
                continue;
            }
            // the compile is paid ONCE per digest, in either direction: a
            // digest already known to load here skips the probe entirely.
            let answer = match self.loadable.contains(&digest) {
                true => CodeVerdict::Loadable,
                false => verdict(&digest),
            };
            match answer {
                CodeVerdict::Loadable => {
                    self.loadable.insert(digest);
                    self.signaled.insert(key.clone());
                    let msg = Msg {
                        target: host::LIFECYCLE_MODULE_ID.into(),
                        payload: lifecycle::encode_msg(&lifecycle::LifecycleMsg::SwapReady {
                            name: key.1.clone(),
                            module_id: key.0.clone(),
                        }),
                    };
                    actions.signals.push((key, msg));
                }
                CodeVerdict::Unloadable { detail } => {
                    self.unloadable.insert(digest);
                    actions.refusals.push((key, detail));
                }
                CodeVerdict::Absent => {
                    if self.fetching.insert(digest) {
                        actions.fetches.push(digest);
                    }
                }
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
                ready_at: ready.then_some(5),
            }),
            history: Vec::new(),
        }
    }

    /// held bytes that load here.
    fn loadable(digest: &[u8; 32], held: u8) -> CodeVerdict {
        match digest == &[held; 32] {
            true => CodeVerdict::Loadable,
            false => CodeVerdict::Absent,
        }
    }

    #[test]
    fn resident_bytes_signal_once_absent_bytes_fetch_once() {
        let mut s = CodeReadinessSignaller::new(me());
        let modules = vec![
            pending("held", "replacement", 1, false, &[]),
            pending("missing", "replacement", 2, false, &[]),
        ];
        let acts = s.decide(&modules, |d| loadable(d, 1));
        assert_eq!(acts.signals.len(), 1);
        assert_eq!(acts.signals[0].0, ("held".into(), "replacement".into()));
        assert_eq!(acts.fetches, vec![[2u8; 32]]);

        // second tick: the signal is latched, the fetch deduped.
        let acts = s.decide(&modules, |d| loadable(d, 1));
        assert!(acts.signals.is_empty());
        assert!(acts.fetches.is_empty());

        // the fetch completes (pump clears the latch) and the bytes are now
        // resident: the swap gets its signal on the next tick.
        s.fetching.clear();
        let acts = s.decide(&modules, |_| CodeVerdict::Loadable);
        assert_eq!(acts.signals.len(), 1);
        assert_eq!(acts.signals[0].0, ("missing".into(), "replacement".into()));
    }

    /// BYTE RESIDENCY IS NOT READINESS. A validator whose binary cannot
    /// instantiate the staged component must stay silent: signalling arms a
    /// swap at R = n that this node then deterministically rejects every op
    /// to while its peers apply them (#1297).
    #[test]
    fn code_this_binary_cannot_load_is_never_signalled_and_is_reported_once() {
        let mut s = CodeReadinessSignaller::new(me());
        let modules = vec![pending("chat", "replacement", 1, false, &[])];
        let refuse = |_: &[u8; 32]| CodeVerdict::Unloadable {
            detail: "unknown import `ducktape:module/host@0.2.0`".into(),
        };

        let acts = s.decide(&modules, refuse);
        assert!(
            acts.signals.is_empty(),
            "holding bytes this binary cannot run is not readiness"
        );
        assert!(
            acts.fetches.is_empty(),
            "the bytes are here; fetching is not the fix"
        );
        assert_eq!(acts.refusals.len(), 1);
        assert_eq!(acts.refusals[0].0, ("chat".into(), "replacement".into()));

        // LATCHED: loading compiles the component and the answer cannot change
        // while this process lives, so neither the work nor the log repeats.
        let acts = s.decide(&modules, |_| {
            panic!("a refused digest must not be re-probed")
        });
        assert!(acts.signals.is_empty());
        assert!(acts.refusals.is_empty());
    }

    #[test]
    fn committed_readiness_and_ready_latch_keep_quiet() {
        let mut s = CodeReadinessSignaller::new(me());
        // our signal already committed: silent.
        let ours = vec![pending("a", "replacement", 1, false, &[me()])];
        assert!(
            s.decide(&ours, |_| CodeVerdict::Loadable)
                .signals
                .is_empty()
        );
        // swap already ready: silent, even though we never signed.
        let armed = vec![pending("b", "replacement", 1, true, &[])];
        let acts = s.decide(&armed, |_| CodeVerdict::Loadable);
        assert!(acts.signals.is_empty() && acts.fetches.is_empty());
        // no pending at all: silent.
        let idle = vec![lifecycle::ModuleCode {
            module_id: "c".into(),
            active_code_hash: vec![0; 32],
            pending: None,
            history: Vec::new(),
        }];
        let acts = s.decide(&idle, |_| CodeVerdict::Loadable);
        assert!(acts.signals.is_empty() && acts.fetches.is_empty());
    }

    #[test]
    fn unlatch_retries_a_failed_submit() {
        let mut s = CodeReadinessSignaller::new(me());
        let modules = vec![pending("a", "replacement", 1, false, &[])];
        assert_eq!(
            s.decide(&modules, |_| CodeVerdict::Loadable).signals.len(),
            1
        );
        assert!(
            s.decide(&modules, |_| CodeVerdict::Loadable)
                .signals
                .is_empty(),
            "latched"
        );
        s.unlatch(&("a".into(), "replacement".into()));
        // ...and the retry does NOT re-compile: the digest already answered
        // once, so the probe is paid once per pending swap however many
        // submits fail.
        assert_eq!(
            s.decide(&modules, |_| panic!(
                "a digest that already loaded must not be re-probed"
            ))
            .signals
            .len(),
            1,
            "retries"
        );
    }
}
