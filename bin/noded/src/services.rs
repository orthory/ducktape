//! The node-local service catalog: which offchain service daemons are
//! currently signaling presence to this node.
//!
//! A service (compute, storage, ...) has no sovereign identity — identity and
//! transport belong to the node — so it cannot place itself on chain and it
//! cannot name itself. All it may do is SIGNAL: `POST /v1/services/hello`
//! declaring its kind, version, the capability tags it offers and the grant
//! scopes it needs. The node remembers that for [`HELLO_TTL`] and nothing
//! more. Nothing here is durable, nothing here is consensus state, and an
//! entry confers no standing whatsoever — `ducktape service enable <kind>`
//! (which writes the workspace's `services.toml`) is the consent boundary.
//!
//! ## why a TTL and not a live connection
//!
//! Presence could ride an open websocket, but `stream_session` has no
//! disconnect seam — every teardown path is a bare `return` — so a connection
//! model would be the first of its kind in the daemon. A refreshed deadline is
//! the shape two existing registries already use (`gateway_ws_token`'s
//! prune-on-write, `nat_traversal::AdvertBook`'s expire-on-read), so a hello
//! that stops arriving simply ages out.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// How long a hello keeps its entry alive. A daemon re-sends well inside this;
/// one that dies leaves at most a `HELLO_TTL` ghost, which `service list`
/// reports as signaling and `service status` would then report as
/// enabled-but-absent once it truly goes.
pub const HELLO_TTL: Duration = Duration::from_secs(30);

/// the compute service: dispatch-placed headless runs.
pub const COMPUTE_KIND: &str = "compute";
/// the agent service: user-attached interactive pty sessions. Sibling to
/// compute, not a layer on it — both link the same provider/sandbox/broker
/// libraries and spawn their own sandboxes, and their bus is the chain.
pub const AGENT_KIND: &str = "agent";

/// Cap on distinct kinds the catalog will hold. The kind in a hello is
/// caller-chosen, so an unbounded map is a trivial memory-exhaustion vector
/// for any local process — and a host running more than this many distinct
/// service daemons is not a shape we are trying to serve.
const MAX_SIGNALING: usize = 64;

/// the longest a kind tag may be — kinds are capability-tag shaped.
const MAX_KIND_LEN: usize = 32;
/// the longest a version string may be.
const MAX_VERSION_LEN: usize = 32;
/// the most capability tags one hello may offer.
///
/// Sized against reality, not a round number: a capability spec expands into
/// one tag per `[[variants]]` entry, so the two BUILT-IN specs alone already
/// declare ~37, and an operator spec dir adds more. A tight cap here does not
/// harden anything — it just refuses ordinary hosts.
const MAX_CAPABILITIES: usize = 512;
/// the most grant scopes / declared needs one hello may carry. These are
/// small by nature: a service asks for a handful of scopes, not hundreds.
const MAX_LIST_LEN: usize = 32;
/// the longest one offered tag / requested scope may be.
const MAX_ITEM_LEN: usize = 64;

/// the body of `POST /v1/services/hello` — what a daemon declares about
/// itself. There is deliberately NO id field: a service cannot choose its own
/// identity, so the catalog keys on the declared kind alone and an instance id
/// exists only once the user grants one.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    /// the service kind tag (`compute`, `storage`, ...).
    pub kind: String,
    /// the daemon's own version. Metadata only — never part of identity.
    pub version: String,
    /// the daemon's BUILD identity, which must equal the node's own. See
    /// [`build_identity`]: a mismatch is refused, never negotiated.
    pub build: String,
    /// the capability tags this daemon offers to run.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// the grant scopes this daemon says it needs.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// service kinds this daemon says it wants present to be fully useful
    /// (an agent daemon wanting `compute` capacity, say).
    ///
    /// INFORMATIONAL ONLY, and deliberately so: an unmet need is rendered as a
    /// warning by `ducktape service list`/`status` and changes nothing else. No
    /// dependency graph, no startup ordering, no readiness gate, no
    /// plug-to-plug call — a service whose need is unmet still enables, still
    /// runs and still serves. That is a standing non-goal, not a gap.
    #[serde(default)]
    pub needs: Vec<String>,
}

