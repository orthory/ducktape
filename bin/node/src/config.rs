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

/// the consensus scheme tag a descriptor must carry — a genesis-wide constant
/// (see `ConsensusScheme`); anything else is a build from the future.
pub const SCHEME_ED25519: &str = "ed25519";

/// the invite blob prefix. UNVERSIONED on purpose (bootstrapping posture): the
/// network re-mints invites on a format change, and a stale paste fails loudly
/// at decode — the old `ducktape-invite-v*:` prefixes no longer decode at all.
const INVITE_PREFIX: &str = "ducktape:";

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
    if !s.len().is_multiple_of(2) {
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
/// unix — this is the NODE's identity (mesh/valset/frame-signing key) only.
/// the user's identity is a separate keypair held by the app
/// (`~/.ducktape/user.key`) and bound to this node's key through the
/// `identity` module (`crates/system/identity`); this file never holds it.
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

/// mint a bind certificate: the USER key's signature over
/// [`identity::bind_preimage`] in the [`identity::IDENTITY_BIND_NS`] domain --
/// the consent artifact `IdentityMsg::BindNode` carries as `user_sig`. chain-
/// and nonce-scoped, so a certificate can never replay across networks or
/// after an unbind bumps the nonce.
pub fn mint_bind_cert(
    user: &ed25519::PrivateKey,
    chain_id: &str,
    node_pub: &[u8],
    nonce: u64,
) -> Vec<u8> {
    user.sign(
        identity::IDENTITY_BIND_NS,
        &identity::bind_preimage(chain_id, node_pub, nonce),
    )
    .as_ref()
    .to_vec()
}

/// mint an unbind certificate (same shape as [`mint_bind_cert`], but signed
/// over [`identity::unbind_preimage`] in the [`identity::IDENTITY_UNBIND_NS`]
/// domain -- the consent artifact `IdentityMsg::UnbindNode` carries).
pub fn mint_unbind_cert(
    user: &ed25519::PrivateKey,
    chain_id: &str,
    node_pub: &[u8],
    nonce: u64,
) -> Vec<u8> {
    user.sign(
        identity::IDENTITY_UNBIND_NS,
        &identity::unbind_preimage(chain_id, node_pub, nonce),
    )
    .as_ref()
    .to_vec()
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
            if matches!(hint.reach, Reach::Direct(_) | Reach::Fronted(_)) {
                typed_keys.insert(hint.expected_key.as_ref().to_vec());
            }
            typed.push(hint);
        }
        bootstrap_by_key.retain(|k, _| !typed_keys.contains(k));
        let mut out: Vec<_> = bootstrap_by_key.into_values().chain(typed).collect();
        out.sort_by_key(|h| h.to_canonical());
        Ok(out)
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

/// Joining through coordinated reach needs the local reachability plane even
/// when the invite does not contain a direct inviter-hosted tunnel bootstrap.
pub fn invite_requires_reachability_defaults(invite: &Invite) -> bool {
    invite.wireguard.is_some() || invite.descriptor.has_coordinated_reach().unwrap_or(false)
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
// invite tokens — the capability an invite blob carries. minted by a member
// (`invite`), presented by the joiner over the lobby channel, redeemed
// in-consensus by governance's `Redeem` op: MINTING IS THE ADMISSION
// DECISION, redemption is mechanical and single-use. the canonical types and
// verification live in `governance::invite` (the same code every validator's
// in-consensus check runs); this module re-exports them and owns the
// node-side pieces — minting (OS randomness) and the on-disk token file.
// ============================================================================

pub use governance::invite::{
    INVITE_GRANT_NAMESPACE, INVITE_NONCE_LEN, InviteToken, sign_join_proof, verify_invite_token,
    verify_join_proof,
};

/// mint a token binding an invite to `binding` (the genesis namespace): fresh
/// OS randomness for the nonce, signed by this member's identity.
pub fn mint_invite_token(signer: &ed25519::PrivateKey, binding: &[u8]) -> InviteToken {
    let mut nonce = [0u8; INVITE_NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let msg = [binding, &nonce].concat();
    InviteToken {
        issuer: signer.public_key(),
        nonce,
        sig: signer.sign(INVITE_GRANT_NAMESPACE, &msg),
    }
}

const INVITE_TOKEN_FILE: &str = "invite.token";
const INVITE_TOKEN_LEN: usize = 32 + INVITE_NONCE_LEN + 64;

fn pack_invite_token(t: &InviteToken) -> Vec<u8> {
    let mut out = Vec::with_capacity(INVITE_TOKEN_LEN);
    out.extend_from_slice(t.issuer.as_ref());
    out.extend_from_slice(&t.nonce);
    out.extend_from_slice(t.sig.encode().as_ref());
    out
}

fn unpack_invite_token(bytes: &[u8]) -> Result<InviteToken, String> {
    if bytes.len() != INVITE_TOKEN_LEN {
        return Err(format!(
            "invite token must be {INVITE_TOKEN_LEN} bytes, got {}",
            bytes.len()
        ));
    }
    let issuer = ed25519::PublicKey::decode(&bytes[..32])
        .map_err(|e| format!("invite token issuer: {e}"))?;
    let mut nonce = [0u8; INVITE_NONCE_LEN];
    nonce.copy_from_slice(&bytes[32..32 + INVITE_NONCE_LEN]);
    let sig = ed25519::Signature::decode(&bytes[32 + INVITE_NONCE_LEN..])
        .map_err(|e| format!("invite token signature: {e}"))?;
    Ok(InviteToken { issuer, nonce, sig })
}

/// persist the token a `join` received beside the descriptor (0600 like the
/// identity: it is a bearer credential until used). overwrites — a re-join
/// with a fresh invite replaces a stale/spent token.
pub fn save_invite_token(dir: &Path, token: &InviteToken) -> Result<(), String> {
    use std::io::Write as _;
    let path = dir.join(INVITE_TOKEN_FILE);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).map_err(|e| format!("create {path:?}: {e}"))?;
    f.write_all(format!("{}\n", hex_bytes(&pack_invite_token(token))).as_bytes())
        .map_err(|e| format!("write {path:?}: {e}"))
}

/// the token a previous `join` stored, if any — a missing file is the normal
/// state for founders, dev-shape nodes, and manual (token-less) joins.
pub fn load_invite_token(dir: &Path) -> Result<Option<InviteToken>, String> {
    let path = dir.join(INVITE_TOKEN_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
    let raw = unhex(text.trim()).map_err(|e| format!("{path:?}: {e}"))?;
    unpack_invite_token(&raw).map(Some)
}

const INVITE_WIREGUARD_FILE: &str = "invite-wireguard.toml";

/// the inviter's WireGuard bootstrap a `join` stored beside the token — what
/// the joining node dials BEFORE any p2p. `issuer` names the inviter (its
/// overlay ULA derives from it), the rest mirrors [`InviteWireGuard`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredInviteWireGuard {
    /// the inviter's ed25519 identity, hex.
    pub issuer: String,
    /// the inviter's X25519 WireGuard public key, hex.
    pub public_key: String,
    /// the inviter's underlay WireGuard UDP endpoint, `host:port`. Absent for
    /// coordinated invites, where the endpoint is resolved through the
    /// rendezvous coordinator at run time.
    pub endpoint: Option<String>,
    /// the inviter's underlay UDP intro endpoint, `host:port`. Absent for
    /// coordinated invites; the intro rides the shared WireGuard underlay
    /// socket after rendezvous.
    pub intro: Option<String>,
    /// the inviter's control-mesh listen port on the overlay.
    pub mesh_port: u16,
}

impl StoredInviteWireGuard {
    /// the inviter's X25519 key, decoded.
    pub fn public_key_bytes(&self) -> Result<[u8; 32], String> {
        let raw = unhex(&self.public_key)?;
        raw.try_into()
            .map_err(|_| "invite wireguard public_key must be 32 bytes".to_string())
    }

    /// the inviter's ed25519 identity, decoded.
    pub fn issuer_key(&self) -> Result<ed25519::PublicKey, String> {
        decode_key(&self.issuer)
    }
}

/// persist the invite's WireGuard bootstrap beside the token. overwrites —
/// a re-join with a fresh invite replaces a stale one.
pub fn save_invite_wireguard(
    dir: &Path,
    issuer: &ed25519::PublicKey,
    wg: &InviteWireGuard,
) -> Result<(), String> {
    let stored = StoredInviteWireGuard {
        issuer: hex_bytes(issuer.as_ref()),
        public_key: hex_bytes(&wg.public_key),
        endpoint: wg.endpoint.clone(),
        intro: wg.intro.clone(),
        mesh_port: wg.mesh_port,
    };
    let path = dir.join(INVITE_WIREGUARD_FILE);
    let text = toml::to_string_pretty(&stored).map_err(|e| format!("encode {path:?}: {e}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write {path:?}: {e}"))
}

/// the WireGuard bootstrap a previous `join` stored, if any.
pub fn load_invite_wireguard(dir: &Path) -> Result<Option<StoredInviteWireGuard>, String> {
    let path = dir.join(INVITE_WIREGUARD_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
    toml::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{path:?}: {e}"))
}

const INVITE_FRONTS_FILE: &str = "invite-fronts.json";

/// the on-disk shape of a persisted [`Front`] — raw key arrays as hex so the
/// file is human-readable and stable, mirroring [`StoredInviteWireGuard`].
#[derive(Serialize, Deserialize)]
struct StoredFront {
    member_key: String,
    wireguard_public_key: String,
    mesh_port: u16,
    endpoint: Option<String>,
}

/// persist the invite's fronts beside the token so a later `run` can race the
/// whole union of first-contact paths, not just the inviter. Empty fronts write
/// nothing (absence decodes as empty). Overwrites — a re-join replaces them.
pub fn save_invite_fronts(dir: &Path, fronts: &[Front]) -> Result<(), String> {
    let path = dir.join(INVITE_FRONTS_FILE);
    if fronts.is_empty() {
        // a re-join with a front-less invite must not leave a stale set behind.
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let stored: Vec<StoredFront> = fronts
        .iter()
        .map(|f| StoredFront {
            member_key: hex_bytes(&f.member_key),
            wireguard_public_key: hex_bytes(&f.wireguard_public_key),
            mesh_port: f.mesh_port,
            endpoint: f.endpoint.clone(),
        })
        .collect();
    let text = serde_json::to_string_pretty(&stored).map_err(|e| format!("encode {path:?}: {e}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write {path:?}: {e}"))
}

/// the fronts a previous `join` stored; empty when absent. Fail-closed decode.
pub fn load_invite_fronts(dir: &Path) -> Result<Vec<Front>, String> {
    let path = dir.join(INVITE_FRONTS_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {path:?}: {e}")),
    };
    let stored: Vec<StoredFront> =
        serde_json::from_str(&text).map_err(|e| format!("decode {path:?}: {e}"))?;
    stored
        .into_iter()
        .map(|s| {
            let member_key = unhex(&s.member_key)?
                .try_into()
                .map_err(|_| "front member_key must be 32 bytes".to_string())?;
            let wireguard_public_key = unhex(&s.wireguard_public_key)?
                .try_into()
                .map_err(|_| "front wireguard_public_key must be 32 bytes".to_string())?;
            Ok(Front {
                member_key,
                wireguard_public_key,
                mesh_port: s.mesh_port,
                endpoint: s.endpoint,
            })
        })
        .collect()
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

// ============================================================================
// the invite blob — the whole join credential packed into one signed line.
//
// ONE format, unversioned (bootstrapping posture — a format change re-mints
// invites; the older `ducktape-invite-v*:` generations no longer decode). the
// blob is a CAPABILITY, not a doorbell: it carries the descriptor (chain-id +
// genesis validators + typed reach hints), the inviter's WireGuard bootstrap
// (when the inviter runs the reachability plane), an expiry, and the invite
// token whose mint IS the admission decision. the whole envelope is signed by
// the token's issuer in a dedicated namespace, and decode FAILS CLOSED:
// envelope signature, then token-against-computed-binding (which transitively
// pins chain-id + validators — tampering either changes the genesis
// fingerprint and kills the token), then expiry.
// ============================================================================

/// ed25519 signing namespace for the invite envelope: the issuer signs every
/// packed byte that precedes the signature.
pub const INVITE_ENVELOPE_NAMESPACE: &[u8] = b"ducktape-invite-envelope";
/// how long a minted invite stays redeemable unless `--ttl-days` says
/// otherwise. single-use bounds the damage of a leaked blob; expiry bounds a
/// LOST one.
pub const DEFAULT_INVITE_TTL_DAYS: u64 = 7;

const INVITE_B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// the inviter's WireGuard bootstrap — everything a joiner needs to bring its
/// tunnel to the inviter up BEFORE any p2p: the inviter's X25519 public key,
/// its underlay UDP WireGuard endpoint, the UDP intro endpoint the joiner
/// announces its own keys to (token-authenticated first contact), and the
/// inviter's control-mesh listen port on the overlay (the joiner dials the
/// inviter's derived ULA at this port once the tunnel routes).
#[derive(Clone, Debug, PartialEq)]
pub struct InviteWireGuard {
    /// the inviter's X25519 WireGuard public key, raw.
    pub public_key: [u8; 32],
    /// the inviter's underlay WireGuard UDP endpoint, `host:port`. `None`
    /// means the invite uses coordinated rendezvous instead of baking in a
    /// direct endpoint.
    pub endpoint: Option<String>,
    /// the inviter's underlay UDP intro endpoint, `host:port`. `None` means
    /// the intro is sent over the coordinated WireGuard underlay socket.
    pub intro: Option<String>,
    /// the inviter's control-mesh listen port, dialed at its overlay ULA.
    pub mesh_port: u16,
}

/// one member the inviter offers as an ADDITIONAL first-contact path: the
/// joiner may bring its tunnel up against this member instead of the inviter
/// (the unified all-paths invite — `docs/superpowers/specs/2026-07-08-fully-nated-inviter-design.md`).
/// Only PUBLIC keys ever ride the wire; the WireGuard private key never leaves
/// the node. `endpoint` is the member's routable WireGuard UNDERLAY endpoint
/// (`host:wg_port`) when it is host-capable — the joiner dials it directly and
/// announces its intro at `wg_port + 1` (the product-wide `invite_listen`
/// default). `None` means the member is only reachable BY IDENTITY through the
/// joiner's ambient coordinator (a punchable, NAT'd member).
///
/// Fronts live OUTSIDE the genesis fingerprint (they are advisory reachability,
/// never validator identity) — see the fingerprint-exclusion test.
#[derive(Clone, Debug, PartialEq)]
pub struct Front {
    /// the member's real ed25519 node identity the joiner authenticates.
    pub member_key: [u8; 32],
    /// the member's X25519 WireGuard public key, raw.
    pub wireguard_public_key: [u8; 32],
    /// the member's control-mesh listen port, dialed at its overlay ULA once
    /// the tunnel routes.
    pub mesh_port: u16,
    /// the member's routable WireGuard underlay endpoint `host:wg_port`, or
    /// `None` for a punchable member reached by identity via the coordinator.
    pub endpoint: Option<String>,
}

/// Map a persisted mesh's signed adverts to invite [`Front`]s, skipping the
/// inviter's own advert (`own`, its raw ed25519 identity — it already rides
/// the invite as the `wireguard` bootstrap). A member with a concrete routable
/// WireGuard underlay endpoint becomes a DIRECT front (`endpoint:
/// Some(host:wg_port)` — the joiner dials it and announces its intro at
/// `wg_port + 1`); a member with no dialable underlay becomes a COORDINATED
/// front (`endpoint: None`, reached BY IDENTITY through the joiner's ambient
/// coordinator). Every registered member is at least punchable, so all
/// non-self adverts are offered as fronts.
pub fn fronts_from_adverts(
    adverts: &[wireguard_upgrade::EndpointAdvertisement],
    own: &[u8; 32],
) -> Vec<Front> {
    adverts
        .iter()
        .map(|advert| &advert.record)
        .filter(|record| &record.validator_identity.0 != own)
        .map(|record| Front {
            member_key: record.validator_identity.0,
            wireguard_public_key: record.wireguard_public_key.0,
            mesh_port: record.control_endpoint.port,
            endpoint: record
                .wireguard_endpoint
                .as_ref()
                .map(|ep| ep.socket_addr().to_string()),
        })
        .collect()
}

/// a decoded, VERIFIED invite — the only constructor is [`decode_invite`].
#[derive(Clone, Debug, PartialEq)]
pub struct Invite {
    pub descriptor: NetworkDescriptor,
    pub token: InviteToken,
    /// `None` when the inviter runs no reachability plane (a TCP-reachable
    /// network) — the joiner then rides the descriptor's reach hints alone.
    pub wireguard: Option<InviteWireGuard>,
    /// additional first-contact paths the inviter offers (its reachable
    /// members). Empty on a pre-feature blob or when the inviter has no
    /// persisted mesh state. Never part of `genesis_namespace`.
    pub fronts: Vec<Front>,
    pub expires_unix_secs: u64,
}

/// encode an invite blob, signing the envelope as the token's issuer (the
/// caller must pass the same identity that minted `token`).
pub fn encode_invite(
    descriptor: &NetworkDescriptor,
    token: &InviteToken,
    wireguard: Option<&InviteWireGuard>,
    fronts: &[Front],
    expires_unix_secs: u64,
    signer: &ed25519::PrivateKey,
) -> Result<String, String> {
    use base64::Engine as _;
    if signer.public_key() != token.issuer {
        return Err("invite envelope must be signed by the token's issuer".into());
    }
    let mut out = pack_invite(descriptor, token, wireguard, fronts, expires_unix_secs)?;
    let sig = signer.sign(INVITE_ENVELOPE_NAMESPACE, &out);
    out.extend_from_slice(sig.encode().as_ref());
    Ok(format!("{INVITE_PREFIX}{}", INVITE_B64.encode(out)))
}

/// decode an invite blob against the real clock. fail-closed: see
/// [`decode_invite_at`].
pub fn decode_invite(blob: &str) -> Result<Invite, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock is past the epoch")
        .as_secs();
    decode_invite_at(blob, now)
}

/// decode an invite blob at an injected clock (deterministic expiry tests).
/// fail-closed order: ① envelope signature by the embedded token issuer over
/// every preceding byte, ② token signature against the binding COMPUTED from
/// the decoded descriptor (pins chain-id + validators transitively), ③ expiry.
pub fn decode_invite_at(blob: &str, now_unix_secs: u64) -> Result<Invite, String> {
    use base64::Engine as _;
    use commonware_cryptography::Verifier as _;
    let body = blob
        .trim()
        .strip_prefix(INVITE_PREFIX)
        .ok_or_else(|| {
            format!(
                "not a ducktape invite (expected {INVITE_PREFIX}...); an older \
                 ducktape-invite-v*: blob no longer decodes — ask for a fresh invite"
            )
        })?;
    let bytes = INVITE_B64
        .decode(body)
        .map_err(|e| format!("invite is not valid base64url: {e}"))?;
    // the trailing 64 bytes are the envelope signature over everything before.
    let Some(signed_len) = bytes.len().checked_sub(64) else {
        return Err("invite payload truncated".into());
    };
    let (signed, sig_bytes) = bytes.split_at(signed_len);
    let sig = ed25519::Signature::decode(sig_bytes).map_err(|e| format!("envelope signature: {e}"))?;
    let invite = unpack_invite(signed, now_unix_secs)?;
    if !invite
        .token
        .issuer
        .verify(INVITE_ENVELOPE_NAMESPACE, signed, &sig)
    {
        return Err("invite envelope signature does not verify".into());
    }
    let binding = invite.descriptor.genesis_namespace();
    if !verify_invite_token(&invite.token, binding.as_bytes()) {
        return Err(
            "invite token does not verify against this blob's own network — the blob was \
             tampered with"
                .into(),
        );
    }
    Ok(invite)
}

/// pack the signed portion of the invite. validator hex is decoded to raw keys
/// (rejecting a malformed descriptor here rather than shipping it); the typed
/// reach hints come from [`NetworkDescriptor::reach_hints`] (the union of
/// `reach` and `bootstrap`-synthesised Direct hints), so a founder that only
/// ever ran `add_bootstrap` still ships a well-formed invite.
fn pack_invite(
    d: &NetworkDescriptor,
    token: &InviteToken,
    wireguard: Option<&InviteWireGuard>,
    fronts: &[Front],
    expires_unix_secs: u64,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();

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

    match wireguard {
        Some(wg) if wg.endpoint.is_some() && wg.intro.is_some() => {
            out.push(1);
            out.extend_from_slice(&wg.public_key);
            put_str_u8(&mut out, wg.endpoint.as_ref().expect("checked"))?;
            put_str_u8(&mut out, wg.intro.as_ref().expect("checked"))?;
            out.extend_from_slice(&wg.mesh_port.to_le_bytes());
        }
        Some(wg) if wg.endpoint.is_none() && wg.intro.is_none() => {
            out.push(2);
            out.extend_from_slice(&wg.public_key);
            out.extend_from_slice(&wg.mesh_port.to_le_bytes());
        }
        Some(_) => return Err("wireguard invite must carry both endpoint and intro, or neither".into()),
        None => out.push(0),
    }

    // the coordination-mode echo: one byte inside the SIGNED envelope,
    // sourced from the descriptor (None resolves to Private, the safe
    // default) — a fresh joiner learns off the invite alone whether the
    // coordinator is private, so it knows to expect (and present) a CoordCap.
    out.push(match d.coordination() {
        Coordination::Public => 0,
        Coordination::Private => 1,
    });

    out.extend_from_slice(&expires_unix_secs.to_le_bytes());
    out.extend_from_slice(&pack_invite_token(token));

    // the fronts block rides AFTER the fixed-length token, inside the signed
    // envelope, but is NEVER fed to `genesis_namespace` (advisory reachability,
    // not validator identity). Absent when empty, so an invite with no fronts
    // stays BYTE-IDENTICAL to a pre-feature blob and old blobs still decode
    // (the reader treats "nothing after the token" as `fronts: []`).
    if !fronts.is_empty() {
        out.push(u8::try_from(fronts.len()).map_err(|_| format!("too many fronts ({})", fronts.len()))?);
        for f in fronts {
            out.extend_from_slice(&f.member_key);
            out.extend_from_slice(&f.wireguard_public_key);
            out.extend_from_slice(&f.mesh_port.to_le_bytes());
            match &f.endpoint {
                Some(endpoint) => {
                    out.push(1);
                    put_str_u8(&mut out, endpoint)?;
                }
                None => out.push(0),
            }
        }
    }
    Ok(out)
}

/// length-prefix (u8) a short utf-8 string into the packed buffer.
fn put_str_u8(out: &mut Vec<u8>, s: &str) -> Result<(), String> {
    let b = s.as_bytes();
    out.push(u8::try_from(b.len()).map_err(|_| format!("string too long ({} bytes): {s:?}", b.len()))?);
    out.extend_from_slice(b);
    Ok(())
}

/// inverse of [`pack_invite`] (the signed portion — the caller has already
/// split the envelope signature off); yields a descriptor canonicalized
/// exactly as [`NetworkDescriptor::from_toml`] would (sorted validators,
/// sorted canonical reach) so the genesis fingerprint of a decoded invite
/// matches the founder's. signature verification is the CALLER's
/// ([`decode_invite_at`]) — this only parses and enforces expiry.
fn unpack_invite(bytes: &[u8], now_unix_secs: u64) -> Result<Invite, String> {
    let mut r = InviteReader::new(bytes);
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
            other => return Err(format!("unknown reach tag {other} in invite")),
        };
        reach.push(ReachHint { expected_key, reach: reach_val }.to_canonical());
    }
    reach.sort();

    let wireguard = match r.u8()? {
        0 => None,
        1 => {
            let mut public_key = [0u8; 32];
            public_key.copy_from_slice(r.take(32)?);
            let endpoint = r.take_str_u8()?;
            let intro = r.take_str_u8()?;
            let mesh_port = u16::from_le_bytes(r.take(2)?.try_into().expect("2 bytes"));
            Some(InviteWireGuard {
                public_key,
                endpoint: Some(endpoint),
                intro: Some(intro),
                mesh_port,
            })
        }
        2 => {
            let mut public_key = [0u8; 32];
            public_key.copy_from_slice(r.take(32)?);
            let mesh_port = u16::from_le_bytes(r.take(2)?.try_into().expect("2 bytes"));
            Some(InviteWireGuard {
                public_key,
                endpoint: None,
                intro: None,
                mesh_port,
            })
        }
        other => return Err(format!("unknown wireguard flag {other} in invite")),
    };

    // the coordination-mode echo — decoded back into the descriptor so the
    // joiner's workspace records whether the coordinator expects a CoordCap.
    let coordination = match r.u8()? {
        0 => Some("public".to_string()),
        1 => Some("private".to_string()),
        other => return Err(format!("unknown coordination mode {other} in invite")),
    };

    let expires_unix_secs = u64::from_le_bytes(r.take(8)?.try_into().expect("8 bytes"));
    if now_unix_secs >= expires_unix_secs {
        return Err("this invite has expired — ask for a fresh one".into());
    }
    let token = unpack_invite_token(r.take(INVITE_TOKEN_LEN)?)?;

    // the optional fronts block. A pre-feature blob has nothing after the
    // token (`r.done()`), decoding to an empty set; a feature blob carries a
    // u8 count then each front. Fail-closed on a malformed entry.
    let fronts = if r.done() {
        Vec::new()
    } else {
        let fcount = r.u8()? as usize;
        let mut fronts = Vec::with_capacity(fcount);
        for _ in 0..fcount {
            let mut member_key = [0u8; 32];
            member_key.copy_from_slice(r.take(32)?);
            let mut wireguard_public_key = [0u8; 32];
            wireguard_public_key.copy_from_slice(r.take(32)?);
            let mesh_port = u16::from_le_bytes(r.take(2)?.try_into().expect("2 bytes"));
            let endpoint = match r.u8()? {
                0 => None,
                1 => Some(r.take_str_u8()?),
                other => return Err(format!("unknown front endpoint flag {other} in invite")),
            };
            fronts.push(Front {
                member_key,
                wireguard_public_key,
                mesh_port,
                endpoint,
            });
        }
        fronts
    };
    if !r.done() {
        return Err("invite payload has trailing bytes".into());
    }
    Ok(Invite {
        descriptor: NetworkDescriptor {
            chain_id,
            scheme: SCHEME_ED25519.into(),
            validators,
            // a decoded invite carries dial hints as typed `reach`; `bootstrap`
            // stays empty and both feed one dial source via `reach_hints`.
            bootstrap: Vec::new(),
            reach,
            coordination,
        },
        token,
        wireguard,
        fronts,
        expires_unix_secs,
    })
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
    /// the UDP endpoint this node advertises for its WireGuard tunnel;
    /// PRESENT stages the node-driven reachability plane (node-local
    /// operator policy, like checkpoint_blocks). absent = plane off.
    pub wireguard_listen: Option<String>,
    /// which `WireGuardEffect` the reachability plane drives: "tun"
    /// (default — configure an actual interface via the userspace WireGuard
    /// runtime; needs root/CAP_NET_ADMIN; "real" is the legacy alias),
    /// "socket" (the ADR's TUN-less in-process backend: no privilege, no
    /// host mutation — overlay reachability exists only inside this
    /// process), or "fake" (record configs in memory; for dev/sim runs, and
    /// for several same-chain nodes on one host, which would otherwise
    /// fight over one interface name).
    pub wireguard_effect: Option<String>,
    /// the UDP endpoint this node's invite intro listener binds — where a
    /// fresh joiner announces its keys (token-authenticated) so the tunnel
    /// can come up before any p2p. defaults to `wireguard_listen` with the
    /// port + 1; only meaningful when the plane runs.
    pub invite_listen: Option<String>,
    /// opt-in shipped-index warm start when joining (node-local operator
    /// policy, like checkpoint_blocks): fetch the sync source's derived
    /// index checkpoints alongside state-sync. the derived tier has no
    /// root, so these bytes are UNVERIFIABLE — off, the default, means the
    /// index heals from verified state instead (indexable spec §7).
    pub sync_index: Option<bool>,
    /// whether this node publishes its discovered provider set into the
    /// capability registry (node-local operator policy, like
    /// checkpoint_blocks; default true). `false` makes an ACCEPT-LANE-ONLY
    /// provider: the node still resolves and executes capabilities its host
    /// carries, but never enters any tag's rendezvous pool — it serves only
    /// UNASSIGNED announcements, by racing `SagaMsg::Accept` like any other
    /// capable node. announcing stays truthful either way: this can hide a
    /// real provider, never fabricate one.
    pub announce_capabilities: Option<bool>,
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
    /// merged from an existing file only (no flag); a WireGuard join seeds a
    /// default AFTER the merge when the invite carries a tunnel bootstrap.
    pub wireguard_listen: Option<String>,
    /// merged from explicit flags or existing file; defaults from
    /// `wireguard_listen` when absent.
    pub invite_listen: Option<String>,
    /// merged like the rest — a hand-set value survives; the desktop app
    /// passes "socket" here (overlay-net ADR phase 4) while the parse
    /// default for a file without the key stays `tun`.
    pub wireguard_effect: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn merged_plumbing(
    dir: &Path,
    listen: Option<&str>,
    advertised: Option<&str>,
    http_listen: Option<&str>,
    rpc_listen: Option<&str>,
    wireguard_effect: Option<&str>,
    wireguard_listen: Option<&str>,
    invite_listen: Option<&str>,
) -> Result<Plumbing, String> {
    let path = dir.join("node.toml");
    let existing: Option<NodeToml> = if path.exists() {
        Some(load_node_toml(&path)?.0)
    } else {
        None
    };
    let e = existing.as_ref();
    // reject a typo'd effect value at the verb, before anything lands on disk
    // — resolve() would only catch it on the node's NEXT boot.
    parse_wireguard_effect(wireguard_effect)?;
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
        wireguard_listen: wireguard_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_listen.clone())),
        invite_listen: invite_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.invite_listen.clone())),
        wireguard_effect: wireguard_effect
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_effect.clone())),
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
    if let Some(w) = &p.wireguard_listen {
        s += &format!("wireguard_listen = \"{w}\"\n");
    }
    if let Some(i) = &p.invite_listen {
        s += &format!("invite_listen = \"{i}\"\n");
    }
    if let Some(w) = &p.wireguard_effect {
        s += &format!("wireguard_effect = \"{w}\"\n");
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
#[cfg(test)]
pub fn choose_sync_source<A>(
    bootstrappers: &[(ed25519::PublicKey, A)],
    validators: &[ed25519::PublicKey],
    me: &ed25519::PublicKey,
) -> Option<ed25519::PublicKey> {
    sync_source_candidates(bootstrappers, validators, me)
        .into_iter()
        .next()
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

/// everything `run_node` needs, shape-independent.
#[derive(Debug)]
pub struct Resolved {
    pub signer: ed25519::PrivateKey,
    /// log prefix: "#<id>" for the dev shape, the identity's short hex
    /// otherwise.
    pub label: String,
    /// the chain-id (network shape) or legacy namespace bytes.
    pub namespace: Vec<u8>,
    /// this network's chain id — the descriptor's own `chain_id` field (network
    /// shape) or the raw configured namespace (dev shape, which has no
    /// fingerprint appended). NOT `namespace`: the network shape's `namespace`
    /// is `genesis_namespace()`, i.e. `chain_id@fingerprint` — a DIFFERENT
    /// string. This is the exact string the desktop app records as
    /// `Workspace.chain_id` (the `init` verb's last stdout line), so modules
    /// that must agree with the app on "this network's id" (e.g. `identity`'s
    /// certificate domain separation) use this field, never `namespace`.
    pub chain_id: String,
    /// the authorized mesh set (unsorted here; the caller builds the ordered
    /// Set discovery tracks).
    pub mesh: Vec<ed25519::PublicKey>,
    /// the genesis consensus participant subset.
    pub validators: Vec<ed25519::PublicKey>,
    /// (identity, dial ingress) pairs to dial at startup; never contains
    /// self. hostname ingresses stay hostnames — dialers re-resolve them.
    pub bootstrappers: Vec<(ed25519::PublicKey, Ingress)>,
    /// reach targets that need the nat client: (target key, coordinator
    /// ingress, coordinator key). empty unless a v3 invite carried Coordinated
    /// hints. the runtime rendezvous/hole-punches through the coordinator to
    /// each target, then authenticates the target's own key end-to-end.
    pub coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)>,
    pub listen: SocketAddr,
    /// this node's self-announced dial address. a HOSTNAME advertised stays a
    /// hostname all the way into the signed peer record, so a node behind a
    /// tunnel with a stable name never needs an address update — and it BOOTS
    /// even while its own name does not resolve.
    pub advertised: Ingress,
    pub storage_dir: PathBuf,
    pub rpc_listen: Option<String>,
    pub http_listen: Option<String>,
    /// the staged WireGuard reachability plane's advertised UDP endpoint;
    /// None = plane off.
    pub wireguard_listen: Option<SocketAddr>,
    /// which `WireGuardEffect` the plane drives when it is on.
    pub wireguard_effect: WireGuardEffectKind,
    /// the invite intro listener endpoint (`invite_listen`, defaulted from
    /// `wireguard_listen` + 1); `None` when the plane is off.
    pub invite_listen: Option<SocketAddr>,
    /// where the node's X25519 WireGuard keypair persists (beside
    /// identity.key in the network shape).
    pub wireguard_key_file: PathBuf,
    /// dev-seed shape marker: gates the boot-time demo op + converged print
    /// (scaffolding a REAL network must not write into its genesis).
    pub dev_demo: bool,
    /// sealed blocks between recovery checkpoints.
    pub checkpoint_blocks: u64,
    /// the invite token a `join` stored beside the descriptor, if any — what a
    /// parked joiner announces over the lobby channel. always `None` for the
    /// dev shape and for manual (token-less) joins.
    pub invite_token: Option<InviteToken>,
    /// the inviter's WireGuard bootstrap a `join` stored, if any — the tunnel
    /// the joining node brings up BEFORE any p2p. always `None` for the dev
    /// shape and for members.
    pub invite_wireguard: Option<StoredInviteWireGuard>,
    /// the inviter's offered member fronts a `join` stored, if any — the
    /// ADDITIONAL first-contact paths the joiner races alongside the inviter.
    /// Empty for the dev shape, for members, and for pre-feature invites.
    pub invite_fronts: Vec<Front>,
    /// opt-in shipped-index warm start when joining; see `NodeToml::sync_index`.
    pub sync_index: bool,
    /// publish the discovered provider set into the capability registry; see
    /// `NodeToml::announce_capabilities`.
    pub announce_capabilities: bool,
    /// the reachability plane's coordination privacy (per-network operational
    /// policy). `Private` (the default) requires a genesis-issued `CoordCap`
    /// for a node outside the genesis validator set; `Public` accepts any
    /// proof-of-possession. The dev shape is always `Private` (it never uses a
    /// real coordinator).
    pub coordination: Coordination,
    /// the genesis-issued admission capability this node presents on every
    /// coordinator request (loaded from `coord.cap` beside the identity).
    /// `None` for a genesis validator (admitted by membership), the dev shape,
    /// or a node that has not been issued one.
    pub coord_cap: Option<nat_traversal::CoordCap>,
    /// the workspace base directory — where `identity.key`, `network.toml`,
    /// `wireguard.key` and `coord.cap` live (the network shape's config
    /// directory; the dev shape's `storage_dir`). Threaded so a parked
    /// joiner's lobby-reply task can persist a `coord.cap` delivered over its
    /// `JoinReply` via `save_coord_cap`.
    pub workspace: PathBuf,
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
    // one dial source of truth: reach_entries() folds bootstrap-synthesised
    // Direct hints in with the typed `reach` hints (their union). Direct/Fronted
    // resolve to a mesh Ingress dialed directly; Coordinated routes are handed
    // to the nat client, which rendezvouses through the coordinator and
    // hole-punches to the target — but the target is still authenticated
    // end-to-end by its own key, so a coordinated peer is a real mesh member
    // either way.
    let mut bootstrap: Vec<(ed25519::PublicKey, Ingress)> = Vec::new();
    let mut coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)> = Vec::new();
    for (key, dial) in descriptor.reach_entries()? {
        match dial {
            ReachDial::Direct(ingress) => bootstrap.push((key, ingress)),
            ReachDial::Coordinated { coord, coord_key } => coordinated.push((key, coord, coord_key)),
        }
    }
    // mesh = validators ∪ every reach identity (direct + coordinated) ∪ the
    // LOBBY identity. A fresh network-shape joiner may be outside this set at
    // genesis; it parks until governance admits it — but it can always be
    // HEARD: the lobby key is derivable from the descriptor alone, so every
    // node folds the same key into the same tracked set (discovery kills peers
    // whose set at a shared index differs) and an invite-holding joiner can
    // complete the handshake to announce itself on the lobby channel.
    let mut mesh = validators.clone();
    for (k, _) in &bootstrap {
        if !mesh.contains(k) {
            mesh.push(k.clone());
        }
    }
    for (k, _, _) in &coordinated {
        if !mesh.contains(k) {
            mesh.push(k.clone());
        }
    }
    let lobby = lobby_identity(descriptor.genesis_namespace().as_bytes()).public_key();
    if !mesh.contains(&lobby) {
        mesh.push(lobby);
    }

    let listen: SocketAddr = raw.listen.parse().map_err(|e| format!("listen: {e}"))?;
    let advertised = resolve_advertised(
        raw.advertised.as_deref(),
        listen,
        &descriptor.genesis_namespace(),
        &me,
    )?;
    let bootstrappers = bootstrap.into_iter().filter(|(k, _)| *k != me).collect();
    let wireguard_listen = parse_wireguard_listen(raw.wireguard_listen.as_deref())?;
    let wireguard_effect = parse_wireguard_effect(raw.wireguard_effect.as_deref())?;
    let invite_listen = wireguard_listen
        .map(|wg| resolved_invite_listen(raw.invite_listen.as_deref(), wg))
        .transpose()?;

    Ok(Resolved {
        label: hex_bytes(&me.as_ref()[..4]),
        namespace: descriptor.genesis_namespace().into_bytes(),
        chain_id: descriptor.chain_id.clone(),
        signer,
        mesh,
        validators,
        bootstrappers,
        coordinated,
        listen,
        advertised,
        storage_dir: base.join(raw.storage_dir.as_deref().unwrap_or("storage")),
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        wireguard_listen,
        wireguard_effect,
        wireguard_key_file: base.join("wireguard.key"),
        invite_listen,
        dev_demo: false,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        invite_token: load_invite_token(base)?,
        invite_wireguard: load_invite_wireguard(base)?,
        invite_fronts: load_invite_fronts(base)?,
        sync_index: raw.sync_index.unwrap_or(false),
        announce_capabilities: raw.announce_capabilities.unwrap_or(true),
        coordination: descriptor.coordination(),
        // the reachability plane presents this on every coordinator request; a
        // genesis validator needs none (admitted by membership), a joiner is
        // issued one beside its identity.
        coord_cap: load_coord_cap(base),
        // the config directory: identity.key / network.toml / coord.cap live
        // here, so a joiner persists a delivered cap into it.
        workspace: base.to_path_buf(),
    })
}

