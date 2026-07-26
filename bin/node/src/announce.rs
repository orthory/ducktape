//! The capability announce: what this node tells the network it can execute.
//!
//! Two facts feed it, and they are deliberately NOT the same fact:
//!
//! - **consent** — the operator approved kind K on this node (`services.toml`,
//!   written by `service enable`/`disable`). Durable, and it changes only when
//!   a human runs a verb.
//! - **liveness** — K's daemon is signaling right now (the volatile
//!   [`noded::services::ServiceCatalog`], 30 s TTL). Ephemeral, and known only
//!   to this node.
//!
//! The announce is their intersection, so it has two triggers and ONE writer:
//!
//! - the **verb** submits on a consent change, synchronously, and reports the
//!   commit height to the human who ran it ([`crate::services::enable`]);
//! - the **watcher** below submits on a liveness transition ([`spawn`]).
//!
//! Both go through the SAME door — `POST /v1/submit`, which frames the op with
//! the node's own key and answers only once consensus has settled. Nothing here
//! submits through `OrderedNode::submit`, and that is the whole design: that
//! call is fire-and-forget by construction and lives inside the drain loop that
//! produces the very commits it would have to await, which is why the pump this
//! module replaces needed a frame latch, a two-tier give-up budget and an
//! outcome route back from the drain. A blocking POST from outside the loop
//! needs none of them.
//!
//! No daemon ever submits an announce: N daemons each doing a declarative
//! replace of one shared tag set would clobber each other. One writer, always
//! the node.

use std::collections::{BTreeMap, BTreeSet};

use crate::services::ServiceGrant;

/// the most tags one node may announce.
///
/// This is `capability`'s own `MAX_CAPABILITIES`, mirrored HOST-SIDE: the
/// module's constant is private to a crate under `crates/modules/`, and merely
/// making it `pub` would rebuild its `component.wasm`, move the Lifecycle
/// digest it is seeded with, and so move the genesis app hash — a flag day for
/// a visibility keyword. `the_announce_cap_matches_the_modules_own` parses the
/// module's source to pin the two together; a comment would not have.
const MAX_ANNOUNCED_TAGS: usize = 64;

/// what one announce carries.
///
/// The pair travels as one value because the module couples them: resources
/// without tags is a consensus-level reject, so they must never be decided in
/// two places that could disagree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnnounceSet {
    pub(crate) capabilities: Vec<String>,
    pub(crate) resources: BTreeMap<String, u64>,
}

/// why a set cannot be announced. A closed domain: both arms are conditions the
/// consent boundary refuses OUTRIGHT rather than silently trimming, because the
/// alternative is an operator who approved a consent screen listing tags this
/// node will never announce.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// tags the registry's own grammar rejects. The hello boundary's item rule
    /// is LOOSER than `capability::validate_tag` (it admits any printable ascii
    /// plus a space), so a third-party daemon or an operator spec dir can
    /// signal `"Claude Sonnet"` and reach this point intact.
    IllegalTags(Vec<String>),
    /// more tags than the registry accepts. An announce is all-or-nothing, so
    /// crossing the cap does not cost the excess — it costs the whole set.
    OverCap { total: usize },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::IllegalTags(tags) => write!(
                f,
                "the capability registry refuses {} (a tag is 1..64 bytes of [a-z0-9._-]) — \
                 fix the daemon's capability spec, then signal again",
                tags.join(", ")
            ),
            Refusal::OverCap { total } => write!(
                f,
                "this node would announce {total} capability tags and the registry accepts at \
                 most {MAX_ANNOUNCED_TAGS} — narrow a grant or the capability spec dir"
            ),
        }
    }
}