/// This binary's build identity — what a daemon must present in its hello and
/// what the node compares against its own. `None` when the build could not be
/// identified at compile time.
///
/// The node and a service daemon are separate processes with independent
/// restart timing, so skew is real even when one binary is on disk: an
/// operator upgrades and restarts the node while yesterday's daemon is still
/// running. Per the repo's no-versioning doctrine a mismatch is REFUSED with a
/// nameable reason — no negotiation, no compat arm, no minimum-version window.
///
/// It is deliberately NOT the package version: version numbering is pinned at
/// v1 permanently, so `CARGO_PKG_VERSION` is a constant and every skewed pair
/// would compare equal — an inert gate that looks like a live one. `build.rs`
/// stamps the commit (plus a working-tree digest when dirty).
///
/// `None` FAILS CLOSED: an unidentifiable build refuses every hello rather
/// than falling back to a constant that admits all of them.
pub fn build_identity() -> Option<&'static str> {
    option_env!("DUCKTAPE_BUILD").filter(|id| !id.is_empty())
}

/// the file a node writes its service-link secret into, next to `node.toml`.
pub const LINK_TOKEN_FILE: &str = "service-link.token";

/// Mint this node's service-link secret and write it 0600 next to `node.toml`.
///
/// Holding the link means BECOMING this node's interactive plane: the holder
/// receives every `TermCreate`, lent-credential records included. Before the
/// carve nothing outside the node process could hold that, so requiring a file
/// read raises the bar from "can dial loopback" — which any local process can —
/// back to "can read the node's own workspace", which is the same bar the node
/// key already sets.
///
/// Freshly minted each boot rather than persisted: a node restart should
/// invalidate a stale holder, and the daemon re-reads the file on every attach,
/// so it costs nothing.
pub fn mint_link_token(workspace: &std::path::Path) -> Result<String, String> {
    use rand::RngCore as _;
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
    let path = workspace.join(LINK_TOKEN_FILE);
    // create 0600 from the start — a world-readable window, however short, is
    // the whole thing this file exists to avoid.
    write_owner_only(&path, &token)
        .map_err(|error| format!("service link token {}: {error}", path.display()))?;
    Ok(token)
}

#[cfg(unix)]
fn write_owner_only(path: &std::path::Path, token: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let _ = std::fs::remove_file(path);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .write_all(token.as_bytes())
}

#[cfg(not(unix))]
fn write_owner_only(path: &std::path::Path, token: &str) -> std::io::Result<()> {
    std::fs::write(path, token)
}

/// Read a node's service-link secret — what a daemon presents when it attaches.
pub fn read_link_token(workspace: &std::path::Path) -> Result<String, String> {
    let path = workspace.join(LINK_TOKEN_FILE);
    std::fs::read_to_string(&path)
        .map(|token| token.trim().to_string())
        .map_err(|error| format!("service link token {}: {error}", path.display()))
}