fn parse_wireguard_listen(raw: Option<&str>) -> Result<Option<SocketAddr>, String> {
    raw.map(|a| {
        a.parse::<SocketAddr>()
            .map_err(|e| format!("wireguard_listen: {e}"))
    })
    .transpose()
}

/// the parsed `wireguard_listen`, for callers working off a raw `NodeToml`
/// (the CLI verbs) rather than a full `resolve`.
pub fn resolved_wireguard_listen(raw: Option<&str>) -> Result<Option<SocketAddr>, String> {
    parse_wireguard_listen(raw)
}

/// the invite intro listener endpoint: explicit `invite_listen`, else the
/// WireGuard listen address with the next port — one convention both the
/// minting side (what lands in the blob) and the serving side (what the
/// plane binds) derive identically.
pub fn resolved_invite_listen(
    raw: Option<&str>,
    wireguard_listen: SocketAddr,
) -> Result<SocketAddr, String> {
    match raw {
        Some(a) => a.parse().map_err(|e| format!("invite_listen: {e}")),
        None => {
            let port = wireguard_listen
                .port()
                .checked_add(1)
                .ok_or("wireguard_listen port has no successor for the intro default")?;
            Ok(SocketAddr::new(wireguard_listen.ip(), port))
        }
    }
}

/// the HOST a minted invite's UDP endpoints carry: the WireGuard listen IP
/// when it is concrete, else the advertised host (an invite must hand the
/// joiner an underlay address that reaches this machine — the usual listen
/// is unspecified, so `advertised` is the truth).
pub fn endpoint_host(
    advertised: Option<&str>,
    listen: &str,
    wireguard_listen: SocketAddr,
) -> Result<String, String> {
    if !wireguard_listen.ip().is_unspecified() {
        return Ok(wireguard_listen.ip().to_string());
    }
    let dial = dialable(advertised, listen)?.ok_or(
        "no dialable host for the WireGuard invite endpoints: set `advertised` (or a \
         concrete wireguard_listen IP) so a joiner can reach this node's tunnel",
    )?;
    // strip the port: `host:port` or `[v6]:port`.
    match dial.rsplit_once(':') {
        Some((host, _)) => Ok(host.trim_matches(['[', ']']).to_string()),
        None => Ok(dial),
    }
}

