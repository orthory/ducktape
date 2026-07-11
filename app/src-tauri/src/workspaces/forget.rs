//! The pre-forget membership probe: whether tearing a workspace down can
//! strand a peer or halt quorum. Destroying `identity.key` is irreversible,
//! so every verdict here FAILS CLOSED.

use std::path::Path;

use crate::daemon::{last_line, run_verb};

use super::node_toml;

/// wrap a "couldn't reach/resolve the node" failure into the honest refusal we
/// show when `workspace_forget` cannot confirm this node has left the valset.
/// FAIL CLOSED: we would rather strand a user behind a startable node than
/// destroy the identity a still-in-set validator needs to finalize its removal.
fn unconfirmed_forget(detail: String) -> String {
    format!(
        "start the node and finish leaving — we can't confirm this workspace has left the \
         validator set ({detail}), and destroying its identity now could permanently halt the \
         network. bring the node up, request to leave, and wait until the other members approve \
         (you drop out of the set) before forgetting this workspace."
    )
}

/// the verdict of the pre-forget membership probe — drives whether teardown is
/// allowed and, when refused, whether a FORCE forget may override the refusal.
pub(super) enum ForgetVerdict {
    /// definitively safe to tear down: this node is out of the valset
    /// (`in-set=false`), or a provably solo network (`in-set=true validators=1`,
    /// no peer to strand).
    Safe,
    /// the running node CONFIRMS it is still a current validator of a set of
    /// two-or-more. tearing it down halts quorum and strands the pending removal,
    /// so this refusal is ABSOLUTE — a force forget cannot override a provably
    /// live multi-member validator; request-leave-and-wait first.
    ConfirmedInSet(String),
    /// membership could NOT be confirmed — node down/bricked, rpc error, no node
    /// binary, or a status line we cannot parse. refused by default (fail
    /// closed), but this is the UNCERTAINTY a force forget overrides: a node that
    /// can never start can never finalize a removal, so keeping it only strands
    /// the user with a workspace they can never remove.
    Unconfirmed(String),
}

/// classify a `member-status` stdout line (`in-set=<bool> validators=<n>`) into a
/// [`ForgetVerdict`]. FAILS CLOSED: only a definitively out-of-set or provably
/// solo line is `Safe`; a confirmed in-set set-of-two-or-more is `ConfirmedInSet`;
/// anything we cannot parse into BOTH fields is `Unconfirmed` — an unreadable
/// status is uncertainty, never an authorization to destroy an identity.
fn classify_status(status_line: &str) -> ForgetVerdict {
    let in_set = if status_line.contains("in-set=true") {
        Some(true)
    } else if status_line.contains("in-set=false") {
        Some(false)
    } else {
        None
    };
    let validators = status_line
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("validators="))
        .and_then(|n| n.parse::<usize>().ok());
    match (in_set, validators) {
        // already left the set, or a provably solo network — safe to forget.
        (Some(false), _) | (Some(true), Some(1)) => ForgetVerdict::Safe,
        // definitively still a current validator of a multi-member set.
        (Some(true), Some(n)) => ForgetVerdict::ConfirmedInSet(format!(
            "this node is still a current validator of {n} — forgetting it now would halt the \
             network's quorum and strand your removal. request to leave first, then wait until \
             the other members approve (you drop out of the set) before forgetting this \
             workspace."
        )),
        // ambiguous/unparseable status — fail closed, do NOT destroy the identity.
        _ => ForgetVerdict::Unconfirmed(
            "couldn't confirm this workspace has left the validator set (the node's membership \
             status was unreadable) — refusing to forget it, because destroying its identity \
             while it may still be a validator could permanently halt the network. bring the \
             node up and finish leaving first."
                .to_string(),
        ),
    }
}

/// probe the RUNNING node for whether tearing this workspace down is safe. any
/// failure to reach, resolve, or read the node's membership collapses to
/// `Unconfirmed` (fail closed) — exactly the uncertainty a force forget may
/// override for a node that can no longer start.
pub(super) fn probe_forget(dir: &Path) -> ForgetVerdict {
    let cfg = node_toml(dir);
    match run_verb(&["member-status", "--config", &cfg.to_string_lossy()]) {
        Ok(status) => classify_status(&last_line(&status)),
        Err(err) => ForgetVerdict::Unconfirmed(unconfirmed_forget(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_status_allows_a_departed_or_solo_node() {
        // already removed from the set: safe regardless of the reported count.
        assert!(matches!(
            classify_status("in-set=false validators=3"),
            ForgetVerdict::Safe
        ));
        assert!(matches!(
            classify_status("in-set=false validators=1"),
            ForgetVerdict::Safe
        ));
        // a provably solo network: no peer to strand, forgetting just drops it.
        assert!(matches!(
            classify_status("in-set=true validators=1"),
            ForgetVerdict::Safe
        ));
    }

    #[test]
    fn classify_status_confirms_a_still_in_set_validator() {
        // in-set of a set of two-or-more: forgetting would strand the pending
        // removal and halt quorum. must be ConfirmedInSet (never force-overridable)
        // and name the count.
        let verdict = classify_status("in-set=true validators=2");
        match &verdict {
            ForgetVerdict::ConfirmedInSet(msg) => {
                assert!(msg.contains("still a current validator of 2"), "{msg}")
            }
            _ => panic!("expected ConfirmedInSet for a set of two"),
        }
        assert!(matches!(
            classify_status("in-set=true validators=3"),
            ForgetVerdict::ConfirmedInSet(_)
        ));
    }

    #[test]
    fn classify_status_is_unconfirmed_on_an_unparseable_status() {
        // FAIL CLOSED: any line we can't read into BOTH fields is Unconfirmed —
        // an unknown membership can never authorize destroying the identity by
        // itself (only an explicit force may override this uncertainty).
        for line in [
            "",
            "in-set=true",             // count missing
            "validators=2",            // membership missing
            "in-set=true validators=", // count unparseable
            "connection refused",      // not a status line at all
            "in-set=maybe validators=two",
        ] {
            assert!(
                matches!(classify_status(line), ForgetVerdict::Unconfirmed(_)),
                "expected Unconfirmed for {line:?}"
            );
        }
    }
}