/// Compare two secrets without leaking their common prefix through timing.
pub fn token_matches(presented: &str, expected: &str) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in presented.bytes().zip(expected.bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Why a hello was turned away. A typed reason rather than a bare string: the
/// status and the stable `reason` token are derived from the variant, so they
/// cannot drift apart and a typo cannot silently downgrade a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloRefusal {
    /// the hello itself is not well-formed.
    Malformed(&'static str),
    /// the daemon is a different build from this node.
    BuildMismatch,
    /// this node cannot identify its OWN build, so it can prove nothing about
    /// anyone else's. Refuses everything by construction.
    BuildIdentityUnavailable,
    /// too many distinct kinds are already signaling.
    CatalogFull,
}

impl HelloRefusal {
    /// the stable snake_case token — greppable, countable, never prose.
    pub fn reason(self) -> &'static str {
        match self {
            HelloRefusal::Malformed(_) => "malformed_hello",
            HelloRefusal::BuildMismatch => "build_mismatch",
            HelloRefusal::BuildIdentityUnavailable => "build_identity_unavailable",
            HelloRefusal::CatalogFull => "catalog_full",
        }
    }

    /// 400 for "you sent nonsense"; 409 for "you do not belong here"; 503 for
    /// "this node cannot serve the check at all".
    pub fn status(self) -> axum::http::StatusCode {
        match self {
            HelloRefusal::Malformed(_) => axum::http::StatusCode::BAD_REQUEST,
            HelloRefusal::BuildMismatch => axum::http::StatusCode::CONFLICT,
            HelloRefusal::BuildIdentityUnavailable => {
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            }
            HelloRefusal::CatalogFull => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// the operator-facing sentence.
    pub fn message(self) -> String {
        match self {
            HelloRefusal::Malformed(detail) => detail.to_string(),
            HelloRefusal::BuildMismatch => format!(
                "this node runs build {}; restart the service daemon from the same build",
                build_identity().unwrap_or("unknown")
            ),
            HelloRefusal::BuildIdentityUnavailable => {
                "this node cannot identify its own build, so it refuses every service hello; \
                 rebuild it from a git checkout"
                    .into()
            }
            HelloRefusal::CatalogFull => {
                "too many services are signaling to this node".into()
            }
        }
    }
}

/// A kind tag is lowercase alphanumeric plus `-`. This is a trust boundary:
/// the value is caller-supplied, lands in a map key, is rendered into CLI
/// output and is written into `services.toml`, so it must not carry a NUL
/// (which would make the instance-id preimage ambiguous), a path separator, or
/// terminal control bytes.
fn kind_is_well_formed(kind: &str) -> bool {
    let len_ok = !kind.is_empty() && kind.len() <= MAX_KIND_LEN;
    len_ok
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// One offered tag / requested scope: printable ascii, bounded, no control
/// bytes (the same reasoning as the kind, minus the preimage concern).
fn item_is_well_formed(item: &str) -> bool {
    let len_ok = !item.is_empty() && item.len() <= MAX_ITEM_LEN;
    len_ok && item.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
}

impl Hello {
    /// Reject a malformed hello at the boundary, naming one stable reason.
    pub fn validate(&self) -> Result<(), HelloRefusal> {
        if !kind_is_well_formed(&self.kind) {
            return Err(HelloRefusal::Malformed("kind must be 1..32 chars of [a-z0-9-]"));
        }
        if self.version.len() > MAX_VERSION_LEN || !item_is_well_formed(&self.version) {
            return Err(HelloRefusal::Malformed("version must be 1..32 printable ascii chars"));
        }
        let lists_ok = self.capabilities.len() <= MAX_CAPABILITIES
            && self.scopes.len() <= MAX_LIST_LEN
            && self.needs.len() <= MAX_LIST_LEN;
        if !lists_ok {
            return Err(HelloRefusal::Malformed(
                "at most 512 capabilities, 32 scopes and 32 needs",
            ));
        }
        let items_ok = self
            .capabilities
            .iter()
            .chain(self.scopes.iter())
            .all(|item| item_is_well_formed(item));
        if !items_ok {
            return Err(HelloRefusal::Malformed("each capability/scope must be 1..64 printable ascii chars"));
        }
        // a need names a KIND, so it obeys the kind grammar — that is what
        // makes it comparable against the grants without any normalizing.
        if !self.needs.iter().all(|need| kind_is_well_formed(need)) {
            return Err(HelloRefusal::Malformed("each need must be a service kind (1..32 chars of [a-z0-9-])"));
        }
        Ok(())
    }
}

/// one live catalog row as `GET /v1/services` renders it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Signaling {
    pub kind: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub scopes: Vec<String>,
    /// the kinds this daemon declared it wants present. Display only.
    #[serde(default)]
    pub needs: Vec<String>,
}

/// the deadline-carrying catalog entry.
#[derive(Clone, Debug)]
struct Entry {
    hello: Hello,
    expires_at: Instant,
}

/// The volatile signaling catalog. `std::sync::Mutex` like every other
/// registry on the handle — each critical section is a map op with no `.await`
/// inside it.
#[derive(Clone, Default)]
pub struct ServiceCatalog(Arc<Mutex<HashMap<String, Entry>>>);

impl ServiceCatalog {
    /// Record (or refresh) a daemon's presence. Returns the TTL the caller
    /// must re-signal within. Full catalog + a NEW kind is refused: refreshing
    /// an existing kind must never fail on capacity, or a full map would
    /// starve the daemons already in it.
    pub fn hello(&self, hello: Hello, now: Instant) -> Result<Duration, HelloRefusal> {
        self.admit(hello, now).inspect_err(|refusal| {
            tracing::warn!(
                target: "ducktape::service",
                reason = refusal.reason(),
                "service hello refused"
            );
        })
    }

    fn admit(&self, hello: Hello, now: Instant) -> Result<Duration, HelloRefusal> {
        hello.validate()?;
        // a node that cannot name its own build can prove nothing about
        // anyone else's, so it refuses everything rather than waving it through.
        let Some(mine) = build_identity() else {
            return Err(HelloRefusal::BuildIdentityUnavailable);
        };
        // loud and total: a daemon built against a different node does not get
        // to signal, let alone be enabled.
        if hello.build != mine {
            return Err(HelloRefusal::BuildMismatch);
        }
        let mut entries = self.0.lock().expect("service catalog lock poisoned");
        expire(&mut entries, now);
        let kind = hello.kind.clone();
        let known = entries.contains_key(&kind);
        if !known && entries.len() >= MAX_SIGNALING {
            return Err(HelloRefusal::CatalogFull);
        }
        entries.insert(
            kind.clone(),
            Entry {
                hello,
                expires_at: now + HELLO_TTL,
            },
        );
        // a kind ENTERING the catalog is a once-per-daemon-session lifecycle
        // fact; the refreshes that follow are per-request and must not evict
        // the ring, so they stay at debug.
        match known {
            true => {
                tracing::debug!(target: "ducktape::service", %kind, "service hello refreshed")
            }
            false => {
                tracing::info!(target: "ducktape::service", %kind, "service signaling")
            }
        }
        Ok(HELLO_TTL)
    }

    /// The live entries, kind-sorted so the CLI's output is stable.
    pub fn live(&self, now: Instant) -> Vec<Signaling> {
        let mut entries = self.0.lock().expect("service catalog lock poisoned");
        expire(&mut entries, now);
        let mut live: Vec<Signaling> = entries
            .values()
            .map(|entry| Signaling {
                kind: entry.hello.kind.clone(),
                version: entry.hello.version.clone(),
                capabilities: entry.hello.capabilities.clone(),
                scopes: entry.hello.scopes.clone(),
                needs: entry.hello.needs.clone(),
            })
            .collect();
        live.sort_by(|a, b| a.kind.cmp(&b.kind));
        live
    }
}

/// Drop every entry whose deadline has passed. Called on both paths, so the
/// catalog needs no sweeper task: an expired entry is unobservable.
fn expire(entries: &mut HashMap<String, Entry>, now: Instant) {
    entries.retain(|kind, entry| {
        let live = entry.expires_at > now;
        if !live {
            tracing::info!(target: "ducktape::service", %kind, reason = "hello_expired", "service gone");
        }
        live
    });
}

// AUTH: both routes are registered on the daemon's `public` router, so they
// inherit the SAME gate as `/v1/submit` and `/v1/term/sessions`:
// `origin_guard::guard` + its CORS allowlist. That surface is trusted-local by
// design (see `origin_guard`) — the CLI sends no `Origin` and is allowed; a
// browser must present an allowlisted one. There is no bearer token because a
// local process can already read the node's key off disk. Signaling is
// deliberately unprivileged: an entry grants NOTHING, so the weakest gate on
// the surface is the right one. Consent happens in `ducktape service enable`.
//
// NOTE the transport assumption: unlike `/v1/submit`, which carries a signed
// frame and is therefore safe wherever it is reachable, a hello is
// UNAUTHENTICATED — it is trusted only because `http_listen` is expected to
// stay on loopback or a private tailnet. Binding the node's HTTP surface to a
// public interface would let any reachable host occupy a kind in this catalog
// (and so appear in `service list` for a user to enable). The cap and TTL
// bound the damage; they do not replace keeping the surface private.

/// POST /v1/services/hello — a local service daemon declares (or refreshes)
/// its presence. Returns the TTL it must re-signal within.
pub async fn hello(
    axum::extract::State(handle): axum::extract::State<crate::handle::NodeHandle>,
    axum::Json(body): axum::Json<Hello>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    match handle.services().hello(body, Instant::now()) {
        Ok(ttl) => (
            axum::http::StatusCode::OK,
            axum::Json(serde_json::json!({ "ttl_secs": ttl.as_secs() })),
        )
            .into_response(),
        Err(refusal) => (
            refusal.status(),
            axum::Json(serde_json::json!({
                "error": refusal.message(),
                "reason": refusal.reason(),
            })),
        )
            .into_response(),
    }
}

/// GET /v1/services — the services currently signaling to this node. Says
/// nothing about what is ENABLED: that lives in the workspace's
/// `services.toml`, which the CLI reads directly off disk.
pub async fn list(
    axum::extract::State(handle): axum::extract::State<crate::handle::NodeHandle>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    let signaling = handle.services().live(Instant::now());
    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "signaling": signaling })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(kind: &str) -> Hello {
        Hello {
            kind: kind.into(),
            version: "1.2.3".into(),
            build: build_identity().expect("a test build is git-identifiable").into(),
            capabilities: vec!["agent.claude".into()],
            scopes: vec!["cred:read".into()],
            needs: Vec::new(),
        }
    }

    #[test]
    fn a_hello_is_live_until_its_ttl_lapses() {
        let catalog = ServiceCatalog::default();
        let start = Instant::now();
        assert_eq!(catalog.hello(hello("compute"), start).unwrap(), HELLO_TTL);

        // still inside the window: present, with the declared metadata intact.
        let live = catalog.live(start + HELLO_TTL - Duration::from_secs(1));
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].kind, "compute");
        assert_eq!(live[0].capabilities, vec!["agent.claude".to_string()]);
        assert_eq!(live[0].scopes, vec!["cred:read".to_string()]);

        // past it: gone, with no sweeper having run.
        assert!(
            catalog
                .live(start + HELLO_TTL + Duration::from_secs(1))
                .is_empty()
        );
    }

    #[test]
    fn a_refresh_extends_the_deadline_and_replaces_the_metadata() {
        let catalog = ServiceCatalog::default();
        let start = Instant::now();
        catalog.hello(hello("compute"), start).unwrap();

        let refreshed_at = start + HELLO_TTL - Duration::from_secs(1);
        let mut second = hello("compute");
        second.version = "2.0.0".into();
        catalog.hello(second, refreshed_at).unwrap();

        // the moment the FIRST hello would have died, the entry is still live.
        let live = catalog.live(start + HELLO_TTL + Duration::from_secs(1));
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].version, "2.0.0");
        // and one kind never becomes two rows.
        assert_eq!(catalog.live(refreshed_at).len(), 1);
    }

    #[test]
    fn the_catalog_is_capped_but_never_starves_a_daemon_already_in_it() {
        let catalog = ServiceCatalog::default();
        let now = Instant::now();
        for index in 0..MAX_SIGNALING {
            catalog.hello(hello(&format!("svc-{index}")), now).unwrap();
        }
        assert_eq!(catalog.live(now).len(), MAX_SIGNALING);
        // a NEW kind is refused ...
        assert!(catalog.hello(hello("one-too-many"), now).is_err());
        // ... but an existing one still refreshes.
        catalog.hello(hello("svc-0"), now).unwrap();
        // and once entries age out, the newcomer is admitted.
        let later = now + HELLO_TTL + Duration::from_secs(1);
        catalog.hello(hello("one-too-many"), later).unwrap();
        assert_eq!(catalog.live(later).len(), 1);
    }

    #[test]
    fn a_malformed_hello_is_refused_at_the_boundary() {
        let catalog = ServiceCatalog::default();
        let now = Instant::now();
        for bad_kind in ["", "Compute", "com pute", "compute/../etc", "a\0b"] {
            let mut bad = hello("compute");
            bad.kind = bad_kind.into();
            assert!(
                catalog.hello(bad, now).is_err(),
                "kind {bad_kind:?} must be refused"
            );
        }
        let mut long_kind = hello("compute");
        long_kind.kind = "a".repeat(MAX_KIND_LEN + 1);
        assert!(catalog.hello(long_kind, now).is_err());

        let mut too_many = hello("compute");
        too_many.capabilities = (0..MAX_CAPABILITIES + 1).map(|i| format!("tag{i}")).collect();
        assert!(catalog.hello(too_many, now).is_err());

        let mut long_item = hello("compute");
        long_item.scopes = vec!["s".repeat(MAX_ITEM_LEN + 1)];
        assert!(catalog.hello(long_item, now).is_err());

        // nothing malformed ever landed.
        assert!(catalog.live(now).is_empty());
    }
}