/// THE decision: what this node may truthfully announce, given what the operator
/// consented to and what is signaling right now.
///
/// For every granted kind whose daemon is live, the kind's own tag rides beside
/// the grant's executor tags intersected with what that SAME kind's hello offers
/// — so neither side can widen the other. A grant with no live daemon
/// contributes nothing; a live daemon with no grant contributes nothing.
///
/// The kind tag rides even when the executor intersection is empty: a daemon
/// that spawns nothing (an airlock plug) still IS that kind, and that is
/// precisely what placement asks about.
///
/// PURE — no clock, no disk, no logging, no `self`. Both triggers call it, which
/// is what keeps the verb's consent screen and the watcher's op describing the
/// same node.
pub(crate) fn announced_set(
    grants: &[ServiceGrant],
    signaling: &[noded::services::Signaling],
    capacity: &BTreeMap<String, u64>,
) -> Result<AnnounceSet, Refusal> {
    let mut tags: BTreeSet<String> = BTreeSet::new();
    for live in signaling {
        // signaling WITHOUT consent offers nothing at all — not its executors
        // and not its kind. Enable is the switch.
        let Some(grant) = grants.iter().find(|grant| grant.kind == live.kind) else {
            continue;
        };
        let offers_now: BTreeSet<&str> = live.capabilities.iter().map(String::as_str).collect();
        tags.insert(live.kind.clone());
        tags.extend(
            grant
                .capabilities
                .iter()
                .filter(|tag| offers_now.contains(tag.as_str()))
                .cloned(),
        );
    }

    // the registry's own rule, applied before anything is submitted. The KIND
    // tags need no separate check: the hello's kind grammar (1..32 of
    // [a-z0-9-]) is a strict subset of the tag rule.
    let illegal: Vec<String> = tags
        .iter()
        .filter(|tag| capability::validate_tag(tag).is_err())
        .cloned()
        .collect();
    if !illegal.is_empty() {
        return Err(Refusal::IllegalTags(illegal));
    }
    if tags.len() > MAX_ANNOUNCED_TAGS {
        return Err(Refusal::OverCap { total: tags.len() });
    }

    // empty tags force empty resources — the module rejects the
    // resources-without-tags shape, so it is never emitted.
    let resources = match tags.is_empty() {
        true => BTreeMap::new(),
        false => capacity.clone(),
    };
    // sorted and deduplicated by construction, and that is load-bearing rather
    // than tidy: the committed registry answers a `BTreeSet` rendered in order,
    // so an unsorted set could never compare equal to it and the watcher would
    // submit forever.
    Ok(AnnounceSet {
        capabilities: tags.into_iter().collect(),
        resources,
    })
}

/// Submit one announce through this node's own `/v1/submit` and return the
/// height it committed at.
///
/// The node re-frames the op with ITS key, which is the registry's identity
/// (`capability` takes the announcing node from the verified submit origin,
/// never from payload data). That is why no caller here holds a signing key —
/// and why a user-signed announce could never have worked.
pub(crate) fn submit(base: &str, set: &AnnounceSet) -> Result<u64, String> {
    let msg = capability::CapabilityMsg::Announce {
        capabilities: set.capabilities.clone(),
        resources: set.resources.clone(),
    };
    let payload = serde_json::to_value(&msg).map_err(|error| error.to_string())?;
    crate::node_http::submit(base, "capability", &payload).map_err(|error| error.to_string())
}

/// This node's committed announce, read once to seed the watcher.
fn committed(base: &str, node_key: &[u8]) -> Option<AnnounceSet> {
    use capability::{CapabilityQuery, CapabilityReply};
    let ask = |query: CapabilityQuery| -> Option<CapabilityReply> {
        let value = serde_json::to_value(&query).ok()?;
        serde_json::from_value(crate::node_http::query(base, "capability", value).ok()?).ok()
    };
    let node = node_key.to_vec();
    let CapabilityReply::Node(capabilities) = ask(CapabilityQuery::Node { node: node.clone() })?
    else {
        return None;
    };
    let CapabilityReply::Resources(resources) = ask(CapabilityQuery::Resources { node })? else {
        return None;
    };
    Some(AnnounceSet {
        capabilities,
        resources,
    })
}

/// how often the watcher re-derives the announce.
///
/// The same period the daemons beat at (`HELLO_TTL / 3`), so a death is noticed
/// within roughly one TTL of the catalog entry lapsing and no faster — sampling
/// quicker would only re-read a catalog that cannot have changed conclusion.
const TICK: std::time::Duration =
    std::time::Duration::from_secs(noded::services::HELLO_TTL.as_secs() / 3);

/// a failure is reported on the FIRST occurrence and every Nth after it,
/// carrying the count. The doctrine's rule for a forever-retry loop: an
/// unconditional `warn!` on a 10 s tick evicts the whole 4096-line ring in
/// half a day and destroys the evidence someone came to read — and the counter
/// IS the diagnosis.
const REPORT_EVERY: u64 = 32;

