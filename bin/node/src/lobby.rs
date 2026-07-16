//! the join-gate wire format (Join Protocol v1, ADR §3) — how a not-yet-
//! admitted joiner asks to join, and how a gating member answers
//! AUTHORITATIVELY.
//!
//! transport: the joiner connects to the mesh AS the network's derived lobby
//! identity (see `config::lobby_identity`) — the one key every member tracks
//! that any invite holder can derive — and speaks on `CHANNEL_LOBBY`. the
//! lobby identity authenticates nothing; every claim in a [`GateMsg::Request`]
//! is verified against the INVITE TOKEN it carries (issuer signature over the
//! genesis namespace) and the joiner's proof-of-possession. UNLIKE the retired
//! advisory announce, the gate is synchronous: a member runs the V1–V9
//! checklist, settles `Redeem` through consensus, and its `Admitted` reply IS
//! the admission — pass the gate and you already hold standing, fail it and you
//! get nothing (no tunnel, no residence, no chain state).
//!
//! json on the wire: matches the module-interface idiom, and this lane is
//! low-volume (one Request per candidate per attempt).

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use serde::{Deserialize, Serialize};

use crate::config::{INVITE_NONCE_LEN, InviteRole, InviteToken};

/// why a gate refused, per ADR §4. wire-stable identifiers (the `detail` prose
/// is free to change; these are not). the terminal bit — whether the joiner
/// stops (exits) rather than failing over — is set by the member per the §3.1
/// checklist, not baked into the code, since `IssuerUnknown` is non-terminal
/// while every other code is terminal.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RejectCode {
    /// V1: the request did not decode / was not well-formed.
    BadEncoding,
    /// V2: the token signature does not verify for this network's binding.
    BadToken,
    /// V3: the announcing key is not the invite's `target`.
    NotTarget,
    /// V4: the invite expired against the member's wall clock.
    Expired,
    /// V5: the joiner's proof-of-possession does not verify.
    BadProof,
    /// V6 (or a consensus race): the invite nonce is already redeemed.
    Spent,
    /// V7: the issuer is not in this member's committed valset. NON-TERMINAL —
    /// a lagging view cannot tell removed from not-yet-seen; the joiner fails
    /// over to another member.
    IssuerUnknown,
    /// V8: the token's role is not redeemable this generation (`Client`).
    RoleUnsupported,
    /// §3.2: the member could not settle the gate in time (timeout / submit
    /// failure). NON-TERMINAL — the joiner tries another member.
    Busy,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GateMsg {
    /// joiner → member. "this key asks to join, invited by `issuer`" — the
    /// token's fields plus the joiner key and its proof-of-possession, all raw
    /// bytes. `target`, `role`, and `expires_unix_secs` are the token's covered
    /// fields; verify enforces `target == joiner`, so a blob holder cannot
    /// announce under a key the invite was not minted for.
    Request {
        issuer: Vec<u8>,
        nonce: Vec<u8>,
        token_sig: Vec<u8>,
        joiner: Vec<u8>,
        proof: Vec<u8>,
        target: Vec<u8>,
        role: u8,
        expires_unix_secs: u64,
    },
    /// member → joiner. the gate refused. `terminal` ⇒ the joiner STOPS
    /// (exits) instead of failing over to another member (ADR R2/§3.3).
    Rejected {
        code: RejectCode,
        detail: String,
        terminal: bool,
    },
    /// member → joiner. the AUTHORITATIVE admission: the member settled
    /// `Redeem` through consensus and it COMMITTED at `height` — the joiner now
    /// holds standing (ADR R3).
    ///
    /// `cap` carries an OPAQUE genesis-issued coordinator capability (packed
    /// `CoordCap` bytes) minted for the joiner when this network coordinates
    /// PRIVATELY and the answering member is a genesis validator — the joiner
    /// cannot receive it on the invite (its key does not exist at invite-mint
    /// time), so this reply is its only delivery channel. lobby.rs stays
    /// crypto-agnostic: it moves bytes and never depends on the cap types.
    Admitted {
        height: u64,
        cap: Option<Vec<u8>>,
    },
}

