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
use commonware_cryptography::{Signer as _, Verifier as _, ed25519};
use serde::{Deserialize, Serialize};

/// the consensus scheme tag a descriptor must carry — a genesis-wide constant
/// (see `ConsensusScheme`); anything else is a build from the future.
pub const SCHEME_ED25519: &str = "ed25519";

/// the v2 invite prefix (PARSE-ONLY now — no production encoder). a v2 paste
/// still decodes to all-`Direct`, unsigned reach hints for backward compat.
const INVITE_PREFIX_V2: &str = "ducktape-invite-v2:";
/// the v3 invite prefix; a v3 paste is visibly distinct from v2 and the two are
/// never confusable (prefix and payload version byte must agree on decode).
const INVITE_PREFIX_V3: &str = "ducktape-invite-v3:";

// ============================================================================
// hex — dependency-free codecs for keys, roots, and the invite blob.
// ============================================================================

pub fn hex_bytes(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn unhex(s: &str) -> Result<Vec<u8>, String> {
    // strict hex digits only: from_str_radix would tolerate '+' signs, and
    // byte-offset slicing below panics mid-codepoint on multibyte utf-8 —
    // this parses PASTED input (invite blobs, rpc hex).
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("hex string contains non-hex characters".into());
    }
    if s.len() % 2 != 0 {
        return Err("hex string has odd length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

// ============================================================================
// identity files — a persisted ed25519 secret, hex-encoded.
// ============================================================================

/// load the identity at `path`, or generate one there from OS randomness.
/// returns the signer and whether it was freshly generated. written 0600 on
/// unix — it is the node's (and for now the user's) whole identity.
pub fn load_or_generate_identity(path: &Path) -> Result<(ed25519::PrivateKey, bool), String> {
    if path.exists() {
        return load_identity(path).map(|k| (k, false));
    }
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
    // every 32-byte string is a valid ed25519 seed (the scheme clamps), so
    // decode cannot fail on fresh OS randomness.
    let key = ed25519::PrivateKey::decode(raw.as_slice()).expect("32 random bytes decode");
    let encoded = hex_bytes(key.encode().as_ref());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {dir:?}: {e}"))?;
    }
    // the file is BORN 0600 (no write-then-chmod window a co-tenant could
    // read the secret through), and create_new makes exists-then-create
    // race-free: a concurrent keygen loses cleanly instead of clobbering.
    {
        use std::io::Write as _;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(path)
            .map_err(|e| format!("create {path:?}: {e}"))?;
        if let Err(e) = f.write_all(format!("{encoded}\n").as_bytes()) {
            // a partial file would shadow every future load with a decode
            // error; remove it so the next run regenerates cleanly.
            let _ = std::fs::remove_file(path);
            return Err(format!("write {path:?}: {e}"));
        }
    }
    Ok((key, true))
}

pub fn load_identity(path: &Path) -> Result<ed25519::PrivateKey, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let raw = unhex(text.trim()).map_err(|e| format!("{path:?}: {e}"))?;
    ed25519::PrivateKey::decode(raw.as_slice())
        .map_err(|e| format!("{path:?} is not an ed25519 secret: {e}"))
}

/// mint a chain-id: the human-readable name plus a short salt, so two
/// unrelated networks that pick the same name still get distinct namespaces
/// (their handshakes fail cleanly instead of colliding). the salt hashes the
/// initiator's identity and the wall clock — unique enough for a network id,
/// no rng plumbing needed.
pub fn mint_chain_id(name: &str, initiator: &ed25519::PublicKey) -> String {
    use commonware_cryptography::{Hasher as _, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock is past the epoch")
        .as_nanos();
    let mut hasher = Sha256::default();
    hasher.update(initiator.as_ref());
    hasher.update(&nanos.to_le_bytes());
    let digest = hasher.finalize();
    format!("{name}#{}", hex_bytes(&digest.as_ref()[..4]))
}

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

    /// bootstrap entries RESOLVED to concrete socket addrs to dial. the host may
    /// be a literal IP or a HOSTNAME — `to_socket_addrs` resolves either (DNS for
    /// a name), so `pubkey@node.example.com:443` works the same as an ip. a
    /// MALFORMED entry (no `@`, bad key) is a config error; a hint that does not
    /// resolve, or resolves to an unspecified ip / port 0, is advisory and
    /// skipped rather than failing startup.
    // retained as descriptor API (and exercised by tests) though the live dial
    // path now goes through `reach_entries`, which synthesises the same Direct
    // entries from `bootstrap` when `reach` is empty.
    #[allow(dead_code)]
    pub fn bootstrap_entries(&self) -> Result<Vec<(ed25519::PublicKey, SocketAddr)>, String> {
        use std::net::ToSocketAddrs as _;
        let mut out = Vec::new();
        for entry in &self.bootstrap {
            let (key, host_port) = entry
                .split_once('@')
                .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
            let key = decode_key(key)?;
            let Some(addr) = host_port.to_socket_addrs().ok().and_then(|mut it| it.next()) else {
                continue; // unresolvable (stale DNS, offline) — advisory, skip.
            };
            if addr.ip().is_unspecified() || addr.port() == 0 {
                continue;
            }
            out.push((key, addr));
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
        if !self.reach.is_empty() {
            return self.reach.iter().map(|s| ReachHint::parse(s)).collect();
        }
        self.bootstrap
            .iter()
            .map(|entry| {
                let (k, addr) = entry
                    .split_once('@')
                    .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
                Ok(ReachHint { expected_key: decode_key(k)?, reach: Reach::Direct(addr.to_string()) })
            })
            .collect()
    }

    /// record a reach hint for a member, replacing any previous hint for the
    /// same expected key (a member's reach can move/upgrade). keeps the list
    /// sorted for stable file diffs — mirrors [`NetworkDescriptor::add_bootstrap`].
    // forward-looking API: the founder/member gains a `Coordinated`/`Fronted`
    // reach via this in Slice 2/4; Slice 1's CLI only ever calls `add_bootstrap`,
    // so it is currently exercised only by tests.
    #[allow(dead_code)]
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

    /// reach hints resolved to `(expected_key, dial_addr)`: what a joiner dials
    /// and the identity it must end up authenticating end-to-end. `Direct`/
    /// `Fronted` dial the hint's own address; `Coordinated` dials the COORDINATOR
    /// while still expecting the target's key. advisory: an unresolvable or
    /// unspecified/port-0 hint is skipped, mirroring [`NetworkDescriptor::bootstrap_entries`].
    pub fn reach_entries(&self) -> Result<Vec<(ed25519::PublicKey, SocketAddr)>, String> {
        use std::net::ToSocketAddrs as _;
        let mut out = Vec::new();
        for hint in self.reach_hints()? {
            let dial = match &hint.reach {
                Reach::Direct(a) | Reach::Fronted(a) => a,
                Reach::Coordinated(c) => &c.coord_addr,
            };
            let Some(addr) = dial.to_socket_addrs().ok().and_then(|mut it| it.next()) else {
                continue; // unresolvable (stale DNS, offline) — advisory, skip.
            };
            if addr.ip().is_unspecified() || addr.port() == 0 {
                continue;
            }
            out.push((hint.expected_key.clone(), addr));
        }
        Ok(out)
    }
}

pub fn decode_key(hex: &str) -> Result<ed25519::PublicKey, String> {
    let raw = unhex(hex.trim())?;
    ed25519::PublicKey::decode(raw.as_slice())
        .map_err(|e| format!("{hex:?} is not an ed25519 public key: {e}"))
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
/// non-dialable listen just means "no hint" (Ok(None)).
pub fn dialable(advertised: Option<&str>, listen: &str) -> Result<Option<String>, String> {
    if let Some(a) = advertised {
        let a = a.trim();
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

/// resolve a `host:port` (IP literal or hostname via DNS) to a single socket
/// addr, erroring if it does not resolve.
fn resolve_one(host_port: &str) -> Result<SocketAddr, String> {
    use std::net::ToSocketAddrs as _;
    host_port
        .to_socket_addrs()
        .map_err(|e| format!("{host_port:?}: {e}"))?
        .next()
        .ok_or_else(|| format!("{host_port:?} did not resolve"))
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

// ============================================================================
// the invite blob — the descriptor packed into a compact, single-line token.
//
// v1 hex-wrapped the whole `network.toml` (field names, quotes, and 64-char hex
// keys carried twice), which ballooned a solo invite past 470 chars. v2 packed
// only what a joiner needs — chain-id, the raw (un-hexed) validator keys, and
// raw key+addr dial hints — and base64url-encoded it. ~4x smaller.
//
// v3 (the production encoder) carries the same chain-id + validators plus TYPED
// reach hints (`Direct`/`Fronted`/`Coordinated`), an expiry, the inviter's
// embedded public key, and a domain-separated ed25519 signature over the whole
// envelope. decode FAILS CLOSED: it verifies the signature against the embedded
// key, requires that key to be a genesis validator, and rejects an expired blob.
// v2 remains PARSE-ONLY (a v2 paste decodes to all-`Direct`, unsigned hints);
// v3 and v2 are never confusable — distinct prefix AND version byte, which must
// agree on decode. flag-day: v1 blobs no longer decode.
// ============================================================================

/// v2 invite payload format tag (the first packed byte). parse-only.
const INVITE_VERSION_V2: u8 = 2;
/// v3 invite payload format tag (the first packed byte). the production encoder.
const INVITE_VERSION_V3: u8 = 3;

/// domain separator for the v3 invite signature (matches the wireguard-upgrade
/// namespace convention, e.g. `ENDPOINT_NS = b"ducktape:wireguard-endpoint:v1"`).
const INVITE_SIG_NS: &[u8] = b"ducktape:invite:v3:";

const INVITE_B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// default invite lifetime if `--ttl-days` is not given.
pub const DEFAULT_INVITE_TTL_DAYS: u64 = 7;

/// current unix time in whole seconds (invite expiry base).
pub fn unix_now_secs() -> Result<u64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock is before the unix epoch".to_string())?
        .as_secs())
}

/// invite expiry (unix secs) = now + ttl_days, erroring rather than overflowing.
pub fn invite_expiry(now_unix: u64, ttl_days: u64) -> Result<u64, String> {
    let secs = ttl_days.checked_mul(86_400).ok_or("--ttl-days too large")?;
    now_unix
        .checked_add(secs)
        .ok_or_else(|| "invite expiry overflow".to_string())
}

/// encode a v3 invite: the descriptor's reach hints + expiry, signed by the
/// inviter. the inviter must be a genesis validator (enforced on decode).
pub fn encode_invite(
    descriptor: &NetworkDescriptor,
    inviter: &ed25519::PrivateKey,
    expires_unix: u64,
) -> Result<String, String> {
    use base64::Engine as _;
    let payload = pack_invite_v3(descriptor, inviter, expires_unix)?;
    Ok(format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(payload)))
}

pub fn decode_invite(blob: &str) -> Result<NetworkDescriptor, String> {
    decode_invite_at(blob, unix_now_secs()?)
}

/// clock-injected decode core so expiry is deterministically testable;
/// [`decode_invite`] reads the real clock and delegates.
fn decode_invite_at(blob: &str, now_unix: u64) -> Result<NetworkDescriptor, String> {
    use base64::Engine as _;
    let blob = blob.trim();
    // choose the codec by prefix; the payload version byte must AGREE (defence in
    // depth: a v2 payload can never ride under a v3 prefix, or vice-versa).
    let (body, prefix_version) = if let Some(b) = blob.strip_prefix(INVITE_PREFIX_V3) {
        (b, INVITE_VERSION_V3)
    } else if let Some(b) = blob.strip_prefix(INVITE_PREFIX_V2) {
        (b, INVITE_VERSION_V2)
    } else {
        return Err(format!(
            "not a ducktape invite (expected {INVITE_PREFIX_V3}... or {INVITE_PREFIX_V2}...)"
        ));
    };
    let bytes = INVITE_B64
        .decode(body)
        .map_err(|e| format!("invite is not valid base64url: {e}"))?;
    let version = *bytes.first().ok_or("invite payload is empty")?;
    if version != prefix_version {
        return Err(format!(
            "invite prefix is v{prefix_version} but payload is v{version}"
        ));
    }
    match version {
        INVITE_VERSION_V2 => unpack_invite_v2(&bytes),
        INVITE_VERSION_V3 => unpack_invite_v3(&bytes, now_unix),
        other => Err(format!(
            "unsupported invite version {other} (this build reads v{INVITE_VERSION_V2}/v{INVITE_VERSION_V3})"
        )),
    }
}

/// pack a descriptor into the v3 payload and sign it. reach hints come from
/// [`NetworkDescriptor::reach_hints`] (which synthesises `Direct` hints from
/// `bootstrap` when `reach` is empty), so a founder that only ever ran
/// `add_bootstrap` still ships a well-formed, signed v3 invite. the 64-byte
/// signature is appended after the inviter's embedded key and is NOT itself
/// signed — so `bytes[..len-64]` on decode is exactly the signed region.
fn pack_invite_v3(
    d: &NetworkDescriptor,
    inviter: &ed25519::PrivateKey,
    expires_unix: u64,
) -> Result<Vec<u8>, String> {
    let mut out = vec![INVITE_VERSION_V3];

    let cid = d.chain_id.as_bytes();
    out.push(u8::try_from(cid.len()).map_err(|_| format!("chain_id too long ({} bytes)", cid.len()))?);
    out.extend_from_slice(cid);

    let vkeys = d.validator_keys()?; // hex -> raw, deduped, rejects malformed here
    out.push(u8::try_from(vkeys.len()).map_err(|_| format!("too many validators ({})", vkeys.len()))?);
    for k in &vkeys {
        out.extend_from_slice(k.as_ref());
    }

    let hints = d.reach_hints()?;
    out.push(u8::try_from(hints.len()).map_err(|_| format!("too many reach hints ({})", hints.len()))?);
    for h in &hints {
        out.extend_from_slice(h.expected_key.as_ref());
        match &h.reach {
            Reach::Direct(a) => {
                out.push(0);
                put_str_u8(&mut out, a)?;
            }
            Reach::Fronted(a) => {
                out.push(1);
                put_str_u8(&mut out, a)?;
            }
            Reach::Coordinated(c) => {
                out.push(2);
                put_str_u8(&mut out, &c.coord_addr)?;
                out.extend_from_slice(c.coord_key.as_ref());
            }
        }
    }

    out.extend_from_slice(&expires_unix.to_le_bytes());
    out.extend_from_slice(inviter.public_key().as_ref());

    // sign everything above; the 64-byte signature is appended and not itself signed.
    let sig = inviter.sign(INVITE_SIG_NS, &out);
    out.extend_from_slice(sig.as_ref());
    Ok(out)
}

/// length-prefix (u8) a short utf-8 string into the packed buffer.
fn put_str_u8(out: &mut Vec<u8>, s: &str) -> Result<(), String> {
    let b = s.as_bytes();
    out.push(u8::try_from(b.len()).map_err(|_| format!("string too long ({} bytes): {s:?}", b.len()))?);
    out.extend_from_slice(b);
    Ok(())
}

/// inverse of [`pack_invite_v3`]. verifies FAIL-CLOSED, in order: signature
/// integrity against the embedded inviter key, inviter-∈-validators membership,
/// then expiry. yields a descriptor canonicalized exactly as
/// [`NetworkDescriptor::from_toml`] would (sorted validators, sorted canonical
/// reach) so the genesis fingerprint of a decoded invite matches the founder's.
/// a decoded v3 descriptor carries `reach` (not `bootstrap`); the two are one
/// dial source of truth via [`NetworkDescriptor::reach_hints`].
fn unpack_invite_v3(bytes: &[u8], now_unix: u64) -> Result<NetworkDescriptor, String> {
    let mut r = InviteReader::new(bytes);
    let version = r.u8()?;
    debug_assert_eq!(version, INVITE_VERSION_V3);

    let cid_len = r.u8()? as usize;
    let chain_id = String::from_utf8(r.take(cid_len)?.to_vec()).map_err(|e| format!("chain_id: {e}"))?;

    let vcount = r.u8()? as usize;
    let mut validators = Vec::with_capacity(vcount);
    for _ in 0..vcount {
        validators.push(hex_bytes(r.take(32)?));
    }
    validators.sort();

    let hcount = r.u8()? as usize;
    let mut reach = Vec::with_capacity(hcount);
    for _ in 0..hcount {
        let expected_key = r.take_key()?;
        let reach_val = match r.u8()? {
            0 => Reach::Direct(r.take_str_u8()?),
            1 => Reach::Fronted(r.take_str_u8()?),
            2 => {
                let coord_addr = r.take_str_u8()?;
                let coord_key = r.take_key()?;
                Reach::Coordinated(CoordRef { coord_addr, coord_key })
            }
            other => return Err(format!("unknown reach tag {other} in v3 invite")),
        };
        reach.push(ReachHint { expected_key, reach: reach_val }.to_canonical());
    }
    reach.sort();

    let expires_unix = u64::from_le_bytes(r.take(8)?.try_into().expect("take(8) yields 8 bytes"));
    let inviter_key = r.take_key()?;

    let signed_len = r.pos; // everything up to (not incl.) the signature
    let sig_bytes = r.take(64)?;
    if !r.done() {
        return Err("invite payload has trailing bytes".into());
    }

    // fail closed, in order: signature integrity, then membership binding, then expiry.
    let signature = ed25519::Signature::decode(sig_bytes)
        .map_err(|e| format!("invite signature is malformed: {e}"))?;
    if !inviter_key.verify(INVITE_SIG_NS, &bytes[..signed_len], &signature) {
        return Err("invite signature does not verify".into());
    }
    if !validators.contains(&hex_bytes(inviter_key.as_ref())) {
        return Err("invite inviter is not a genesis validator".into());
    }
    if now_unix >= expires_unix {
        return Err(format!("invite expired (expires {expires_unix}, now {now_unix})"));
    }

    Ok(NetworkDescriptor {
        chain_id,
        scheme: SCHEME_ED25519.into(),
        validators,
        bootstrap: Vec::new(),
        reach,
    })
}

/// inverse of the v2 packer (PARSE-ONLY in production). yields a descriptor
/// canonicalized exactly as [`NetworkDescriptor::from_toml`] would so the
/// genesis fingerprint of a decoded v2 invite matches the founder's. carries
/// `bootstrap` (not `reach`); [`NetworkDescriptor::reach_hints`] synthesises
/// all-`Direct` hints from it.
fn unpack_invite_v2(bytes: &[u8]) -> Result<NetworkDescriptor, String> {
    let mut r = InviteReader::new(bytes);
    let version = r.u8()?;
    debug_assert_eq!(version, INVITE_VERSION_V2);
    let cid_len = r.u8()? as usize;
    let chain_id = String::from_utf8(r.take(cid_len)?.to_vec()).map_err(|e| format!("chain_id: {e}"))?;

    let vcount = r.u8()? as usize;
    let mut validators = Vec::with_capacity(vcount);
    for _ in 0..vcount {
        validators.push(hex_bytes(r.take(32)?));
    }
    validators.sort();

    let bcount = r.u8()? as usize;
    let mut bootstrap = Vec::with_capacity(bcount);
    for _ in 0..bcount {
        let key = hex_bytes(r.take(32)?);
        let hp_len = r.u8()? as usize;
        let host_port =
            std::str::from_utf8(r.take(hp_len)?).map_err(|e| format!("bootstrap addr: {e}"))?;
        bootstrap.push(format!("{key}@{host_port}"));
    }
    if !r.done() {
        return Err("invite payload has trailing bytes".into());
    }
    Ok(NetworkDescriptor {
        chain_id,
        scheme: SCHEME_ED25519.into(),
        validators,
        bootstrap,
        reach: Vec::new(),
    })
}

/// test-only v2 encoder — v2 is PARSE-ONLY in production, but tests must be able
/// to synthesise real v2 blobs to prove parse-compatibility and non-confusability.
#[cfg(test)]
fn encode_invite_v2(d: &NetworkDescriptor) -> Result<String, String> {
    use base64::Engine as _;
    Ok(format!("{INVITE_PREFIX_V2}{}", INVITE_B64.encode(pack_invite_v2(d)?)))
}

#[cfg(test)]
fn pack_invite_v2(d: &NetworkDescriptor) -> Result<Vec<u8>, String> {
    let mut out = vec![INVITE_VERSION_V2];
    let cid = d.chain_id.as_bytes();
    out.push(u8::try_from(cid.len()).map_err(|_| format!("chain_id too long ({} bytes)", cid.len()))?);
    out.extend_from_slice(cid);
    let vkeys = d.validator_keys()?;
    out.push(u8::try_from(vkeys.len()).map_err(|_| format!("too many validators ({})", vkeys.len()))?);
    for k in &vkeys {
        out.extend_from_slice(k.as_ref());
    }
    out.push(u8::try_from(d.bootstrap.len()).map_err(|_| format!("too many bootstrap hints ({})", d.bootstrap.len()))?);
    for entry in &d.bootstrap {
        let (key, host_port) = entry
            .split_once('@')
            .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
        let key = decode_key(key)?;
        out.extend_from_slice(key.as_ref());
        put_str_u8(&mut out, host_port)?;
    }
    Ok(out)
}

/// a bounds-checked forward cursor over the packed invite bytes.
struct InviteReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> InviteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn u8(&mut self) -> Result<u8, String> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| "invite payload truncated".to_string())?;
        self.pos += 1;
        Ok(byte)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| "invite length overflow".to_string())?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| "invite payload truncated".to_string())?;
        self.pos = end;
        Ok(slice)
    }

    fn done(&self) -> bool {
        self.pos == self.bytes.len()
    }

    /// take a u8-length-prefixed utf-8 string (inverse of [`put_str_u8`]).
    fn take_str_u8(&mut self) -> Result<String, String> {
        let len = self.u8()? as usize;
        Ok(std::str::from_utf8(self.take(len)?)
            .map_err(|e| format!("invite string: {e}"))?
            .to_string())
    }

    /// take a raw 32-byte ed25519 public key.
    fn take_key(&mut self) -> Result<ed25519::PublicKey, String> {
        ed25519::PublicKey::decode(self.take(32)?).map_err(|e| format!("invite public key: {e}"))
    }
}

