//! node configuration: identity files, the shareable network descriptor, and
//! resolution of both config shapes into one runnable form.
//!
//! ## the two shapes
//!
//! **network shape** (production): `node.toml` names a `key_file` (a persisted
//! ed25519 identity) and a `network` descriptor file. `network.toml` is THE
//! shareable genesis artifact — identical content on every member is what
//! makes genesis deterministic, exactly like a chain-id + genesis file:
//! `chain_id` doubles as the commonware namespace (it domain-separates the
//! discovery handshake, the simplex scheme, and the epoch genesis floor), and
//! `validators` seeds the valset module at genesis.
//!
//! **dev-seed shape** (tests/demos/the app's generated solo config): the flat
//! `id`/`peer_seeds`/`validator_seeds` form, identities derived with
//! `PrivateKey::from_seed`. kept verbatim so examples and demo scripts stay
//! runnable; `resolve` treats a config WITHOUT a `network` field as this shape.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519};
use commonware_p2p::Ingress;
use commonware_utils::Hostname;
use serde::{Deserialize, Serialize};

mod identity;
mod invite;
mod node_toml;
mod resolve;

pub use identity::*;
pub use invite::*;
pub use node_toml::*;
pub use resolve::*;

/// the consensus scheme tag a descriptor must carry — a genesis-wide constant
/// (see `ConsensusScheme`); anything else is a build from the future.
pub const SCHEME_ED25519: &str = "ed25519";

// hex codecs for keys, roots, and pasted blobs — the shared home is
// duckfs-core (`to_hex`/`unhex`); these re-exports keep the long-standing
// `config::hex_bytes`/`config::unhex` call sites working unchanged.
pub use duckfs_core::{to_hex as hex_bytes, unhex};

// ============================================================================
// the network descriptor — network.toml, the shareable genesis artifact.
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkDescriptor {
    /// human name + salt (e.g. "ducktape#a1b2c3d4"); doubles as the namespace.
    pub chain_id: String,
    /// consensus scheme tag; must equal [`SCHEME_ED25519`] for this build.
    pub scheme: String,
    /// genesis validator identities, hex ed25519 public keys. a SET — order
    /// never affects genesis (valset is order-independent) — kept sorted for
    /// stable file diffs.
    pub validators: Vec<String>,
    /// dial hints, "hexpubkey@host:port". advisory: joiners bootstrap off the
    /// first reachable entry that is not themselves.
    #[serde(default)]
    pub bootstrap: Vec<String>,
    /// typed reach hints (v3), canonical strings like `direct:<hex>@host:port`.
    /// advisory and EXCLUDED from the genesis fingerprint, exactly like
    /// `bootstrap`. empty for v2/legacy descriptors — then [`NetworkDescriptor::reach_hints`]
    /// synthesises all-`Direct` hints from `bootstrap`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reach: Vec<String>,
    /// Coordination privacy for the reachability plane. `None` => `Private`
    /// (the safer default). Operational policy, parsed like the reach hints —
    /// NOT part of `genesis_namespace` (validator identity only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordination: Option<String>,
}