/// everything the watcher needs. Owned, because it outlives the boot frame it
/// is built in.
pub(crate) struct Watch {
    /// this node's own `/v1` base — the only transport it uses.
    pub(crate) base: String,
    /// this node's consensus public key, to seed from the committed registry.
    pub(crate) node_key: Vec<u8>,
    /// the workspace whose `services.toml` carries the operator's consent.
    pub(crate) workspace: std::path::PathBuf,
    /// the volatile signaling catalog — the live half.
    pub(crate) services: noded::services::ServiceCatalog,
    /// the capacity announced beside the tags: probed host totals from the
    /// `[sandbox]` table, EMPTY on a node that configures no isolation.
    pub(crate) capacity: BTreeMap<String, u64>,
}

/// Start the liveness watcher on its own OS thread.
///
/// A plain thread with a blocking client, exactly like the daemons' own
/// heartbeat: the node's host must never leave the commonware runner thread,
/// and this wants to BLOCK on a settling submit — which is the one thing a task
/// on that thread must not do.
pub(crate) fn spawn(watch: Watch) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("announce-watch".into())
        .spawn(move || run(watch))
        .map(|_| ())
}

fn run(watch: Watch) {
    // what the registry holds for this node, as far as this thread knows.
    let mut last: Option<AnnounceSet> = None;
    let mut failures: u64 = 0;
    loop {
        std::thread::sleep(TICK);
        // ONE committed read, retried until the node can answer it, and then
        // never again. Retried rather than given up on because an unseeded
        // watcher has no idea what the registry holds, and a guess is worse
        // than a wait in BOTH directions: guessing "empty" would submit a
        // pointless announce on every node boot — and on a node not yet
        // admitted, one per tick forever — while guessing "whatever we
        // compute" would leave a dead daemon's tags standing after a restart.
        // A booting node answers within a tick or two; until then this thread
        // does nothing at all, which is exactly right.
        if last.is_none() {
            let Some(seed) = committed(&watch.base, &watch.node_key) else {
                continue;
            };
            last = Some(seed);
        }
        let want = match desired(&watch) {
            Ok(want) => want,
            Err(reason) => {
                failures += 1;
                report(failures, &reason);
                continue;
            }
        };
        if last.as_ref() == Some(&want) {
            continue;
        }
        match submit(&watch.base, &want) {
            Ok(height) => {
                failures = 0;
                tracing::info!(
                    target: "ducktape::service",
                    height,
                    capabilities = ?want.capabilities,
                    "capabilities announced"
                );
                last = Some(want);
            }
            // `last` is deliberately NOT advanced: the next tick retries, which
            // is what a node that has not been admitted yet needs. Bounded by
            // the tick plus the submit's own hold, so ~one attempt per 20 s.
            Err(reason) => {
                failures += 1;
                report(failures, &reason);
            }
        }
    }
}

/// the announce this node would emit right now. Reads the two live inputs and
/// delegates the decision; it decides nothing itself.
fn desired(watch: &Watch) -> Result<AnnounceSet, String> {
    let signaling = watch.services.live(std::time::Instant::now());
    // consent that cannot be read is not consent — but silently retracting a
    // live node's whole announce because someone corrupted a toml is exactly
    // the failure that must not be quiet, so this is an error, not an empty set.
    let services = crate::services::load(&watch.workspace)?;
    announced_set(&services.grants, &signaling, &watch.capacity)
        .map_err(|refusal| refusal.to_string())
}