// ============================================================================
// node.toml — the raw file shape (both config shapes live in one struct).
// ============================================================================

#[derive(Default, serde::Deserialize)]
pub struct NodeToml {
    // --- the network shape ---
    /// path to the network descriptor; PRESENT means the network shape.
    pub network: Option<String>,
    /// path to the identity secret; default "identity.key" beside node.toml.
    pub key_file: Option<String>,

    // --- the dev-seed shape (legacy; see module docs) ---
    pub id: Option<u64>,
    pub namespace: Option<String>,
    pub peer_seeds: Option<Vec<u64>>,
    pub validator_seeds: Option<Vec<u64>>,
    pub bootstrapper_addr: Option<String>,

    // --- shared plumbing ---
    pub listen: String,
    pub advertised: Option<String>,
    pub storage_dir: Option<String>,
    pub rpc_listen: Option<String>,
    pub http_listen: Option<String>,
    /// sealed blocks between recovery checkpoints (node-local operator
    /// policy — never part of the network descriptor). default 32.
    pub checkpoint_blocks: Option<u64>,
}

/// read a raw node.toml plus its base directory (which relative paths inside
/// the file resolve against).
pub fn load_node_toml(cfg_path: &Path) -> Result<(NodeToml, PathBuf), String> {
    let text = std::fs::read_to_string(cfg_path).map_err(|e| format!("read {cfg_path:?}: {e}"))?;
    let raw: NodeToml = toml::from_str(&text).map_err(|e| format!("{cfg_path:?}: {e}"))?;
    let base = cfg_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok((raw, base))
}