#[cfg(test)]
mod build_gate_tests {
    use super::*;

    /// the build this test binary was compiled as; `build_identity` must be
    /// Some here, since the tests only ever run from a git checkout.
    static TEST_BUILD: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        build_identity()
            .expect("tests run from a git checkout, so a build id exists")
            .to_string()
    });

    fn hello_from_build(build: &str) -> Hello {
        Hello {
            kind: "compute".into(),
            version: "1.2.3".into(),
            build: build.into(),
            capabilities: vec![],
            scopes: vec![],
            needs: vec![],
        }
    }

    #[test]
    fn a_hello_from_a_different_build_is_refused_and_never_enters_the_catalog() {
        let catalog = ServiceCatalog::default();
        let now = Instant::now();

        // skew in either direction is refused — there is no minimum-version
        // window and no negotiation, only equality.
        for skewed in ["0.0.0-ancient", "99.99.99", ""] {
            assert_eq!(
                catalog.hello(hello_from_build(skewed), now),
                Err(HelloRefusal::BuildMismatch),
                "build {skewed:?} must be refused"
            );
        }
        assert!(
            catalog.live(now).is_empty(),
            "a refused hello leaves nothing behind"
        );

        // the node's own build is what passes.
        catalog
            .hello(hello_from_build(TEST_BUILD.as_str()), now)
            .expect("matching build is admitted");
        assert_eq!(catalog.live(now).len(), 1);
    }

    #[test]
    fn declared_needs_ride_the_hello_through_to_the_catalog() {
        let catalog = ServiceCatalog::default();
        let now = Instant::now();
        let mut hello = hello_from_build(TEST_BUILD.as_str());
        hello.kind = "agent".into();
        hello.needs = vec!["compute".into()];
        catalog.hello(hello, now).unwrap();
        assert_eq!(catalog.live(now)[0].needs, vec!["compute".to_string()]);

        // a need is a KIND, so a malformed one is refused like any other.
        let mut bad = hello_from_build(TEST_BUILD.as_str());
        bad.needs = vec!["Not A Kind".into()];
        assert!(catalog.hello(bad, now).is_err());
    }
}