pub fn encode_msg(m: &GateMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<GateMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// build the gate request for `token` as `joiner` — the proof binds the
/// announced key to its secret holder.
pub fn gate_request(
    joiner: &ed25519::PrivateKey,
    binding: &[u8],
    token: &InviteToken,
) -> GateMsg {
    use commonware_codec::Encode as _;
    use commonware_cryptography::Signer as _;
    let proof = crate::config::sign_join_proof(joiner, binding, token);
    GateMsg::Request {
        issuer: token.issuer.as_ref().to_vec(),
        nonce: token.nonce.to_vec(),
        token_sig: token.sig.encode().as_ref().to_vec(),
        joiner: joiner.public_key().as_ref().to_vec(),
        proof: proof.encode().as_ref().to_vec(),
        // empty target bytes = bearer, the wire-wide rule.
        target: token
            .target
            .as_ref()
            .map(|t| t.as_ref().to_vec())
            .unwrap_or_default(),
        role: token.role.as_u8(),
        expires_unix_secs: token.expires_unix_secs,
    }
}

/// map a [`verify_join_request`] error to its ADR §3.1 reject code (V1–V3/V5).
/// every crypto/decode failure is TERMINAL; the caller sets the bit.
pub fn verify_reject_code(err: &str) -> RejectCode {
    if err.contains("does not verify for this network") {
        RejectCode::BadToken // V2
    } else if err.contains("locked to a different key") {
        RejectCode::NotTarget // V3
    } else if err.contains("proof-of-possession") {
        RejectCode::BadProof // V5
    } else {
        RejectCode::BadEncoding // V1: malformed key/nonce/role/signature bytes
    }
}

/// map a governance `Redeem` consensus-reject reason (ADR §3.2) to a gate
/// reject code + terminal bit. these fire only on a race that slips past the
/// member's V1–V8 pre-filter (chiefly a nonce another joiner redeemed first).
/// an unrecognized reason is a transient `Busy` (non-terminal) rather than a
/// permanent kill — the joiner retries and re-runs the checklist.
pub fn redeem_reject_outcome(reason: Option<&str>) -> (RejectCode, bool) {
    let r = reason.unwrap_or("");
    if r.contains("already redeemed") {
        (RejectCode::Spent, true)
    } else if r.contains("locked to another key") {
        (RejectCode::NotTarget, true)
    } else if r.contains("proof-of-possession") {
        (RejectCode::BadProof, true)
    } else if r.contains("does not verify for this network") {
        (RejectCode::BadToken, true)
    } else if r.contains("client invites are not redeemable") {
        (RejectCode::RoleUnsupported, true)
    } else if r.contains("no longer part of this network") {
        (RejectCode::IssuerUnknown, false) // V7 stays non-terminal
    } else {
        // "already a validator" / "already holds resident standing" (the joiner
        // gained standing between the V9 check and the drain) or any unknown
        // reason: let it retry — it hits the V9 idempotent Admitted next round.
        (RejectCode::Busy, false)
    }
}

/// a decoded, signature-verified join request — what a member records for
/// approval. `verify_join_request` is the only constructor.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedJoinRequest {
    pub joiner: ed25519::PublicKey,
    pub issuer: ed25519::PublicKey,
    pub nonce: [u8; INVITE_NONCE_LEN],
    /// the token's role — `joiner` IS the target (verify enforced it), so no
    /// separate target field is carried.
    pub role: InviteRole,
    /// the token's unix-seconds expiry, carried through to the redeem op.
    pub expires_unix_secs: u64,
}