/// a workspace's plumbing (everything in node.toml that is not the network
/// reference), merged from three layers: explicit flags win, else values an
/// EXISTING node.toml already carries — network- or dev-shape alike, so
/// joining inside the desktop app's solo dir inherits its http port instead
/// of resetting it — else defaults. always writing the merged result makes
/// init/join idempotent AND partial-flag-safe (one flag never resets the
/// others).
pub struct Plumbing {
    pub listen: String,
    pub advertised: Option<String>,
    pub http_listen: Option<String>,
    pub rpc_listen: Option<String>,
    /// merged like the rest — a hand-edited storage_dir survives rewrites.
    pub storage_dir: String,
}

pub fn merged_plumbing(
    dir: &Path,
    listen: Option<&str>,
    advertised: Option<&str>,
    http_listen: Option<&str>,
    rpc_listen: Option<&str>,
) -> Result<Plumbing, String> {
    let path = dir.join("node.toml");
    let existing: Option<NodeToml> = if path.exists() {
        Some(load_node_toml(&path)?.0)
    } else {
        None
    };
    let e = existing.as_ref();
    Ok(Plumbing {
        listen: listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.listen.clone()))
            .unwrap_or_else(|| "127.0.0.1:0".into()),
        advertised: advertised
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.advertised.clone())),
        http_listen: http_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.http_listen.clone())),
        rpc_listen: rpc_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.rpc_listen.clone())),
        storage_dir: e
            .and_then(|r| r.storage_dir.clone())
            .unwrap_or_else(|| "storage".into()),
    })
}