impl NetworkDescriptor {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let mut d: Self = toml::from_str(text).map_err(|e| format!("network descriptor: {e}"))?;
        // canonicalize at the boundary: hex is many-to-one under decode
        // (case, whitespace), so every descriptor INSIDE the program carries
        // trimmed, lowercase, sorted validator entries — string comparisons
        // (admit, membership hints) and the genesis fingerprint then agree
        // with what decode_key actually accepts.
        for v in &mut d.validators {
            *v = v.trim().to_ascii_lowercase();
        }
        d.validators.sort();
        Ok(d)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("descriptor serializes")
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
        Self::from_toml(&text)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_toml()).map_err(|e| format!("write {path:?}: {e}"))
    }

    pub fn validator_keys(&self) -> Result<Vec<ed25519::PublicKey>, String> {
        // dedup on the DECODED key: hex spelling is many-to-one (case), and a
        // duplicate that slipped through here would panic much later at
        // run_node's Set::try_from.
        let keys: Vec<ed25519::PublicKey> = self
            .validators
            .iter()
            .map(|h| decode_key(h))
            .collect::<Result<_, _>>()?;
        let mut seen = std::collections::BTreeSet::new();
        for k in &keys {
            if !seen.insert(k.as_ref().to_vec()) {
                return Err(format!(
                    "duplicate validator {} in network {}",
                    hex_bytes(k.as_ref()),
                    self.chain_id
                ));
            }
        }
        Ok(keys)
    }

    /// the namespace this network's nodes actually run under: the chain-id
    /// plus a GENESIS FINGERPRINT (sha256 over scheme + the sorted validator
    /// set; bootstrap hints excluded — they are advisory and legitimately
    /// differ between members). because the namespace domain-separates the
    /// discovery handshake, the simplex scheme, and the epoch genesis floor,
    /// a member holding a STALE descriptor (e.g. it missed a pre-genesis
    /// `admit` and kept the old validator list) cannot even connect — genesis
    /// divergence is a loud connectivity failure, never a silent state fork.
    pub fn genesis_namespace(&self) -> String {
        use commonware_cryptography::{Hasher as _, Sha256};
        // canonical form regardless of how the struct was built (from_toml
        // normalizes, but a hand-constructed descriptor must fingerprint
        // identically): trimmed, lowercased, sorted.
        let mut sorted: Vec<String> = self
            .validators
            .iter()
            .map(|v| v.trim().to_ascii_lowercase())
            .collect();
        sorted.sort();
        let mut hasher = Sha256::default();
        hasher.update(b"ducktape:genesis:v1:");
        hasher.update(self.scheme.as_bytes());
        for v in &sorted {
            hasher.update(b"\n");
            hasher.update(v.as_bytes());
        }
        let digest = hasher.finalize();
        // 128 bits: a 32-bit suffix is grindable (~2^32 hashes finds an
        // admitted key that leaves the fingerprint unchanged, resurrecting
        // the silent stale-descriptor fork this exists to prevent).
        format!("{}@{}", self.chain_id, hex_bytes(&digest.as_ref()[..16]))
    }

    /// bootstrap entries as dial INGRESSES. an IP literal becomes a socket
    /// ingress; a HOSTNAME stays a hostname (`Ingress::Dns`) and is re-resolved
    /// by the dialer at EVERY attempt — so `pubkey@node.example.com:443` keeps
    /// working when the tunnel behind the name moves, and an offline name never
    /// blocks startup. a MALFORMED entry (no `@`, bad key, not host:port) is a
    /// config error; an unspecified ip / port 0 is advisory and skipped. the
    /// live dial path is [`NetworkDescriptor::reach_entries`], which folds these
    /// Direct entries in alongside the typed `reach` hints.
    #[cfg(test)]
    pub fn bootstrap_entries(&self) -> Result<Vec<(ed25519::PublicKey, Ingress)>, String> {
        let mut out = Vec::new();
        for entry in &self.bootstrap {
            let (key, host_port) = entry
                .split_once('@')
                .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
            let key = decode_key(key)?;
            match ingress_of(host_port).map_err(|e| format!("bootstrap entry {entry:?}: {e}"))? {
                Some(ingress) => out.push((key, ingress)),
                None => continue, // unspecified ip / port 0 — advisory noise.
            }
        }
        Ok(out)
    }

    /// add a validator identity (pre-genesis membership — see `admit`).
    /// idempotent; keeps the list sorted.
    pub fn admit(&mut self, key: &ed25519::PublicKey) {
        let hex = hex_bytes(key.as_ref());
        if !self.validators.contains(&hex) {
            self.validators.push(hex);
            self.validators.sort();
        }
    }

    /// record a dial hint (`host:port`, an IP or a hostname) for `key`, replacing
    /// any previous hint for the same key (a member's advertised addr can move).
    pub fn add_bootstrap(&mut self, key: &ed25519::PublicKey, addr: &str) {
        let hex = hex_bytes(key.as_ref());
        self.bootstrap
            .retain(|e| !e.starts_with(&format!("{hex}@")));
        self.bootstrap.push(format!("{hex}@{addr}"));
        self.bootstrap.sort();
    }

    /// the reach hints, typed. if the descriptor carries explicit v3 `reach`
    /// entries they parse to those; otherwise every `bootstrap` entry is a
    /// `Direct` hint (so a v2/legacy descriptor yields all-`Direct` hints with
    /// no data duplicated and no double-dial).
    pub fn reach_hints(&self) -> Result<Vec<ReachHint>, String> {
        // the UNION of typed `reach` and legacy `bootstrap`: explicit typed
        // hints win over bootstrap-synthesised Direct hints for the same
        // member, but typed entries are a route set, not a per-key map. A
        // node may need both a rendezvous route and a tunnel-overlay route for
        // the same expected key.
        let mut bootstrap_by_key: std::collections::BTreeMap<Vec<u8>, ReachHint> =
            std::collections::BTreeMap::new();
        for entry in &self.bootstrap {
            let (k, addr) = entry
                .split_once('@')
                .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
            let expected_key = decode_key(k)?;
            bootstrap_by_key.insert(
                expected_key.as_ref().to_vec(),
                ReachHint { expected_key, reach: Reach::Direct(addr.to_string()) },
            );
        }
        let mut typed = Vec::new();
        let mut typed_keys = std::collections::BTreeSet::new();
        for s in &self.reach {
            let hint = ReachHint::parse(s)?;
            // only a typed DIRECT/FRONTED route supersedes a bootstrap-
            // synthesised Direct for the same key (the member's dial address
            // moved/upgraded). a Coordinated route is an ADDITIONAL rendezvous
            // path, not a replacement — it must not erase a real direct dial
            // hint, or a founder that advertises a public address AND enables a
            // coordinator would ship coordinator-only reach and lose its direct
            // fallback (terminal once a punch fails, since there is no relay).
            // an overlay-ULA Direct (this chain's tunnel /48) is likewise an
            // ADDITIONAL route: it is only dialable once the reachability
            // plane's tunnels apply, so letting it evict the underlay hint
            // strands the member behind a plane that has not assembled yet
            // (first join, promotion reboot, same-host tests).
            if matches!(hint.reach, Reach::Direct(_) | Reach::Fronted(_))
                && !self.overlay_route(&hint)?
            {
                typed_keys.insert(hint.expected_key.as_ref().to_vec());
            }
            typed.push(hint);
        }
        bootstrap_by_key.retain(|k, _| !typed_keys.contains(k));
        let mut out: Vec<_> = bootstrap_by_key.into_values().chain(typed).collect();
        out.sort_by_key(|h| h.to_canonical());
        Ok(out)
    }

    /// whether a hint's address lives inside this chain's overlay ULA /48 —
    /// the tunnel-plane addresses [`wireguard::ula_v6_member_addr`]
    /// derives. such a route needs applied tunnels to be dialable, so it is
    /// classified as an overlay route, never an underlay replacement.
    fn overlay_route(&self, hint: &ReachHint) -> Result<bool, String> {
        let addr = match &hint.reach {
            Reach::Direct(a) | Reach::Fronted(a) => a,
            Reach::Coordinated(_) => return Ok(false),
        };
        let Some(Ingress::Socket(sock)) = ingress_of(addr)? else {
            return Ok(false); // hostnames and advisory noise are underlay-class.
        };
        let std::net::IpAddr::V6(v6) = sock.ip() else {
            return Ok(false);
        };
        let prefix = wireguard::ula_v6_prefix(&self.genesis_namespace()).octets();
        Ok(v6.octets()[..6] == prefix[..6])
    }

    /// record a reach hint for a member, replacing any previous hint for the
    /// same expected key (a member's reach can move/upgrade). keeps the list
    /// sorted for stable file diffs — mirrors [`NetworkDescriptor::add_bootstrap`].
    pub fn add_reach(&mut self, hint: &ReachHint) {
        let ek = hex_bytes(hint.expected_key.as_ref());
        self.reach.retain(|s| {
            ReachHint::parse(s)
                .map(|h| hex_bytes(h.expected_key.as_ref()) != ek)
                .unwrap_or(true)
        });
        self.reach.push(hint.to_canonical());
        self.reach.sort();
    }

    /// record one explicit typed reach route without collapsing other typed
    /// routes for the same expected key. Use this when the descriptor needs a
    /// real route set, such as rendezvous plus a tunnel-overlay ingress.
    pub fn add_reach_route(&mut self, hint: &ReachHint) {
        let canonical = hint.to_canonical();
        if !self.reach.contains(&canonical) {
            self.reach.push(canonical);
            self.reach.sort();
        }
    }

    /// reach hints resolved to typed dial routes, hostname-native: `Direct`/
    /// `Fronted` become an [`Ingress`] the mesh dials (a hostname stays a
    /// hostname, re-resolved per attempt); `Coordinated` becomes a route the
    /// nat client hole-punches through while still authenticating the
    /// target's own key end-to-end. advisory: an entry that cannot form a
    /// dialable ingress (unspecified ip / port 0 / malformed host) is skipped.
    pub fn reach_entries(&self) -> Result<Vec<(ed25519::PublicKey, ReachDial)>, String> {
        let mut out = Vec::new();
        // one DIRECT ingress per key, underlay preferred: discovery keeps one
        // dial address per peer, and an overlay ULA only answers once the
        // reachability plane's tunnels apply — so when a key carries both an
        // underlay route and its overlay route, the mesh dialer gets the
        // underlay and the plane owns the tunnel path. a key whose ONLY route
        // is the overlay (a fully-NATed member) still dials it, as before.
        let mut direct_at: std::collections::BTreeMap<Vec<u8>, (usize, bool)> =
            std::collections::BTreeMap::new();
        for hint in self.reach_hints()? {
            let dial = match &hint.reach {
                Reach::Direct(a) | Reach::Fronted(a) => match ingress_of(a)? {
                    Some(ingress) => ReachDial::Direct(ingress),
                    None => continue, // unspecified ip / port 0 — advisory noise.
                },
                Reach::Coordinated(c) => match ingress_of(&c.coord_addr)? {
                    Some(coord) => ReachDial::Coordinated { coord, coord_key: c.coord_key.clone() },
                    None => continue,
                },
            };
            if matches!(dial, ReachDial::Direct(_)) {
                let key = hint.expected_key.as_ref().to_vec();
                let overlay = self.overlay_route(&hint)?;
                match direct_at.get(&key) {
                    None => {
                        direct_at.insert(key, (out.len(), overlay));
                    }
                    Some(&(at, held_overlay)) => {
                        if held_overlay && !overlay {
                            // the held slot is the overlay route — replace it
                            // in place with the underlay one.
                            out[at] = (hint.expected_key.clone(), dial);
                            direct_at.insert(key, (at, false));
                        }
                        continue;
                    }
                }
            }
            out.push((hint.expected_key.clone(), dial));
        }
        Ok(out)
    }
}

