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
    /// the daemon's build identity — METADATA, never a gate. Rendered beside
    /// the node's own by `ducktape service status` so an operator can see skew;
    /// see [`build_identity`] for why equality is not an admission rule.
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

/// This binary's build identity, as `build.rs` stamped it: the short commit,
/// plus a working-tree digest when the tree was dirty. `None` when the build
/// could not be identified at compile time (a source tarball, a vendored
/// build, any checkout without `.git`).
///
/// **Diagnostic only.** The node and a service daemon are separate processes
/// with independent restart timing, so skew is real — an operator upgrades and
/// restarts the node while yesterday's daemon is still running. Naming that
/// skew is worth doing; REFUSING on it is not, and this node does not:
///
/// - it authenticates nobody. A stamp is not a secret: it is compiled into a
///   binary any local process can read, and the caller does not even need to —
///   before this was metadata, the node handed its own stamp back in the
///   refusal body of the first wrong guess.
/// - it excludes every legitimately separately-compiled daemon by
///   construction. A third-party service would have to hardcode this
///   operator's exact commit, and on a dirty tree a `DefaultHasher` digest
///   `build.rs` itself notes is not toolchain-stable, at ITS compile time.
/// - it protected no correctness property. The node↔daemon protocol decodes
///   every frame at its boundary, so skew already degrades to named refusals
///   (`malformed_command`, `BadFrame`, `deny_unknown_fields`) rather than
///   corruption. Under the no-compat doctrine "speak the current protocol or
///   be refused" is enforced per frame, where it belongs.
/// - and `None` FAILING CLOSED made a git-absent build a node with no compute,
///   no agent pty and no airlock, whose only symptom was a bare 503.
///
/// It is deliberately NOT the package version: version numbering is pinned at
/// v1 permanently, so `CARGO_PKG_VERSION` is a constant that could never
/// distinguish two builds.
pub fn build_identity() -> Option<&'static str> {
    option_env!("DUCKTAPE_BUILD").filter(|id| !id.is_empty())
}

/// what a build with no identifiable stamp reports as. An honest "unknown"
/// rather than a value that pretends to name a commit.
pub const UNKNOWN_BUILD: &str = "unknown";

/// This binary's build identity, or [`UNKNOWN_BUILD`] — the rendering form.
pub fn build_identity_or_unknown() -> &'static str {
    build_identity().unwrap_or(UNKNOWN_BUILD)
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
    mint_secret_file(workspace, LINK_TOKEN_FILE)
}

/// Read a node's service-link secret — what a daemon presents when it attaches.
pub fn read_link_token(workspace: &std::path::Path) -> Result<String, String> {
    read_secret_file(workspace, LINK_TOKEN_FILE)
}

/// A fresh 32-byte secret, hex — the one generator behind every workspace
/// credential. Exposed for the embedder that holds a node IN-PROCESS and so IS
/// the operator (the test harness): it has no workspace to write into, but its
/// credential must be the same unguessable 32 bytes as a real one.
pub fn new_secret() -> String {
    use rand::RngCore as _;
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Mint a fresh secret and write it 0600 into `dir/name`.
///
/// THE writer for every workspace credential — the service link
/// ([`LINK_TOKEN_FILE`]) and the admin namespace's operator token
/// ([`crate::admin::ADMIN_TOKEN_FILE`]). One function on purpose: the two were
/// byte-identical copies, so an fsync, an atomic mint-to-temp-then-rename, or
/// an EINTR fix would land in one and silently not the other.
///
/// Freshly minted each boot rather than persisted: a node restart must
/// invalidate a stale holder, and every reader opens the file per call, so it
/// costs nothing.
///
/// The error names the PATH, never the secret — the path already carries the
/// file name, so no second label is needed to tell the two apart.
pub(crate) fn mint_secret_file(dir: &std::path::Path, name: &str) -> Result<String, String> {
    let secret = new_secret();
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("secret dir {}: {error}", dir.display()))?;
    let path = dir.join(name);
    // create 0600 from the start — a world-readable window, however short, is
    // the whole thing these files exist to avoid.
    write_owner_only(&path, &secret)
        .map_err(|error| format!("secret file {}: {error}", path.display()))?;
    Ok(secret)
}

/// Read a secret [`mint_secret_file`] wrote. Trims, because an operator who
/// `echo`s the file into a variable picks up the newline their shell added.
pub(crate) fn read_secret_file(dir: &std::path::Path, name: &str) -> Result<String, String> {
    let path = dir.join(name);
    std::fs::read_to_string(&path)
        .map(|secret| secret.trim().to_string())
        .map_err(|error| format!("secret file {}: {error}", path.display()))
}

