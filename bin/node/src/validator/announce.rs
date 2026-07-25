use std::collections::BTreeMap;

use host::Host;
use sdk::Msg;

/// the capability self-announcer: it polls the committed
/// registry each pump tick and, when this node's announced set differs from
/// what it can truthfully offer, self-submits ONE declarative
/// [`CapabilityMsg::Announce`]. state-driven (survives restart/late-join) and
/// idempotent: once the committed set matches, it stays quiet. a node with no
/// providers announces nothing.
///
/// ## where the offered set comes from
///
/// The node discovers nothing any more — the compute plane is a standalone
/// daemon — so the offered set is `grant ∩ live hello`:
///
/// - **grant**: the tags the user reviewed and consented to at `service enable`
///   (`services.toml`). Consent can only narrow.
/// - **live hello**: what a daemon is signaling to this node RIGHT NOW
///   ([`noded::services::ServiceCatalog`]). Truth can only narrow.
///
/// Neither side may widen the other, and BOTH are re-read every tick — so a
/// node holding a grant with no daemon signaling announces NOTHING, a stopped
/// daemon retracts within the hello TTL, and `service enable`/`disable` take
/// effect without restarting the node. (Re-reading `services.toml` per tick is
/// free beside the two committed queries this pump already issues.)
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
    /// the (tags, resources) pair we last SUBMITTED (not yet observed
    /// committed), latched so an in-flight announce is not re-sent every tick.
    pub(crate) announced: Option<(Vec<String>, BTreeMap<String, u64>)>,
    /// whether the last grant read failed — latched so a corrupt
    /// `services.toml` is reported once, not once per drain tick.
    grant_unreadable: bool,
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
            grant_unreadable: false,
        }
    }

    /// the tags the user's compute grant consents to right now.
    ///
    /// An absent record grants nothing — the ordinary un-enabled node. An
    /// UNREADABLE one also announces nothing (consent that cannot be read is
    /// not consent), but it says so: silently retracting a live node's whole
    /// announce because someone corrupted a toml is exactly the failure that
    /// must not be quiet. Latched, because this runs on the drain tick.
    fn granted(&mut self) -> Vec<String> {
        match crate::services::grant_for(&self.workspace, crate::services::COMPUTE_KIND) {
            Ok(grant) => {
                if self.grant_unreadable {
                    self.grant_unreadable = false;
                    tracing::info!(target: "ducktape::service", "compute grant readable again");
                }
                grant.map(|grant| grant.capabilities).unwrap_or_default()
            }
            Err(error) => {
                if !self.grant_unreadable {
                    self.grant_unreadable = true;
                    tracing::warn!(
                        target: "ducktape::service",
                        reason = "grant_unreadable",
                        "compute grant cannot be read; this node announces nothing until it is \
                         repaired: {error}"
                    );
                }
                Vec::new()
            }
        }
    }

    /// what this node may truthfully announce right now: the user's grant
    /// INTERSECTED with what a daemon is currently offering over its hello.
    ///
    /// The live half is read FIRST and short-circuits: with nothing signaling
    /// the intersection is empty whatever the grant says, so the common case (a
    /// node with no compute daemon) never touches the disk — this runs on the
    /// async drain tick at ~10 Hz.
    pub(crate) fn offered(&mut self) -> Vec<String> {
        let signaling = self.services.live(std::time::Instant::now());
        let live: std::collections::BTreeSet<&str> = signaling
            .iter()
            .flat_map(|entry| entry.capabilities.iter().map(String::as_str))
            .collect();
        if live.is_empty() {
            return Vec::new();
        }
        self.granted()
            .into_iter()
            .filter(|tag| live.contains(tag.as_str()))
            .collect()
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

    /// a workspace whose `services.toml` grants `granted` to compute. The
    /// announcer reads consent off disk per tick, so a test grant IS a file.
    fn granted_workspace(granted: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("scratch workspace");
        let capabilities = granted
            .iter()
            .map(|tag| format!("    {tag:?},"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(
            dir.path().join(crate::services::FILE_NAME),
            format!(
                "version = 1\n\n[[service]]\nkind = \"compute\"\n\
                 instance = {:?}\nnonce = {:?}\ngranted_unix = 1\n\
                 capabilities = [\n{capabilities}\n]\nscopes = []\n",
                "aa".repeat(32),
                "bb".repeat(16),
            ),
        )
        .expect("write the grant");
        dir
    }

    /// an announcer whose grant is `granted` and whose catalog is empty.
    fn announcer(
        me: u8,
        workspace: &tempfile::TempDir,
        resources: BTreeMap<String, u64>,
    ) -> CapabilityAnnouncer {
        CapabilityAnnouncer::new(
            vec![me; 32],
            workspace.path().to_path_buf(),
            noded::services::ServiceCatalog::default(),
            resources,
        )
    }

    /// register a live compute hello offering `offered`.
    fn signal(catalog: &noded::services::ServiceCatalog, offered: &[&str]) {
        catalog
            .hello(
                noded::services::Hello {
                    kind: "compute".into(),
                    version: "1".into(),
                    build: noded::services::build_identity()
                        .expect("tests run from a git checkout")
                        .into(),
                    capabilities: tags(offered),
                    scopes: Vec::new(),
                    needs: Vec::new(),
                },
                std::time::Instant::now(),
            )
            .expect("a matching-build hello is admitted");
    }

    #[test]
    fn the_offered_set_is_the_grant_intersected_with_a_live_hello() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&["agent.claude", "agent.codex"]);
        let mut a = CapabilityAnnouncer::new(
            vec![9u8; 32],
            workspace.path().to_path_buf(),
            catalog.clone(),
            caps(8),
        );

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

        // a daemon signals: the intersection appears.
        signal(&catalog, &["agent.claude", "agent.codex", "agent.extra"]);
        assert_eq!(
            a.offered(),
            tags(&["agent.claude", "agent.codex"]),
            "the daemon cannot widen the grant"
        );

        // ... and the grant cannot widen the daemon either.
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&["agent.claude", "never-offered"]);
        let mut a = CapabilityAnnouncer::new(
            vec![9u8; 32],
            workspace.path().to_path_buf(),
            catalog.clone(),
            caps(8),
        );
        signal(&catalog, &["agent.claude"]);
        assert_eq!(a.offered(), tags(&["agent.claude"]));

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

    #[test]
    fn a_daemon_that_stops_signaling_retracts_the_announce() {
        let catalog = noded::services::ServiceCatalog::default();
        let workspace = granted_workspace(&["agent.claude"]);
        let mut a = CapabilityAnnouncer::new(
            vec![7u8; 32],
            workspace.path().to_path_buf(),
            catalog.clone(),
            caps(4),
        );
        signal(&catalog, &["agent.claude"]);
        let offered = a.offered();
        assert_eq!(
            a.decide(&offered, &[], &BTreeMap::new()),
            Some((tags(&["agent.claude"]), caps(4)))
        );

        // the hello ages out (the daemon died / was stopped): the registry
        // still says we serve it, so the next decision RETRACTS — a node must
        // never leave capacity advertised that nothing can serve. Empty tags
        // force empty resources, which is the module's own rule.
        let expired = noded::services::ServiceCatalog::default();
        let mut a =
            CapabilityAnnouncer::new(vec![7u8; 32], workspace.path().to_path_buf(), expired, caps(4));
        let offered = a.offered();
        assert_eq!(
            a.decide(&offered, &tags(&["agent.claude"]), &caps(4)),
            Some((Vec::new(), BTreeMap::new())),
            "an absent daemon retracts both the tags and the capacity"
        );
    }

    #[test]
    fn re_announces_when_only_resources_drift() {
        // a Podman node: tags already committed, but its announced capacity
        // differs from what the registry holds → re-announce the pair.
        let workspace = granted_workspace(&["codex"]);
        let mut a = announcer(1, &workspace, caps(8));
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
        let workspace = granted_workspace(&["codex"]);
        let mut a = announcer(2, &workspace, BTreeMap::new());
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
        let workspace = granted_workspace(&[]);
        let mut a = announcer(3, &workspace, caps(8));
        assert_eq!(
            a.decide(&[], &[], &BTreeMap::new()),
            None,
            "no tags + nothing recorded: genesis silence"
        );
    }

    #[test]
    fn the_in_flight_latch_covers_the_pair() {
        let workspace = granted_workspace(&["codex"]);
        let mut a = announcer(4, &workspace, caps(8));
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
}