/// coordination privacy for the reachability plane — per-network operational
/// policy (like `checkpoint_blocks`), NOT part of the genesis fingerprint.
/// `Public` = the coordinator admits any proof-of-possession request;
/// `Private` (the default) also requires a genesis-issued `CoordCap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coordination {
    Public,
    Private,
}

/// Shared public rendezvous coordinator used when a network is created without
/// an explicit direct-only override.
pub const DEFAULT_PRIMARY_COORDINATOR: &str = "p2p.ducktape.byeongsu.dev:3478";

/// The typed invite format still carries a coordinator key, but the deployed
/// coordinator is intentionally keyless. Keep one stable valid key in the
/// signed envelope until coordinator response signing exists.
pub fn keyless_coordinator_placeholder_key() -> ed25519::PublicKey {
    ed25519::PrivateKey::from_seed(0).public_key()
}

/// Resolve the primary coordinator option. `None` means "use the product
/// default"; `"none"`/`"off"` keeps the old direct-only posture.
pub fn primary_coordinator_or_default(raw: Option<&str>) -> Result<Option<String>, String> {
    let coord = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PRIMARY_COORDINATOR);
    if matches!(coord, "none" | "off" | "direct") {
        return Ok(None);
    }
    match ingress_of(coord)? {
        Some(_) => Ok(Some(coord.to_string())),
        None => Err(format!("primary coordinator {coord:?} is not dialable")),
    }
}

/// Resolve the ambient coordinator to a dial [`Ingress`] — the AMBIENT source
/// a joiner's NAT resolver binds (config/default), never one carried in an
/// invite. `None` when coordination is disabled (`"none"`/`"off"`/`"direct"`).
pub fn coordinator_ingress(raw: Option<&str>) -> Result<Option<Ingress>, String> {
    match primary_coordinator_or_default(raw)? {
        Some(addr) => ingress_of(&addr),
        None => Ok(None),
    }
}

impl NetworkDescriptor {
    pub fn coordination(&self) -> Coordination {
        match self.coordination.as_deref() {
            Some("public") => Coordination::Public,
            _ => Coordination::Private,
        }
    }

    /// Make `key` reachable through the configured public coordinator. This is
    /// advisory reachability state, not part of the genesis fingerprint.
    pub fn apply_primary_coordinator(
        &mut self,
        key: &ed25519::PublicKey,
        coord_addr: &str,
    ) -> Result<(), String> {
        let coord_addr = primary_coordinator_or_default(Some(coord_addr))?
            .ok_or("primary coordinator cannot be disabled here")?;
        self.coordination = Some("public".into());
        self.add_reach(&ReachHint {
            expected_key: key.clone(),
            reach: Reach::Coordinated(CoordRef {
                coord_addr,
                coord_key: keyless_coordinator_placeholder_key(),
            }),
        });
        Ok(())
    }

    pub fn has_coordinated_reach(&self) -> Result<bool, String> {
        Ok(self
            .reach_hints()?
            .iter()
            .any(|h| matches!(h.reach, Reach::Coordinated(_))))
    }
}

/// a reach hint resolved to how the mesh actually reaches a member. `Direct`
/// dials the ingress and authenticates `expected_key` end-to-end (a fronted
/// path is transparent, so it looks the same to the dialer); `Coordinated`
/// carries the coordinator's own ingress + identity so the nat client can
/// rendezvous and hole-punch to the target.
#[derive(Clone, Debug)]
pub enum ReachDial {
    Direct(Ingress),
    Coordinated {
        coord: Ingress,
        coord_key: ed25519::PublicKey,
    },
}

pub fn decode_key(hex: &str) -> Result<ed25519::PublicKey, String> {
    let raw = unhex(hex.trim())?;
    ed25519::PublicKey::decode(raw.as_slice())
        .map_err(|e| format!("{hex:?} is not an ed25519 public key: {e}"))
}

// ============================================================================
// coordinator capability — the private-mode admission token a node presents on
// each rendezvous request. Minted by a genesis validator (`mint_coord_cap`),
// persisted 0600 beside the descriptor like `invite.token`. Genesis validators
// need none (the coordinator's pinned set covers them).
// ============================================================================

const COORD_CAP_FILE: &str = "coord.cap";
const COORD_CAP_LEN: usize = 32 + 8 + 64;

pub fn pack_coord_cap(cap: &nat_traversal::CoordCap) -> Vec<u8> {
    let mut out = Vec::with_capacity(COORD_CAP_LEN);
    out.extend_from_slice(cap.issuer.as_ref());
    out.extend_from_slice(&cap.not_after.to_be_bytes());
    out.extend_from_slice(cap.issuer_sig.encode().as_ref());
    out
}

pub fn unpack_coord_cap(bytes: &[u8]) -> Result<nat_traversal::CoordCap, String> {
    if bytes.len() != COORD_CAP_LEN {
        return Err(format!(
            "coord cap must be {COORD_CAP_LEN} bytes, got {}",
            bytes.len()
        ));
    }
    let issuer =
        ed25519::PublicKey::decode(&bytes[..32]).map_err(|e| format!("coord cap issuer: {e}"))?;
    let mut na = [0u8; 8];
    na.copy_from_slice(&bytes[32..40]);
    let not_after = u64::from_be_bytes(na);
    let issuer_sig =
        ed25519::Signature::decode(&bytes[40..]).map_err(|e| format!("coord cap sig: {e}"))?;
    Ok(nat_traversal::CoordCap { issuer, not_after, issuer_sig })
}

pub fn save_coord_cap(dir: &Path, cap: &nat_traversal::CoordCap) -> Result<(), String> {
    use std::io::Write as _;
    let path = dir.join(COORD_CAP_FILE);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).map_err(|e| format!("create {path:?}: {e}"))?;
    f.write_all(format!("{}\n", hex_bytes(&pack_coord_cap(cap))).as_bytes())
        .map_err(|e| format!("write {path:?}: {e}"))
}

pub fn load_coord_cap(dir: &Path) -> Option<nat_traversal::CoordCap> {
    let path = dir.join(COORD_CAP_FILE);
    let raw = std::fs::read_to_string(&path).ok()?;
    let bytes = unhex(raw.trim()).ok()?;
    unpack_coord_cap(&bytes).ok()
}

// ============================================================================
// the lobby identity — a keypair every holder of this network's descriptor can
// DERIVE (seeded from the genesis namespace, which is public to members and
// invitees alike). it authenticates NOTHING: it exists so a not-yet-admitted
// joiner can complete the discovery handshake and be heard on the lobby
// channel at all — authorization is the invite token it then presents. every
// member folds this key into its tracked mesh, so the set stays identical
// across nodes (discovery kills peers whose set at a shared index differs).
// ============================================================================

pub fn lobby_identity(binding: &[u8]) -> ed25519::PrivateKey {
    use commonware_cryptography::{Hasher as _, Sha256};
    let mut hasher = Sha256::default();
    hasher.update(b"ducktape-lobby-v1:");
    hasher.update(binding);
    let digest = hasher.finalize();
    // every 32-byte string is a valid ed25519 seed (the scheme clamps).
    ed25519::PrivateKey::decode(digest.as_ref()).expect("32 digest bytes decode")
}