/// verify a wire join request against this network's binding: token issuer
/// signature and the joiner's proof-of-possession. membership checks (issuer
/// still a member? joiner already one?) are the CALLER's — they need current
/// state, this needs only crypto.
pub fn verify_join_request(msg: &GateMsg, binding: &[u8]) -> Result<VerifiedJoinRequest, String> {
    let GateMsg::Request {
        issuer,
        nonce,
        token_sig,
        joiner,
        proof,
        target,
        role,
        expires_unix_secs,
    } = msg
    else {
        return Err("not a join request".into());
    };
    let issuer = ed25519::PublicKey::decode(issuer.as_slice())
        .map_err(|e| format!("issuer key: {e}"))?;
    let joiner = ed25519::PublicKey::decode(joiner.as_slice())
        .map_err(|e| format!("joiner key: {e}"))?;
    // empty target bytes = a BEARER token (Client-role-only by mint rule; a
    // bearer Resident could only be a forgery and dies on the signature).
    let target = if target.is_empty() {
        None
    } else {
        Some(
            ed25519::PublicKey::decode(target.as_slice())
                .map_err(|e| format!("target key: {e}"))?,
        )
    };
    let role = InviteRole::from_u8(*role)?;
    if nonce.len() != INVITE_NONCE_LEN {
        return Err(format!("nonce must be {INVITE_NONCE_LEN} bytes"));
    }
    let mut nonce_arr = [0u8; INVITE_NONCE_LEN];
    nonce_arr.copy_from_slice(nonce);
    let sig = ed25519::Signature::decode(token_sig.as_slice())
        .map_err(|e| format!("token signature: {e}"))?;
    let proof = ed25519::Signature::decode(proof.as_slice())
        .map_err(|e| format!("join proof: {e}"))?;

    let token = InviteToken {
        issuer: issuer.clone(),
        nonce: nonce_arr,
        target: target.clone(),
        role,
        expires_unix_secs: *expires_unix_secs,
        sig,
    };
    // signature first (kills a tampered target/role/expiry), THEN the target
    // lock (named BEFORE the proof check so the error names the real problem:
    // a blob holder announcing under its own valid self-proof). a BEARER
    // token has no lock — any key may claim it; the ROLE gates downstream
    // (ingress V8, the intro doorbell) are what keep it off the resident
    // plane.
    if !crate::config::verify_invite_token(&token, binding) {
        return Err("invite token signature does not verify for this network".into());
    }
    if let Some(t) = &token.target
        && *t != joiner
    {
        return Err(
            "invite is locked to a different key — this invite was minted for someone else".into(),
        );
    }
    // NO expiry check here: the lobby fn stays pure crypto (same division as
    // the membership checks) — decode enforces wall-clock expiry, consensus
    // enforces block-time expiry.
    if !crate::config::verify_join_proof(&joiner, binding, &token, &proof) {
        return Err("joiner proof-of-possession does not verify".into());
    }
    Ok(VerifiedJoinRequest {
        joiner,
        issuer,
        nonce: nonce_arr,
        role,
        expires_unix_secs: *expires_unix_secs,
    })
}

// ============================================================================
// the UDP intro — the same trust dance as the lobby announce, one transport
// earlier: a fresh joiner has NO path to the mesh yet (that is what the
// tunnel is for), so it announces its keys in a single datagram to the
// inviter's intro listener. the token authenticates the request (mint was
// the admission decision), the join proof binds the announced ed25519 key to
// its holder, and a third signature binds the announced WIREGUARD key to
// that same identity — the receiving node then installs the tunnel peer and
// acks. everything after (lobby announce, redemption, statesync) rides the
// tunnel-borne mesh.
// ============================================================================

/// ed25519 signing namespace binding the announced X25519 WireGuard key to
/// the joiner identity: `sign(INTRO_WG_NAMESPACE, binding ‖ nonce ‖ wg_key)`.
pub const INTRO_WG_NAMESPACE: &[u8] = b"ducktape-invite-intro-v1";

/// the joiner's first-contact datagram. carries the whole lobby
/// [`GateMsg::Request`] payload plus the WireGuard half.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IntroRequest {
    pub issuer: Vec<u8>,
    pub nonce: Vec<u8>,
    pub token_sig: Vec<u8>,
    pub joiner: Vec<u8>,
    pub proof: Vec<u8>,
    pub target: Vec<u8>,
    pub role: u8,
    pub expires_unix_secs: u64,
    /// the joiner's X25519 WireGuard public key, raw.
    pub wg_public_key: Vec<u8>,
    /// the joiner's signature binding `wg_public_key` to its identity.
    pub wg_sig: Vec<u8>,
}

/// the inviter's answer: the nonce echoes so the joiner matches the ack to
/// its request; `installed` is false with a reason when the tunnel could not
/// be brought up.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IntroAck {
    pub nonce: Vec<u8>,
    pub installed: bool,
    pub detail: String,
}

