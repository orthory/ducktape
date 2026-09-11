//! The periodic controller kick: re-offer whatever the epoch is still
//! waiting on, heal behind peers, and run the role's endpoint-less
//! rendezvous sweep. Both sweeps ride the same backoff + bounded per-epoch
//! budget, which keeps the nudge cadence from hammering the coordinator and
//! from sweeping an unpunchable peer forever — a sweep goes quiet once the
//! budget is spent and re-arms only at the next epoch's `Retarget`.

use wireguard::ValidatorIdentity;

use crate::binding;
use crate::contract::Effect;
use crate::epoch::{EpochState, Role};

use super::pending::PendingOp;
use super::{Driver, UNTARGETED_NUDGE_GRACE};

impl Driver {
    pub(crate) fn nudge(&mut self, epoch: Option<&mut EpochState>) {
        self.nudges += 1;
        let Some(state) = epoch else {
            // a boot retarget suspended behind its restore IS targeted —
            // its epoch lands when the restore settles; only a plane that
            // was never told its epoch is the black hole below.
            if self.pending_restore.is_some() {
                return;
            }
            // A few ticks of this are the boot race — the fresh-boot path
            // sends its Retarget right after wiring, and a nudge can beat
            // it. Past the grace it is a wiring defect: this plane will
            // never gossip and never accept, for the life of the process.
            self.untargeted_nudges += 1;
            let past_grace = self.untargeted_nudges >= UNTARGETED_NUDGE_GRACE;
            let periodic = self.untargeted_nudges.is_multiple_of(64);
            if past_grace && (self.untargeted_nudges == UNTARGETED_NUDGE_GRACE || periodic) {
                tracing::warn!(
                    target: "ducktape::reachability",
                    reason = "no_epoch_target",
                    attempts = self.untargeted_nudges,
                    "this reachability plane was never told its epoch — it is dropping \
                     every record and advert it receives and sending none of its own"
                );
            }
            return;
        };
        self.untargeted_nudges = 0;
        state.expire_relays(self.view);
        for (peer, msg) in state.reoffers() {
            self.send_msg(state, peer, &msg);
        }
        match state.role {
            Role::Member => {
                self.heal_behind_peers(state);
                self.sweep_member_rendezvous_fallback(state);
            }
            Role::Standby => self.sweep_standby_rendezvous_fallback(state),
        }
    }

    /// THE HEAL: a peer still gossiping phase-A at a node whose sets have
    /// locked is a peer that never got our half. Its record and advert are
    /// dropped by the phase gates, but the drop RECORDS the ask
    /// (`request_heal`), and one nudge later this sends our record and
    /// advert back.
    ///
    /// Without this, missing one fan-out is permanent: the exchange is
    /// one-shot and the sender moves on. That loss is routine — a member
    /// learns how to DIAL a promoted joiner from the very record that
    /// completes its own assembly, so its reply goes out microseconds before
    /// the link exists and the lane drops it; the joiner then retries
    /// forever into a node that will not answer until the next cutover.
    /// Rate: at most one record+advert pair per asking peer per tick, and
    /// only to a peer that asked by gossiping at us.
    fn heal_behind_peers(&mut self, state: &mut EpochState) {
        for (peer, msg) in state.heal_sends(self.nudges) {
            self.send_msg(state, peer, &msg);
        }
    }

    /// Retry the by-identity rendezvous fallback for any MEMBER peer that
    /// is still endpoint-less and that NO layer can dial — the
    /// handshake-time attempt can lose the race against the peer's own
    /// coordinator registration (both sides often boot together). Each
    /// resolution that lands writes its own override through to the live
    /// interface (`peer_rendezvous_resolved`), so the sweep only STARTS
    /// work here.
    fn sweep_member_rendezvous_fallback(&mut self, state: &mut EpochState) {
        let retry_targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| {
                // dialable through ANY layer — a resolved override, or the
                // invite layer's observed address grafted onto the
                // endpoint-less record — means there is nothing to punch
                // for: the interface can already initiate.
                let undialable = self.dial_endpoint(state, *peer).is_none();
                // the peer's CURRENT life decides whether it is
                // endpoint-less: a post-lock re-advertisement supersedes
                // the record the view locked.
                let record = state
                    .readvertised
                    .get(peer)
                    .map(|signed| &signed.record)
                    .or_else(|| state.view().and_then(|view| view.record(*peer)));
                let endpoint_less =
                    record.is_some_and(|record| record.wireguard_endpoint.is_none());
                undialable && endpoint_less
            })
            .collect();
        for peer in retry_targets {
            self.request_resolve_peer(state, peer);
        }
    }

    /// The standby's half of the sweep: rendezvous any member it holds an
    /// entry for and [`Driver::dial_endpoint`] cannot dial. After a reboot
    /// that is every fully-NATed member: the restore reinstalls them
    /// endpoint-less from the persisted mesh, and their live records cannot
    /// arrive to replace them — plane gossip rides the very tunnels the
    /// missing endpoints keep down. A member with no entry in either layer
    /// is NOT swept: with no record there is no WireGuard key to install,
    /// and live assembly still owes us the record itself.
    fn sweep_standby_rendezvous_fallback(&mut self, state: &mut EpochState) {
        if self.config.coordinators.is_empty() {
            return;
        }
        let targets: Vec<ValidatorIdentity> = state
            .peers
            .iter()
            .copied()
            .filter(|peer| {
                let known = state.prewarm_peers.contains_key(peer)
                    || self
                        .base_peers
                        .as_ref()
                        .is_some_and(|base| base.contains_key(peer));
                // an endpoint-less pre-warm entry is NOT undialable on its
                // own: the invite layer grafts its observed address onto
                // exactly that entry on every push, and a fresh resident's
                // first tunnel is carrying traffic on it.
                let undialable = self.dial_endpoint(state, *peer).is_none();
                known && undialable
            })
            .collect();
        for peer in targets {
            self.request_rendezvous_by_identity(
                state,
                peer,
                PendingOp::StandbyPrewarmRendezvous { peer },
            );
        }
    }

    /// The by-identity rendezvous attempt every fallback shares: burn one
    /// unit of the per-epoch budget and start the lookup, parking `op` for
    /// its outcome. `false` means the budget refused the attempt this round
    /// — non-fatal, a later `Nudge` retries.
    pub(crate) fn request_rendezvous_by_identity(
        &mut self,
        state: &mut EpochState,
        peer: ValidatorIdentity,
        op: PendingOp,
    ) -> bool {
        if !state.claim_rendezvous_attempt(peer, self.now_ms) {
            return false;
        }
        let req = self.mint_req();
        self.effects.push(Effect::RendezvousStart {
            req,
            peer: binding::node_key(peer),
        });
        self.pending.insert(req, op);
        true
    }
}