/// guard a join against clobbering a DIFFERENT network's descriptor: a
/// workspace dir only ever holds one chain-id. a refreshed invite for the
/// SAME chain-id (the documented re-join after a pre-genesis admit) may
/// replace it; anything else is almost certainly a paste into the wrong dir —
/// and for a founder, an unrecoverable one (the time-salted chain-id cannot
/// be re-minted).
pub fn guard_join_descriptor(dir: &Path, incoming: &NetworkDescriptor) -> Result<(), String> {
    let path = dir.join("network.toml");
    if !path.exists() {
        return Ok(());
    }
    let existing = NetworkDescriptor::load(&path)?;
    if existing.chain_id != incoming.chain_id {
        return Err(format!(
            "{} already belongs to network {} — refusing to replace its descriptor with an \
             invite for {}; join a different network with a fresh --dir",
            dir.display(),
            existing.chain_id,
            incoming.chain_id
        ));
    }
    Ok(())
}

/// the `host:port` peers should dial, if one is real: prefer `advertised`, else
/// the listen addr when it is concrete. the returned STRING is what lands in the
/// descriptor verbatim — a HOSTNAME stays a hostname (resolved at dial time), so
/// `node.example.com:443` is a valid advertised addr. an IP that is UNSPECIFIED
/// (0.0.0.0/[::]) or on port 0 is never dialable — writing one would hand every
/// joiner a hint that resolves to their own loopback. an explicitly-passed
/// advertised that is not dialable is an ERROR (the caller asked for it); a
/// non-dialable listen just means "no hint" (Ok(None)). the `"overlay"`
/// sentinel is also "no hint", not an error: the overlay ULA is dialable only
/// over an established tunnel, so it must never be minted into an invite a
/// fresh joiner would dial cold.
pub fn dialable(advertised: Option<&str>, listen: &str) -> Result<Option<String>, String> {
    if let Some(a) = advertised {
        let a = a.trim();
        if a == "overlay" {
            return Ok(None);
        }
        match a.parse::<SocketAddr>() {
            // an IP literal must be concrete.
            Ok(addr) if addr.ip().is_unspecified() || addr.port() == 0 => {
                return Err(format!(
                    "advertised addr {addr} is not dialable (unspecified ip or port 0)"
                ));
            }
            Ok(_) => {}
            // not an IP → a hostname; require a non-empty host and a real port.
            Err(_) if !is_host_port(a) => {
                return Err(format!("advertised addr {a:?} is not host:port"));
            }
            Err(_) => {}
        }
        return Ok(Some(a.to_string()));
    }
    let l: SocketAddr = listen.parse().map_err(|e| format!("listen: {e}"))?;
    Ok((!l.ip().is_unspecified() && l.port() != 0).then(|| l.to_string()))
}

/// a lightweight `host:port` shape check for a non-IP advertised hint: a
/// non-empty host and a numeric, non-zero port. resolution happens later, at
/// dial time (see [`NetworkDescriptor::bootstrap_entries`]).
fn is_host_port(s: &str) -> bool {
    match s.rsplit_once(':') {
        Some((host, port)) => !host.is_empty() && port.parse::<u16>().is_ok_and(|p| p != 0),
        None => false,
    }
}

/// parse a `host:port` into a dial ingress WITHOUT resolving anything: an IP
/// literal becomes `Ingress::Socket`; a host:port-shaped name stays a hostname
/// (`Ingress::Dns`), re-resolved by dialing peers at each attempt. `Ok(None)`
/// = syntactically fine but never dialable (unspecified ip / port 0); a shape
/// that is neither an ip nor host:port is an error.
fn ingress_of(host_port: &str) -> Result<Option<Ingress>, String> {
    if let Ok(addr) = host_port.parse::<SocketAddr>() {
        if addr.ip().is_unspecified() || addr.port() == 0 {
            return Ok(None);
        }
        return Ok(Some(Ingress::Socket(addr)));
    }
    let Some((host, port)) = host_port.rsplit_once(':') else {
        return Err(format!("{host_port:?} is not host:port"));
    };
    let port: u16 = port
        .parse()
        .map_err(|_| format!("{host_port:?} has no valid port"))?;
    if port == 0 {
        return Ok(None);
    }
    let host = Hostname::new(host).map_err(|e| format!("{host_port:?}: bad hostname: {e:?}"))?;
    Ok(Some(Ingress::Dns { host, port }))
}

// ============================================================================
// typed reachability — how to reach a member's REAL node.
// ============================================================================

/// how to reach a member's REAL node. advisory (never part of the genesis
/// fingerprint); the mesh still authenticates the peer by its ed25519 key
/// end-to-end regardless of which socket got dialed.
#[derive(Clone, Debug, PartialEq)]
pub enum Reach {
    /// dial this `host:port` directly (today's bootstrap behaviour).
    Direct(String),
    /// dial a transport forwarder that splices to the target.
    Fronted(String),
    /// dial a coordinator (`coord_addr`) and ask it for a path to the target.
    Coordinated(CoordRef),
}

/// how to reach a coordinator, plus the key it authenticates its channel with.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordRef {
    pub coord_addr: String,
    pub coord_key: ed25519::PublicKey,
}

/// a signed-invite reach hint: the REAL node identity a joiner must end up
/// authenticating, plus how to get a path to it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReachHint {
    pub expected_key: ed25519::PublicKey,
    pub reach: Reach,
}

impl ReachHint {
    /// canonical single-line form stored in `network.toml`'s `reach` array and
    /// parsed by [`ReachHint::parse`]. `@` separates the expected key from the
    /// address, `#` separates a coordinator address from its key; neither char
    /// occurs in a host:port (IPv6 uses `[..]:port`), so the split is unambiguous.
    pub fn to_canonical(&self) -> String {
        let ek = hex_bytes(self.expected_key.as_ref());
        match &self.reach {
            Reach::Direct(a) => format!("direct:{ek}@{a}"),
            Reach::Fronted(a) => format!("fronted:{ek}@{a}"),
            Reach::Coordinated(c) => {
                format!("coordinated:{ek}@{}#{}", c.coord_addr, hex_bytes(c.coord_key.as_ref()))
            }
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let (tag, rest) = s
            .split_once(':')
            .ok_or_else(|| format!("reach hint {s:?} missing a tag"))?;
        let (ek_hex, addr_part) = rest
            .split_once('@')
            .ok_or_else(|| format!("reach hint {s:?} is not tag:key@addr"))?;
        let expected_key = decode_key(ek_hex)?;
        let reach = match tag {
            "direct" => Reach::Direct(addr_part.to_string()),
            "fronted" => Reach::Fronted(addr_part.to_string()),
            "coordinated" => {
                let (coord_addr, ck_hex) = addr_part
                    .rsplit_once('#')
                    .ok_or_else(|| format!("coordinated hint {s:?} missing #coord_key"))?;
                Reach::Coordinated(CoordRef {
                    coord_addr: coord_addr.to_string(),
                    coord_key: decode_key(ck_hex)?,
                })
            }
            other => return Err(format!("unknown reach tag {other:?} in {s:?}")),
        };
        Ok(Self { expected_key, reach })
    }
}

/// EVERY candidate statesync source, ordered: bootstrap-hinted validators
/// first (a dial path is already configured), then the remaining validators.
/// the rotating client fails over down this list — any validator can serve,
/// because every payload verifies against consensus-agreed roots.
pub fn sync_source_candidates<A>(
    bootstrappers: &[(ed25519::PublicKey, A)],
    validators: &[ed25519::PublicKey],
    me: &ed25519::PublicKey,
) -> Vec<ed25519::PublicKey> {
    let mut out: Vec<ed25519::PublicKey> = bootstrappers
        .iter()
        .map(|(k, _)| k)
        .filter(|k| *k != me && validators.contains(k))
        .cloned()
        .collect();
    for k in validators {
        if k != me && !out.contains(k) {
            out.push(k.clone());
        }
    }
    out
}

// ============================================================================
// the workspace registry — the desktop app materializes one directory per
// network under `~/.ducktape/workspaces/<id>/` (node.toml + network.toml +
// identity.key). `--network <chain id>` resolves through it, so the CLI can
// address a node by the name humans actually know.
// ============================================================================

/// the registry root: `$DUCKTAPE_HOME/workspaces` when the override is set
/// (tests, portable setups), else `~/.ducktape/workspaces`.
pub fn workspaces_root() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(home).join("workspaces"));
    }
    let home = std::env::var_os("HOME")
        .ok_or("cannot resolve $HOME — pass --config <node.toml> instead of --network")?;
    Ok(PathBuf::from(home).join(".ducktape").join("workspaces"))
}

