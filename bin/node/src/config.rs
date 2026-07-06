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

/// the invite blob prefix; versioned so a stale-format paste fails loudly. the
/// prefix stays "v2" while the PAYLOAD version byte (2 or 3) is authoritative:
/// v2 = descriptor only (manual flow), v3 = descriptor + reach hints + an
/// appended lobby token. a decoded v3 descriptor carries typed `reach` hints;
/// admission is still a member ballot, so the blob is a low-trust doorbell —
/// its integrity comes from the genesis fingerprint, not a signature.
const INVITE_PREFIX: &str = "ducktape-invite-v2:";

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

    /// bootstrap entries as dial INGRESSES. an IP literal becomes a socket
    /// ingress; a HOSTNAME stays a hostname (`Ingress::Dns`) and is re-resolved
    /// by the dialer at EVERY attempt — so `pubkey@node.example.com:443` keeps
    /// working when the tunnel behind the name moves, and an offline name never
    /// blocks startup. a MALFORMED entry (no `@`, bad key, not host:port) is a
    /// config error; an unspecified ip / port 0 is advisory and skipped. the
    /// live dial path is [`NetworkDescriptor::reach_entries`], which folds these
    /// Direct entries in alongside the typed `reach` hints.
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
        // the UNION of typed `reach` and `bootstrap`, keyed by expected pubkey:
        // a typed reach hint wins over a bootstrap-synthesised Direct for the
        // same member, but a member present only in `bootstrap` (e.g. a
        // second-generation inviter that ran `add_bootstrap`) is never dropped.
        // returning `reach` XOR `bootstrap` would silently lose that inviter's
        // own dial hint from every re-issued invite.
        let mut by_key: std::collections::BTreeMap<Vec<u8>, ReachHint> =
            std::collections::BTreeMap::new();
        for entry in &self.bootstrap {
            let (k, addr) = entry
                .split_once('@')
                .ok_or_else(|| format!("bootstrap entry {entry:?} is not pubkey@addr"))?;
            let expected_key = decode_key(k)?;
            by_key.insert(
                expected_key.as_ref().to_vec(),
                ReachHint { expected_key, reach: Reach::Direct(addr.to_string()) },
            );
        }
        for s in &self.reach {
            let hint = ReachHint::parse(s)?;
            by_key.insert(hint.expected_key.as_ref().to_vec(), hint);
        }
        Ok(by_key.into_values().collect())
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
// invite tokens — the bearer credential a v3 invite blob carries. minted by a
// member (`invite`), presented by the joiner's parked node over the lobby
// channel, verified by each RECEIVING member node before it records the join
// request for manual approval. the token authenticates that an announce comes
// from a real invitation (and names the inviter); it does NOT admit by itself
// — admission stays a member decision through the normal governance ballots.
// ============================================================================

/// ed25519 signing namespace for the grant an issuer mints:
/// `sign(INVITE_GRANT_NAMESPACE, binding ‖ nonce)`.
pub const INVITE_GRANT_NAMESPACE: &[u8] = b"ducktape-invite-grant-v1";
/// ed25519 signing namespace for the joiner's proof-of-possession:
/// `sign(INVITE_JOIN_NAMESPACE, binding ‖ nonce ‖ joiner)`.
pub const INVITE_JOIN_NAMESPACE: &[u8] = b"ducktape-invite-join-v1";
/// invite token nonce width in bytes.
pub const INVITE_NONCE_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct InviteToken {
    /// the minting member — checked against CURRENT membership on receipt.
    pub issuer: ed25519::PublicKey,
    /// per-invite randomness: distinguishes tokens, keys announce dedup.
    pub nonce: [u8; INVITE_NONCE_LEN],
    /// issuer's signature over `binding ‖ nonce` in the invite-grant namespace.
    pub sig: ed25519::Signature,
}

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

/// the joiner's proof-of-possession over its own key for `token` — binds the
/// announced pubkey to someone actually holding its secret, so a blob holder
/// cannot park a join request under a key that never asked to join.
pub fn sign_join_proof(
    joiner: &ed25519::PrivateKey,
    binding: &[u8],
    token: &InviteToken,
) -> ed25519::Signature {
    let msg = [
        binding,
        token.nonce.as_slice(),
        joiner.public_key().as_ref(),
    ]
    .concat();
    joiner.sign(INVITE_JOIN_NAMESPACE, &msg)
}

/// verify a token on receipt: issuer signature over `binding ‖ nonce`.
pub fn verify_invite_token(token: &InviteToken, binding: &[u8]) -> bool {
    use commonware_cryptography::Verifier as _;
    let msg = [binding, token.nonce.as_slice()].concat();
    token
        .issuer
        .verify(INVITE_GRANT_NAMESPACE, &msg, &token.sig)
}