fn report(failures: u64, reason: &str) {
    let first = failures == 1;
    let every_nth = failures.is_multiple_of(REPORT_EVERY);
    if !first && !every_nth {
        return;
    }
    tracing::warn!(
        target: "ducktape::service",
        attempts = failures,
        reason = "announce_failed",
        "this node is not announcing what it offers; retrying: {reason}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn caps(cores: u64) -> BTreeMap<String, u64> {
        BTreeMap::from([("cores".to_string(), cores)])
    }

    fn grant(kind: &str, capabilities: &[&str]) -> ServiceGrant {
        ServiceGrant {
            kind: kind.into(),
            instance: "aa".repeat(32),
            nonce: "bb".repeat(16),
            granted_unix: 1,
            capabilities: tags(capabilities),
            scopes: Vec::new(),
        }
    }

    fn signal(kind: &str, capabilities: &[&str]) -> noded::services::Signaling {
        noded::services::Signaling {
            kind: kind.into(),
            version: "1".into(),
            build: "b".into(),
            capabilities: tags(capabilities),
            scopes: Vec::new(),
            needs: Vec::new(),
        }
    }

    #[test]
    fn the_set_is_the_grant_intersected_with_a_live_hello() {
        // a grant with NO daemon signaling offers nothing: the node discovers
        // nothing itself, so consent alone is not evidence anything can run.
        let grants = [grant("compute", &["claude", "codex"])];
        let set = announced_set(&grants, &[], &caps(8)).unwrap();
        assert!(set.capabilities.is_empty(), "a grant without a hello offers nothing");
        assert!(set.resources.is_empty(), "empty tags force empty resources");

        // a daemon signals: the intersection appears, kind tag included, and
        // the daemon cannot widen the grant.
        let live = [signal("compute", &["claude", "codex", "extra"])];
        let set = announced_set(&grants, &live, &caps(8)).unwrap();
        assert_eq!(set.capabilities, tags(&["claude", "codex", "compute"]));
        assert_eq!(set.resources, caps(8));

        // ... and the grant cannot widen the daemon either.
        let grants = [grant("compute", &["claude", "never-offered"])];
        let live = [signal("compute", &["claude"])];
        let set = announced_set(&grants, &live, &caps(8)).unwrap();
        assert_eq!(set.capabilities, tags(&["claude", "compute"]));

        // signaling with NO grant announces nothing at all.
        let set = announced_set(&[], &live, &caps(8)).unwrap();
        assert!(set.capabilities.is_empty(), "no consent, no announce");
    }

    #[test]
    fn every_granted_and_signaling_kind_contributes_its_own_tag() {
        let grants = [grant("agent", &["claude"]), grant("compute", &["codex"])];
        let live = [signal("agent", &["claude"]), signal("compute", &["codex"])];
        let set = announced_set(&grants, &live, &caps(4)).unwrap();
        assert_eq!(set.capabilities, tags(&["agent", "claude", "codex", "compute"]));
    }

    #[test]
    fn a_kind_with_no_executors_still_announces_itself() {
        // the airlock plug: it spawns nothing, so its executor set is empty —
        // but "which nodes run airlock" is exactly what placement asks.
        let grants = [grant("airlock", &[])];
        let live = [signal("airlock", &[])];
        let set = announced_set(&grants, &live, &BTreeMap::new()).unwrap();
        assert_eq!(set.capabilities, tags(&["airlock"]));
    }

    #[test]
    fn an_absent_daemon_drops_only_its_own_kind() {
        let grants = [grant("agent", &["claude"]), grant("compute", &["codex"])];
        let live = [signal("agent", &["claude"])];
        let set = announced_set(&grants, &live, &caps(4)).unwrap();
        assert_eq!(
            set.capabilities,
            tags(&["agent", "claude"]),
            "compute's grant survives, but nothing of compute is announced"
        );
    }

    #[test]
    fn an_illegal_tag_refuses_the_whole_set() {
        // the hello boundary admits a space; the registry does not. Refusing
        // here is the point: the announce is all-or-nothing, so trimming would
        // hide it until an operator wondered why their node was in no pool.
        let grants = [grant("compute", &["Claude Sonnet", "codex"])];
        let live = [signal("compute", &["Claude Sonnet", "codex"])];
        assert_eq!(
            announced_set(&grants, &live, &caps(8)),
            Err(Refusal::IllegalTags(tags(&["Claude Sonnet"])))
        );
    }

    #[test]
    fn crossing_the_registry_cap_refuses_the_whole_set() {
        let many: Vec<String> = (0..MAX_ANNOUNCED_TAGS).map(|n| format!("e{n}")).collect();
        let borrowed: Vec<&str> = many.iter().map(String::as_str).collect();
        let grants = [grant("compute", &borrowed)];
        let live = [signal("compute", &borrowed)];
        // MAX executors + the kind tag = one over.
        assert_eq!(
            announced_set(&grants, &live, &caps(8)),
            Err(Refusal::OverCap {
                total: MAX_ANNOUNCED_TAGS + 1
            })
        );
    }

    #[test]
    fn the_announce_cap_matches_the_modules_own() {
        // the mirrored constant, pinned to the module's source. A comment would
        // not have caught a drift; making the module's const `pub` would have
        // moved the genesis app hash for a visibility keyword.
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/modules/system/capability/src/lib.rs"
        ))
        .expect("the capability module's source");
        let module_cap: usize = source
            .lines()
            .find_map(|line| line.trim().strip_prefix("const MAX_CAPABILITIES: usize = "))
            .expect("capability declares MAX_CAPABILITIES")
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
}
