use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use host::Host;
use sdk::Msg;

/// how long a RESIDENT's relayed announce may await its consensus fate before
/// the pump un-latches and re-decides from committed state. Sized above the
/// relay lane's own 10s SUBMIT_HOLD, so a swept-but-applied frame usually
/// shows up in the committed registry before this fires and the state read
/// stays quiet. A duplicate announce is harmless: the module applies a
/// declarative replace, so a re-send converges on the same state.
const ANNOUNCE_RETRY: Duration = Duration::from_secs(15);

/// how many finalized blocks a VALIDATOR's submitted announce may sit out
/// before the pump gives up on it. Generous on purpose: a frame stays in the
/// orderer's `outstanding` until a batch carrying it finalizes (a cutover
/// re-flushes it under the same id), so needing more than a couple of blocks
/// already means something is wrong, and the cost of waiting is silence in the
/// registry rather than a duplicate in the mempool.
///
/// ponytail: a fixed block budget, and the ceiling is the validator COUNT. A
/// frame waits at most ~2V blocks (`eager_flush_due` requires `orderer_idle`,
/// so this node has one batch outstanding at a time and round-robin leadership
/// gives it a slot every V), which is comfortable at V ≤ 8, marginal near
/// V ≈ 16 and wrong above it. The damage is bounded — the duplicate carries a
/// fresh `seq` so it is a distinct `FrameId`, the first fate then fails
/// `owns()` and drops silently, and the second lands as a declarative-replace
/// no-op — so the upgrade (scale the budget by `current_members().len()`) waits
/// for a network that big to exist.
const ANNOUNCE_RETRY_BLOCKS: u64 = 32;

/// a failed announce is reported on the FIRST failure and every Nth after it,
/// carrying the count. The doctrine's rule for a forever-retry loop: an
/// unconditional `warn!` on a ~1s round trip evicts the whole 4096-line ring in
/// half an hour and destroys the evidence someone came to read — and the
/// counter IS the diagnosis.
const FAILURE_REPORT_EVERY: u64 = 32;

/// the give-up reasons — a submitted announce whose consensus fate never
/// arrived at all, which is a different diagnosis per lane than a fate that
/// arrived and said no.
const REASON_NEVER_ORDERED: &str = "announce_never_ordered";
const REASON_REPLY_LOST: &str = "announce_reply_lost";

/// what un-latches a submitted announce whose consensus fate never arrived.
///
/// The two tiers lose a frame for DIFFERENT reasons, so one constant cannot
/// size both — and sizing the authoritative lane in wall-clock seconds is
/// actively wrong: a chain that takes longer than the deadline to finalize is
/// STALLED, and re-submitting a duplicate into the same `outstanding` /
/// `pending_batch` is precisely what a stall does not need.
#[derive(Clone, Copy)]
pub(crate) enum Rearm {
    /// the RESIDENT tier. Its frame rides the LOSSY submit-relay lane and sits
    /// in ANOTHER node's custody, so a dropped Reply or a crashed relay target
    /// really does mean the fate is never coming — and wall-clock silence is
    /// the only evidence a resident has.
    Silence,
    /// the VALIDATOR tier. Its frame goes into its OWN orderer, stays in
    /// `outstanding` until a batch carrying it finalizes, and the drain routes
    /// every non-Discarded frame's fate back here. Silence is therefore NOT
    /// evidence of loss; it is evidence the chain is not finalizing. What IS
    /// evidence: blocks going by without our frame among them — and a stall
    /// produces no blocks, so this can never fire during one.
    Unordered,
}

/// the give-up budget of a submitted announce, in the unit its own lane can
/// actually lose a frame in.
enum Expiry {
    /// [`Rearm::Silence`]: give up at this instant.
    Silent(Instant),
    /// [`Rearm::Unordered`]: give up once this many more blocks finalize.
    Unordered(u64),
}

/// the submitted announce awaiting its consensus fate.
struct InFlight {
    /// the frame's content address — what the drain (or the relay Reply)
    /// matches this pump's own announce on.
    frame: node::FrameId,
    /// what THAT frame announced. Kept here so the applied log reports the set
    /// the registry actually took, rather than whatever the offered set has
    /// drifted to by the time the outcome lands.
    capabilities: Vec<String>,
    expiry: Expiry,
}

/// the consensus fate of this pump's OWN announce: decided here, merely
/// reported by the caller. Not a `bool` — the two outcomes carry different
/// evidence, and the one bug this seam has already had was a caller reading
/// the wrong side of a boolean.
pub(crate) enum Fate {
    /// the registry took it, carrying the set the FRAME announced.
    Applied { capabilities: Vec<String> },
    /// consensus refused it; the next tick re-decides and retries. Reported,
    /// carrying the consecutive-rejection count.
    Rejected { attempts: u64 },
    /// refused as well, and deliberately silent: one of the rejections BETWEEN
    /// reports (see [`FAILURE_REPORT_EVERY`]).
    RejectedQuietly,
}

/// the most tags one node may announce.
///
/// This is `capability`'s own `MAX_CAPABILITIES`, mirrored HOST-SIDE: the
/// module's constant is private to a crate under `crates/modules/`, and merely
/// making it `pub` would rebuild its `component.wasm`, move the Lifecycle
/// digest it is seeded with, and so move the genesis app hash — a flag day for
/// a visibility keyword. An over-cap announce is rejected at EXECUTE, which
/// costs a whole consensus round trip and (before the outcome route below
/// existed) wedged the announcer forever, so the host refuses to emit one at
/// all. `the_announce_cap_matches_the_modules_own` parses the module's source
/// to pin the two together — a comment would not have.
const MAX_ANNOUNCED_TAGS: usize = 64;