/// verify a joiner's proof-of-possession against `token`.
pub fn verify_join_proof(
    joiner: &ed25519::PublicKey,
    binding: &[u8],
    token: &InviteToken,
    proof: &ed25519::Signature,
) -> bool {
    use commonware_cryptography::Verifier as _;
    let msg = [binding, token.nonce.as_slice(), joiner.as_ref()].concat();
    joiner.verify(INVITE_JOIN_NAMESPACE, &msg, proof)
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
// v2 is REJECTED on decode (SECURITY, Slice 1 review: a v2 blob is unsigned, so
// trusting one would let an attacker downgrade past v3's signature and persist an
// arbitrary descriptor). the production v2 parser is gone; only a test-only v2
// encoder survives, to prove the join path refuses a well-formed v2 blob. v3 and
// v2 stay non-confusable — distinct prefix AND version byte. flag-day: v1 AND v2
// blobs no longer decode; v3 is the only invite a joiner trusts.
// ============================================================================

/// invite payload format tags (the first packed byte). v2 = descriptor only
/// (the manual flow: the joiner's key travels out-of-band and a member runs
/// `invite-accept`); v3 = descriptor + typed reach hints + an appended
/// [`InviteToken`] — the bearer doorbell that lets the joiner announce itself
/// over the lobby channel. neither is signed: the blob is a low-trust invite
/// whose integrity is the genesis fingerprint, and admission stays a ballot.
const INVITE_VERSION_V2: u8 = 2;
const INVITE_VERSION_V3: u8 = 3;

const INVITE_B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// encode an invite blob: v3 when a token rides along (carrying reach hints),
/// v2 (descriptor only, manual flow) when not.
pub fn encode_invite(
    descriptor: &NetworkDescriptor,
    token: Option<&InviteToken>,
) -> Result<String, String> {
    use base64::Engine as _;
    Ok(format!(
        "{INVITE_PREFIX}{}",
        INVITE_B64.encode(pack_invite(descriptor, token)?)
    ))
}

/// decode an invite blob; the token is `None` for a v2 (manual-flow) blob.
pub fn decode_invite(blob: &str) -> Result<(NetworkDescriptor, Option<InviteToken>), String> {
    use base64::Engine as _;
    let body = blob
        .trim()
        .strip_prefix(INVITE_PREFIX)
        .ok_or_else(|| format!("not a ducktape invite (expected {INVITE_PREFIX}...)"))?;
    let bytes = INVITE_B64
        .decode(body)
        .map_err(|e| format!("invite is not valid base64url: {e}"))?;
    unpack_invite(&bytes)
}

/// pack a descriptor into the compact v2/v3 payload. validator hex is decoded
/// to raw keys (rejecting a malformed descriptor here rather than shipping it);
/// the typed reach hints come from [`NetworkDescriptor::reach_hints`] (the
/// union of `reach` and `bootstrap`-synthesised Direct hints), so a founder
/// that only ever ran `add_bootstrap` still ships a well-formed invite. a v3
/// payload appends the lobby [`InviteToken`]; nothing is signed.
fn pack_invite(d: &NetworkDescriptor, token: Option<&InviteToken>) -> Result<Vec<u8>, String> {
    let mut out = vec![if token.is_some() {
        INVITE_VERSION_V3
    } else {
        INVITE_VERSION_V2
    }];

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
    if let Some(t) = token {
        out.extend_from_slice(&pack_invite_token(t));
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

/// inverse of [`pack_invite`]; yields a descriptor canonicalized exactly as
/// [`NetworkDescriptor::from_toml`] would (sorted validators, sorted canonical
/// reach) so the genesis fingerprint of a decoded invite matches the founder's.
/// a v3 payload also carries the lobby [`InviteToken`]; nothing is signed — the
/// blob's integrity is the genesis fingerprint and admission is a ballot.
fn unpack_invite(bytes: &[u8]) -> Result<(NetworkDescriptor, Option<InviteToken>), String> {
    let mut r = InviteReader::new(bytes);
    let version = r.u8()?;
    if version != INVITE_VERSION_V2 && version != INVITE_VERSION_V3 {
        return Err(format!(
            "unsupported invite version {version} (this build reads v{INVITE_VERSION_V2} and \
             v{INVITE_VERSION_V3})"
        ));
    }
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
    // a v3 payload carries the lobby token; v2 stops after the descriptor.
    let token = if version == INVITE_VERSION_V3 {
        Some(unpack_invite_token(r.take(INVITE_TOKEN_LEN)?)?)
    } else {
        None
    };
    if !r.done() {
        return Err("invite payload has trailing bytes".into());
    }
    Ok((
        NetworkDescriptor {
            chain_id,
            scheme: SCHEME_ED25519.into(),
            validators,
            // a decoded invite carries dial hints as typed `reach`; `bootstrap`
            // stays empty and both feed one dial source via `reach_hints`.
            bootstrap: Vec::new(),
            reach,
        },
        token,
    ))
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
    /// which `WireGuardEffect` the reachability plane drives: "real"
    /// (default — configure an actual interface via the userspace WireGuard
    /// runtime; needs root/CAP_NET_ADMIN) or "fake" (record configs in
    /// memory; for dev/sim runs, and for several same-chain nodes on one
    /// host, which would otherwise fight over one interface name).
    pub wireguard_effect: Option<String>,
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
pub fn choose_sync_source<A>(
    bootstrappers: &[(ed25519::PublicKey, A)],
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
    /// opt-in shipped-index warm start when joining; see `NodeToml::sync_index`.
    pub sync_index: bool,
    /// publish the discovered provider set into the capability registry; see
    /// `NodeToml::announce_capabilities`.
    pub announce_capabilities: bool,
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

    Ok(Resolved {
        label: hex_bytes(&me.as_ref()[..4]),
        namespace: descriptor.genesis_namespace().into_bytes(),
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
        dev_demo: false,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        invite_token: load_invite_token(base)?,
        sync_index: raw.sync_index.unwrap_or(false),
        announce_capabilities: raw.announce_capabilities.unwrap_or(true),
    })
}

fn parse_wireguard_listen(raw: Option<&str>) -> Result<Option<SocketAddr>, String> {
    raw.map(|a| {
        a.parse::<SocketAddr>()
            .map_err(|e| format!("wireguard_listen: {e}"))
    })
    .transpose()
}

/// which `WireGuardEffect` implementation the reachability plane drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireGuardEffectKind {
    /// configure an actual interface through the userspace WireGuard runtime.
    Real,
    /// record configurations in memory without touching the network stack.
    Fake,
}

fn parse_wireguard_effect(raw: Option<&str>) -> Result<WireGuardEffectKind, String> {
    match raw {
        None | Some("real") => Ok(WireGuardEffectKind::Real),
        Some("fake") => Ok(WireGuardEffectKind::Fake),
        Some(other) => Err(format!(
            "wireguard_effect: {other:?} is not \"real\" or \"fake\""
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
        storage_dir,
        rpc_listen: raw.rpc_listen,
        http_listen: raw.http_listen,
        wireguard_listen,
        wireguard_effect,
        dev_demo: true,
        checkpoint_blocks: raw.checkpoint_blocks.unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        invite_token: None,
        sync_index: raw.sync_index.unwrap_or(false),
        announce_capabilities: raw.announce_capabilities.unwrap_or(true),
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
    fn invite_blob_roundtrips_the_descriptor() {
        let me = ed25519::PrivateKey::from_seed(7).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(me.as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        d.add_bootstrap(&me, "127.0.0.1:52200");
        // token-less blob is the v2 manual flow. decode NORMALISES bootstrap
        // hints into typed Direct reach hints, so the decoded descriptor dials
        // the same members via `reach` even though it carries no `bootstrap`.
        let (decoded, token) =
            decode_invite(&encode_invite(&d, None).expect("encode")).expect("roundtrip");
        assert_eq!(token, None, "a token-less blob is the v2 manual flow");
        assert_eq!(
            decoded.reach_hints().expect("decoded hints"),
            d.reach_hints().expect("source hints"),
            "the dial hints survive as typed reach hints"
        );

        // a HOSTNAME dial hint survives verbatim (stored as a string, resolved
        // only at dial time — never at encode/decode).
        let other = ed25519::PrivateKey::from_seed(8).public_key();
        d.add_bootstrap(&other, "node.ducktape.industries:443");
        let (decoded, _) =
            decode_invite(&encode_invite(&d, None).expect("encode")).expect("roundtrip");
        assert!(
            decoded
                .reach
                .iter()
                .any(|r| r.ends_with("@node.ducktape.industries:443"))
        );
    }

    #[test]
    fn invite_blob_roundtrips_the_token_and_verifies() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
        };
        let binding = d.genesis_namespace();
        let token = mint_invite_token(&issuer, binding.as_bytes());
        let (decoded, carried) =
            decode_invite(&encode_invite(&d, Some(&token)).expect("encode")).expect("roundtrip");
        assert_eq!(decoded, d);
        let carried = carried.expect("v3 carries the token");
        assert_eq!(carried, token);
        assert!(verify_invite_token(&carried, binding.as_bytes()));
        assert!(
            !verify_invite_token(&carried, b"other-net"),
            "a token binds to its network"
        );

        // the joiner's proof-of-possession verifies for the signing key only.
        let joiner = ed25519::PrivateKey::from_seed(9);
        let proof = sign_join_proof(&joiner, binding.as_bytes(), &carried);
        assert!(verify_join_proof(
            &joiner.public_key(),
            binding.as_bytes(),
            &carried,
            &proof
        ));
        let thief = ed25519::PrivateKey::from_seed(10).public_key();
        assert!(
            !verify_join_proof(&thief, binding.as_bytes(), &carried, &proof),
            "a substituted key fails the proof"
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
        // validators + the derived lobby identity (the join-request door).
        assert_eq!(r.mesh.len(), 3);
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
            reach: vec![],
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
        assert_eq!(choose_sync_source(no_hints, &[me.clone()], &me), None);
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
    fn wireguard_effect_defaults_real_and_rejects_unknown_values() {
        let dir = tmp("wgeffect");
        let base = "id = 0\nlisten = \"127.0.0.1:52230\"\nnamespace = \"demo\"\npeer_seeds = [0]\n";
        std::fs::write(dir.join("node.toml"), base).expect("write");
        let r = resolve(&dir.join("node.toml")).expect("resolve");
        assert_eq!(r.wireguard_effect, WireGuardEffectKind::Real);

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