/// which `WireGuardEffect` implementation the reachability plane drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireGuardEffectKind {
    /// the TUN-less in-process backend (overlay-net ADR): BoringTun `Tunn`s
    /// + smoltcp behind the overlay seam, no privilege, no host mutation.
    Socket,
    /// configure an actual interface through the userspace WireGuard runtime.
    Tun,
    /// record configurations in memory without touching the network stack.
    Fake,
}

fn parse_wireguard_effect(raw: Option<&str>) -> Result<WireGuardEffectKind, String> {
    match raw {
        Some("socket") => Ok(WireGuardEffectKind::Socket),
        // "real" predates the socket backend and stays as an alias for the
        // interface-backed path it always meant.
        None | Some("tun") | Some("real") => Ok(WireGuardEffectKind::Tun),
        Some("fake") => Ok(WireGuardEffectKind::Fake),
        Some(other) => Err(format!(
            "wireguard_effect: {other:?} is not \"socket\", \"tun\" (alias \"real\"), or \"fake\""
        )),
    }
}

/// resolve the `advertised` config value into a dial ingress. the sentinel
/// `"overlay"` advertises this node's chain-derived WireGuard overlay address
/// (`ula_v6_member_addr(namespace, identity)`) at the mesh listen port — the
/// address peers can dial once a tunnel to this node is up, and the RIGHT
/// advertisement for a member with no dialable underlay address (NAT, zero
/// exposed ports). the overlay is IPv6, so it requires an IPv6 mesh listener
/// (`listen = "[::]:port"` accepts both families on a default dual-stack
/// host); a v4-only listener would never see the tunnel's SYNs.
fn resolve_advertised(
    raw: Option<&str>,
    listen: SocketAddr,
    namespace: &str,
    me: &ed25519::PublicKey,
) -> Result<Ingress, String> {
    match raw {
        Some("overlay") => {
            if !listen.is_ipv6() {
                return Err(format!(
                    "advertised = \"overlay\" needs an IPv6 mesh listener to accept tunnel \
                     traffic — set listen = \"[::]:{}\"",
                    listen.port()
                ));
            }
            let identity = wireguard_upgrade::ValidatorIdentity::try_from(me.as_ref())
                .map_err(|e| format!("advertised: {e:?}"))?;
            let ula = wireguard_upgrade::ula_v6_member_addr(namespace, identity);
            Ok(Ingress::Socket(SocketAddr::new(
                std::net::IpAddr::V6(ula),
                listen.port(),
            )))
        }
        // an explicitly-configured advertised that can never be dialed is a
        // config error; a hostname is kept VERBATIM (no boot-time DNS).
        Some(a) => ingress_of(a)
            .map_err(|e| format!("advertised: {e}"))?
            .ok_or_else(|| format!("advertised addr {a:?} is not dialable")),
        None => Ok(Ingress::Socket(listen)),
    }
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
        vec![(key_of(boot_seed), Ingress::Socket(boot_addr))]
            .into_iter()
            .filter(|(k, _)| *k != ed25519::PrivateKey::from_seed(id).public_key())
            .collect()
    };

    let listen: SocketAddr = raw.listen.parse().map_err(|e| format!("listen: {e}"))?;
    let advertised = resolve_advertised(
        raw.advertised.as_deref(),
        listen,
        &namespace,
        &ed25519::PrivateKey::from_seed(id).public_key(),
    )?;

    let storage_dir = raw
        .storage_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("ducktape-node-{id}")));
    let wireguard_listen = parse_wireguard_listen(raw.wireguard_listen.as_deref())?;
    let wireguard_effect = parse_wireguard_effect(raw.wireguard_effect.as_deref())?;
    Ok(Resolved {
        signer: ed25519::PrivateKey::from_seed(id),
        label: format!("#{id}"),
        // the dev shape's namespace carries no fingerprint suffix (unlike the
        // network shape's `genesis_namespace()`), so it IS the chain id here.
        chain_id: namespace.clone(),
        namespace: namespace.into_bytes(),
        mesh,
        validators,
        bootstrappers,
        // the dev-seed shape never uses coordinated reach — direct sockets only.
        coordinated: Vec::new(),
        listen,
        advertised,
        // the dev shape has no identity.key directory; the wireguard key
        // lives with the node's other per-process state.
        wireguard_key_file: storage_dir.join("wireguard.key"),
        // the dev shape has no config directory; its per-process state dir
        // stands in as the workspace base (it never delivers a real cap).
        workspace: storage_dir.clone(),
        storage_dir,
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        wireguard_listen,
        wireguard_effect,
        invite_listen: wireguard_listen
            .map(|wg| resolved_invite_listen(raw.invite_listen.as_deref(), wg))
            .transpose()?,
        dev_demo: true,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        invite_token: None,
        invite_wireguard: None,
        invite_fronts: Vec::new(),
        sync_index: raw.sync_index.unwrap_or(false),
        announce_capabilities: raw.announce_capabilities.unwrap_or(true),
        // the dev shape wires direct sockets only — no real coordinator, so
        // the coordination mode defaults to Private and no cap is presented.
        coordination: Coordination::Private,
        coord_cap: None,
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

    #[test]
    fn coordinated_invite_needs_reachability_defaults_without_tunnel_bootstrap() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let mut d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.apply_primary_coordinator(&issuer.public_key(), "p2p.ducktape.byeongsu.dev:3478")
            .expect("coordinator hint");
        let invite = decode_invite(&encode_test_invite(&d, &issuer, None)).expect("decode");

        assert!(
            invite_requires_reachability_defaults(&invite),
            "a coordinated invite must start the joiner's reachability plane even without a direct tunnel bootstrap"
        );
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

    /// mint + encode with the test defaults: issuer-signed, far-future expiry.
    fn encode_test_invite(
        d: &NetworkDescriptor,
        issuer: &ed25519::PrivateKey,
        wireguard: Option<&InviteWireGuard>,
    ) -> String {
        let token = mint_invite_token(issuer, d.genesis_namespace().as_bytes());
        encode_invite(d, &token, wireguard, &[], u64::MAX, issuer).expect("encode")
    }

    // ---- user-key bind/unbind certificates ---------------------------------

    #[test]
    fn mint_bind_cert_verifies_against_module_preimage() {
        use commonware_cryptography::{
            Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(1);
        let node_pub = [9u8; 32];
        let cert = mint_bind_cert(&user, "chain-a", &node_pub, 0);
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::bind_preimage("chain-a", &node_pub, 0);
        assert!(user.public_key().verify(identity::IDENTITY_BIND_NS, &preimage, &sig));
    }

    #[test]
    fn mint_bind_cert_is_chain_scoped() {
        use commonware_cryptography::{
            Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(1);
        let node_pub = [9u8; 32];
        let cert = mint_bind_cert(&user, "chain-a", &node_pub, 0);
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        // a cert minted for chain-a must NOT verify against chain-b's preimage.
        let preimage_b = identity::bind_preimage("chain-b", &node_pub, 0);
        assert!(!user.public_key().verify(identity::IDENTITY_BIND_NS, &preimage_b, &sig));
    }

    #[test]
    fn mint_bind_cert_does_not_verify_under_unbind_namespace() {
        use commonware_cryptography::{
            Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(1);
        let node_pub = [9u8; 32];
        let cert = mint_bind_cert(&user, "chain-a", &node_pub, 0);
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::bind_preimage("chain-a", &node_pub, 0);
        // signed under IDENTITY_BIND_NS -- must NOT verify under the unbind ns.
        assert!(!user.public_key().verify(identity::IDENTITY_UNBIND_NS, &preimage, &sig));
    }

    #[test]
    fn mint_unbind_cert_verifies_against_module_preimage() {
        use commonware_cryptography::{
            Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(2);
        let node_pub = [11u8; 32];
        let cert = mint_unbind_cert(&user, "chain-a", &node_pub, 3);
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::unbind_preimage("chain-a", &node_pub, 3);
        assert!(user.public_key().verify(identity::IDENTITY_UNBIND_NS, &preimage, &sig));
    }

    #[test]
    fn mint_unbind_cert_is_chain_scoped() {
        use commonware_cryptography::{
            Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(2);
        let node_pub = [11u8; 32];
        let cert = mint_unbind_cert(&user, "chain-a", &node_pub, 3);
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage_b = identity::unbind_preimage("chain-b", &node_pub, 3);
        assert!(!user.public_key().verify(identity::IDENTITY_UNBIND_NS, &preimage_b, &sig));
    }

    #[test]
    fn mint_unbind_cert_does_not_verify_under_bind_namespace() {
        use commonware_cryptography::{
            Verifier as _,
            ed25519::Signature,
        };
        let user = ed25519::PrivateKey::from_seed(2);
        let node_pub = [11u8; 32];
        let cert = mint_unbind_cert(&user, "chain-a", &node_pub, 3);
        let sig = Signature::decode(cert.as_slice()).expect("valid signature encoding");
        let preimage = identity::unbind_preimage("chain-a", &node_pub, 3);
        // signed under IDENTITY_UNBIND_NS -- must NOT verify under the bind ns.
        assert!(!user.public_key().verify(identity::IDENTITY_BIND_NS, &preimage, &sig));
    }

    /// mirrors exactly what `cmd_user_sign_bind`/`cmd_user_sign_unbind` build,
    /// so this is a stand-in for hand-verifying the CLI's JSON output: encode
    /// through `identity::encode_msg`, decode through `identity::decode_msg`,
    /// and check the message the module actually consumes round-trips.
    #[test]
    fn user_sign_messages_round_trip_through_identity_codec() {
        let user = ed25519::PrivateKey::from_seed(3);
        let node_pub = [42u8; 32];

        let bind_sig = mint_bind_cert(&user, "test@abc", &node_pub, 0);
        let bind_msg = identity::IdentityMsg::BindNode {
            user_key: user.public_key().as_ref().to_vec(),
            user_sig: bind_sig,
        };
        let encoded = identity::encode_msg(&bind_msg);
        // the wire contract: a single utf-8 JSON line, decodable as-is.
        assert_eq!(String::from_utf8(encoded.clone()).unwrap().lines().count(), 1);
        assert_eq!(identity::decode_msg(&encoded).unwrap(), bind_msg);

        let unbind_sig = mint_unbind_cert(&user, "test@abc", &node_pub, 1);
        let unbind_msg = identity::IdentityMsg::UnbindNode {
            node_key: node_pub.to_vec(),
            user_sig: unbind_sig,
        };
        let encoded = identity::encode_msg(&unbind_msg);
        assert_eq!(String::from_utf8(encoded.clone()).unwrap().lines().count(), 1);
        assert_eq!(identity::decode_msg(&encoded).unwrap(), unbind_msg);
    }

    #[test]
    fn invite_blob_roundtrips_and_verifies() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let me = issuer.public_key();
        let mut d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&me, "127.0.0.1:52200");
        // decode NORMALISES bootstrap hints into typed Direct reach hints, so
        // the decoded descriptor dials the same members via `reach` even
        // though it carries no `bootstrap`.
        let invite = decode_invite(&encode_test_invite(&d, &issuer, None)).expect("roundtrip");
        assert_eq!(
            invite.descriptor.reach_hints().expect("decoded hints"),
            d.reach_hints().expect("source hints"),
            "the dial hints survive as typed reach hints"
        );
        assert_eq!(invite.wireguard, None);
        let binding = d.genesis_namespace();
        assert!(verify_invite_token(&invite.token, binding.as_bytes()));

        // a HOSTNAME dial hint survives verbatim (stored as a string, resolved
        // only at dial time — never at encode/decode).
        let other = ed25519::PrivateKey::from_seed(8).public_key();
        d.add_bootstrap(&other, "node.ducktape.industries:443");
        let invite = decode_invite(&encode_test_invite(&d, &issuer, None)).expect("roundtrip");
        assert!(
            invite
                .descriptor
                .reach
                .iter()
                .any(|r| r.ends_with("@node.ducktape.industries:443"))
        );

        // the joiner's proof-of-possession verifies for the signing key only.
        let joiner = ed25519::PrivateKey::from_seed(9);
        let proof = sign_join_proof(&joiner, binding.as_bytes(), &invite.token);
        assert!(verify_join_proof(
            &joiner.public_key(),
            binding.as_bytes(),
            &invite.token,
            &proof
        ));
        let thief = ed25519::PrivateKey::from_seed(10).public_key();
        assert!(
            !verify_join_proof(&thief, binding.as_bytes(), &invite.token, &proof),
            "a substituted key fails the proof"
        );
    }

    #[test]
    fn invite_blob_carries_the_wireguard_bootstrap() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            // a token invite now packs v4, which echoes the coordination mode;
            // a `None` source resolves to Private, so it decodes as an EXPLICIT
            // "private" (semantically identical — see `coordination()`).
            coordination: Some("private".into()),
        };
        let wg = InviteWireGuard {
            public_key: [42u8; 32],
            endpoint: Some("203.0.113.7:51820".into()),
            intro: Some("203.0.113.7:51821".into()),
            mesh_port: 52200,
        };
        let invite = decode_invite(&encode_test_invite(&d, &issuer, Some(&wg))).expect("decode");
        assert_eq!(invite.wireguard, Some(wg));
        // a wireguard invite with no fronts decodes to an empty set.
        assert!(invite.fronts.is_empty());
    }

    /// mint + encode with the test defaults, carrying a set of fronts.
    fn encode_test_invite_with_fronts(
        d: &NetworkDescriptor,
        issuer: &ed25519::PrivateKey,
        wireguard: Option<&InviteWireGuard>,
        fronts: &[Front],
    ) -> String {
        let token = mint_invite_token(issuer, d.genesis_namespace().as_bytes());
        encode_invite(d, &token, wireguard, fronts, u64::MAX, issuer).expect("encode")
    }

    fn front_descriptor(issuer: &ed25519::PrivateKey) -> NetworkDescriptor {
        NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: Some("private".into()),
        }
    }

    #[test]
    fn invite_blob_roundtrips_fronts() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = front_descriptor(&issuer);
        // one host-capable member (direct endpoint), one punchable (coordinated).
        let fronts = vec![
            Front {
                member_key: [11u8; 32],
                wireguard_public_key: [12u8; 32],
                mesh_port: 52201,
                endpoint: Some("198.51.100.9:51820".into()),
            },
            Front {
                member_key: [21u8; 32],
                wireguard_public_key: [22u8; 32],
                mesh_port: 52202,
                endpoint: None,
            },
        ];
        let blob = encode_test_invite_with_fronts(&d, &issuer, None, &fronts);
        let invite = decode_invite(&blob).expect("decode");
        assert_eq!(invite.fronts, fronts);
    }

    #[test]
    fn pre_feature_invite_decodes_to_empty_fronts() {
        use base64::Engine as _;
        // an invite minted WITHOUT fronts carries NO fronts block — its signed
        // payload ends at the fixed-length token, byte-for-byte like a
        // pre-feature blob — so an old blob (nothing after the token) decodes
        // to an empty set. Token nonces are random, so we compare the ENCODED
        // LENGTH (identical when the fronts block is absent), not the bytes.
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = front_descriptor(&issuer);
        let empty = encode_test_invite_with_fronts(&d, &issuer, None, &[]);
        let baseline = encode_test_invite(&d, &issuer, None);
        let len = |blob: &str| {
            INVITE_B64
                .decode(blob.strip_prefix(INVITE_PREFIX).unwrap())
                .unwrap()
                .len()
        };
        assert_eq!(
            len(&empty),
            len(&baseline),
            "an empty-fronts invite adds no bytes over a pre-feature blob"
        );
        assert!(decode_invite(&empty).expect("decode").fronts.is_empty());
    }

    fn sample_advert(
        seed: u64,
        octet: u8,
        wireguard_endpoint: Option<u16>,
    ) -> wireguard_upgrade::EndpointAdvertisement {
        use std::net::{IpAddr, Ipv4Addr};
        use wireguard_upgrade::{
            AdmissionRoot, Endpoint, EndpointRecord, MeshVersion, PortPolicy, Root, Transport,
            ValidatorIdentity, X25519PublicKey,
        };
        let policy = PortPolicy::production();
        let signer = ed25519::PrivateKey::from_seed(seed);
        let endpoint = |port: u16, transport| {
            Endpoint::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet)), port, transport, &policy)
                .unwrap()
        };
        let record = EndpointRecord {
            namespace: "net#fronts".into(),
            epoch: 3,
            valset_root: Root([1; 32]),
            admission_root: AdmissionRoot([2; 32]),
            validator_identity: ValidatorIdentity::try_from(signer.public_key().as_ref()).unwrap(),
            wireguard_public_key: X25519PublicKey([octet; 32]),
            control_endpoint: endpoint(443, Transport::Tcp),
            wireguard_endpoint: wireguard_endpoint.map(|port| endpoint(port, Transport::Udp)),
            capabilities: vec![],
            expires_at_view: 50,
            nonce: 1,
        };
        wireguard_upgrade::EndpointAdvertisement::sign(record, MeshVersion([7; 32]), &signer)
    }

    #[test]
    fn fronts_from_adverts_maps_reachable_members_and_skips_self() {
        // three adverts: self (skipped), a host-capable member (direct
        // endpoint), and a punchable member (no underlay endpoint → coordinated).
        let me = ed25519::PrivateKey::from_seed(1);
        let host_capable = sample_advert(2, 20, Some(51820));
        let punchable = sample_advert(3, 30, None);
        let adverts = vec![sample_advert(1, 10, Some(51820)), host_capable.clone(), punchable.clone()];

        let own: [u8; 32] = me.public_key().as_ref().try_into().unwrap();
        let fronts = fronts_from_adverts(&adverts, &own);

        assert_eq!(fronts.len(), 2, "the inviter's own advert is skipped");
        let direct = fronts
            .iter()
            .find(|f| f.member_key == host_capable.record.validator_identity.0)
            .expect("host-capable front");
        assert_eq!(direct.endpoint.as_deref(), Some("8.8.8.20:51820"));
        assert_eq!(direct.mesh_port, 443);
        assert_eq!(direct.wireguard_public_key, [20u8; 32]);
        let coordinated = fronts
            .iter()
            .find(|f| f.member_key == punchable.record.validator_identity.0)
            .expect("punchable front");
        assert_eq!(coordinated.endpoint, None);
    }

    #[test]
    fn fronts_are_excluded_from_the_genesis_fingerprint() {
        // two invites that differ ONLY in their fronts must fingerprint
        // identically: fronts are advisory reachability, never validator
        // identity, so `genesis_namespace` cannot see them.
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = front_descriptor(&issuer);
        let a = decode_invite(&encode_test_invite_with_fronts(&d, &issuer, None, &[]))
            .expect("decode a");
        let b = decode_invite(&encode_test_invite_with_fronts(
            &d,
            &issuer,
            None,
            &[Front {
                member_key: [11u8; 32],
                wireguard_public_key: [12u8; 32],
                mesh_port: 52201,
                endpoint: Some("198.51.100.9:51820".into()),
            }],
        ))
        .expect("decode b");
        assert_ne!(a.fronts, b.fronts, "the two invites differ only in fronts");
        assert_eq!(
            a.descriptor.genesis_namespace(),
            b.descriptor.genesis_namespace(),
            "fronts must not perturb the genesis fingerprint"
        );
        // Non-tautological both ways: the round-tripped fingerprint equals the
        // source descriptor's (fronts on the wire never fold into it), AND the
        // fingerprint IS sensitive to validator identity — proving it tracks the
        // consensus set, not the advisory reachability payload.
        assert_eq!(
            a.descriptor.genesis_namespace(),
            d.genesis_namespace(),
            "encoding/decoding fronts must not change the source fingerprint"
        );
        let mut with_extra_validator = front_descriptor(&issuer);
        with_extra_validator
            .validators
            .push(hex_bytes(ed25519::PrivateKey::from_seed(8).public_key().as_ref()));
        assert_ne!(
            d.genesis_namespace(),
            with_extra_validator.genesis_namespace(),
            "the fingerprint must change when the validator set changes"
        );
    }

    #[test]
    fn a_tampered_or_expired_or_stale_prefix_invite_is_refused() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        let token = mint_invite_token(&issuer, d.genesis_namespace().as_bytes());

        // expiry is enforced at decode, deterministically via the injected clock.
        let blob = encode_invite(&d, &token, None, &[], 1_000, &issuer).expect("encode");
        assert!(decode_invite_at(&blob, 999).is_ok());
        let err = decode_invite_at(&blob, 1_000).expect_err("expired");
        assert!(err.contains("expired"), "{err}");

        // a flipped payload bit kills the envelope signature.
        use base64::Engine as _;
        let body = blob.strip_prefix(INVITE_PREFIX).unwrap();
        let mut bytes = INVITE_B64.decode(body).unwrap();
        bytes[2] ^= 0x01;
        let tampered = format!("{INVITE_PREFIX}{}", INVITE_B64.encode(&bytes));
        let err = decode_invite_at(&tampered, 0).expect_err("tampered");
        assert!(
            err.contains("signature") || err.contains("tampered"),
            "{err}"
        );

        // an envelope signed by someone other than the token's issuer is
        // refused at encode (and would fail decode's issuer verify anyway).
        let outsider = ed25519::PrivateKey::from_seed(8);
        assert!(encode_invite(&d, &token, None, &[], u64::MAX, &outsider).is_err());

        // the old versioned prefixes are gone: a stale paste fails loudly
        // with re-mint guidance.
        let err = decode_invite_at("ducktape-invite-v2:AAAA", 0).expect_err("stale prefix");
        assert!(err.contains("fresh invite"), "{err}");
    }

    #[test]
    fn invite_echoes_the_coordination_mode() {
        // the mode byte rides the SIGNED envelope: a joiner learns off the
        // invite alone whether the coordinator expects a CoordCap.
        let issuer = ed25519::PrivateKey::from_seed(7);
        let base = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        for (mode, expect) in [("public", Coordination::Public), ("private", Coordination::Private)]
        {
            let mut d = base.clone();
            d.coordination = Some(mode.to_string());
            let invite =
                decode_invite(&encode_test_invite(&d, &issuer, None)).expect("roundtrip");
            assert_eq!(
                invite.descriptor.coordination(),
                expect,
                "the {mode} mode byte roundtrips"
            );
            assert_eq!(invite.descriptor.coordination.as_deref(), Some(mode));
        }
        // an unset source resolves to Private, the safe default, and decodes
        // as the EXPLICIT "private" (semantically identical).
        let invite =
            decode_invite(&encode_test_invite(&base, &issuer, None)).expect("roundtrip");
        assert_eq!(invite.descriptor.coordination.as_deref(), Some("private"));
    }

    /// helper: a descriptor whose only validator is `issuer` (the minimal
    /// well-formed shape the invite tests need).
    fn front_test_descriptor(issuer: &ed25519::PrivateKey) -> NetworkDescriptor {
        NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        }
    }

    #[test]
    fn invite_blob_roundtrips_member_fronts() {
        // (a) two fronts — one with a direct endpoint, one reachable only by
        // identity (endpoint None) — survive the signed envelope byte-for-byte.
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = front_test_descriptor(&issuer);
        let token = mint_invite_token(&issuer, d.genesis_namespace().as_bytes());
        let fronts = vec![
            Front {
                member_key: [11u8; 32],
                wireguard_public_key: [12u8; 32],
                mesh_port: 52210,
                endpoint: Some("198.51.100.9:51820".into()),
            },
            Front {
                member_key: [21u8; 32],
                wireguard_public_key: [22u8; 32],
                mesh_port: 52211,
                endpoint: None,
            },
        ];
        let blob =
            encode_invite(&d, &token, None, &fronts, u64::MAX, &issuer).expect("encode");
        let invite = decode_invite(&blob).expect("decode");
        assert_eq!(invite.fronts, fronts, "fronts roundtrip through the envelope");
    }

    #[test]
    fn a_pre_feature_blob_decodes_to_empty_fronts() {
        // (b) the fronts block is OMITTED when empty, so a zero-fronts encode is
        // byte-identical to a blob minted BEFORE this feature (which ended at
        // the token). such a blob must decode to `fronts: vec![]`, never error
        // on a "missing" block.
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = front_test_descriptor(&issuer);
        let token = mint_invite_token(&issuer, d.genesis_namespace().as_bytes());

        let empty_blob =
            encode_invite(&d, &token, None, &[], u64::MAX, &issuer).expect("encode");
        let invite = decode_invite(&empty_blob).expect("decode pre-feature-shaped blob");
        assert!(invite.fronts.is_empty(), "no fronts block => empty fronts");

        // and the omission is real: adding a front lengthens the blob, proving
        // the empty case appended nothing (i.e. matches the old wire).
        let with_front = encode_invite(
            &d,
            &token,
            None,
            &[Front {
                member_key: [1u8; 32],
                wireguard_public_key: [2u8; 32],
                mesh_port: 1,
                endpoint: None,
            }],
            u64::MAX,
            &issuer,
        )
        .expect("encode");
        assert!(
            with_front.len() > empty_blob.len(),
            "a front appends bytes the empty blob lacks"
        );
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
    fn invite_token_file_roundtrips() {
        let dir = tmp("invitetoken");
        assert_eq!(load_invite_token(&dir).expect("absent is fine"), None);
        let issuer = ed25519::PrivateKey::from_seed(7);
        let token = mint_invite_token(&issuer, b"net#00000000@feedface");
        save_invite_token(&dir, &token).expect("save");
        assert_eq!(load_invite_token(&dir).expect("load"), Some(token));
    }

    #[test]
    fn a_hostname_advertised_boots_without_dns_and_stays_a_hostname() {
        // the tunnel case: a stable name whose IP moves (or does not resolve
        // right now) must neither block boot nor be frozen to one lookup —
        // it stays a DNS ingress that dialing peers re-resolve every attempt.
        let dir = tmp("dnsadvertised");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let d = NetworkDescriptor {
            chain_id: "net#44444444".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![format!(
                "{}@definitely-not-resolvable.ducktape.invalid:443",
                hex_bytes(me.public_key().as_ref())
            )],
            reach: vec![],
            coordination: None,
        };
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52250\"\n\
             advertised = \"my-tunnel.example.com:443\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("hostnames never block boot");
        assert!(
            matches!(&r.advertised, Ingress::Dns { port: 443, .. }),
            "advertised stays a hostname: {:?}",
            r.advertised
        );
        // the unresolvable bootstrap hint is KEPT as a hostname too (self is
        // filtered from bootstrappers, so check via the descriptor directly).
        let entries = d.bootstrap_entries().expect("hints parse");
        assert!(
            matches!(&entries[0].1, Ingress::Dns { port: 443, .. }),
            "hint stays a hostname: {:?}",
            entries[0].1
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
    fn lobby_identity_is_deterministic_and_lands_in_the_mesh() {
        let a = lobby_identity(b"net#11111111@aa");
        let b = lobby_identity(b"net#11111111@aa");
        assert_eq!(a.public_key(), b.public_key(), "derivable by every holder");
        let c = lobby_identity(b"net#22222222@bb");
        assert_ne!(
            a.public_key(),
            c.public_key(),
            "distinct networks get distinct lobby doors"
        );

        // resolve() folds the lobby key into the tracked mesh (but never into
        // the consensus validator set).
        let dir = tmp("lobbymesh");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let d = NetworkDescriptor {
            chain_id: "net#33333333".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:52240\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        let lobby = lobby_identity(d.genesis_namespace().as_bytes()).public_key();
        assert!(r.mesh.contains(&lobby), "lobby key is tracked");
        assert!(
            !r.validators.contains(&lobby),
            "lobby key never becomes a participant"
        );
    }

    #[test]
    fn a_non_wg_joiner_resolves_with_zero_reachability_config() {
        // the zero-config joiner contract, non-WG shape: a network-shape
        // config with NO `advertised` and a listen that is not dialable
        // (loopback-ephemeral — cmd_join's non-WG default plumbing) must
        // resolve: the joiner only ever dials OUT to the descriptor's reach
        // hints, so nothing may demand it be reachable itself.
        let dir = tmp("nonwgjoin");
        let (me, _) = load_or_generate_identity(&dir.join("identity.key")).expect("keygen");
        let founder = ed25519::PrivateKey::from_seed(7).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "net#44444444".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(founder.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&founder, "203.0.113.7:41000");
        d.save(&dir.join("network.toml")).expect("save");
        std::fs::write(
            dir.join("node.toml"),
            "network = \"network.toml\"\nlisten = \"127.0.0.1:0\"\n",
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect(
            "a joiner with no advertised and a non-dialable listen must resolve",
        );
        assert_eq!(r.signer.public_key(), me.public_key());
        assert_eq!(r.bootstrappers.len(), 1, "it dials the founder's hint");
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
            coordination: None,
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
        // validators + the derived lobby identity (the join-request door).
        assert_eq!(r.mesh.len(), 3);
        // self never appears in bootstrappers; the other member does.
        assert_eq!(r.bootstrappers.len(), 1);
        assert_eq!(r.bootstrappers[0].0, other);
        assert!(!r.dev_demo);
        assert_eq!(r.signer.public_key(), me.public_key());
        assert_eq!(r.storage_dir, dir.join("storage"));
        // the workspace base is the config directory — where a joiner would
        // persist a `coord.cap` delivered over its JoinReply.
        assert_eq!(r.workspace, dir);
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
            coordination: None,
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
        let lobby = lobby_identity(d.genesis_namespace().as_bytes()).public_key();
        assert_eq!(r.mesh, vec![other, lobby]);
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
    fn announce_capabilities_defaults_on_and_parses_off() {
        let dir = tmp("announce");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52221\"\nnamespace = \"demo\"\npeer_seeds = [0]\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve default");
        assert!(
            resolved.announce_capabilities,
            "announcing is the default posture"
        );

        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nlisten = \"127.0.0.1:52221\"\nnamespace = \"demo\"\npeer_seeds = [0]\n\
             announce_capabilities = false\n",
        )
        .expect("write");
        let resolved = resolve(&dir.join("node.toml")).expect("resolve suppressed");
        assert!(
            !resolved.announce_capabilities,
            "false makes an accept-lane-only provider"
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
    fn unhex_rejects_sign_characters() {
        // from_str_radix would tolerate a leading '+' per pair.
        assert!(unhex("+1ab").is_err());
        assert!(unhex("-1ab").is_err());
    }

    #[test]
    fn sync_source_prefers_validator_hints_and_never_self() {
        let me = ed25519::PrivateKey::from_seed(31).public_key();
        let resident = ed25519::PrivateKey::from_seed(32).public_key();
        let validator = ed25519::PrivateKey::from_seed(33).public_key();
        let addr: SocketAddr = "127.0.0.1:52200".parse().unwrap();
        let validators = vec![me.clone(), validator.clone()];

        // a non-validator hint sorts first but can never serve — skipped.
        let hints = vec![(resident.clone(), addr), (validator.clone(), addr)];
        assert_eq!(
            choose_sync_source(&hints, &validators, &me),
            Some(validator.clone())
        );

        // no usable hint: any validator that is not us.
        let no_hints: &[(ed25519::PublicKey, SocketAddr)] = &[];
        assert_eq!(
            choose_sync_source(no_hints, &validators, &me),
            Some(validator.clone())
        );

        // solo network: nobody can serve.
        assert_eq!(choose_sync_source(no_hints, std::slice::from_ref(&me), &me), None);
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
        let p = merged_plumbing(
            &dir,
            Some("127.0.0.1:53000"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("merge");
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
    fn plumbing_wireguard_effect_flag_wins_absence_preserves_and_typos_abort() {
        let dir = tmp("plumbing-wg-effect");
        // fresh dir + flag (the desktop app's init/join): written to disk.
        let p =
            merged_plumbing(&dir, None, None, None, None, Some("socket"), None, None)
                .expect("merge");
        assert_eq!(p.wireguard_effect.as_deref(), Some("socket"));
        write_node_toml(&dir, &p).expect("write");

        // no flag: the hand-settable value on disk survives a re-merge.
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None)
            .expect("re-merge");
        assert_eq!(p.wireguard_effect.as_deref(), Some("socket"));

        // the flag wins over the file (merged_plumbing's standing precedence).
        let p = merged_plumbing(&dir, None, None, None, None, Some("tun"), None, None)
            .expect("override");
        assert_eq!(p.wireguard_effect.as_deref(), Some("tun"));

        // a typo aborts the verb before anything is written.
        let err = merged_plumbing(&dir, None, None, None, None, Some("sokcet"), None, None)
            .err()
            .expect("a bad effect value must abort the merge");
        assert!(err.contains("wireguard_effect"), "{err}");
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
    fn wireguard_effect_defaults_tun_and_rejects_unknown_values() {
        let dir = tmp("wgeffect");
        let base = "id = 0\nlisten = \"127.0.0.1:52230\"\nnamespace = \"demo\"\npeer_seeds = [0]\n";
        std::fs::write(dir.join("node.toml"), base).expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Tun);

        // "real" is the legacy alias for the interface-backed path.
        for spelled in ["tun", "real"] {
            std::fs::write(
                dir.join("node.toml"),
                format!("{base}wireguard_effect = \"{spelled}\"\n"),
            )
            .expect("write");
            let r = resolve(&dir.join("node.toml")).expect("resolve");
            assert_eq!(r.wireguard_effect, WireGuardEffectKind::Tun, "{spelled}");
        }

        std::fs::write(
            dir.join("node.toml"),
            format!("{base}wireguard_effect = \"socket\"\n"),
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Socket);

        std::fs::write(
            dir.join("node.toml"),
            format!("{base}wireguard_effect = \"fake\"\n"),
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Fake);

        std::fs::write(
            dir.join("node.toml"),
            format!("{base}wireguard_effect = \"simulated\"\n"),
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("unknown effect refused");
        assert!(err.contains("wireguard_effect"), "{err}");
    }

    #[test]
    fn overlay_advertised_derives_the_ula_and_requires_v6_listen() {
        let dir = tmp("overlay-advertised");
        let base = "id = 1\nnamespace = \"demo\"\npeer_seeds = [0, 1]\n\
                    bootstrapper_addr = \"127.0.0.1:52240\"\n";
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}listen = \"[::]:52241\"\nadvertised = \"overlay\"\n"),
        )
        .expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        let identity = wireguard_upgrade::ValidatorIdentity::try_from(
            ed25519::PrivateKey::from_seed(1).public_key().as_ref(),
        )
        .unwrap();
        let ula = wireguard_upgrade::ula_v6_member_addr("demo", identity);
        assert_eq!(
            r.advertised,
            Ingress::Socket(SocketAddr::new(std::net::IpAddr::V6(ula), 52241)),
            "the overlay sentinel advertises the chain-derived ULA at the listen port"
        );

        // the overlay is v6: a v4-only listener would never see tunnel SYNs.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}listen = \"0.0.0.0:52241\"\nadvertised = \"overlay\"\n"),
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("v4 listener refused");
        assert!(err.contains("IPv6"), "{err}");
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
}