/// the capability self-announcer: it polls the committed
/// registry each pump tick and, when this node's announced set differs from
/// what it can truthfully offer, self-submits ONE declarative
/// [`CapabilityMsg::Announce`]. state-driven (survives restart/late-join) and
/// idempotent: once the committed set matches, it stays quiet. a node with no
/// providers announces nothing.
///
/// ## where the offered set comes from
///
/// The node discovers nothing any more — every service is a standalone daemon
/// — so the offered set is, PER GRANTED KIND, `grant ∩ live hello`:
///
/// - **grant**: the tags the user reviewed and consented to at `service enable`
///   (`services.toml`). Consent can only narrow.
/// - **live hello**: what that kind's daemon is signaling to this node RIGHT
///   NOW ([`noded::services::ServiceCatalog`]). Truth can only narrow.
///
/// Neither side may widen the other, and BOTH are re-read every tick — so a
/// node holding a grant with no daemon signaling announces NOTHING, a stopped
/// daemon retracts within the hello TTL, and `service enable`/`disable` take
/// effect without restarting the node. (Re-reading `services.toml` per tick is
/// free beside the two committed queries this pump already issues.)
///
/// ## and the KIND itself is announced
///
/// A granted-and-signaling kind contributes its own tag (`compute`, `agent`,
/// …) beside the executor tags, so "which nodes run kind X" resolves through
/// the registry query `saga` already draws its rendezvous pool from. This costs
/// no module change and no new encoding: the hello's kind grammar (1..32 bytes
/// of `[a-z0-9-]`, `noded::services::kind_is_well_formed`) is a strict subset
/// of `capability::validate_tag`'s (1..64 bytes of `[a-z0-9._-]`), so a service
/// kind already IS a legal capability tag.
///
/// ## both tiers share this
///
/// A validator drives it from its drain loop and a resident over the relay
/// lane, but the decision core, the in-flight latch and the retry deadline are
/// the same code — a second copy is how one tier's wedge fix misses the other.
pub(crate) struct CapabilityAnnouncer {
    /// this node's own validator pubkey bytes — the registry identity.
    me: Vec<u8>,
    /// the workspace whose `services.toml` carries the user's consent. Read
    /// per tick rather than latched at boot: consent is the operator's live
    /// decision, and a grant that needed a restart to take effect would make
    /// `disable` a suggestion rather than a revocation.
    workspace: std::path::PathBuf,
    /// the volatile signaling catalog — the live half of the intersection.
    services: noded::services::ServiceCatalog,
    /// the numeric capacity announced ALONGSIDE the tags: probed host totals
    /// for a Podman node, EMPTY for a direct-spawn one (a direct node makes no
    /// capacity promise). Forced empty whenever `capabilities` is empty —
    /// resources-without-tags is a consensus-level reject, never emitted.
    pub(crate) resources: BTreeMap<String, u64>,
    /// the (tags, resources) pair we last DECIDED, latched so an identical
    /// announce is not re-decided every tick. PRIVATE on purpose: every path
    /// that clears it is a named method below, because the one bug this file
    /// has already had is a caller leaving it set after a rejection.
    announced: Option<(Vec<String>, BTreeMap<String, u64>)>,
    /// how this tier gives up on a frame whose fate never arrives.
    rearm: Rearm,
    /// the submitted announce awaiting its consensus fate.
    in_flight: Option<InFlight>,
    /// consecutive failures to get an announce APPLIED since the last one that
    /// was (or boot) — counting BOTH a fate that came back rejected and a
    /// frame whose fate never came at all, because "this node has failed to
    /// enter the registry N times running" is the operator's one question and
    /// both paths retry the same loop. Deliberately not reset when the offered
    /// set changes: the pair is re-latched identically after every failure, so
    /// a per-pair reset would pin the counter at 1 and silence the throttle.
    failures: u64,
    /// whether the last grant read failed — latched so a corrupt
    /// `services.toml` is reported once, not once per drain tick.
    grant_unreadable: bool,
    /// whether the offered set last exceeded [`MAX_ANNOUNCED_TAGS`] — latched
    /// for the same reason.
    over_cap: bool,
    /// whether a signaling daemon last offered a tag the registry's own rule
    /// refuses — latched for the same reason.
    illegal_tags: bool,
}

impl CapabilityAnnouncer {
    pub(crate) fn new(
        me: Vec<u8>,
        workspace: std::path::PathBuf,
        services: noded::services::ServiceCatalog,
        resources: BTreeMap<String, u64>,
        rearm: Rearm,
    ) -> Self {
        Self {
            me,
            workspace,
            services,
            resources,
            announced: None,
            rearm,
            in_flight: None,
            failures: 0,
            grant_unreadable: false,
            over_cap: false,
            illegal_tags: false,
        }
    }

    /// every service grant the user has minted on this node right now.
    ///
    /// An absent record grants nothing — the ordinary un-enabled node. An
    /// UNREADABLE one also announces nothing (consent that cannot be read is
    /// not consent), but it says so: silently retracting a live node's whole
    /// announce because someone corrupted a toml is exactly the failure that
    /// must not be quiet. Latched, because this runs on the drain tick.
    fn granted(&mut self) -> Vec<crate::services::ServiceGrant> {
        match crate::services::load(&self.workspace) {
            Ok(services) => {
                if self.grant_unreadable {
                    self.grant_unreadable = false;
                    tracing::info!(target: "ducktape::service", "service grants readable again");
                }
                services.grants
            }
            Err(error) => {
                if !self.grant_unreadable {
                    self.grant_unreadable = true;
                    tracing::warn!(
                        target: "ducktape::service",
                        reason = "grant_unreadable",
                        "service grants cannot be read; this node announces nothing until they \
                         are repaired: {error}"
                    );
                }
                Vec::new()
            }
        }
    }

    /// what this node may truthfully announce right now: for EVERY granted
    /// kind whose daemon is signaling, that kind's tag plus the grant's
    /// executor tags intersected with what the SAME kind's hello offers.
    ///
    /// The live half is read FIRST and short-circuits: with nothing signaling
    /// the intersection is empty whatever the grants say, so the common case (a
    /// node with no service daemon) never touches the disk — this runs on the
    /// async drain tick at ~10 Hz.
    ///
    /// The result is sorted and deduplicated, and that is load-bearing rather
    /// than tidy: the committed registry answers a `BTreeSet` rendered in
    /// order, so an unsorted offer could never compare equal to it and the
    /// announcer would never quiesce.
    ///
    /// PRIVATE, and `now` is threaded in rather than read from the clock: this
    /// re-reads `services.toml`, takes the catalog lock and mutates the
    /// warning latches, so it is a decide-fn that must never be reachable from
    /// a `tracing` field position (level-gated side effects) — the applied log
    /// reports what the FRAME carried, off [`InFlight::capabilities`].
    fn offered(&mut self, now: Instant) -> Vec<String> {
        let signaling = self.services.live(now);
        if signaling.is_empty() {
            return Vec::new();
        }
        let grants = self.granted();
        let mut kinds: BTreeSet<String> = BTreeSet::new();
        let mut executors: BTreeSet<String> = BTreeSet::new();
        for live in &signaling {
            // signaling WITHOUT consent offers nothing at all — not its
            // executors and not its kind. Enable is the switch.
            let Some(grant) = grants.iter().find(|grant| grant.kind == live.kind) else {
                continue;
            };
            let offers_now: BTreeSet<&str> = live.capabilities.iter().map(String::as_str).collect();
            // the kind rides even when the intersection is empty: a daemon that
            // spawns nothing (an airlock-style plug) still IS that kind, and
            // that is precisely what placement asks about.
            kinds.insert(live.kind.clone());
            executors.extend(
                grant
                    .capabilities
                    .iter()
                    .filter(|tag| offers_now.contains(tag.as_str()))
                    .cloned(),
            );
        }
        let executors = self.legal(executors);
        self.within_cap(kinds, executors)
    }

