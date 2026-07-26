use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use host::Host;
use sdk::Msg;

/// how long a submitted announce may await its consensus fate before the pump
/// un-latches and re-decides from committed state.
///
/// BOTH tiers need this and for the same reason, though they reach consensus
/// differently: a resident's frame rides the LOSSY relay lane (this sits
/// comfortably above that lane's 10s SUBMIT_HOLD, so a swept-but-applied frame
/// usually shows up in the registry before the deadline fires and the state
/// read stays quiet), and a validator's rides its own drain — authoritative,
/// but not a thing to bet permanent silence on. A duplicate announce is
/// harmless: the module applies a declarative replace, so a re-send converges
/// on the same state.
const ANNOUNCE_RETRY: Duration = Duration::from_secs(15);

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
    /// the submitted announce awaiting its consensus fate: the frame's content
    /// address plus the give-up deadline.
    in_flight: Option<(node::FrameId, Instant)>,
    /// whether the last grant read failed — latched so a corrupt
    /// `services.toml` is reported once, not once per drain tick.
    grant_unreadable: bool,
    /// whether the offered set last exceeded [`MAX_ANNOUNCED_TAGS`] — latched
    /// for the same reason.
    over_cap: bool,
}

impl CapabilityAnnouncer {
    pub(crate) fn new(
        me: Vec<u8>,
        workspace: std::path::PathBuf,
        services: noded::services::ServiceCatalog,
        resources: BTreeMap<String, u64>,
    ) -> Self {
        Self {
            me,
            workspace,
            services,
            resources,
            announced: None,
            in_flight: None,
            grant_unreadable: false,
            over_cap: false,
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
    pub(crate) fn offered(&mut self) -> Vec<String> {
        let signaling = self.services.live(Instant::now());
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
            let offers_now: BTreeSet<&str> =
                live.capabilities.iter().map(String::as_str).collect();
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
        self.within_cap(kinds, executors)
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
    /// the pump stays quiet while its consensus fate is pending.
    pub(crate) fn sent(&mut self, frame: node::FrameId, now: Instant) {
        self.in_flight = Some((frame, now + ANNOUNCE_RETRY));
    }

    /// the SUBMIT itself failed (a validator's local submit errored, a
    /// resident's relay knew no validator): un-latch immediately so the next
    /// tick retries.
    pub(crate) fn submit_failed(&mut self) {
        self.in_flight = None;
        self.announced = None;
    }

    /// this announce's CONSENSUS fate. `Some(applied)` when `frame` is the one
    /// this pump submitted (the caller logs it), `None` when it belongs to
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
    pub(crate) fn on_outcome(&mut self, frame: &node::FrameId, applied: bool) -> Option<bool> {
        let is_ours = self.in_flight.as_ref().is_some_and(|(id, _)| id == frame);
        if !is_ours {
            return None;
        }
        self.in_flight = None;
        if !applied {
            self.announced = None;
        }
        Some(applied)
    }

    /// deadline liveness: a frame whose fate never arrived (dropped reply,
    /// crashed relay target, swept hold) stops blocking once its deadline
    /// passes — un-latch and let the next decision re-read the committed
    /// registry, which is quiet if the announce actually landed.
    fn rearm_if_stale(&mut self, now: Instant) {
        let gave_up = self
            .in_flight
            .as_ref()
            .is_some_and(|(_, deadline)| now >= *deadline);
        if gave_up {
            self.submit_failed();
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
        let offered = self.offered();
        let (capabilities, resources) = self.decide(&offered, &committed_tags, &committed_resources)?;
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

    /// an announcer over `workspace` and `catalog`.
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
        )
    }

    /// register a live hello for `kind` offering `offered`.
    fn signal(catalog: &noded::services::ServiceCatalog, kind: &str, offered: &[&str]) {
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
                Instant::now(),
            )
            .expect("a matching-build hello is admitted");
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
        assert!(a.offered().is_empty(), "a grant without a hello offers nothing");
        let offered = a.offered();
        assert_eq!(
            a.decide(&offered, &[], &BTreeMap::new()),
            None,
            "nothing offered and nothing recorded: silence"
        );

        // a daemon signals: the intersection appears, with the KIND beside it.
        signal(&catalog, "compute", &["agent.claude", "agent.codex", "agent.extra"]);
        assert_eq!(
            a.offered(),
            tags(&["agent.claude", "agent.codex", "compute"]),
            "the daemon cannot widen the grant, and the kind is announced"
        );

        // ... and the grant cannot widen the daemon either.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["agent.claude", "never-offered"])]);
        let mut a = announcer(9, &workspace, &catalog, caps(8));
        signal(&catalog, "compute", &["agent.claude"]);
        assert_eq!(a.offered(), tags(&["agent.claude", "compute"]));

        // a workspace with NO grant at all announces nothing, however loudly a
        // daemon signals: consent is the switch, and an unreadable record is
        // not consent.
        let ungranted = tempfile::tempdir().expect("scratch workspace");
        let mut a = CapabilityAnnouncer::new(
            vec![9u8; 32],
            ungranted.path().to_path_buf(),
            catalog.clone(),
            caps(8),
        );
        assert!(a.offered().is_empty(), "no grant on disk: announce nothing");
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
            a.offered(),
            tags(&["agent", "agent.claude", "codex", "compute"]),
            "both live kinds ride, sorted, with their intersected executors; \
             the granted-but-absent `storage` does not"
        );

        // a kind that signals WITHOUT a grant contributes neither its executors
        // nor its own tag — enable is the consent boundary for both.
        signal(&catalog, "airlock", &["airlock.lend"]);
        assert_eq!(
            a.offered(),
            tags(&["agent", "agent.claude", "codex", "compute"]),
            "signaling without consent announces nothing"
        );

        // and the intersection is PER KIND: one daemon's hello can never
        // validate another kind's granted tag.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace =
            granted_workspace(&[("agent", &["agent.claude"]), ("compute", &["codex"])]);
        let mut a = announcer(1, &workspace, &catalog, caps(8));
        // agent signals compute's tag; compute itself is silent.
        signal(&catalog, "agent", &["codex"]);
        assert_eq!(
            a.offered(),
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
        assert_eq!(a.offered(), tags(&["airlock"]));
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
        let workspace =
            granted_workspace(&[("agent", &many), ("compute", &many)]);
        let mut a = announcer(3, &workspace, &catalog, caps(8));
        signal(&catalog, "agent", &many);
        signal(&catalog, "compute", &many);

        let offered = a.offered();
        assert_eq!(
            offered.len(),
            MAX_ANNOUNCED_TAGS,
            "the emitted set never exceeds what the registry accepts"
        );
        assert!(
            offered.contains(&"agent".to_string()) && offered.contains(&"compute".to_string()),
            "the kinds are kept whatever is dropped"
        );
        assert!(a.over_cap, "and crossing the cap is latched for one warning");

        // sorted, so it can compare equal to the committed BTreeSet.
        let mut sorted = offered.clone();
        sorted.sort();
        assert_eq!(offered, sorted);

        // dropping back under the cap clears the latch (one log per transition).
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["codex"])]);
        let mut a = announcer(3, &workspace, &catalog, caps(8));
        signal(&catalog, "compute", &["codex"]);
        a.over_cap = true;
        assert_eq!(a.offered(), tags(&["codex", "compute"]));
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