#[cfg(unix)]
fn write_owner_only(path: &std::path::Path, secret: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let _ = std::fs::remove_file(path);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .write_all(secret.as_bytes())
}

#[cfg(not(unix))]
fn write_owner_only(path: &std::path::Path, secret: &str) -> std::io::Result<()> {
    std::fs::write(path, secret)
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
    /// too many distinct kinds are already signaling.
    CatalogFull,
}

impl HelloRefusal {
    /// the stable snake_case token — greppable, countable, never prose.
    pub fn reason(self) -> &'static str {
        match self {
            HelloRefusal::Malformed(_) => "malformed_hello",
            HelloRefusal::CatalogFull => "catalog_full",
        }
    }

    /// 400 for "you sent nonsense"; 503 for "this node cannot serve you right
    /// now".
    pub fn status(self) -> axum::http::StatusCode {
        match self {
            HelloRefusal::Malformed(_) => axum::http::StatusCode::BAD_REQUEST,
            HelloRefusal::CatalogFull => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// the operator-facing sentence. It describes only what the CALLER sent or
    /// what this node's capacity is — never a fact about this node the caller
    /// did not already have, since the route is unauthenticated.
    pub fn message(self) -> String {
        match self {
            HelloRefusal::Malformed(detail) => detail.to_string(),
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
        // the build is no longer compared, but it IS rendered — `service
        // status` prints it — so it stays a validated trust boundary: a
        // caller-supplied string reaching a terminal must carry no control
        // bytes and no unbounded length.
        //
        // Sized as an ITEM (1..64), not a version (1..32), and deliberately: a
        // stamp is `<sha>-<u64 hex>`, which reaches 57 chars under
        // `core.abbrev = 40`. A cap that refused an honest daemon's own stamp
        // would be the same fail-closed trap the build gate was.
        if !item_is_well_formed(&self.build) {
            return Err(HelloRefusal::Malformed("build must be 1..64 printable ascii chars"));
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
    /// the daemon's build stamp, carried through for display. This is the
    /// diagnostic that replaced the old build gate: `service status` prints it
    /// beside the node's own, so ordinary dev-loop skew is visible instead of
    /// being a refusal.
    pub build: String,
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

    // NOTE: admission consults [`build_identity`] nowhere, deliberately. A
    // build stamp is not a credential and equality was never a correctness
    // rule; making it one turned every git-absent build into a node that
    // refused all three service planes. Skew is a DIAGNOSTIC now — the hello
    // OK body carries this node's build so a daemon can name its own.
    fn admit(&self, hello: Hello, now: Instant) -> Result<Duration, HelloRefusal> {
        hello.validate()?;
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
                build: entry.hello.build.clone(),
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
/// its presence. Returns the TTL it must re-signal within, and this node's own
/// build so the daemon can name any skew between them.
///
/// The build rides the OK body and NEVER a refusal body: a caller that got a
/// 200 has already been admitted, whereas answering an unauthenticated
/// rejection with a fact about this node is what the deleted build gate did
/// wrong. Nothing secret goes in either body.
pub async fn hello(
    axum::extract::State(handle): axum::extract::State<crate::handle::NodeHandle>,
    axum::Json(body): axum::Json<Hello>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    match handle.services().hello(body, Instant::now()) {
        Ok(ttl) => (
            axum::http::StatusCode::OK,
            axum::Json(serde_json::json!({
                "ttl_secs": ttl.as_secs(),
                "build": build_identity_or_unknown(),
            })),
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
            build: "deadbeef".into(),
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

        // the build is no longer compared, but it is still RENDERED, so it
        // stays a validated boundary: no control bytes, bounded length.
        for bad_build in ["", "a\0b", "esc\x1b[31m"] {
            let mut bad = hello("compute");
            bad.build = bad_build.into();
            assert!(
                catalog.hello(bad, now).is_err(),
                "build {bad_build:?} must be refused"
            );
        }
        let mut long_build = hello("compute");
        long_build.build = "b".repeat(MAX_ITEM_LEN + 1);
        assert!(catalog.hello(long_build, now).is_err());

        // nothing malformed ever landed.
        assert!(catalog.live(now).is_empty());
    }

    #[test]
    fn declared_needs_ride_the_hello_through_to_the_catalog() {
        let catalog = ServiceCatalog::default();
        let now = Instant::now();
        let mut signal = hello("agent");
        signal.needs = vec!["compute".into()];
        catalog.hello(signal, now).unwrap();
        assert_eq!(catalog.live(now)[0].needs, vec!["compute".to_string()]);

        // a need is a KIND, so a malformed one is refused like any other.
        let mut bad = hello("agent");
        bad.needs = vec!["Not A Kind".into()];
        assert!(catalog.hello(bad, now).is_err());
    }
}

/// The regression guard for the deleted build gate.
///
/// The gate refused every hello and every service link when `build_identity()`
/// was `None`, which is what a build without `.git` produces — a node with no
/// compute, no agent pty and no airlock, reporting only a bare 503. These tests
/// pin that admission never consults the stamp again.
///
/// `build_identity()` is `option_env!`, resolved at COMPILE time, so no test
/// can make it `None` at runtime. The behavioural half below therefore proves
/// the stamp is not compared, and the source-parsing half proves the two
/// admission paths never read it at all — which is the same guarantee, stated
/// where a test can actually assert it.
#[cfg(test)]
mod build_is_metadata_not_a_gate {
    use super::*;

    fn hello_from_build(build: &str) -> Hello {
        Hello {
            kind: "compute".into(),
            version: "1.2.3".into(),
            build: build.into(),
            capabilities: vec!["compute.small".into()],
            scopes: vec![],
            needs: vec![],
        }
    }

    #[test]
    fn a_hello_from_any_build_is_admitted_and_its_stamp_is_carried_through() {
        let catalog = ServiceCatalog::default();
        let now = Instant::now();

        // skew in either direction, and a stamp this node could never have
        // minted: all admitted. There is no equality rule left to fail.
        // the widest stamp `build.rs` can actually mint is in this list on
        // purpose: a 40-char sha under `core.abbrev = 40` plus a u64 digest is
        // 57 chars, so a cap sized for a version string would have refused an
        // honest daemon its own build — the fail-closed trap all over again.
        let widest = format!("{}-{:x}", "a".repeat(40), u64::MAX);
        for foreign in [
            "0.0.0-ancient",
            "99.99.99",
            UNKNOWN_BUILD,
            "a-third-party-daemon",
            widest.as_str(),
        ] {
            catalog
                .hello(hello_from_build(foreign), now)
                .unwrap_or_else(|refusal| {
                    panic!("build {foreign:?} must be admitted, got {refusal:?}")
                });
            let live = catalog.live(now);
            assert_eq!(live.len(), 1, "one kind is one row whatever its build");
            assert_eq!(
                live[0].build, foreign,
                "the daemon's stamp reaches the catalog as metadata"
            );
        }
    }

    #[test]
    fn no_refusal_message_leaks_this_node_s_build() {
        // the deleted `BuildMismatch` interpolated `build_identity()` into a
        // message that `hello()` returned verbatim in the 409 body — handing
        // an unauthenticated caller the correct stamp on its first wrong
        // guess. Every surviving refusal describes the CALLER's input or this
        // node's capacity, and nothing else.
        let messages = [
            HelloRefusal::Malformed("kind must be 1..32 chars of [a-z0-9-]").message(),
            HelloRefusal::CatalogFull.message(),
        ];
        let mine = build_identity_or_unknown();
        for message in messages {
            assert!(
                !message.contains(mine),
                "a refusal body must not publish this node's build: {message:?}"
            );
        }
    }

    /// A source-parsing lint, because `option_env!` cannot be made `None` at
    /// runtime: neither admission path may name `build_identity` again.
    #[test]
    fn neither_admission_path_consults_the_build_stamp() {
        let paths = [
            (file!(), "fn admit("),
            ("bin/noded/src/stream.rs", "fn take_service_link("),
        ];
        for (relative, signature) in paths {
            let source = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(relative),
            )
            .unwrap_or_else(|error| panic!("{relative}: {error}"));
            let body = function_body(&source, signature)
                .unwrap_or_else(|| panic!("{relative} no longer defines `{signature}`"));
            assert!(
                !body.contains("build_identity"),
                "{signature} in {relative} must not gate on the build stamp — a \
                 git-absent build would refuse every hello and every service link"
            );
        }
    }

    /// The brace-balanced body of the function whose signature starts with
    /// `signature`. Deliberately dumb: it only has to serve the two callers
    /// above, and a parse that ever gets confused fails the test loudly.
    fn function_body<'a>(source: &'a str, signature: &str) -> Option<&'a str> {
        let start = source.find(signature)? + signature.len();
        let open = start + source[start..].find('{')?;
        let mut depth = 0usize;
        for (offset, byte) in source[open..].bytes().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&source[open..open + offset]);
                    }
                }
                _ => {}
            }
        }
        None
    }
}