    /// the executor tags the registry's OWN rule would accept.
    ///
    /// `capability::validate_tag` is the one definition of a well-formed tag
    /// — but the daemon hello boundary's item grammar is LOOSER (any printable
    /// ascii plus a space, `noded::services::item_is_well_formed`), and
    /// `service enable` copies a hello's tags into `services.toml` verbatim.
    /// So a third-party daemon or an operator spec dir signaling
    /// `"Claude Sonnet"` reaches this decision intact, and the announce is
    /// ALL-OR-NOTHING: emitting it costs a consensus round trip, a
    /// module-level reject that suppresses this node's LEGAL tags too, and a
    /// retry loop that can never converge. Refuse it host-side, keep the rest,
    /// and say so once. (The KIND tags need no filter: the hello's kind
    /// grammar, 1..32 of `[a-z0-9-]`, is a strict subset of the tag rule.)
    fn legal(&mut self, executors: BTreeSet<String>) -> BTreeSet<String> {
        let signaled = executors.len();
        let kept: BTreeSet<String> = executors
            .into_iter()
            .filter(|tag| capability::validate_tag(tag).is_ok())
            .collect();
        self.note_illegal(signaled - kept.len());
        kept
    }

    /// report crossing (and clearing) the tag-grammar refusal exactly once per
    /// transition — a per-tick `warn!` here would evict the ring.
    fn note_illegal(&mut self, dropped: usize) {
        if dropped == 0 {
            if self.illegal_tags {
                self.illegal_tags = false;
                tracing::info!(
                    target: "ducktape::service",
                    "every offered capability tag is well-formed again"
                );
            }
            return;
        }
        if self.illegal_tags {
            return;
        }
        self.illegal_tags = true;
        tracing::warn!(
            target: "ducktape::service",
            reason = "announce_tag_illegal",
            dropped,
            "a signaling daemon offers capability tags the registry refuses (want \
             [a-z0-9._-], at most 64 bytes); they are NOT announced and work tagged \
             with them will never be placed here — fix the daemon's capability spec"
        );
    }

    /// the announced set, bounded BELOW the registry's own cap.
    ///
    /// The kind tags are kept first and whole: there is at most one per granted
    /// daemon (the signaling catalog holds 64 kinds at the very most), they are
    /// what placement queries, and they are the cheapest thing on the list. The
    /// executor tags then fill whatever budget remains, in sorted order so the
    /// choice is deterministic rather than whatever the filesystem handed back.
    /// The overflow is dropped with a counted, named warning — latched, because
    /// this decides on every drain tick and an unconditional `warn!` here would
    /// evict the ring in minutes.
    fn within_cap(&mut self, kinds: BTreeSet<String>, executors: BTreeSet<String>) -> Vec<String> {
        let kept_kinds = kinds.len().min(MAX_ANNOUNCED_TAGS);
        let executor_budget = MAX_ANNOUNCED_TAGS - kept_kinds;
        let dropped = (kinds.len() - kept_kinds) + executors.len().saturating_sub(executor_budget);
        let announced: BTreeSet<String> = kinds
            .into_iter()
            .take(kept_kinds)
            .chain(executors.into_iter().take(executor_budget))
            .collect();
        self.note_cap(dropped);
        announced.into_iter().collect()
    }

    /// report crossing (and clearing) the announce cap exactly once per
    /// transition. Loud and local, which is the whole point: the alternative is
    /// a submit the chain rejects at execute, and an operator with no idea why
    /// their node vanished from every rendezvous pool.
    fn note_cap(&mut self, dropped: usize) {
        if dropped == 0 {
            if self.over_cap {
                self.over_cap = false;
                tracing::info!(
                    target: "ducktape::service",
                    "announce is back within the {MAX_ANNOUNCED_TAGS}-tag cap"
                );
            }
            return;
        }
        if self.over_cap {
            return;
        }
        self.over_cap = true;
        tracing::warn!(
            target: "ducktape::service",
            reason = "announce_over_cap",
            dropped,
            cap = MAX_ANNOUNCED_TAGS,
            "this node can offer more capability tags than the registry accepts; the excess is \
             NOT announced and work tagged with it will never be placed here — narrow the \
             grants or the capability spec dir"
        );
    }