pub fn encode_intro(m: &IntroRequest) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_intro(b: &[u8]) -> Result<IntroRequest, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_intro_ack(m: &IntroAck) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_intro_ack(b: &[u8]) -> Result<IntroAck, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// build the intro for `token` as `joiner`, announcing `wg_public_key`.
pub fn intro_request(
    joiner: &ed25519::PrivateKey,
    binding: &[u8],
    token: &InviteToken,
    wg_public_key: [u8; 32],
) -> IntroRequest {
    use commonware_codec::Encode as _;
    use commonware_cryptography::Signer as _;
    let proof = crate::config::sign_join_proof(joiner, binding, token);
    let wg_msg = [binding, token.nonce.as_slice(), &wg_public_key].concat();
    let wg_sig = joiner.sign(INTRO_WG_NAMESPACE, &wg_msg);
    IntroRequest {
        issuer: token.issuer.as_ref().to_vec(),
        nonce: token.nonce.to_vec(),
        token_sig: token.sig.encode().as_ref().to_vec(),
        joiner: joiner.public_key().as_ref().to_vec(),
        proof: proof.encode().as_ref().to_vec(),
        target: token
            .target
            .as_ref()
            .map(|t| t.as_ref().to_vec())
            .unwrap_or_default(),
        role: token.role.as_u8(),
        expires_unix_secs: token.expires_unix_secs,
        wg_public_key: wg_public_key.to_vec(),
        wg_sig: wg_sig.encode().as_ref().to_vec(),
    }
}

/// a decoded, signature-verified intro — the only constructor is
/// [`verify_intro`].
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedIntro {
    pub joiner: ed25519::PublicKey,
    pub issuer: ed25519::PublicKey,
    pub nonce: [u8; INVITE_NONCE_LEN],
    pub wg_public_key: [u8; 32],
}