/// write a network-shape node.toml into a workspace dir (init/join). the file
/// references its siblings relatively, so the whole dir is relocatable.
/// replaces a dev-shape file wholesale (its plumbing survives via
/// [`merged_plumbing`]) — a join must actually take effect.
pub fn write_node_toml(dir: &Path, p: &Plumbing) -> Result<PathBuf, String> {
    let mut s = String::from(
        "# ducktape node config (network shape) — see network.toml for the network.\n\
         network = \"network.toml\"\nkey_file = \"identity.key\"\n",
    );
    s += &format!("listen = \"{}\"\n", p.listen);
    if let Some(a) = &p.advertised {
        s += &format!("advertised = \"{a}\"\n");
    }
    s += &format!("storage_dir = '{}'\n", p.storage_dir);
    if let Some(h) = &p.http_listen {
        s += &format!("http_listen = \"{h}\"\n");
    }
    if let Some(r) = &p.rpc_listen {
        s += &format!("rpc_listen = \"{r}\"\n");
    }
    let path = dir.join("node.toml");
    std::fs::write(&path, s).map_err(|e| format!("write {path:?}: {e}"))?;
    Ok(path)
}

/// the statesync source a joiner pulls from. only VALIDATORS serve the
/// statesync channel (a --sync-only process syncs and exits; a non-validator
/// never runs SyncServer), so a bootstrap hint is only a candidate when its
/// key is in the validator set — otherwise a joiner pins a peer that can
/// never answer and retries forever. preference: first validator bootstrap
/// hint, else any validator that is not us. None = nobody can serve (solo).
pub fn choose_sync_source(
    bootstrappers: &[(ed25519::PublicKey, SocketAddr)],
    validators: &[ed25519::PublicKey],
    me: &ed25519::PublicKey,
) -> Option<ed25519::PublicKey> {
    bootstrappers
        .iter()
        .map(|(k, _)| k)
        .find(|k| *k != me && validators.contains(k))
        .or_else(|| validators.iter().find(|k| *k != me))
        .cloned()
}