    /// this announce's resources, with the invariant applied: empty tags carry
    /// no resources (resources-without-tags is a module-level reject).
    fn effective_resources(&self, offered: &[String]) -> BTreeMap<String, u64> {
        if offered.is_empty() {
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
        offered: &[String],
        committed_tags: &[String],
        committed_resources: &BTreeMap<String, u64>,
    ) -> Option<(Vec<String>, BTreeMap<String, u64>)> {
        let resources = self.effective_resources(offered);
        // nothing to provide and nothing recorded: stay silent (genesis state).
        if offered.is_empty() && committed_tags.is_empty() && committed_resources.is_empty() {
            return None;
        }
        // the registry already reflects what we offer AND our resources.
        if committed_tags == offered && committed_resources == &resources {
            self.announced = None;
            return None;
        }
        // an announce for this exact pair is already in flight.
        if self.announced.as_ref() == Some(&(offered.to_vec(), resources.clone())) {
            return None;
        }
        self.announced = Some((offered.to_vec(), resources.clone()));
        Some((offered.to_vec(), resources))
    }

    /// a decided announce left this node: latch the frame's content address so
    /// the pump stays quiet while its consensus fate is pending, with the
    /// give-up budget this tier's lane is measured in.
    pub(crate) fn sent(&mut self, frame: node::FrameId, now: Instant) {
        let expiry = match self.rearm {
            Rearm::Silence => Expiry::Silent(now + ANNOUNCE_RETRY),
            Rearm::Unordered => Expiry::Unordered(ANNOUNCE_RETRY_BLOCKS),
        };
        // `decide` latched this pair moments ago and no further decision can
        // run while a frame is in flight, so this IS what the frame carries.
        let capabilities = self
            .announced
            .as_ref()
            .map(|(tags, _)| tags.clone())
            .unwrap_or_default();
        self.in_flight = Some(InFlight {
            frame,
            capabilities,
            expiry,
        });
    }

    /// whether `frame` is this pump's own in-flight announce — a PURE query,
    /// so a caller can tell that its generic per-frame reporting must stand
    /// down before [`Self::on_outcome`] consumes the latch.
    pub(crate) fn owns(&self, frame: &node::FrameId) -> bool {
        self.in_flight
            .as_ref()
            .is_some_and(|flight| &flight.frame == frame)
    }

    /// the SUBMIT itself failed (a validator's local submit errored, a
    /// resident's relay knew no validator): un-latch immediately so the next
    /// tick retries.
    pub(crate) fn submit_failed(&mut self) {
        self.in_flight = None;
        self.announced = None;
    }

    /// this announce's CONSENSUS fate. `Some(fate)` when `frame` is the one
    /// this pump submitted (the caller reports it), `None` when it belongs to
    /// somebody else.
    ///
    /// A non-applied fate (rejected at execute, Refused on the relay) un-latches
    /// so the next tick re-decides and retries; an applied one stays latched
    /// until the committed registry confirms, which is the announcer's own
    /// state-driven quiesce.
    ///
    /// Without this route an announce that SUBMITS fine and is REJECTED at
    /// execute leaves the decide latch set forever: the offered set has not
    /// changed, so every later decision matches the latch and returns `None`,
    /// and the node announces nothing again ever — silently out of every
    /// rendezvous pool, with nothing anywhere saying why.
    pub(crate) fn on_outcome(&mut self, frame: &node::FrameId, applied: bool) -> Option<Fate> {
        if !self.owns(frame) {
            return None;
        }
        let flight = self.in_flight.take().expect("owns() proved it is latched");
        if applied {
            self.failures = 0;
            return Some(Fate::Applied {
                capabilities: flight.capabilities,
            });
        }
        self.announced = None;
        if self.count_failure() {
            return Some(Fate::Rejected {
                attempts: self.failures,
            });
        }
        Some(Fate::RejectedQuietly)
    }

    /// count one more consecutive failure and answer whether THIS one is
    /// reported: the first, then every [`FAILURE_REPORT_EVERY`]th.
    fn count_failure(&mut self) -> bool {
        self.failures += 1;
        let first = self.failures == 1;
        let every_nth = self.failures.is_multiple_of(FAILURE_REPORT_EVERY);
        first || every_nth
    }

    /// abandon the in-flight announce: its consensus fate never arrived at all.
    ///
    /// This is the SIBLING of a rejection and gets the same treatment, because
    /// it is the same forever-retry loop seen from the other side. Silence here
    /// was the worse bug of the two: a rejection at least carries a module
    /// reason, while a frame nobody ever answered leaves an operator with a
    /// node that is out of every rendezvous pool and a log that says nothing at
    /// all at the default level.
    fn give_up(&mut self, reason: &'static str) {
        if self.count_failure() {
            tracing::warn!(
                target: "ducktape::modules",
                reason,
                attempts = self.failures,
                "a submitted capability announce never reached a consensus fate; \
                 re-deciding and retrying from committed state"
            );
        }
        self.submit_failed();
    }

    /// `blocks` more blocks finalized on this node's own lane: charge them
    /// against a submitted announce's budget, and give up when it runs out.
    ///
    /// The VALIDATOR lane's liveness backstop. A frame whose fate never
    /// arrives here is one the orderer never ordered, and that shows up as
    /// blocks going by WITHOUT it — never as wall-clock silence, which on this
    /// lane only means the chain is stalled.
    pub(crate) fn on_blocks(&mut self, blocks: u64) {
        let Some(flight) = self.in_flight.as_mut() else {
            return;
        };
        let left = match &mut flight.expiry {
            // the resident lane is charged in silence, not blocks.
            Expiry::Silent(_) => return,
            Expiry::Unordered(left) => {
                *left = left.saturating_sub(blocks);
                *left
            }
        };
        let gave_up = left == 0;
        if gave_up {
            self.give_up(REASON_NEVER_ORDERED);
        }
    }

    /// the RESIDENT lane's liveness backstop: a relayed frame whose fate never
    /// arrived (dropped Reply, crashed relay target, swept hold) stops blocking
    /// once its deadline passes — un-latch and let the next decision re-read the
    /// committed registry, which is quiet if the announce actually landed.
    fn rearm_if_stale(&mut self, now: Instant) {
        let Some(flight) = self.in_flight.as_ref() else {
            return;
        };
        let gave_up = match flight.expiry {
            Expiry::Silent(deadline) => now >= deadline,
            // charged by `on_blocks`, and a stall must never fire it.
            Expiry::Unordered(_) => false,
        };
        if gave_up {
            self.give_up(REASON_REPLY_LOST);
        }
    }

    /// query this node's committed capability set AND resources and, when an
    /// announce is due, build the external-origin `Announce` op. Quiet while a
    /// submitted frame's fate is still pending. gracefully `None` when the
    /// module is absent (pre-retrofit net) or a reply is unreadable.
    pub(crate) async fn maybe_announce(&mut self, host: &Host, now: Instant) -> Option<Msg> {
        use capability::{
            CapabilityMsg, CapabilityQuery, CapabilityReply, decode_reply, encode_msg, encode_query,
        };
        self.rearm_if_stale(now);
        if self.in_flight.is_some() {
            return None;
        }
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
        let offered = self.offered(now);
        let (capabilities, resources) =
            self.decide(&offered, &committed_tags, &committed_resources)?;
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

    /// a workspace whose `services.toml` grants each `(kind, tags)` pair. The
    /// announcer reads consent off disk per tick, so a test grant IS a file.
    fn granted_workspace(grants: &[(&str, &[&str])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("scratch workspace");
        let mut body = String::from("version = 1\n");
        // `Services::validate` requires kinds to be unique AND sorted.
        let mut sorted = grants.to_vec();
        sorted.sort_by_key(|(kind, _)| *kind);
        for (index, (kind, capabilities)) in sorted.iter().enumerate() {
            let tags = capabilities
                .iter()
                .map(|tag| format!("    {tag:?},"))
                .collect::<Vec<_>>()
                .join("\n");
            body.push_str(&format!(
                "\n[[service]]\nkind = {kind:?}\ninstance = {:?}\nnonce = {:?}\n\
                 granted_unix = 1\ncapabilities = [\n{tags}\n]\nscopes = []\n",
                format!("{:02x}", index + 0xa0).repeat(32),
                "bb".repeat(16),
            ));
        }
        std::fs::write(dir.path().join(crate::services::FILE_NAME), body)
            .expect("write the grants");
        dir
    }

    /// an announcer over `workspace` and `catalog`. The offered-set tests do
    /// not exercise the in-flight lane, so they all take the resident's.
    fn announcer(
        me: u8,
        workspace: &tempfile::TempDir,
        catalog: &noded::services::ServiceCatalog,
        resources: BTreeMap<String, u64>,
    ) -> CapabilityAnnouncer {
        CapabilityAnnouncer::new(
            vec![me; 32],
            workspace.path().to_path_buf(),
            catalog.clone(),
            resources,
            Rearm::Silence,
        )
    }

    /// register a live hello for `kind` offering `offered`, at `at`.
    fn signal_at(
        catalog: &noded::services::ServiceCatalog,
        kind: &str,
        offered: &[&str],
        at: Instant,
    ) {
        catalog
            .hello(
                noded::services::Hello {
                    kind: kind.into(),
                    version: "1".into(),
                    build: noded::services::build_identity()
                        .expect("tests run from a git checkout")
                        .into(),
                    capabilities: tags(offered),
                    scopes: Vec::new(),
                    needs: Vec::new(),
                },
                at,
            )
            .expect("a matching-build hello is admitted");
    }

    /// register a live hello for `kind` offering `offered`, right now.
    fn signal(catalog: &noded::services::ServiceCatalog, kind: &str, offered: &[&str]) {
        signal_at(catalog, kind, offered, Instant::now());
    }

    /// what the announcer would offer at this instant.
    fn offered(a: &mut CapabilityAnnouncer) -> Vec<String> {
        a.offered(Instant::now())
    }

    fn frame(byte: u8) -> node::FrameId {
        node::frame_id(&[byte])
    }

    #[test]
    fn the_offered_set_is_the_grant_intersected_with_a_live_hello() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["agent.claude", "agent.codex"])]);
        let mut a = announcer(9, &workspace, &catalog, caps(8));

