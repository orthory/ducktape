//! invite tokens and the signed invite blob — the whole join credential,
//! plus the on-disk token/bootstrap files a `join` persists.

use std::path::Path;

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519};
use serde::{Deserialize, Serialize};

use super::{
    CoordRef, Coordination, NetworkDescriptor, Reach, ReachHint, SCHEME_ED25519, decode_key,
    hex_bytes, unhex,
};

/// the invite blob prefix. UNVERSIONED on purpose (bootstrapping posture): the
/// network re-mints invites on a format change, and a stale paste fails loudly
/// at decode — the old `ducktape:` / `ducktape-invite-v*:` prefixes no longer
/// decode at all.
const INVITE_PREFIX: &str = "🦆";

/// Joining through coordinated reach needs the local reachability plane even
/// when the invite does not contain a direct inviter-hosted tunnel bootstrap.
pub fn invite_requires_reachability_defaults(invite: &Invite) -> bool {
    invite.wireguard.is_some() || invite.descriptor.has_coordinated_reach().unwrap_or(false)
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
    INVITE_GRANT_NAMESPACE, INVITE_NONCE_LEN, InviteRole, InviteToken, sign_join_proof,
    verify_invite_token, verify_join_proof,
};

/// mint a token binding an invite to `binding` (the genesis namespace) and to
/// the single `target` key it admits, with `role` and `expires_unix_secs`:
/// fresh OS randomness for the nonce, signed by this member's identity. minting
/// IS the admission decision FOR THAT KEY.
pub fn mint_invite_token(
    signer: &ed25519::PrivateKey,
    binding: &[u8],
    target: &ed25519::PublicKey,
    role: InviteRole,
    expires_unix_secs: u64,
) -> InviteToken {
    let mut nonce = [0u8; INVITE_NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let msg = [
        binding,
        &nonce,
        target.as_ref(),
        &[role.as_u8()],
        &expires_unix_secs.to_le_bytes(),
    ]
    .concat();
    InviteToken {
        issuer: signer.public_key(),
        nonce,
        target: target.clone(),
        role,
        expires_unix_secs,
        sig: signer.sign(INVITE_GRANT_NAMESPACE, &msg),
    }
}

const INVITE_TOKEN_FILE: &str = "invite.token";
/// packed token: `issuer(32) ‖ nonce(16) ‖ target(32) ‖ role(1) ‖ expires_le(8)
/// ‖ sig(64)` = 153 bytes.
const INVITE_TOKEN_LEN: usize = 32 + INVITE_NONCE_LEN + 32 + 1 + 8 + 64;

fn pack_invite_token(t: &InviteToken) -> Vec<u8> {
    let mut out = Vec::with_capacity(INVITE_TOKEN_LEN);
    out.extend_from_slice(t.issuer.as_ref());
    out.extend_from_slice(&t.nonce);
    out.extend_from_slice(t.target.as_ref());
    out.push(t.role.as_u8());
    out.extend_from_slice(&t.expires_unix_secs.to_le_bytes());
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
    let mut pos = 32 + INVITE_NONCE_LEN;
    let target = ed25519::PublicKey::decode(&bytes[pos..pos + 32])
        .map_err(|e| format!("invite token target: {e}"))?;
    pos += 32;
    let role = InviteRole::from_u8(bytes[pos])?;
    pos += 1;
    let expires_unix_secs = u64::from_le_bytes(bytes[pos..pos + 8].try_into().expect("8 bytes"));
    pos += 8;
    let sig = ed25519::Signature::decode(&bytes[pos..])
        .map_err(|e| format!("invite token signature: {e}"))?;
    Ok(InviteToken {
        issuer,
        nonce,
        target,
        role,
        expires_unix_secs,
        sig,
    })
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

/// delete the stored invite token — a CONSUMED credential must not survive to
/// confuse a later boot (ADR §6). called once the join gate reports `Admitted`:
/// the invite has done its one job. best-effort; a missing file is success.
pub fn delete_invite_token(dir: &Path) {
    let path = dir.join(INVITE_TOKEN_FILE);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => eprintln!("warning: could not delete spent invite token {path:?}: {e}"),
    }
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
    adverts: &[wireguard::EndpointAdvertisement],
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
    signer: &ed25519::PrivateKey,
) -> Result<String, String> {
    use base64::Engine as _;
    if signer.public_key() != token.issuer {
        return Err("invite envelope must be signed by the token's issuer".into());
    }
    let mut out = pack_invite(descriptor, token, wireguard, fronts)?;
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
                 ducktape:/ducktape-invite-v*: blob no longer decodes — ask for a \
                 fresh invite"
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

// ============================================================================
// short invites — `🦆://<name>/<id>`. The full blob is published to the
// coordinator by content id (PR2); the short URL carries only the network name
// and that id. The blob stays self-authenticating, so the coordinator is
// untrusted storage: a joiner fetches the bytes back, re-wraps them, and runs
// the SAME `decode_invite` verification the full-blob path does.
// ============================================================================

/// the short-invite scheme. Note it BEGINS with [`INVITE_PREFIX`] (`🦆`), so a
/// short URL and a full blob are distinguished by the `://` — always try
/// [`parse_short_invite`] before [`decode_invite`].
pub const SHORT_INVITE_SCHEME: &str = "🦆://";

/// `🦆://<name>/<base64url(id)>` — `name` is the chain_id's human half (the
/// part before `#<salt>`).
pub fn short_invite_url(chain_id: &str, id: &[u8; 16]) -> String {
    use base64::Engine as _;
    let name = chain_id.split('#').next().unwrap_or(chain_id);
    format!("{SHORT_INVITE_SCHEME}{name}/{}", INVITE_B64.encode(id))
}

/// parse a short url -> `(name, id)`. `None` when `s` is not the short scheme
/// or the id is not exactly 16 base64url bytes.
pub fn parse_short_invite(s: &str) -> Option<(String, [u8; 16])> {
    use base64::Engine as _;
    let rest = s.trim().strip_prefix(SHORT_INVITE_SCHEME)?;
    let (name, id_b64) = rest.split_once('/')?;
    if name.is_empty() {
        return None;
    }
    let id: [u8; 16] = INVITE_B64.decode(id_b64).ok()?.try_into().ok()?;
    Some((name.to_string(), id))
}

/// the decoded (raw) bytes of an encoded `🦆<base64>` blob — what the
/// coordinator shelves and what [`invite_blob_id`] hashes.
pub fn invite_blob_bytes(blob: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let body = blob
        .trim()
        .strip_prefix(INVITE_PREFIX)
        .ok_or("not a ducktape invite blob")?;
    INVITE_B64
        .decode(body)
        .map_err(|e| format!("invite is not valid base64url: {e}"))
}

/// re-wrap raw invite bytes (as fetched from the coordinator) into the
/// `🦆<base64>` blob form the normal [`decode_invite`] path consumes. Inverse
/// of [`invite_blob_bytes`].
pub fn wrap_invite_bytes(raw: &[u8]) -> String {
    use base64::Engine as _;
    format!("{INVITE_PREFIX}{}", INVITE_B64.encode(raw))
}

/// the content id of an encoded `🦆<base64>` blob: the coordinator's own
/// `invite_id` (sha256 of the decoded bytes, first 16). Using the shelf's
/// hasher guarantees the published id matches what the coordinator indexes.
pub fn invite_blob_id(blob: &str) -> Result<[u8; 16], String> {
    Ok(nat_traversal::invite_id(&invite_blob_bytes(blob)?))
}

/// a throwaway OS-random signer for read-only coordinator fetches (the short
/// invite fetch runs before the workspace identity exists). Mirrors the
/// identity-generation idiom in `config::identity`.
pub fn ephemeral_signer() -> ed25519::PrivateKey {
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut raw);
    ed25519::PrivateKey::decode(raw.as_slice()).expect("32 random bytes decode")
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

    // the token now carries its OWN expiry (no separate blob-level field) —
    // decode enforces it from `token.expires_unix_secs`.
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

    // the token carries its own expiry now (no separate blob-level field);
    // enforce it against the injected clock right after unpacking.
    let token = unpack_invite_token(r.take(INVITE_TOKEN_LEN)?)?;
    let expires_unix_secs = token.expires_unix_secs;
    if now_unix_secs >= expires_unix_secs {
        return Err("this invite has expired — ask for a fresh one".into());
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    /// mint + encode with the test defaults: issuer-signed, targeting the
    /// issuer's own key, Resident role, far-future expiry.
    fn encode_test_invite(
        d: &NetworkDescriptor,
        issuer: &ed25519::PrivateKey,
        wireguard: Option<&InviteWireGuard>,
    ) -> String {
        let token = mint_invite_token(
            issuer,
            d.genesis_namespace().as_bytes(),
            &issuer.public_key(),
            InviteRole::Resident,
            u64::MAX,
        );
        encode_invite(d, &token, wireguard, &[], issuer).expect("encode")
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
        let token = mint_invite_token(
            issuer,
            d.genesis_namespace().as_bytes(),
            &issuer.public_key(),
            InviteRole::Resident,
            u64::MAX,
        );
        encode_invite(d, &token, wireguard, fronts, issuer).expect("encode")
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
    ) -> wireguard::EndpointAdvertisement {
        use std::net::{IpAddr, Ipv4Addr};
        use wireguard::{
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
            expires_at_view: 50,
            nonce: 1,
        };
        wireguard::EndpointAdvertisement::sign(record, MeshVersion([7; 32]), &signer)
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
        // the token carries its OWN expiry now (no separate blob param).
        let token = mint_invite_token(
            &issuer,
            d.genesis_namespace().as_bytes(),
            &issuer.public_key(),
            InviteRole::Resident,
            1_000,
        );

        // expiry is enforced at decode, deterministically via the injected clock.
        let blob = encode_invite(&d, &token, None, &[], &issuer).expect("encode");
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
        assert!(encode_invite(&d, &token, None, &[], &outsider).is_err());

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
        let token = mint_invite_token(
            &issuer,
            d.genesis_namespace().as_bytes(),
            &issuer.public_key(),
            InviteRole::Resident,
            u64::MAX,
        );
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
        let blob = encode_invite(&d, &token, None, &fronts, &issuer).expect("encode");
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
        let token = mint_invite_token(
            &issuer,
            d.genesis_namespace().as_bytes(),
            &issuer.public_key(),
            InviteRole::Resident,
            u64::MAX,
        );

        let empty_blob = encode_invite(&d, &token, None, &[], &issuer).expect("encode");
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
            &issuer,
        )
        .expect("encode");
        assert!(
            with_front.len() > empty_blob.len(),
            "a front appends bytes the empty blob lacks"
        );
    }

    #[test]
    fn invite_token_file_roundtrips() {
        let dir = tmp("invitetoken");
        assert_eq!(load_invite_token(&dir).expect("absent is fine"), None);
        let issuer = ed25519::PrivateKey::from_seed(7);
        let token = mint_invite_token(
            &issuer,
            b"net#00000000@feedface",
            &issuer.public_key(),
            InviteRole::Resident,
            u64::MAX,
        );
        save_invite_token(&dir, &token).expect("save");
        assert_eq!(load_invite_token(&dir).expect("load"), Some(token));
    }

    #[test]
    fn short_invite_url_roundtrips_and_rejects_junk() {
        let id = [0x5a; 16];
        let url = short_invite_url("ducktape#a1b2c3d4", &id);
        assert!(url.starts_with("🦆://ducktape/"), "{url}");
        assert_eq!(parse_short_invite(&url), Some(("ducktape".into(), id)));
        // a full blob is NOT a short url; junk ids and junk schemes are None.
        assert_eq!(parse_short_invite("🦆AbCdEf"), None);
        assert_eq!(parse_short_invite("🦆://name/not-base64!!!"), None);
        assert_eq!(parse_short_invite("duck://name/AAAA"), None);
    }

    #[test]
    fn invite_blob_id_is_the_content_hash_of_the_decoded_bytes() {
        let issuer = ed25519::PrivateKey::from_seed(7);
        let d = front_test_descriptor(&issuer);
        let mint = || {
            mint_invite_token(
                &issuer,
                d.genesis_namespace().as_bytes(),
                &issuer.public_key(),
                InviteRole::Resident,
                u64::MAX,
            )
        };
        let blob = encode_invite(&d, &mint(), None, &[], &issuer).expect("encode");
        let id = invite_blob_id(&blob).expect("id");
        // stable under re-parse, changes when the blob changes (fresh nonce).
        assert_eq!(id, invite_blob_id(&blob).unwrap());
        let other = encode_invite(&d, &mint(), None, &[], &issuer).unwrap();
        assert_ne!(id, invite_blob_id(&other).unwrap());
        // the raw bytes re-wrap to the exact original blob (fetch/join path).
        assert_eq!(wrap_invite_bytes(&invite_blob_bytes(&blob).unwrap()), blob);
    }
}