/// everything `run_node` needs, shape-independent.
#[derive(Debug)]
pub struct Resolved {
    pub signer: ed25519::PrivateKey,
    /// log prefix: "#<id>" for the dev shape, the identity's short hex
    /// otherwise.
    pub label: String,
    /// the chain-id (network shape) or legacy namespace bytes.
    pub namespace: Vec<u8>,
    /// the authorized mesh set (unsorted here; the caller builds the ordered
    /// Set discovery tracks).
    pub mesh: Vec<ed25519::PublicKey>,
    /// the genesis consensus participant subset.
    pub validators: Vec<ed25519::PublicKey>,
    /// (identity, addr) pairs to dial at startup; never contains self.
    pub bootstrappers: Vec<(ed25519::PublicKey, SocketAddr)>,
    pub listen: SocketAddr,
    pub advertised: SocketAddr,
    pub storage_dir: PathBuf,
    pub rpc_listen: Option<String>,
    pub http_listen: Option<String>,
    /// dev-seed shape marker: gates the boot-time demo op + converged print
    /// (scaffolding a REAL network must not write into its genesis).
    pub dev_demo: bool,
    /// sealed blocks between recovery checkpoints.
    pub checkpoint_blocks: u64,
}

/// default recovery checkpoint cadence: small enough that boot replay stays
/// cheap, large enough that snapshotting the in-memory cohort is amortized.
pub const DEFAULT_CHECKPOINT_BLOCKS: u64 = 32;

/// read + resolve a config file into its runnable form. paths inside the file
/// (network, key_file, storage_dir) resolve relative to the file's directory,
/// so a workspace directory is relocatable.
pub fn resolve(cfg_path: &Path) -> Result<Resolved, String> {
    let text = std::fs::read_to_string(cfg_path).map_err(|e| format!("read {cfg_path:?}: {e}"))?;
    let raw: NodeToml = toml::from_str(&text).map_err(|e| format!("{cfg_path:?}: {e}"))?;
    let base = cfg_path.parent().unwrap_or_else(|| Path::new("."));
    if raw.network.is_some() {
        resolve_network_shape(base, raw)
    } else {
        resolve_dev_shape(raw)
    }
}

fn resolve_network_shape(base: &Path, raw: NodeToml) -> Result<Resolved, String> {
    let descriptor_path = base.join(raw.network.as_deref().expect("checked by caller"));
    let descriptor = NetworkDescriptor::load(&descriptor_path)?;
    if descriptor.scheme != SCHEME_ED25519 {
        return Err(format!(
            "network {} uses scheme {:?}; this build runs {SCHEME_ED25519:?}",
            descriptor.chain_id, descriptor.scheme
        ));
    }
    let key_path = base.join(raw.key_file.as_deref().unwrap_or("identity.key"));
    let signer = load_identity(&key_path).map_err(|e| {
        format!("{e} — run `ducktape-node init` or `ducktape-node join <invite>` first")
    })?;
    let me = signer.public_key();

    let validators = descriptor.validator_keys()?;
    if validators.is_empty() {
        return Err(format!("network {} has no validators", descriptor.chain_id));
    }
    // one dial source of truth: reach_entries() falls through to bootstrap
    // synthesis for v2/legacy descriptors, so existing behaviour is preserved
    // and Coordinated/Fronted hints route their dial target correctly.
    let bootstrap = descriptor.reach_entries()?;
    // mesh = validators ∪ bootstrap identities. A fresh network-shape joiner
    // may be outside this set at genesis; it parks until governance admits it.
    let mut mesh = validators.clone();
    for (k, _) in &bootstrap {
        if !mesh.contains(k) {
            mesh.push(k.clone());
        }
    }

    let listen: SocketAddr = raw.listen.parse().map_err(|e| format!("listen: {e}"))?;
    let advertised: SocketAddr = match raw.advertised.as_deref() {
        // resolve (DNS for a hostname) — the mesh wants a concrete socket addr
        // for our self-announced dial address.
        Some(a) => resolve_one(a).map_err(|e| format!("advertised: {e}"))?,
        None => listen,
    };
    let bootstrappers = bootstrap.into_iter().filter(|(k, _)| *k != me).collect();

    Ok(Resolved {
        label: hex_bytes(&me.as_ref()[..4]),
        namespace: descriptor.genesis_namespace().into_bytes(),
        signer,
        mesh,
        validators,
        bootstrappers,
        listen,
        advertised,
        storage_dir: base.join(raw.storage_dir.as_deref().unwrap_or("storage")),
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        dev_demo: false,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
    })
}