/// verify an intro against this network's binding: the token issuer
/// signature, the joiner's proof-of-possession, and the WireGuard-key
/// binding signature. membership checks are the CALLER's, exactly like
/// [`verify_join_request`].
pub fn verify_intro(msg: &IntroRequest, binding: &[u8]) -> Result<VerifiedIntro, String> {
    use commonware_cryptography::Verifier as _;
    let verified = verify_join_request(
        &GateMsg::Request {
            issuer: msg.issuer.clone(),
            nonce: msg.nonce.clone(),
            token_sig: msg.token_sig.clone(),
            joiner: msg.joiner.clone(),
            proof: msg.proof.clone(),
            target: msg.target.clone(),
            role: msg.role,
            expires_unix_secs: msg.expires_unix_secs,
        },
        binding,
    )?;
    let wg_public_key: [u8; 32] = msg
        .wg_public_key
        .as_slice()
        .try_into()
        .map_err(|_| "wireguard key must be 32 bytes".to_string())?;
    let wg_sig = ed25519::Signature::decode(msg.wg_sig.as_slice())
        .map_err(|e| format!("wireguard key signature: {e}"))?;
    let wg_msg = [binding, verified.nonce.as_slice(), &wg_public_key].concat();
    if !verified
        .joiner
        .verify(INTRO_WG_NAMESPACE, &wg_msg, &wg_sig)
    {
        return Err("wireguard key binding does not verify".into());
    }
    Ok(VerifiedIntro {
        joiner: verified.joiner,
        issuer: verified.issuer,
        nonce: verified.nonce,
        wg_public_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::mint_invite_token;
    use commonware_cryptography::Signer as _;

    const BINDING: &[u8] = b"net#00000000@feedface";

    /// mint targeting `target` with the far-future Resident defaults the lobby
    /// tests use.
    fn mint_for(
        issuer: &ed25519::PrivateKey,
        target: &ed25519::PublicKey,
    ) -> InviteToken {
        mint_invite_token(issuer, BINDING, target, InviteRole::Resident, u64::MAX)
    }

    #[test]
    fn a_join_request_roundtrips_and_verifies() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint_for(&issuer, &joiner.public_key());
        let msg = gate_request(&joiner, BINDING, &token);
        let decoded = decode_msg(&encode_msg(&msg)).expect("roundtrip");
        assert_eq!(decoded, msg);

        let verified = verify_join_request(&decoded, BINDING).expect("verifies");
        assert_eq!(verified.joiner, joiner.public_key());
        assert_eq!(verified.issuer, issuer.public_key());
        assert_eq!(verified.nonce, token.nonce);
        assert_eq!(verified.role, InviteRole::Resident);
        assert_eq!(verified.expires_unix_secs, u64::MAX);
    }

    #[test]
    fn a_non_target_key_is_refused_by_name() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let target = ed25519::PrivateKey::from_seed(2);
        let thief = ed25519::PrivateKey::from_seed(3);
        let token = mint_for(&issuer, &target.public_key());
        // the thief holds the blob and announces under its OWN key with a VALID
        // self-proof — exactly the bearer hole this feature closes.
        let msg = gate_request(&thief, BINDING, &token);
        let err = verify_join_request(&msg, BINDING).expect_err("refused");
        assert!(err.contains("locked to a different key"), "{err}");
        // the real target still verifies.
        let msg = gate_request(&target, BINDING, &token);
        assert!(verify_join_request(&msg, BINDING).is_ok());
    }

    #[test]
    fn a_non_target_key_is_refused_by_name_over_the_intro() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let target = ed25519::PrivateKey::from_seed(2);
        let thief = ed25519::PrivateKey::from_seed(3);
        let token = mint_for(&issuer, &target.public_key());
        let msg = intro_request(&thief, BINDING, &token, [9u8; 32]);
        let err = verify_intro(&msg, BINDING).expect_err("refused");
        assert!(err.contains("locked to a different key"), "{err}");
        let msg = intro_request(&target, BINDING, &token, [9u8; 32]);
        assert!(verify_intro(&msg, BINDING).is_ok());
    }

    #[test]
    fn a_foreign_binding_or_forged_proof_is_refused() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint_for(&issuer, &joiner.public_key());
        let msg = gate_request(&joiner, BINDING, &token);

        // another network refuses the same announce.
        assert!(verify_join_request(&msg, b"other-net").is_err());

        // target == joiner (the lock passes), but a proof signed by a DIFFERENT
        // key fails the proof-of-possession. (a substituted JOINER key is the
        // target-lock case, covered by `a_non_target_key_is_refused_by_name`.)
        let GateMsg::Request {
            issuer: i,
            nonce,
            token_sig,
            joiner: j,
            target,
            role,
            expires_unix_secs,
            ..
        } = msg
        else {
            unreachable!()
        };
        let bad_proof = crate::config::sign_join_proof(
            &ed25519::PrivateKey::from_seed(3),
            BINDING,
            &token,
        );
        use commonware_codec::Encode as _;
        let forged = GateMsg::Request {
            issuer: i,
            nonce,
            token_sig,
            joiner: j,
            proof: bad_proof.encode().as_ref().to_vec(),
            target,
            role,
            expires_unix_secs,
        };
        let err = verify_join_request(&forged, BINDING).expect_err("refused");
        assert!(err.contains("proof-of-possession"), "{err}");
    }

    #[test]
    fn a_bearer_token_at_the_gate_verifies_as_client_and_dies_on_the_role_gates() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = crate::config::mint_bearer_client_token(&issuer, BINDING, u64::MAX);

        // crypto passes — ANY key may claim a bearer token (no target lock) —
        let msg = gate_request(&joiner, BINDING, &token);
        let verified = verify_join_request(&msg, BINDING).expect("bearer verifies");
        // — but it comes out role=Client, which ingress V8 and the intro
        // doorbell terminally refuse: no bearer path onto the resident plane.
        assert_eq!(verified.role, InviteRole::Client);
        assert_eq!(verified.joiner, joiner.public_key());

        // the intro half rides the same verify and pins the same role.
        let intro = intro_request(&joiner, BINDING, &token, [9u8; 32]);
        let verified = verify_intro(&intro, BINDING).expect("bearer intro verifies");
        assert_eq!(verified.joiner, joiner.public_key());
        assert!(
            intro.role != InviteRole::Resident.as_u8(),
            "the doorbell's role gate sees Client and refuses a tunnel"
        );
    }

    #[test]
    fn an_admitted_reply_roundtrips() {
        let msg = GateMsg::Admitted {
            height: 42,
            cap: None,
        };
        assert_eq!(decode_msg(&encode_msg(&msg)).expect("roundtrip"), msg);
    }

    #[test]
    fn a_rejected_reply_roundtrips_each_variant() {
        for (code, terminal) in [
            (RejectCode::BadEncoding, true),
            (RejectCode::BadToken, true),
            (RejectCode::NotTarget, true),
            (RejectCode::Expired, true),
            (RejectCode::BadProof, true),
            (RejectCode::Spent, true),
            (RejectCode::IssuerUnknown, false),
            (RejectCode::RoleUnsupported, true),
            (RejectCode::Busy, false),
        ] {
            let msg = GateMsg::Rejected {
                code,
                detail: "prose".into(),
                terminal,
            };
            assert_eq!(decode_msg(&encode_msg(&msg)).expect("roundtrip"), msg);
        }
    }

    #[test]
    fn reject_codes_are_wire_stable() {
        // the code identifiers are wire-stable (ADR §4): pin the snake_case
        // strings so a rename cannot silently break a deployed joiner.
        let cases = [
            (RejectCode::BadEncoding, "bad_encoding"),
            (RejectCode::BadToken, "bad_token"),
            (RejectCode::NotTarget, "not_target"),
            (RejectCode::Expired, "expired"),
            (RejectCode::BadProof, "bad_proof"),
            (RejectCode::Spent, "spent"),
            (RejectCode::IssuerUnknown, "issuer_unknown"),
            (RejectCode::RoleUnsupported, "role_unsupported"),
            (RejectCode::Busy, "busy"),
        ];
        for (code, wire) in cases {
            let json = serde_json::to_string(&code).expect("serialize");
            assert_eq!(json, format!("\"{wire}\""), "{code:?}");
            let back: RejectCode = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, code);
        }
    }

    #[test]
    fn an_intro_roundtrips_verifies_and_pins_the_wireguard_key() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint_for(&issuer, &joiner.public_key());
        let wg_key = [9u8; 32];
        let msg = intro_request(&joiner, BINDING, &token, wg_key);
        let decoded = decode_intro(&encode_intro(&msg)).expect("roundtrip");
        assert_eq!(decoded, msg);

        let verified = verify_intro(&decoded, BINDING).expect("verifies");
        assert_eq!(verified.joiner, joiner.public_key());
        assert_eq!(verified.issuer, issuer.public_key());
        assert_eq!(verified.wg_public_key, wg_key);

        // a substituted WireGuard key fails its binding signature.
        let mut forged = msg.clone();
        forged.wg_public_key = vec![8u8; 32];
        let err = verify_intro(&forged, BINDING).expect_err("refused");
        assert!(err.contains("wireguard key binding"), "{err}");

        // another network refuses the same intro.
        assert!(verify_intro(&msg, b"other-net").is_err());
    }

    #[test]
    fn an_admitted_reply_carrying_a_cap_roundtrips() {
        // the cap is opaque bytes to lobby.rs — any blob roundtrips verbatim.
        let msg = GateMsg::Admitted {
            height: 7,
            cap: Some(vec![1, 2, 3, 4, 5]),
        };
        assert_eq!(decode_msg(&encode_msg(&msg)).expect("roundtrip"), msg);
    }

    #[test]
    fn verify_errors_map_to_the_checklist_codes() {
        assert_eq!(
            verify_reject_code("invite token signature does not verify for this network"),
            RejectCode::BadToken
        );
        assert_eq!(
            verify_reject_code("invite is locked to a different key — minted for someone else"),
            RejectCode::NotTarget
        );
        assert_eq!(
            verify_reject_code("joiner proof-of-possession does not verify"),
            RejectCode::BadProof
        );
        assert_eq!(verify_reject_code("issuer key: bad length"), RejectCode::BadEncoding);
    }

    #[test]
    fn redeem_reasons_map_to_codes_and_terminal_bits() {
        // the classic race the gate must not mis-report: a spent nonce is a
        // permanent Spent, not a transient Busy.
        assert_eq!(
            redeem_reject_outcome(Some("invite already redeemed")),
            (RejectCode::Spent, true)
        );
        assert_eq!(
            redeem_reject_outcome(Some("the inviting member is no longer part of this network")),
            (RejectCode::IssuerUnknown, false)
        );
        // an unrecognized / already-standing reason retries rather than kills.
        assert_eq!(
            redeem_reject_outcome(Some("joiner is already a validator")),
            (RejectCode::Busy, false)
        );
        assert_eq!(redeem_reject_outcome(None), (RejectCode::Busy, false));
    }

    #[test]
    fn the_retired_announce_wire_no_longer_decodes() {
        // NO backward compat (ADR §7): the pre-gate `join_reply` announce is a
        // dead shape — a member/joiner speaking it does not interop.
        let old_wire = br#"{"join_reply":{"recorded":true,"detail":"awaiting approval"}}"#;
        assert!(decode_msg(old_wire).is_err());
    }
}