/// resolve `--network <chain id>` to a workspace's node.toml: scan the
/// registry for descriptors whose chain-id matches `needle` — exact first,
/// else a unique prefix (so `ducktape` finds `ducktape#a1b2c3d4`). ambiguity
/// and absence are loud errors that name what WAS found.
pub fn find_workspace_config(needle: &str) -> Result<PathBuf, String> {
    find_workspace_config_in(&workspaces_root()?, needle)
}

fn find_workspace_config_in(root: &Path, needle: &str) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(root).map_err(|e| {
        format!("no workspace registry at {root:?} ({e}) — pass --config <node.toml>")
    })?;
    let mut matches: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let descriptor_path = dir.join("network.toml");
        if !descriptor_path.is_file() {
            continue;
        }
        // an unreadable descriptor in one workspace must not break addressing
        // the others — skip it.
        let Ok(d) = NetworkDescriptor::load(&descriptor_path) else {
            continue;
        };
        if d.chain_id == needle {
            return Ok(dir.join("node.toml"));
        }
        if d.chain_id.starts_with(needle) {
            matches.push((d.chain_id, dir.join("node.toml")));
        }
    }
    match matches.len() {
        0 => Err(format!(
            "no workspace under {root:?} matches network {needle:?}"
        )),
        1 => Ok(matches.swap_remove(0).1),
        _ => Err(format!(
            "network {needle:?} is ambiguous — matches: {}",
            matches
                .iter()
                .map(|(c, _)| c.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ducktape-config-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn coord_cap_roundtrips_through_pack_and_files() {
        use commonware_cryptography::{Signer as _, ed25519};
        use nat_traversal::{NodeKey, mint_coord_cap};
        let g = ed25519::PrivateKey::from_seed(7);
        let subject = NodeKey([0x11; 32]);
        let cap = mint_coord_cap(&g, subject, 4_000_000);
        let bytes = pack_coord_cap(&cap);
        assert_eq!(bytes.len(), 32 + 8 + 64);
        assert_eq!(unpack_coord_cap(&bytes).unwrap(), cap);

        let dir = tempfile::tempdir().unwrap();
        assert!(load_coord_cap(dir.path()).is_none());
        save_coord_cap(dir.path(), &cap).unwrap();
        assert_eq!(load_coord_cap(dir.path()).unwrap(), cap);
    }

    #[test]
    fn coordination_defaults_to_private_and_parses_public() {
        let mut d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // default (field unset) -> Private
        assert_eq!(d.coordination(), Coordination::Private);
        d.coordination = Some("public".to_string());
        assert_eq!(d.coordination(), Coordination::Public);
        d.coordination = Some("private".to_string());
        assert_eq!(d.coordination(), Coordination::Private);
    }

    #[test]
    fn primary_coordinator_defaults_to_deployed_public_rendezvous() {
        let coord = primary_coordinator_or_default(None).expect("default coordinator");
        assert_eq!(coord.as_deref(), Some("p2p.ducktape.byeongsu.dev:3478"));

        let disabled = primary_coordinator_or_default(Some("none")).expect("disabled");
        assert_eq!(disabled, None);
    }

    /// `coordinator_ingress` — the AMBIENT source change (1) threads into
    /// both runtime call sites (main.rs:954 joiner, main.rs:3201 member) —
    /// resolves an explicit override, honors the disable sentinel, and
    /// falls back to the compiled default exactly like `coordinator_ingress
    /// (None)` did before change (1) existed (bit-identical absent case).
    #[test]
    fn coordinator_ingress_resolves_an_explicit_override() {
        match coordinator_ingress(Some("203.0.113.9:3478")).expect("dialable override") {
            Some(Ingress::Socket(addr)) => assert_eq!(addr, "203.0.113.9:3478".parse().unwrap()),
            other => panic!("expected a concrete socket ingress, got {other:?}"),
        }
        assert_eq!(
            coordinator_ingress(Some("none")).expect("disabled"),
            None,
            "the sentinel disables coordination outright — no ingress to bind"
        );
        match coordinator_ingress(None).expect("compiled default") {
            Some(Ingress::Dns { port, .. }) => assert_eq!(port, 3478),
            other => panic!("expected the compiled default's hostname ingress, got {other:?}"),
        }
    }

    #[test]
    fn apply_primary_coordinator_records_public_coordinated_self_hint() {
        let me = ed25519::PrivateKey::from_seed(7).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };

        d.apply_primary_coordinator(&me, "p2p.ducktape.byeongsu.dev:3478")
            .expect("coordinator hint");

        assert_eq!(d.coordination(), Coordination::Public);
        let hints = d.reach_hints().expect("hints");
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].expected_key, me);
        match &hints[0].reach {
            Reach::Coordinated(coord) => {
                assert_eq!(coord.coord_addr, "p2p.ducktape.byeongsu.dev:3478");
                assert_eq!(coord.coord_key, keyless_coordinator_placeholder_key());
            }
            other => panic!("expected coordinated reach hint, got {other:?}"),
        }
    }

    /// the FULL delivery chain at the crypto level: a genesis validator mints a
    /// cap for a joiner, it is packed for the wire (`pack_coord_cap`), the
    /// joiner unpacks it (`unpack_coord_cap`) and presents it on an
    /// authenticated request — and the coordinator's private `verify_request`
    /// admits the joiner off that delivered cap. Proves the cap the member
    /// hands over its `JoinReply` actually authorizes the holder.
    #[test]
    fn delivered_cap_admits_the_joiner_under_private_policy() {
        use commonware_cryptography::Signer as _;
        use nat_traversal::{
            mint_coord_cap, now_secs, sign_authenticator, verify_request, AuthPolicy, NodeKey,
            COORD_CAP_TTL_SECS, DEFAULT_FRESHNESS_WINDOW_SECS,
        };

        let genesis = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let mut subj = [0u8; 32];
        subj.copy_from_slice(joiner.public_key().as_ref());
        let subject = NodeKey(subj);

        let now = now_secs();
        // MINT (member side) -> PACK (wire) -> UNPACK (joiner side).
        let minted = mint_coord_cap(&genesis, subject, now + COORD_CAP_TTL_SECS);
        let wire = pack_coord_cap(&minted);
        let delivered = unpack_coord_cap(&wire).expect("joiner unpacks the delivered cap");
        assert_eq!(delivered, minted, "the cap survives the wire byte-for-byte");

        // the joiner builds an authenticated request carrying the delivered cap.
        let inner = b"\x03register-request-bytes";
        let auth = sign_authenticator(&joiner, inner, now, Some(delivered));

        // the coordinator, pinned to this genesis key, admits the joiner.
        let policy = AuthPolicy::Private {
            genesis_set: vec![genesis.public_key()],
        };
        assert_eq!(
            verify_request(
                &policy,
                now,
                DEFAULT_FRESHNESS_WINDOW_SECS,
                subject,
                inner,
                &auth
            ),
            Ok(()),
            "the delivered cap admits the joiner"
        );

        // control: WITHOUT the cap the same private policy rejects the joiner.
        let bare = sign_authenticator(&joiner, inner, now, None);
        assert!(
            verify_request(
                &policy,
                now,
                DEFAULT_FRESHNESS_WINDOW_SECS,
                subject,
                inner,
                &bare
            )
            .is_err(),
            "a joiner with no cap is not admitted to a private network"
        );
    }

    #[test]
    fn network_flag_resolves_workspaces_by_chain_id_prefix() {
        let root = tmp("registry");
        for (ws, chain) in [("a", "ducktape#a1b2c3d4"), ("b", "kitchen#99887766")] {
            let dir = root.join(ws);
            std::fs::create_dir_all(&dir).expect("mk workspace");
            let d = NetworkDescriptor {
                chain_id: chain.into(),
                scheme: SCHEME_ED25519.into(),
                validators: vec![hex_bytes(
                    ed25519::PrivateKey::from_seed(40).public_key().as_ref(),
                )],
                bootstrap: vec![],
                reach: vec![],
                coordination: None,
            };
            d.save(&dir.join("network.toml")).expect("save");
        }
        // stray non-workspace entries are skipped, not errors.
        std::fs::create_dir_all(root.join("not-a-workspace")).expect("mk stray");

        // exact and unique-prefix both land on the workspace's node.toml.
        assert_eq!(
            find_workspace_config_in(&root, "ducktape#a1b2c3d4").expect("exact"),
            root.join("a").join("node.toml")
        );
        assert_eq!(
            find_workspace_config_in(&root, "kitchen").expect("prefix"),
            root.join("b").join("node.toml")
        );
        // absence and ambiguity are loud.
        assert!(find_workspace_config_in(&root, "nope").is_err());
        let err = find_workspace_config_in(&root, "").expect_err("ambiguous");
        assert!(err.contains("ambiguous"), "{err}");
    }

    #[test]
    fn admit_is_idempotent_and_sorted() {
        let a = ed25519::PrivateKey::from_seed(1).public_key();
        let b = ed25519::PrivateKey::from_seed(2).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "t#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.admit(&a);
        d.admit(&b);
        d.admit(&a);
        assert_eq!(d.validators.len(), 2);
        let mut sorted = d.validators.clone();
        sorted.sort();
        assert_eq!(d.validators, sorted);
    }

    #[test]
    fn duplicate_validators_are_a_config_error() {
        let a = ed25519::PrivateKey::from_seed(4).public_key();
        let d = NetworkDescriptor {
            chain_id: "dup#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref()), hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        assert!(
            d.validator_keys().is_err(),
            "dups must not reach Set::try_from"
        );
    }

    #[test]
    fn genesis_namespace_fingerprints_the_validator_set_not_the_hints() {
        let a = ed25519::PrivateKey::from_seed(5).public_key();
        let b = ed25519::PrivateKey::from_seed(6).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "net#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        let founder_only = d.genesis_namespace();
        assert!(founder_only.starts_with("net#00000000@"));

        // bootstrap hints are advisory and legitimately differ per member —
        // they must NOT move the namespace.
        d.add_bootstrap(&a, "127.0.0.1:52200");
        assert_eq!(d.genesis_namespace(), founder_only);

        // admitting a member DOES move it: a stale descriptor can no longer
        // handshake, so genesis divergence is loud, not a silent fork.
        d.admit(&b);
        assert_ne!(d.genesis_namespace(), founder_only);

        // and it is order-independent (canonical over the sorted set).
        let mut reversed = d.clone();
        reversed.validators.reverse();
        assert_eq!(reversed.genesis_namespace(), d.genesis_namespace());
    }

    #[test]
    fn dialable_rejects_unspecified_ips_and_port_zero() {
        // listen fallback: concrete -> hint, unspecified/port-0 -> no hint.
        assert!(dialable(None, "127.0.0.1:52200").unwrap().is_some());
        assert!(dialable(None, "0.0.0.0:52200").unwrap().is_none());
        assert!(dialable(None, "[::]:52200").unwrap().is_none());
        assert!(dialable(None, "127.0.0.1:0").unwrap().is_none());
        // an EXPLICIT advertised addr that is not dialable is an error.
        assert!(dialable(Some("0.0.0.0:52200"), "127.0.0.1:52200").is_err());
        assert!(dialable(Some("1.2.3.4:0"), "127.0.0.1:52200").is_err());
        assert_eq!(
            dialable(Some("1.2.3.4:5"), "127.0.0.1:0").unwrap(),
            Some("1.2.3.4:5".to_string())
        );
        // a HOSTNAME advertised is kept verbatim (resolved at dial time), not
        // rejected — invites can carry a domain like node.example.com:443.
        assert_eq!(
            dialable(Some("node.example.com:443"), "127.0.0.1:0").unwrap(),
            Some("node.example.com:443".to_string())
        );
        assert!(dialable(Some("node.example.com:0"), "127.0.0.1:52200").is_err());
        assert!(dialable(Some("not-an-addr"), "127.0.0.1:52200").is_err());
        // the overlay sentinel is "no underlay hint", never an error — an
        // invite minted from an overlay-advertised member carries only the
        // descriptor's existing reach hints.
        assert!(dialable(Some("overlay"), "[::]:52200").unwrap().is_none());
    }

    #[test]
    fn join_guard_refuses_a_foreign_descriptor_but_allows_the_refresh() {
        let a = ed25519::PrivateKey::from_seed(11).public_key();
        let dir = tmp("joinguard");
        let ours = NetworkDescriptor {
            chain_id: "home#11111111".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // empty dir: anything goes.
        assert!(guard_join_descriptor(&dir, &ours).is_ok());
        ours.save(&dir.join("network.toml")).expect("save");

        // the refreshed invite (same chain-id, more members) is the re-join.
        let mut refreshed = ours.clone();
        refreshed.admit(&ed25519::PrivateKey::from_seed(12).public_key());
        assert!(guard_join_descriptor(&dir, &refreshed).is_ok());

        // a different network's invite must never clobber this workspace.
        let foreign = NetworkDescriptor {
            chain_id: "other#22222222".into(),
            ..ours.clone()
        };
        let err = guard_join_descriptor(&dir, &foreign).expect_err("foreign refused");
        assert!(
            err.contains("home#11111111"),
            "error names the resident network: {err}"
        );
    }

    #[test]
    fn mixed_case_duplicate_validators_are_caught_at_the_decoded_key() {
        let a = ed25519::PrivateKey::from_seed(21).public_key();
        let lower = hex_bytes(a.as_ref());
        let upper = lower.to_ascii_uppercase();
        // constructed directly (bypassing from_toml's normalization) — the
        // dedup must hold on the DECODED key, not the spelling.
        let d = NetworkDescriptor {
            chain_id: "case#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![lower, upper],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        assert!(
            d.validator_keys().is_err(),
            "case variants decode to one key"
        );
    }

    #[test]
    fn descriptors_canonicalize_on_load_and_fingerprint_canonically() {
        let a = ed25519::PrivateKey::from_seed(22).public_key();
        let b = ed25519::PrivateKey::from_seed(23).public_key();
        let canonical = NetworkDescriptor {
            chain_id: "canon#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref()), hex_bytes(b.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // a hand-edited twin: uppercase, whitespace, different order.
        let messy = NetworkDescriptor {
            chain_id: "canon#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![
                format!("  {}  ", hex_bytes(b.as_ref()).to_ascii_uppercase()),
                hex_bytes(a.as_ref()),
            ],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // identical decoded sets MUST run under the identical namespace.
        assert_eq!(messy.genesis_namespace(), canonical.genesis_namespace());
        // the fingerprint is 128-bit (32 hex chars) — wide enough that
        // grinding an admit that keeps it unchanged is infeasible.
        let ns = canonical.genesis_namespace();
        assert_eq!(ns.split('@').nth(1).unwrap().len(), 32);
        // and loading the messy spelling from toml normalizes it away.
        let reloaded = NetworkDescriptor::from_toml(&messy.to_toml()).unwrap();
        assert_eq!(reloaded.validators, {
            let mut v = vec![hex_bytes(a.as_ref()), hex_bytes(b.as_ref())];
            v.sort();
            v
        });
    }

    #[test]
    fn undialable_bootstrap_hints_are_skipped_not_dialed() {
        let a = ed25519::PrivateKey::from_seed(24).public_key();
        let b = ed25519::PrivateKey::from_seed(25).public_key();
        let d = NetworkDescriptor {
            chain_id: "hints#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![
                format!("{}@0.0.0.0:52200", hex_bytes(a.as_ref())),
                format!("{}@127.0.0.1:0", hex_bytes(a.as_ref())),
                format!("{}@127.0.0.1:52200", hex_bytes(b.as_ref())),
            ],
            reach: vec![],
            coordination: None,
        };
        let entries = d.bootstrap_entries().expect("well-formed hints parse");
        assert_eq!(
            entries.len(),
            1,
            "0.0.0.0 and port-0 hints are advisory noise"
        );
        assert_eq!(entries[0].0, b);
        // malformed is still an error, never a skip.
        let bad = NetworkDescriptor {
            bootstrap: vec!["nope".into()],
            reach: vec![],
            coordination: None,
            ..d
        };
        assert!(bad.bootstrap_entries().is_err());
    }

    #[test]
    fn reach_hint_canonical_roundtrips_every_kind() {
        let ek = ed25519::PrivateKey::from_seed(11).public_key();
        let ck = ed25519::PrivateKey::from_seed(12).public_key();
        let cases = [
            ReachHint { expected_key: ek.clone(), reach: Reach::Direct("127.0.0.1:9000".into()) },
            ReachHint { expected_key: ek.clone(), reach: Reach::Fronted("front.example.com:443".into()) },
            ReachHint {
                expected_key: ek.clone(),
                reach: Reach::Coordinated(CoordRef {
                    coord_addr: "p2p.ducktape.industries:7777".into(),
                    coord_key: ck.clone(),
                }),
            },
        ];
        for h in cases {
            let s = h.to_canonical();
            assert_eq!(ReachHint::parse(&s).expect("parse"), h, "roundtrip {s}");
        }
    }

    #[test]
    fn reach_hint_parse_rejects_malformed() {
        assert!(ReachHint::parse("nope").is_err(), "no tag");
        assert!(ReachHint::parse("direct:deadbeef").is_err(), "no @addr");
        assert!(ReachHint::parse("bogus:00@host:1").is_err(), "unknown tag");
        assert!(ReachHint::parse("direct:zz@host:1").is_err(), "bad hex key");
        // coordinated without the #coord_key delimiter:
        assert!(ReachHint::parse("coordinated:00@host:1").is_err(), "missing #coord_key");
    }

    #[test]
    fn reach_field_defaults_empty_and_toml_roundtrips() {
        let a = ed25519::PrivateKey::from_seed(21).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "r#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // an existing network.toml without a [reach] array still parses (serde default),
        // and an empty reach is not serialised (skip_serializing_if).
        assert!(!d.to_toml().contains("reach"));
        d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("10.0.0.1:9000".into()) });
        let back = NetworkDescriptor::from_toml(&d.to_toml()).expect("roundtrip");
        assert_eq!(back.reach, d.reach);
    }

    #[test]
    fn reach_hints_synthesizes_direct_from_bootstrap_when_reach_empty() {
        let a = ed25519::PrivateKey::from_seed(22).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "r#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&a, "127.0.0.1:52200");
        let hints = d.reach_hints().expect("hints");
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0], ReachHint { expected_key: a, reach: Reach::Direct("127.0.0.1:52200".into()) });
    }

    #[test]
    fn add_reach_dedups_by_expected_key_and_sorts() {
        let a = ed25519::PrivateKey::from_seed(23).public_key();
        let coord = ed25519::PrivateKey::from_seed(24).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "r#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("1.1.1.1:1".into()) });
        // same expected_key, different reach — replaces, never duplicates.
        d.add_reach(&ReachHint {
            expected_key: a.clone(),
            reach: Reach::Coordinated(CoordRef { coord_addr: "c:2".into(), coord_key: coord }),
        });
        assert_eq!(d.reach.len(), 1);
        assert!(matches!(d.reach_hints().unwrap()[0].reach, Reach::Coordinated(_)));
        let mut sorted = d.reach.clone();
        sorted.sort();
        assert_eq!(d.reach, sorted);
    }

    #[test]
    fn add_reach_route_keeps_coordinated_and_overlay_routes_for_same_key() {
        let a = ed25519::PrivateKey::from_seed(25).public_key();
        let coord = ed25519::PrivateKey::from_seed(26).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "r#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_reach(&ReachHint {
            expected_key: a.clone(),
            reach: Reach::Coordinated(CoordRef {
                coord_addr: "127.0.0.1:3478".into(),
                coord_key: coord,
            }),
        });
        d.add_reach_route(&ReachHint {
            expected_key: a.clone(),
            reach: Reach::Direct("[fd87::1]:52200".into()),
        });

        let hints = d.reach_hints().expect("hints");
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().any(|h| matches!(h.reach, Reach::Coordinated(_))));
        assert!(hints.iter().any(|h| matches!(h.reach, Reach::Direct(_))));

        let entries = d.reach_entries().expect("entries");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|(_, r)| matches!(r, ReachDial::Coordinated { .. })));
        assert!(entries.iter().any(|(_, r)| matches!(r, ReachDial::Direct(_))));
    }

    #[test]
    fn a_coordinated_hint_does_not_suppress_a_founders_direct_bootstrap_route() {
        // a founder that advertises a real dial address AND enables a
        // coordinator must keep BOTH: the direct bootstrap route (punch-free
        // first choice) and the coordinated rendezvous route. a Coordinated
        // typed hint must not erase the bootstrap-synthesised Direct for the
        // same key — otherwise every invite ships coordinator-only reach and a
        // failed punch is terminal (no relay fallback).
        let me = ed25519::PrivateKey::from_seed(41).public_key();
        let coord = ed25519::PrivateKey::from_seed(42).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "fp#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&me, "203.0.113.7:52200");
        d.add_reach(&ReachHint {
            expected_key: me.clone(),
            reach: Reach::Coordinated(CoordRef {
                coord_addr: "127.0.0.1:3478".into(),
                coord_key: coord,
            }),
        });

        let hints = d.reach_hints().expect("hints");
        assert_eq!(
            hints.len(),
            2,
            "the direct bootstrap route must survive alongside the coordinated hint"
        );
        assert!(
            hints
                .iter()
                .any(|h| matches!(&h.reach, Reach::Direct(a) if a == "203.0.113.7:52200")),
            "the founder's advertised direct route was dropped"
        );
        assert!(hints.iter().any(|h| matches!(h.reach, Reach::Coordinated(_))));

        // a typed DIRECT hint for the same key still supersedes the bootstrap
        // Direct (the member's dial address moved) — no stale duplicate.
        d.add_reach_route(&ReachHint {
            expected_key: me.clone(),
            reach: Reach::Direct("[fd87::2]:52200".into()),
        });
        let hints = d.reach_hints().expect("hints");
        let directs: Vec<_> = hints
            .iter()
            .filter(|h| matches!(h.reach, Reach::Direct(_)))
            .collect();
        assert_eq!(
            directs.len(),
            1,
            "a typed Direct supersedes the bootstrap Direct for the same key"
        );
        assert!(matches!(&directs[0].reach, Reach::Direct(a) if a == "[fd87::2]:52200"));
    }

    #[test]
    fn an_overlay_ula_route_keeps_the_underlay_bootstrap_dial() {
        // the join path records the inviter's tunnel address (the chain's
        // overlay ULA) as a typed Direct route. that route is only dialable
        // once the reachability plane's tunnels apply, so it must ride
        // ALONGSIDE the underlay bootstrap hint — not evict it — and the mesh
        // dialer must keep dialing the underlay while the plane assembles.
        let me = ed25519::PrivateKey::from_seed(51).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "fp#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&me, "203.0.113.7:52200");
        // the EXACT derivation cmd_join uses for the inviter's tunnel route.
        let identity = wireguard::ValidatorIdentity::try_from(me.as_ref())
            .expect("test key is a valid identity");
        let ula = wireguard::ula_v6_member_addr(&d.genesis_namespace(), identity);
        d.add_reach_route(&ReachHint {
            expected_key: me.clone(),
            reach: Reach::Direct(format!("[{ula}]:52200")),
        });

        // the union keeps BOTH routes…
        let hints = d.reach_hints().expect("hints");
        let directs: Vec<_> = hints
            .iter()
            .filter(|h| matches!(h.reach, Reach::Direct(_)))
            .collect();
        assert_eq!(
            directs.len(),
            2,
            "the overlay ULA route must not evict the underlay bootstrap hint"
        );

        // …and the dialer gets ONE Direct ingress for the key: the underlay.
        let entries = d.reach_entries().expect("entries");
        let dials: Vec<_> = entries
            .iter()
            .filter(|(k, r)| *k == me && matches!(r, ReachDial::Direct(_)))
            .collect();
        assert_eq!(dials.len(), 1, "one Direct ingress per key");
        assert!(
            matches!(
                &dials[0].1,
                ReachDial::Direct(Ingress::Socket(s)) if s.to_string() == "203.0.113.7:52200"
            ),
            "the mesh dialer must prefer the underlay route, got {:?}",
            dials[0].1
        );
    }

    #[test]
    fn reach_hints_are_excluded_from_the_genesis_fingerprint() {
        let v = ed25519::PrivateKey::from_seed(31).public_key();
        let coord = ed25519::PrivateKey::from_seed(32).public_key();
        let base = NetworkDescriptor {
            chain_id: "fp#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(v.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        let ns0 = base.genesis_namespace();

        let mut with_reach = base.clone();
        with_reach.add_bootstrap(&v, "127.0.0.1:52200");
        with_reach.add_reach(&ReachHint {
            expected_key: v.clone(),
            reach: Reach::Coordinated(CoordRef { coord_addr: "p2p:7777".into(), coord_key: coord }),
        });

        // advisory reach + bootstrap NEVER move the consensus identity.
        assert_eq!(with_reach.genesis_namespace(), ns0);
        // two descriptors differing ONLY in reach fingerprint identically.
        let mut other_reach = base.clone();
        other_reach.add_reach(&ReachHint { expected_key: v, reach: Reach::Direct("9.9.9.9:9".into()) });
        assert_eq!(other_reach.genesis_namespace(), ns0);
    }

    // ---- Task 7: reach resolution to dial targets ----

    #[test]
    fn coordinated_hint_resolves_dial_target_to_coord_addr_with_expected_key() {
        let target = ed25519::PrivateKey::from_seed(71).public_key();
        let coord = ed25519::PrivateKey::from_seed(72).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "co#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(target.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // a coordinated hint routes through the coordinator, but the identity
        // we expect end-to-end is the TARGET; the coordinator's own ingress and
        // key ride along for the nat client to rendezvous through.
        d.add_reach(&ReachHint {
            expected_key: target.clone(),
            reach: Reach::Coordinated(CoordRef { coord_addr: "127.0.0.1:59999".into(), coord_key: coord.clone() }),
        });
        let entries = d.reach_entries().expect("resolve");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, target); // expect target key
        match &entries[0].1 {
            ReachDial::Coordinated { coord: c, coord_key } => {
                assert_eq!(*c, Ingress::Socket("127.0.0.1:59999".parse().unwrap()));
                assert_eq!(*coord_key, coord);
            }
            other => panic!("expected a coordinated dial, got {other:?}"),
        }
    }

    #[test]
    fn reach_entries_folds_bootstrap_and_reach_into_direct_ingresses() {
        let a = ed25519::PrivateKey::from_seed(73).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "co#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        // a bootstrap hint alone synthesises a Direct reach ingress...
        d.add_bootstrap(&a, "127.0.0.1:52200");
        let entries = d.reach_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, a);
        assert!(matches!(&entries[0].1, ReachDial::Direct(i) if *i == Ingress::Socket("127.0.0.1:52200".parse().unwrap())));
        // ...and an explicit reach hint for the same key wins over it (union,
        // reach-preferred), still one Direct entry.
        d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("127.0.0.1:52201".into()) });
        let entries = d.reach_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0].1, ReachDial::Direct(i) if *i == Ingress::Socket("127.0.0.1:52201".parse().unwrap())));
    }

    #[test]
    fn sync_source_candidates_prefer_validator_hints_and_never_self() {
        let me = ed25519::PrivateKey::from_seed(31).public_key();
        let resident = ed25519::PrivateKey::from_seed(32).public_key();
        let validator = ed25519::PrivateKey::from_seed(33).public_key();
        let addr: SocketAddr = "127.0.0.1:52200".parse().unwrap();
        let validators = vec![me.clone(), validator.clone()];

        // a non-validator hint sorts first but can never serve — skipped.
        let hints = vec![(resident.clone(), addr), (validator.clone(), addr)];
        assert_eq!(
            sync_source_candidates(&hints, &validators, &me),
            vec![validator.clone()]
        );

        // no usable hint: any validator that is not us.
        let no_hints: &[(ed25519::PublicKey, SocketAddr)] = &[];
        assert_eq!(
            sync_source_candidates(no_hints, &validators, &me),
            vec![validator.clone()]
        );

        // solo network: nobody can serve.
        assert!(sync_source_candidates(no_hints, std::slice::from_ref(&me), &me).is_empty());
    }
}