        // a grant with NO daemon signaling offers nothing. This is the whole
        // point of the transition: the node discovers nothing itself, so a
        // grant alone is not evidence that anything can run.
        assert!(
            offered(&mut a).is_empty(),
            "a grant without a hello offers nothing"
        );
        let nothing = offered(&mut a);
        assert_eq!(
            a.decide(&nothing, &[], &BTreeMap::new()),
            None,
            "nothing offered and nothing recorded: silence"
        );

        // a daemon signals: the intersection appears, with the KIND beside it.
        signal(
            &catalog,
            "compute",
            &["agent.claude", "agent.codex", "agent.extra"],
        );
        assert_eq!(
            offered(&mut a),
            tags(&["agent.claude", "agent.codex", "compute"]),
            "the daemon cannot widen the grant, and the kind is announced"
        );

        // ... and the grant cannot widen the daemon either.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["agent.claude", "never-offered"])]);
        let mut a = announcer(9, &workspace, &catalog, caps(8));
        signal(&catalog, "compute", &["agent.claude"]);
        assert_eq!(offered(&mut a), tags(&["agent.claude", "compute"]));

        // a workspace with NO grant at all announces nothing, however loudly a
        // daemon signals: consent is the switch, and an unreadable record is
        // not consent.
        let ungranted = tempfile::tempdir().expect("scratch workspace");
        let mut a = CapabilityAnnouncer::new(
            vec![9u8; 32],
            ungranted.path().to_path_buf(),
            catalog.clone(),
            caps(8),
            Rearm::Silence,
        );
        assert!(
            offered(&mut a).is_empty(),
            "no grant on disk: announce nothing"
        );
    }

    /// DEFECT: the announcer used to read the compute grant and nothing else,
    /// so an agent (or any other) daemon was invisible on chain and no node's
    /// service kind was queryable.
    #[test]
    fn every_granted_and_signaling_kind_is_announced() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[
            ("agent", &["agent.claude"]),
            ("compute", &["codex"]),
            ("storage", &["blob"]),
        ]);
        let mut a = announcer(1, &workspace, &catalog, caps(8));

        // two of the three grants have a live daemon; the third does not.
        signal(&catalog, "agent", &["agent.claude", "agent.codex"]);
        signal(&catalog, "compute", &["codex"]);
        assert_eq!(
            offered(&mut a),
            tags(&["agent", "agent.claude", "codex", "compute"]),
            "both live kinds ride, sorted, with their intersected executors; \
             the granted-but-absent `storage` does not"
        );

        // a kind that signals WITHOUT a grant contributes neither its executors
        // nor its own tag — enable is the consent boundary for both.
        signal(&catalog, "airlock", &["airlock.lend"]);
        assert_eq!(
            offered(&mut a),
            tags(&["agent", "agent.claude", "codex", "compute"]),
            "signaling without consent announces nothing"
        );

        // and the intersection is PER KIND: one daemon's hello can never
        // validate another kind's granted tag.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("agent", &["agent.claude"]), ("compute", &["codex"])]);
        let mut a = announcer(1, &workspace, &catalog, caps(8));
        // agent signals compute's tag; compute itself is silent.
        signal(&catalog, "agent", &["codex"]);
        assert_eq!(
            offered(&mut a),
            tags(&["agent"]),
            "agent's hello does not vouch for compute's granted `codex`"
        );
    }

    /// a granted-and-signaling daemon that offers no executor tags at all (a
    /// plug that spawns nothing) still announces its KIND — that is exactly
    /// what placement asks about.
    #[test]
    fn a_kind_with_no_executors_still_announces_itself() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("airlock", &[])]);
        let mut a = announcer(2, &workspace, &catalog, caps(8));
        signal(&catalog, "airlock", &[]);
        assert_eq!(offered(&mut a), tags(&["airlock"]));
    }

    /// the announce is bounded HOST-SIDE, because crossing `MAX_CAPABILITIES`
    /// is a consensus-level reject — and the kinds survive the trim, since
    /// they are what placement queries.
    #[test]
    fn an_over_cap_offer_is_trimmed_locally_and_keeps_the_kinds() {
        let many: Vec<String> = (0..MAX_ANNOUNCED_TAGS + 10)
            .map(|index| format!("codex.v{index:03}"))
            .collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("agent", &many), ("compute", &many)]);
        let mut a = announcer(3, &workspace, &catalog, caps(8));
        signal(&catalog, "agent", &many);
        signal(&catalog, "compute", &many);

        let set = offered(&mut a);
        assert_eq!(
            set.len(),
            MAX_ANNOUNCED_TAGS,
            "the emitted set never exceeds what the registry accepts"
        );
        assert!(
            set.contains(&"agent".to_string()) && set.contains(&"compute".to_string()),
            "the kinds are kept whatever is dropped"
        );
        assert!(
            a.over_cap,
            "and crossing the cap is latched for one warning"
        );

        // sorted, so it can compare equal to the committed BTreeSet.
        let mut sorted = set.clone();
        sorted.sort();
        assert_eq!(set, sorted);

        // dropping back under the cap clears the latch (one log per transition).
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["codex"])]);
        let mut a = announcer(3, &workspace, &catalog, caps(8));
        signal(&catalog, "compute", &["codex"]);
        a.over_cap = true;
        assert_eq!(offered(&mut a), tags(&["codex", "compute"]));
        assert!(!a.over_cap);
    }

    /// the host-side cap is a MIRROR of a constant that lives in a module
    /// crate; parse the module's source so drift breaks the build lane rather
    /// than the network.
    #[test]
    fn the_announce_cap_matches_the_modules_own() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/modules/system/capability/src/lib.rs"
        ))
        .expect("the capability module's source");
        let declared = source
            .lines()
            .find_map(|line| line.trim().strip_prefix("const MAX_CAPABILITIES: usize = "))
            .expect("capability declares MAX_CAPABILITIES");
        let module_cap: usize = declared
            .trim_end_matches(';')
            .trim()
            .parse()
            .expect("a literal cap");
        assert_eq!(
            module_cap, MAX_ANNOUNCED_TAGS,
            "capability::MAX_CAPABILITIES moved; the host-side bound must move with it \
             (raising it is a flag day — the module's wasm digest is genesis state)"
        );
    }

    /// the `Rearm` bound at a construction site, parsed out of `source`.
    fn bound_rearm(source: &str) -> String {
        source
            .lines()
            .skip_while(|line| !line.contains("CapabilityAnnouncer::new("))
            .take_while(|line| !line.contains(");"))
            .find_map(|line| {
                let at = line.find("Rearm::")?;
                Some(
                    line[at + "Rearm::".len()..]
                        .trim_end_matches(&[',', ' '][..])
                        .to_string(),
                )
            })
            .expect("the construction site binds a Rearm variant")
    }

    /// EACH TIER MUST BIND ITS OWN discriminant, and only two lines in the
    /// tree decide that. Swapping them type-checks and leaves every behaviour
    /// test green — [`the_validator_lane_ignores_the_clock_and_gives_up_on_blocks`]
    /// proves the TYPE behaves, never that a lane HAS the right type.
    ///
    /// The wrong half is not cosmetic. `on_blocks` is called from the drain and
    /// nowhere else, so a resident holding an `Expiry::Unordered` has a budget
    /// nothing ever charges and a `rearm_if_stale` that refuses it by
    /// construction: `in_flight` never clears and the pump is wedged forever on
    /// one lost relay Reply — this file's original defect, on the very lane
    /// whose stated design assumption is that its reply lane is lossy.
    #[test]
    fn each_tier_binds_its_own_rearm() {
        let validator =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/validator/run.rs"))
                .expect("the validator loop's source");
        let resident =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/replica/park.rs"))
                .expect("the resident park loop's source");
        assert_eq!(
            bound_rearm(&validator),
            "Unordered",
            "the validator submits into its OWN orderer and the drain charges \
             `on_blocks`; wall-clock silence there is a stall, not a loss"
        );
        assert_eq!(
            bound_rearm(&resident),
            "Silence",
            "the resident's frame sits in another node's custody and NOTHING calls \
             `on_blocks` for it — an `Unordered` budget here never gets charged and \
             wedges the pump on the first lost Reply"
        );
    }

    /// the drain's announce wiring, guarded at the source: it has no unit-test
    /// seam of its own (the loop needs a live `ValidatorRuntime`), and every
    /// claim below survives deletion with 373 tests and the e2e still green.
    #[test]
    fn the_drain_routes_the_announce_and_suppresses_the_generic_warn() {
        let drain = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/validator/run/drain.rs"
        ))
        .expect("the drain's source");
        // ONE rejection must produce ONE line: the generic per-frame
        // "op rejected in consensus" warn stands down for the frame the
        // announce route already reports with the module detail AND `attempts`.
        assert!(
            drain.contains("!announce_is_ours"),
            "the generic rejection warn must exclude this pump's own announce frame"
        );
        // the ownership question is asked BEFORE the route consumes the latch.
        let owns_at = drain
            .find("announcer.owns(&d.id)")
            .expect("the drain reads ownership before routing");
        let routes_at = drain
            .find("announcer.on_outcome(")
            .expect("the drain routes the announce outcome");
        assert!(
            owns_at < routes_at,
            "`owns` answers nothing once `on_outcome` has taken the latch"
        );
        // all three fates are routed — a new variant must not compile away.
        for fate in ["Fate::Applied", "Fate::Rejected", "Fate::RejectedQuietly"] {
            assert!(drain.contains(fate), "the drain must route {fate}");
        }
        // and the block budget is charged AFTER the per-frame loop, so a fate
        // arriving in this very drain settles the flight before it is billed.
        let charge_at = drain
            .find("announcer.on_blocks(")
            .expect("the drain charges the validator lane's block budget");
        assert!(
            routes_at < charge_at,
            "charging the budget before routing would drop the fate that just arrived"
        );
    }

    /// the TTL retraction, EXERCISED rather than simulated: one catalog, one
    /// hello, and the clock the announcer decides against moved past
    /// [`noded::services::HELLO_TTL`] — so the catalog's own expiry runs. (A
    /// second, empty catalog would assert the same shape while proving
    /// nothing about aging out, which is the interesting half.)
    #[test]
    fn a_daemon_that_stops_signaling_retracts_the_announce() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["agent.claude"])]);
        let mut a = announcer(7, &workspace, &catalog, caps(4));
        let signaled_at = Instant::now();
        signal_at(&catalog, "compute", &["agent.claude"], signaled_at);

        let live = a.offered(signaled_at);
        assert_eq!(
            a.decide(&live, &[], &BTreeMap::new()),
            Some((tags(&["agent.claude", "compute"]), caps(4)))
        );

        // the hello ages out (the daemon died / was stopped): the registry
        // still says we serve it, so the next decision RETRACTS — a node must
        // never leave capacity advertised that nothing can serve. Empty tags
        // force empty resources, which is the module's own rule.
        let aged_out = signaled_at + noded::services::HELLO_TTL + Duration::from_secs(1);
        let stale = a.offered(aged_out);
        assert!(stale.is_empty(), "the expired hello offers nothing");
        assert_eq!(
            a.decide(&stale, &tags(&["agent.claude", "compute"]), &caps(4)),
            Some((Vec::new(), BTreeMap::new())),
            "an absent daemon retracts both the tags and the capacity"
        );
    }

    /// one kind going quiet retracts ITS tags and leaves the other's alone.
    #[test]
    fn an_absent_daemon_retracts_only_its_own_kind() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("agent", &["agent.claude"]), ("compute", &["codex"])]);
        let mut a = announcer(8, &workspace, &catalog, caps(4));
        signal(&catalog, "compute", &["codex"]);
        assert_eq!(
            offered(&mut a),
            tags(&["codex", "compute"]),
            "only the live kind is announced"
        );
    }

    #[test]
    fn re_announces_when_only_resources_drift() {
        // a Podman node: tags already committed, but its announced capacity
        // differs from what the registry holds → re-announce the pair.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["codex"])]);
        let mut a = announcer(1, &workspace, &catalog, caps(8));
        let offered = tags(&["codex"]);
        assert_eq!(
            a.decide(&offered, &tags(&["codex"]), &caps(4)),
            Some((tags(&["codex"]), caps(8))),
            "matching tags but drifted resources still re-announce"
        );
        // once the registry reflects both, it goes quiet.
        assert_eq!(a.decide(&offered, &tags(&["codex"]), &caps(8)), None);
    }

    #[test]
    fn a_direct_backend_announcer_carries_no_resources() {
        // empty capacity (direct spawn): the announce is tags-only, and it
        // never emits resources even when told the registry is bare.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["codex"])]);
        let mut a = announcer(2, &workspace, &catalog, BTreeMap::new());
        let (announced_tags, announced_res) = a
            .decide(&tags(&["codex"]), &[], &BTreeMap::new())
            .expect("a fresh direct node announces its tags");
        assert_eq!(announced_tags, tags(&["codex"]));
        assert!(announced_res.is_empty(), "direct: never any resources");
    }

    #[test]
    fn empty_tags_force_empty_resources() {
        // resources-without-tags is a consensus-level reject: even if a
        // Podman node somehow discovered no executors, it never emits the
        // resources-only shape (and with nothing recorded, it stays silent).
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[]);
        let mut a = announcer(3, &workspace, &catalog, caps(8));
        assert_eq!(
            a.decide(&[], &[], &BTreeMap::new()),
            None,
            "no tags + nothing recorded: genesis silence"
        );
    }

    #[test]
    fn the_in_flight_latch_covers_the_pair() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["codex"])]);
        let mut a = announcer(4, &workspace, &catalog, caps(8));
        let offered = tags(&["codex"]);
        // first decide latches the pair.
        assert_eq!(
            a.decide(&offered, &[], &BTreeMap::new()),
            Some((tags(&["codex"]), caps(8)))
        );
        // an identical decision while it is still in flight stays quiet.
        assert_eq!(
            a.decide(&offered, &[], &BTreeMap::new()),
            None,
            "the latch dedups the exact (tags, resources) pair"
        );
    }

    /// a pump whose decision core has already latched an announce, as it would
    /// be the instant after `maybe_announce` handed a `Msg` to the caller.
    fn decided(rearm: Rearm) -> CapabilityAnnouncer {
        let mut a = CapabilityAnnouncer::new(
            vec![1u8; 32],
            std::path::PathBuf::from("/nonexistent-workspace"),
            noded::services::ServiceCatalog::default(),
            BTreeMap::new(),
            rearm,
        );
        assert_eq!(
            a.decide(&tags(&["codex"]), &[], &BTreeMap::new()),
            Some((tags(&["codex"]), BTreeMap::new())),
            "an empty committed registry decides an announce"
        );
        a
    }

    /// THE DEFECT: an announce that SUBMITS fine and is REJECTED at execute
    /// used to leave the decide latch set forever — the node silently out of
    /// every rendezvous pool with nothing anywhere saying why. The outcome
    /// route un-latches it and the next tick re-decides.
    #[test]
    fn an_execute_rejection_unlatches_and_the_next_tick_re_announces() {
        let mut a = decided(Rearm::Unordered);
        a.sent(frame(1), Instant::now());
        let Some(Fate::Rejected { attempts }) = a.on_outcome(&frame(1), false) else {
            panic!("the first rejection is reported");
        };
        assert_eq!(attempts, 1);
        assert!(a.in_flight.is_none(), "flight settled");
        assert!(
            a.announced.is_none(),
            "rejected: un-latched so the next tick re-decides"
        );
        assert_eq!(
            a.decide(&tags(&["codex"]), &[], &BTreeMap::new()),
            Some((tags(&["codex"]), BTreeMap::new())),
            "and the re-decision announces again"
        );
    }

    /// the doctrine's forever-retry rule: attempt 1, then every Nth, carrying
    /// the count — an unconditional warn on a ~1s round trip evicts the ring.
    #[test]
    fn a_permanently_rejected_announce_reports_attempt_one_then_every_nth() {
        let mut a = decided(Rearm::Unordered);
        let mut reported = Vec::new();
        for round in 1..=FAILURE_REPORT_EVERY * 2 {
            // the pump re-decides and re-submits after every rejection.
            a.decide(&tags(&["codex"]), &[], &BTreeMap::new());
            a.sent(frame(round as u8), Instant::now());
            match a.on_outcome(&frame(round as u8), false) {
                Some(Fate::Rejected { attempts }) => reported.push(attempts),
                Some(Fate::RejectedQuietly) => {}
                Some(Fate::Applied { .. }) | None => panic!("every round is ours and rejected"),
            }
        }
        assert_eq!(
            reported,
            vec![1, FAILURE_REPORT_EVERY, FAILURE_REPORT_EVERY * 2],
            "64 rejections produce THREE warnings, each carrying its count"
        );

        // and an applied fate resets the counter, so the next wedge reports
        // from 1 again rather than staying silent for another N rounds.
        a.decide(&tags(&["codex"]), &[], &BTreeMap::new());
        a.sent(frame(200), Instant::now());
        assert!(matches!(
            a.on_outcome(&frame(200), true),
            Some(Fate::Applied { .. })
        ));
        // a DIFFERENT offered set, so the decide latch does not dedup it.
        a.decide(&tags(&["codex", "quack"]), &[], &BTreeMap::new());
        a.sent(frame(201), Instant::now());
        assert!(matches!(
            a.on_outcome(&frame(201), false),
            Some(Fate::Rejected { attempts: 1 })
        ));
    }

    #[test]
    fn an_applied_outcome_clears_flight_but_keeps_the_decide_latch() {
        let mut a = decided(Rearm::Unordered);
        a.sent(frame(1), Instant::now());
        let Some(Fate::Applied { capabilities }) = a.on_outcome(&frame(1), true) else {
            panic!("applied");
        };
        assert_eq!(
            capabilities,
            tags(&["codex"]),
            "the applied log reports what the FRAME carried, not a fresh read"
        );
        assert!(a.in_flight.is_none(), "flight settled");
        assert!(
            a.announced.is_some(),
            "applied: stay latched until the committed registry confirms"
        );
    }

    #[test]
    fn a_foreign_outcome_is_not_ours_and_changes_nothing() {
        let mut a = decided(Rearm::Unordered);
        a.sent(frame(1), Instant::now());
        assert!(!a.owns(&frame(2)), "somebody else's frame");
        assert!(a.on_outcome(&frame(2), true).is_none(), "not our frame");
        assert!(a.in_flight.is_some(), "the in-flight latch is untouched");
        assert!(a.announced.is_some(), "the decide latch holds");
    }

    #[test]
    fn a_silent_lane_rearms_only_after_the_deadline() {
        let mut a = decided(Rearm::Silence);
        let now = Instant::now();
        a.sent(frame(1), now);

        a.rearm_if_stale(now + ANNOUNCE_RETRY - Duration::from_secs(1));
        assert!(a.in_flight.is_some(), "before the deadline: still waiting");
        assert!(a.announced.is_some());

        // blocks are not this lane's unit: a resident charged in blocks would
        // re-arm off another node's chain progress, which says nothing about
        // its own relayed frame.
        a.on_blocks(ANNOUNCE_RETRY_BLOCKS * 4);
        assert!(a.in_flight.is_some(), "blocks do not charge the relay lane");

        a.rearm_if_stale(now + ANNOUNCE_RETRY);
        assert!(a.in_flight.is_none(), "at the deadline: gave up");
        assert!(
            a.announced.is_none(),
            "un-latched so the next tick re-decides from committed state"
        );
        // and the give-up is COUNTED, so the very next line the operator sees
        // is `reason = announce_reply_lost, attempts = 1` rather than the
        // silence this path used to emit at the default level.
        assert_eq!(a.failures, 1, "abandoning a frame is a reported failure");
    }

    /// THE REGRESSION the hoist introduced: the validator inherited the
    /// resident's 15s wall-clock deadline, so a chain that took longer than
    /// that to finalize (a stall, a cutover, view churn) drew a DUPLICATE
    /// announce every 15s — each landing in the same `outstanding`, the
    /// superseded frame's outcome then matching nothing. The authoritative
    /// lane is charged in BLOCKS, which a stall does not produce.
    #[test]
    fn the_validator_lane_ignores_the_clock_and_gives_up_on_blocks() {
        let mut a = decided(Rearm::Unordered);
        let now = Instant::now();
        a.sent(frame(1), now);

        a.rearm_if_stale(now + ANNOUNCE_RETRY * 1000);
        assert!(
            a.in_flight.is_some(),
            "a stalled chain must NOT draw a duplicate announce, however long it stalls"
        );

        a.on_blocks(ANNOUNCE_RETRY_BLOCKS - 1);
        assert!(a.in_flight.is_some(), "within budget: still waiting");

        a.on_blocks(1);
        assert!(
            a.in_flight.is_none(),
            "blocks went by without our frame among them: that IS the loss"
        );
        assert!(a.announced.is_none(), "and the next tick re-decides");
        assert_eq!(a.failures, 1, "abandoning a frame is a reported failure");

        // the give-up path shares the rejection throttle rather than running a
        // second one: a node failing to enter the registry is ONE story to an
        // operator however the individual frames died, and a silent forever-
        // retry was this round's worst regression.
        for _ in 1..FAILURE_REPORT_EVERY {
            a.decide(&tags(&["codex"]), &[], &BTreeMap::new());
            a.sent(frame(2), Instant::now());
            a.on_blocks(ANNOUNCE_RETRY_BLOCKS);
        }
        assert_eq!(
            a.failures, FAILURE_REPORT_EVERY,
            "every abandonment counts toward the same attempt-1-then-every-Nth report"
        );
    }

    #[test]
    fn a_submit_failure_unlatches_immediately() {
        let mut a = decided(Rearm::Silence);
        a.sent(frame(1), Instant::now());
        a.submit_failed();
        assert!(a.in_flight.is_none());
        assert!(a.announced.is_none());
    }

    /// DEFECT: the daemon hello boundary admits any printable ascii plus a
    /// space, and `service enable` copies those tags into `services.toml`
    /// verbatim — so a third-party daemon's `"Claude Sonnet"` reached the
    /// announce intact, consensus rejected the whole (all-or-nothing) frame,
    /// and the outcome route turned that into a forever retry that also
    /// suppressed the node's LEGAL tags. It is refused host-side now.
    #[test]
    fn an_illegal_tag_is_refused_host_side_and_the_legal_ones_survive() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace =
            granted_workspace(&[("compute", &["Claude Sonnet", "agent.claude", "UPPER"])]);
        let mut a = announcer(5, &workspace, &catalog, caps(8));
        // the hello boundary admits all three — that is the looser copy of
        // the rule this filter exists to absorb.
        signal(
            &catalog,
            "compute",
            &["Claude Sonnet", "agent.claude", "UPPER"],
        );

        assert_eq!(
            offered(&mut a),
            tags(&["agent.claude", "compute"]),
            "the illegal tags are dropped; the legal ones (and the kind) ride"
        );
        assert!(a.illegal_tags, "and the refusal is latched for one warning");
        for tag in offered(&mut a) {
            assert!(
                capability::validate_tag(&tag).is_ok(),
                "nothing the registry would reject is ever emitted: {tag:?}"
            );
        }

        // a daemon that cleans up its spec clears the latch (one log per
        // transition, not one per drain tick).
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["agent.claude"])]);
        let mut a = announcer(5, &workspace, &catalog, caps(8));
        signal(&catalog, "compute", &["agent.claude"]);
        a.illegal_tags = true;
        assert_eq!(offered(&mut a), tags(&["agent.claude", "compute"]));
        assert!(!a.illegal_tags);
    }
}