    #[test]
    fn a_daemon_that_stops_signaling_retracts_the_announce() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&[("compute", &["agent.claude"])]);
        let mut a = announcer(7, &workspace, &catalog, caps(4));
        signal(&catalog, "compute", &["agent.claude"]);
        let offered = a.offered();
        assert_eq!(
            a.decide(&offered, &[], &BTreeMap::new()),
            Some((tags(&["agent.claude", "compute"]), caps(4)))
        );

        // the hello ages out (the daemon died / was stopped): the registry
        // still says we serve it, so the next decision RETRACTS — a node must
        // never leave capacity advertised that nothing can serve. Empty tags
        // force empty resources, which is the module's own rule.
        let expired = noded::services::ServiceCatalog::default();
        let mut a = announcer(7, &workspace, &expired, caps(4));
        let offered = a.offered();
        assert_eq!(
            a.decide(&offered, &tags(&["agent.claude", "compute"]), &caps(4)),
            Some((Vec::new(), BTreeMap::new())),
            "an absent daemon retracts both the tags and the capacity"
        );
    }

    /// one kind going quiet retracts ITS tags and leaves the other's alone.
    #[test]
    fn an_absent_daemon_retracts_only_its_own_kind() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace =
            granted_workspace(&[("agent", &["agent.claude"]), ("compute", &["codex"])]);
        let mut a = announcer(8, &workspace, &catalog, caps(4));
        signal(&catalog, "compute", &["codex"]);
        assert_eq!(
            a.offered(),
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
    fn decided() -> CapabilityAnnouncer {
        let mut a = CapabilityAnnouncer::new(
            vec![1u8; 32],
            std::path::PathBuf::from("/nonexistent-workspace"),
            noded::services::ServiceCatalog::default(),
            BTreeMap::new(),
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
        let mut a = decided();
        a.sent(frame(1), Instant::now());
        assert_eq!(a.on_outcome(&frame(1), false), Some(false));
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

    #[test]
    fn an_applied_outcome_clears_flight_but_keeps_the_decide_latch() {
        let mut a = decided();
        a.sent(frame(1), Instant::now());
        assert_eq!(a.on_outcome(&frame(1), true), Some(true));
        assert!(a.in_flight.is_none(), "flight settled");
        assert!(
            a.announced.is_some(),
            "applied: stay latched until the committed registry confirms"
        );
    }

    #[test]
    fn a_foreign_outcome_is_not_ours_and_changes_nothing() {
        let mut a = decided();
        a.sent(frame(1), Instant::now());
        assert_eq!(a.on_outcome(&frame(2), true), None, "not our frame");
        assert!(a.in_flight.is_some(), "the in-flight latch is untouched");
        assert!(a.announced.is_some(), "the decide latch holds");
    }

    #[test]
    fn a_silent_lane_rearms_only_after_the_deadline() {
        let mut a = decided();
        let now = Instant::now();
        a.sent(frame(1), now);

        a.rearm_if_stale(now + ANNOUNCE_RETRY - Duration::from_secs(1));
        assert!(a.in_flight.is_some(), "before the deadline: still waiting");
        assert!(a.announced.is_some());

        a.rearm_if_stale(now + ANNOUNCE_RETRY);
        assert!(a.in_flight.is_none(), "at the deadline: gave up");
        assert!(
            a.announced.is_none(),
            "un-latched so the next tick re-decides from committed state"
        );
    }

    #[test]
    fn a_submit_failure_unlatches_immediately() {
        let mut a = decided();
        a.sent(frame(1), Instant::now());
        a.submit_failed();
        assert!(a.in_flight.is_none());
        assert!(a.announced.is_none());
    }
}