/// the dev-seed shape, replicating the historical semantics exactly: node 0
/// bootstraps nobody; everyone else dials peer_seeds[0] at bootstrapper_addr.
fn resolve_dev_shape(raw: NodeToml) -> Result<Resolved, String> {
    let id = raw
        .id
        .ok_or("a dev-shape config needs `id` (or add `network = ...`)")?;
    let namespace = raw
        .namespace
        .ok_or("a dev-shape config needs `namespace`")?;
    let peer_seeds = raw
        .peer_seeds
        .ok_or("a dev-shape config needs `peer_seeds`")?;
    let validator_seeds = raw
        .validator_seeds
        .clone()
        .unwrap_or_else(|| peer_seeds.clone());
    // duplicates would otherwise panic much later at run_node's Set::try_from.
    for (kind, seeds) in [
        ("peer_seeds", &peer_seeds),
        ("validator_seeds", &validator_seeds),
    ] {
        let mut seen = std::collections::BTreeSet::new();
        for s in seeds {
            if !seen.insert(*s) {
                return Err(format!("duplicate seed {s} in {kind}"));
            }
        }
    }

    let key_of = |s: u64| ed25519::PrivateKey::from_seed(s).public_key();
    let mesh: Vec<_> = peer_seeds.iter().map(|s| key_of(*s)).collect();
    let validators: Vec<_> = validator_seeds.iter().map(|s| key_of(*s)).collect();

    let bootstrappers = if id == 0 {
        Vec::new()
    } else {
        let boot_seed = *peer_seeds
            .first()
            .ok_or("a bootstrapping node needs peer_seeds[0] = node 0")?;
        let boot_addr: SocketAddr = raw
            .bootstrapper_addr
            .as_deref()
            .ok_or("a non-zero node needs bootstrapper_addr set")?
            .parse()
            .map_err(|e| format!("bootstrapper_addr: {e}"))?;
        // self-filter matches the Resolved.bootstrappers contract: a config
        // with peer_seeds[0] == id would otherwise dial (and statesync) itself.
        vec![(key_of(boot_seed), boot_addr)]
            .into_iter()
            .filter(|(k, _)| *k != ed25519::PrivateKey::from_seed(id).public_key())
            .collect()
    };

    let listen: SocketAddr = raw.listen.parse().map_err(|e| format!("listen: {e}"))?;
    let advertised: SocketAddr = match raw.advertised.as_deref() {
        // resolve (DNS for a hostname) — the mesh wants a concrete socket addr
        // for our self-announced dial address.
        Some(a) => resolve_one(a).map_err(|e| format!("advertised: {e}"))?,
        None => listen,
    };

    Ok(Resolved {
        signer: ed25519::PrivateKey::from_seed(id),
        label: format!("#{id}"),
        namespace: namespace.into_bytes(),
        mesh,
        validators,
        bootstrappers,
        listen,
        advertised,
        storage_dir: raw
            .storage_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("ducktape-node-{id}"))),
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        dev_demo: true,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
    })
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
    fn identity_roundtrips_and_reuses() {
        let dir = tmp("identity");
        let path = dir.join("identity.key");
        let (a, generated) = load_or_generate_identity(&path).expect("generate");
        assert!(generated);
        let (b, generated) = load_or_generate_identity(&path).expect("reuse");
        assert!(
            !generated,
            "an existing identity is reused, never clobbered"
        );
        assert_eq!(a.public_key(), b.public_key());
    }

    #[test]
    fn v2_invite_blob_roundtrips_the_descriptor() {
        // v2 is parse-only in production; the test-only encoder lets us prove a
        // real v2 blob still round-trips through the (unsigned) v2 decode path.
        let me = ed25519::PrivateKey::from_seed(7).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.add_bootstrap(&me, "127.0.0.1:52200");
        let decoded = decode_invite(&encode_invite_v2(&d).expect("encode")).expect("roundtrip");
        assert_eq!(decoded, d);

        // a HOSTNAME dial hint survives the compact encode/decode verbatim (it is
        // stored as a string and resolved only at dial time).
        let other = ed25519::PrivateKey::from_seed(8).public_key();
        d.add_bootstrap(&other, "node.ducktape.industries:443");
        let decoded = decode_invite(&encode_invite_v2(&d).expect("encode")).expect("roundtrip");
        assert_eq!(decoded, d);
        assert!(
            decoded
                .bootstrap
                .iter()
                .any(|b| b.ends_with("@node.ducktape.industries:443"))
        );
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
    fn network_shape_resolves_membership_and_bootstrap() {
        let dir = tmp("resolve");
        let key_path = dir.join("identity.key");
        let (me, _) = load_or_generate_identity(&key_path).expect("keygen");
        let other = ed25519::PrivateKey::from_seed(9).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "net#11223344".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![
                hex_bytes(me.public_key().as_ref()),
                hex_bytes(other.as_ref()),
            ],
            bootstrap: vec![],
            reach: vec![],
        };
        d.add_bootstrap(&other, "127.0.0.1:52200");
        d.save(&dir.join("network.toml")).expect("save descriptor");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52201\"\n",
        )
        .expect("write node.toml");

        let r = resolve(&dir.join("node.toml")).expect("resolve");
        // the running namespace is the chain-id plus the genesis fingerprint.
        assert_eq!(r.namespace, d.genesis_namespace().into_bytes());
        assert!(String::from_utf8_lossy(&r.namespace).starts_with("net#11223344@"));
        assert_eq!(r.validators.len(), 2);
        assert_eq!(r.mesh.len(), 2);
        // self never appears in bootstrappers; the other member does.
        assert_eq!(r.bootstrappers.len(), 1);
        assert_eq!(r.bootstrappers[0].0, other);
        assert!(!r.dev_demo);
        assert_eq!(r.signer.public_key(), me.public_key());
        assert_eq!(r.storage_dir, dir.join("storage"));
    }

    #[test]
    fn a_non_member_identity_resolves_as_a_pending_joiner() {
        let dir = tmp("nonmember");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let other = ed25519::PrivateKey::from_seed(3).public_key();
        let d = NetworkDescriptor {
            chain_id: "closed#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(other.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52202\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("non-member resolves as a joiner");
        assert_eq!(r.signer.public_key(), me.public_key());
        assert!(!r.validators.contains(&me.public_key()));
        assert_eq!(r.validators, vec![other.clone()]);
        assert_eq!(r.mesh, vec![other]);
    }

    #[test]
    fn unhex_rejects_non_ascii_without_panicking() {
        // fixed-offset slicing panics mid-codepoint unless ascii is enforced —
        // this parses PASTED invite blobs and rpc hex, so Err, never panic.
        assert!(unhex("a\u{2026}").is_err());
        assert!(unhex("caf\u{e9}").is_err());
        assert!(unhex("zz").is_err());
        assert_eq!(unhex("00ff").unwrap(), vec![0x00, 0xff]);
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
    }

    #[cfg(unix)]
    #[test]
    fn identity_file_is_born_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tmp("perms");
        let path = dir.join("identity.key");
        load_or_generate_identity(&path).expect("generate");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "secret must never be world-readable, even transiently"
        );
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
    fn dev_shape_duplicate_seeds_are_a_config_error() {
        let dir = tmp("devdups");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52220\"\nnamespace = \"demo\"\npeer_seeds = [0, 1, 1]\n",
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("dup seeds refused");
        assert!(err.contains("duplicate seed"), "{err}");
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
            ..d
        };
        assert!(bad.bootstrap_entries().is_err());
    }

    #[test]
    fn unhex_rejects_sign_characters() {
        // from_str_radix would tolerate a leading '+' per pair.
        assert!(unhex("+1ab").is_err());
        assert!(unhex("-1ab").is_err());
    }

    #[test]
    fn sync_source_prefers_validator_hints_and_never_self() {
        let me = ed25519::PrivateKey::from_seed(31).public_key();
        let observer = ed25519::PrivateKey::from_seed(32).public_key();
        let validator = ed25519::PrivateKey::from_seed(33).public_key();
        let addr: SocketAddr = "127.0.0.1:52200".parse().unwrap();
        let validators = vec![me.clone(), validator.clone()];

        // a non-validator hint sorts first but can never serve — skipped.
        let hints = vec![(observer.clone(), addr), (validator.clone(), addr)];
        assert_eq!(
            choose_sync_source(&hints, &validators, &me),
            Some(validator.clone())
        );

        // no usable hint: any validator that is not us.
        assert_eq!(
            choose_sync_source(&[], &validators, &me),
            Some(validator.clone())
        );

        // solo network: nobody can serve.
        assert_eq!(choose_sync_source(&[], &[me.clone()], &me), None);
    }

    #[test]
    fn plumbing_merges_flags_over_existing_file_over_defaults() {
        let dir = tmp("plumbing");
        // an existing DEV-shape file (the desktop app's solo config).
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:0\"\nnamespace = \"ducktape-local\"\npeer_seeds = [0]\nhttp_listen = \"127.0.0.1:8844\"\nstorage_dir = '/data/ducktape'\n",
        )
        .expect("write");
        // one flag overrides ONLY its field; the http port AND a hand-edited
        // storage_dir survive.
        let p = merged_plumbing(&dir, Some("127.0.0.1:53000"), None, None, None).expect("merge");
        assert_eq!(p.listen, "127.0.0.1:53000");
        assert_eq!(p.http_listen.as_deref(), Some("127.0.0.1:8844"));
        assert_eq!(p.storage_dir, "/data/ducktape");
        assert!(p.rpc_listen.is_none());
        // and the merged write is network-shape.
        write_node_toml(&dir, &p).expect("write");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.network.as_deref(), Some("network.toml"));
        assert_eq!(raw.http_listen.as_deref(), Some("127.0.0.1:8844"));
        assert_eq!(raw.listen, "127.0.0.1:53000");
        assert_eq!(raw.storage_dir.as_deref(), Some("/data/ducktape"));
    }

    #[test]
    fn dev_shape_never_bootstraps_itself() {
        let dir = tmp("devself");
        std::fs::write(
            dir.join("node.toml"),
            "id = 1\nlisten = \"127.0.0.1:52230\"\nnamespace = \"demo\"\npeer_seeds = [1, 0]\nbootstrapper_addr = \"127.0.0.1:52231\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert!(
            r.bootstrappers.is_empty(),
            "peer_seeds[0] == id must not dial itself"
        );
    }

    #[test]
    fn dev_shape_matches_historical_semantics() {
        let toml = r#"
id = 1
listen = "127.0.0.1:52210"
namespace = "demo"
peer_seeds = [0, 1, 2]
validator_seeds = [0, 1]
bootstrapper_addr = "127.0.0.1:52200"
"#;
        let dir = tmp("dev");
        std::fs::write(dir.join("node.toml"), toml).expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert!(r.dev_demo);
        assert_eq!(r.label, "#1");
        assert_eq!(r.mesh.len(), 3);
        assert_eq!(r.validators.len(), 2);
        assert_eq!(r.bootstrappers.len(), 1);
        assert_eq!(
            r.bootstrappers[0].0,
            ed25519::PrivateKey::from_seed(0).public_key(),
            "non-zero nodes dial peer_seeds[0]"
        );
        assert_eq!(
            r.signer.public_key(),
            ed25519::PrivateKey::from_seed(1).public_key()
        );
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
    fn reach_hints_are_excluded_from_the_genesis_fingerprint() {
        let v = ed25519::PrivateKey::from_seed(31).public_key();
        let coord = ed25519::PrivateKey::from_seed(32).public_key();
        let base = NetworkDescriptor {
            chain_id: "fp#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(v.as_ref())],
            bootstrap: vec![],
            reach: vec![],
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

    // ---- Task 4: v3 pack + signature (encoder side) ----

    #[test]
    fn pack_invite_v3_layout_and_signature_are_exact() {
        let inviter = ed25519::PrivateKey::from_seed(41);
        let ipk = inviter.public_key();
        let member = ed25519::PrivateKey::from_seed(42).public_key();
        let coord = ed25519::PrivateKey::from_seed(43).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "pk#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(ipk.as_ref()), hex_bytes(member.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.validators.sort();
        d.add_reach(&ReachHint { expected_key: member.clone(), reach: Reach::Direct("10.0.0.2:9000".into()) });
        d.add_reach(&ReachHint {
            expected_key: ipk.clone(),
            reach: Reach::Coordinated(CoordRef { coord_addr: "p2p:7777".into(), coord_key: coord }),
        });

        let bytes = pack_invite_v3(&d, &inviter, 5_000).expect("pack");
        // header
        assert_eq!(bytes[0], INVITE_VERSION_V3);
        let cid = d.chain_id.as_bytes();
        assert_eq!(bytes[1] as usize, cid.len());
        assert_eq!(&bytes[2..2 + cid.len()], cid);
        // last 64 bytes are the signature over everything before them, domain-separated.
        let split = bytes.len() - 64;
        let sig = ed25519::Signature::decode(&bytes[split..]).expect("sig decodes");
        assert!(ipk.verify(INVITE_SIG_NS, &bytes[..split], &sig), "signature verifies over payload-wo-sig");
        // and the wrong domain must NOT verify (domain separation is real).
        assert!(!ipk.verify(b"ducktape:invite:v2:", &bytes[..split], &sig));

        // the textual blob carries the v3 prefix.
        let blob = encode_invite(&d, &inviter, 5_000).expect("encode");
        assert!(blob.starts_with(INVITE_PREFIX_V3));
    }

    // ---- Task 5: v3 unpack + verify (fail-closed) ----

    fn v3_fixture(_expires: u64) -> (ed25519::PrivateKey, NetworkDescriptor) {
        let inviter = ed25519::PrivateKey::from_seed(51);
        let ipk = inviter.public_key();
        let member = ed25519::PrivateKey::from_seed(52).public_key();
        let coord = ed25519::PrivateKey::from_seed(53).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "v3#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(ipk.as_ref()), hex_bytes(member.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.validators.sort();
        d.add_reach(&ReachHint { expected_key: member.clone(), reach: Reach::Direct("10.0.0.2:9000".into()) });
        d.add_reach(&ReachHint { expected_key: member, reach: Reach::Fronted("front:443".into()) }); // replaces
        d.add_reach(&ReachHint {
            expected_key: ipk,
            reach: Reach::Coordinated(CoordRef { coord_addr: "p2p:7777".into(), coord_key: coord }),
        });
        (inviter, d)
    }

    #[test]
    fn v3_roundtrips_all_reach_kinds_and_verifies() {
        let (inviter, d) = v3_fixture(5_000);
        let blob = encode_invite(&d, &inviter, 5_000).expect("encode");
        let got = decode_invite_at(&blob, 4_000).expect("decode within ttl");
        assert_eq!(got.chain_id, d.chain_id);
        assert_eq!(got.validators, d.validators);
        assert_eq!(got.reach, d.reach); // canonical reach round-trips exactly
        assert!(got.bootstrap.is_empty()); // v3 carries reach, not bootstrap
        // and the decoded descriptor fingerprints identically to the founder's.
        assert_eq!(got.genesis_namespace(), d.genesis_namespace());
    }

    #[test]
    fn v3_rejects_a_tampered_expected_key() {
        use base64::Engine as _;
        let (inviter, d) = v3_fixture(5_000);
        let mut bytes = pack_invite_v3(&d, &inviter, 5_000).unwrap();
        // flip one byte inside the first reach hint's expected_key region.
        let cid = d.chain_id.len();
        let flip = 1 + 1 + cid + 1 + 32 * d.validators.len() + 1 + 1; // into expected_key
        bytes[flip] ^= 0x01;
        let blob = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(bytes));
        assert!(decode_invite_at(&blob, 4_000).is_err(), "tamper must break the signature");
    }

    #[test]
    fn v3_rejects_expired() {
        let (inviter, d) = v3_fixture(5_000);
        let blob = encode_invite(&d, &inviter, 5_000).unwrap();
        assert!(decode_invite_at(&blob, 5_000).is_err(), "now == expires is expired");
        assert!(decode_invite_at(&blob, 6_000).is_err());
    }

    #[test]
    fn v3_rejects_inviter_not_in_validators() {
        let outsider = ed25519::PrivateKey::from_seed(99); // not in the validator set
        let (_inviter, d) = v3_fixture(5_000);
        let blob = encode_invite(&d, &outsider, 5_000).unwrap();
        assert!(decode_invite_at(&blob, 4_000).is_err(), "inviter must be a genesis validator");
    }

    #[test]
    fn v3_rejects_trailing_and_truncated() {
        use base64::Engine as _;
        let (inviter, d) = v3_fixture(5_000);
        let good = pack_invite_v3(&d, &inviter, 5_000).unwrap();
        let mut trailing = good.clone();
        trailing.push(0);
        let blob = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(trailing));
        assert!(decode_invite_at(&blob, 4_000).is_err(), "trailing bytes rejected");
        let blob = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(&good[..good.len() - 1]));
        assert!(decode_invite_at(&blob, 4_000).is_err(), "truncation rejected");
    }

    // ---- Task 6: v2 parse-only + non-confusability ----

    #[test]
    fn v2_blob_decodes_to_all_direct_unsigned_hints() {
        let a = ed25519::PrivateKey::from_seed(61).public_key();
        let b = ed25519::PrivateKey::from_seed(62).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "v2#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref()), hex_bytes(b.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.validators.sort();
        d.add_bootstrap(&a, "127.0.0.1:52200");
        d.add_bootstrap(&b, "node.example.com:443");

        let blob = encode_invite_v2(&d).expect("v2 encode (test-only)");
        assert!(blob.starts_with(INVITE_PREFIX_V2));
        let got = decode_invite_at(&blob, 4_000).expect("v2 decodes, no signature/expiry");
        assert_eq!(got.bootstrap, d.bootstrap);
        assert!(got.reach.is_empty(), "v2 stores no explicit reach");
        // the TYPED view is all-Direct.
        let hints = got.reach_hints().unwrap();
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().all(|h| matches!(h.reach, Reach::Direct(_))));
    }

    #[test]
    fn v2_and_v3_are_never_confusable() {
        use base64::Engine as _;
        let inviter = ed25519::PrivateKey::from_seed(63);
        let ipk = inviter.public_key();
        let mut d = NetworkDescriptor {
            chain_id: "x#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(ipk.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.validators.sort();

        let v3 = pack_invite_v3(&d, &inviter, 5_000).unwrap();
        let v2 = pack_invite_v2(&d).unwrap();
        // a v3 payload under a v2 prefix (and vice-versa) is rejected on the agreement check.
        let mislabelled_v3 = format!("{INVITE_PREFIX_V2}{}", INVITE_B64.encode(&v3));
        let mislabelled_v2 = format!("{INVITE_PREFIX_V3}{}", INVITE_B64.encode(&v2));
        assert!(decode_invite_at(&mislabelled_v3, 4_000).is_err());
        assert!(decode_invite_at(&mislabelled_v2, 4_000).is_err());
        // an unknown version tag is rejected.
        let mut bogus = v2.clone();
        bogus[0] = 9;
        let blob = format!("{INVITE_PREFIX_V2}{}", INVITE_B64.encode(bogus));
        assert!(decode_invite_at(&blob, 4_000).is_err());
        // a garbage prefix is rejected.
        assert!(decode_invite_at("ducktape-invite-v1:AAAA", 4_000).is_err());
    }

    #[test]
    fn invite_expiry_adds_ttl_days_and_saturates_cleanly() {
        assert_eq!(invite_expiry(1_000, 7).unwrap(), 1_000 + 7 * 86_400);
        assert_eq!(invite_expiry(0, 1).unwrap(), 86_400);
        assert!(invite_expiry(0, u64::MAX).is_err(), "absurd ttl errors, never overflows");
        assert!(invite_expiry(u64::MAX, 1).is_err(), "expiry overflow errors");
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
        };
        // dial the coordinator's socket, but the identity we expect is the TARGET.
        d.add_reach(&ReachHint {
            expected_key: target.clone(),
            reach: Reach::Coordinated(CoordRef { coord_addr: "127.0.0.1:59999".into(), coord_key: coord }),
        });
        let entries = d.reach_entries().expect("resolve");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, target); // expect target key
        assert_eq!(entries[0].1, "127.0.0.1:59999".parse().unwrap()); // dial coordinator
    }

    #[test]
    fn reach_entries_falls_back_to_bootstrap_for_v2_and_skips_unresolvable() {
        let a = ed25519::PrivateKey::from_seed(73).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "co#00000000".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.add_bootstrap(&a, "127.0.0.1:52200"); // resolvable
        d.add_reach(&ReachHint { expected_key: a.clone(), reach: Reach::Direct("127.0.0.1:52200".into()) });
        // reach present -> parsed from reach; the direct entry resolves.
        let entries = d.reach_entries().unwrap();
        assert_eq!(entries, vec![(a, "127.0.0.1:52200".parse().unwrap())]);
    }
}
