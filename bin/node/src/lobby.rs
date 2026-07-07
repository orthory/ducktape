//! the lobby channel wire format — how a not-yet-admitted joiner delivers its
//! pubkey, and how members answer.
//!
//! transport: the joiner connects to the mesh AS the network's derived lobby
//! identity (see `config::lobby_identity`) — the one key every member tracks
//! that any invite holder can derive — and speaks on `CHANNEL_LOBBY`. the
//! lobby identity authenticates nothing; every claim in a [`LobbyMsg`] is
//! verified against the INVITE TOKEN it carries (issuer signature over the
//! genesis namespace) and the joiner's proof-of-possession. a valid request is
//! only RECORDED on the member for manual approval (`invite-accept` / the
//! app's approve button) — the lobby never admits anyone by itself.
//!
//! json on the wire: matches the module-interface idiom, and this lane is
//! low-volume (a parked joiner re-announcing every few seconds).

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use serde::{Deserialize, Serialize};

use crate::config::{INVITE_NONCE_LEN, InviteToken};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LobbyMsg {
    /// "this key asks to join, invited by `issuer`" — the token's fields plus
    /// the joiner key and its proof-of-possession, all raw bytes.
    JoinRequest {
        issuer: Vec<u8>,
        nonce: Vec<u8>,
        token_sig: Vec<u8>,
        joiner: Vec<u8>,
        proof: Vec<u8>,
    },
    /// a member's answer, purely informational for the parked node's logs:
    /// `recorded` means the request now awaits approval on that member.
    ///
    /// `cap` carries an OPAQUE genesis-issued coordinator capability (packed
    /// `CoordCap` bytes) minted for the joiner when this network coordinates
    /// PRIVATELY and the answering member is a genesis validator — the joiner
    /// cannot receive it on the invite (its key does not exist at invite-mint
    /// time), so the lobby reply is its only delivery channel. lobby.rs stays
    /// crypto-agnostic: it moves bytes and never depends on the cap types.
    /// `#[serde(default)]` keeps the wire back-compatible — an older peer's
    /// reply (no `cap` field) deserializes with `cap == None`.
    JoinReply {
        recorded: bool,
        detail: String,
        #[serde(default)]
        cap: Option<Vec<u8>>,
        /// this refusal is PERMANENT for this invite (its single-use token
        /// is already redeemed by another key): the joiner must stop
        /// re-announcing and ask for a fresh invite. `#[serde(default)]`
        /// keeps the wire back-compatible — an older member's reply (no
        /// `fatal` field) deserializes as non-fatal, i.e. keep retrying.
        #[serde(default)]
        fatal: bool,
    },
}

pub fn encode_msg(m: &LobbyMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<LobbyMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// build the announce for `token` as `joiner` — the proof binds the announced
/// key to its secret holder.
pub fn join_request(
    joiner: &ed25519::PrivateKey,
    binding: &[u8],
    token: &InviteToken,
) -> LobbyMsg {
    use commonware_codec::Encode as _;
    use commonware_cryptography::Signer as _;
    let proof = crate::config::sign_join_proof(joiner, binding, token);
    LobbyMsg::JoinRequest {
        issuer: token.issuer.as_ref().to_vec(),
        nonce: token.nonce.to_vec(),
        token_sig: token.sig.encode().as_ref().to_vec(),
        joiner: joiner.public_key().as_ref().to_vec(),
        proof: proof.encode().as_ref().to_vec(),
    }
}

/// a decoded, signature-verified join request — what a member records for
/// approval. `verify_join_request` is the only constructor.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedJoinRequest {
    pub joiner: ed25519::PublicKey,
    pub issuer: ed25519::PublicKey,
    pub nonce: [u8; INVITE_NONCE_LEN],
}

/// verify a wire join request against this network's binding: token issuer
/// signature and the joiner's proof-of-possession. membership checks (issuer
/// still a member? joiner already one?) are the CALLER's — they need current
/// state, this needs only crypto.
pub fn verify_join_request(msg: &LobbyMsg, binding: &[u8]) -> Result<VerifiedJoinRequest, String> {
    let LobbyMsg::JoinRequest {
        issuer,
        nonce,
        token_sig,
        joiner,
        proof,
    } = msg
    else {
        return Err("not a join request".into());
    };
    let issuer = ed25519::PublicKey::decode(issuer.as_slice())
        .map_err(|e| format!("issuer key: {e}"))?;
    let joiner = ed25519::PublicKey::decode(joiner.as_slice())
        .map_err(|e| format!("joiner key: {e}"))?;
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
        sig,
    };
    if !crate::config::verify_invite_token(&token, binding) {
        return Err("invite token signature does not verify for this network".into());
    }
    if !crate::config::verify_join_proof(&joiner, binding, &token, &proof) {
        return Err("joiner proof-of-possession does not verify".into());
    }
    Ok(VerifiedJoinRequest {
        joiner,
        issuer,
        nonce: nonce_arr,
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
/// [`LobbyMsg::JoinRequest`] payload plus the WireGuard half.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IntroRequest {
    pub issuer: Vec<u8>,
    pub nonce: Vec<u8>,
    pub token_sig: Vec<u8>,
    pub joiner: Vec<u8>,
    pub proof: Vec<u8>,
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
        &LobbyMsg::JoinRequest {
            issuer: msg.issuer.clone(),
            nonce: msg.nonce.clone(),
            token_sig: msg.token_sig.clone(),
            joiner: msg.joiner.clone(),
            proof: msg.proof.clone(),
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

    #[test]
    fn a_join_request_roundtrips_and_verifies() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint_invite_token(&issuer, BINDING);
        let msg = join_request(&joiner, BINDING, &token);
        let decoded = decode_msg(&encode_msg(&msg)).expect("roundtrip");
        assert_eq!(decoded, msg);

        let verified = verify_join_request(&decoded, BINDING).expect("verifies");
        assert_eq!(verified.joiner, joiner.public_key());
        assert_eq!(verified.issuer, issuer.public_key());
        assert_eq!(verified.nonce, token.nonce);
    }

    #[test]
    fn a_foreign_binding_or_forged_key_is_refused() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint_invite_token(&issuer, BINDING);
        let msg = join_request(&joiner, BINDING, &token);

        // another network refuses the same announce.
        assert!(verify_join_request(&msg, b"other-net").is_err());

        // a substituted joiner key fails the proof-of-possession.
        let LobbyMsg::JoinRequest {
            issuer: i,
            nonce,
            token_sig,
            proof,
            ..
        } = msg
        else {
            unreachable!()
        };
        let forged = LobbyMsg::JoinRequest {
            issuer: i,
            nonce,
            token_sig,
            joiner: ed25519::PrivateKey::from_seed(3)
                .public_key()
                .as_ref()
                .to_vec(),
            proof,
        };
        let err = verify_join_request(&forged, BINDING).expect_err("refused");
        assert!(err.contains("proof-of-possession"), "{err}");
    }

    #[test]
    fn a_reply_roundtrips() {
        let msg = LobbyMsg::JoinReply {
            recorded: true,
            detail: "awaiting approval".into(),
            cap: None,
            fatal: false,
        };
        assert_eq!(decode_msg(&encode_msg(&msg)).expect("roundtrip"), msg);
    }

    #[test]
    fn an_intro_roundtrips_verifies_and_pins_the_wireguard_key() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint_invite_token(&issuer, BINDING);
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
    fn a_reply_carrying_a_cap_roundtrips() {
        // the cap is opaque bytes to lobby.rs — any blob roundtrips verbatim.
        let msg = LobbyMsg::JoinReply {
            recorded: true,
            detail: "admitted; cap delivered".into(),
            cap: Some(vec![1, 2, 3, 4, 5]),
            fatal: false,
        };
        assert_eq!(decode_msg(&encode_msg(&msg)).expect("roundtrip"), msg);
    }

    #[test]
    fn a_reply_missing_the_cap_field_defaults_to_none() {
        // an OLD peer (pre-cap wire) omits `cap` entirely; serde default fills
        // it as None so the new joiner stays back-compatible.
        let wire = br#"{"join_reply":{"recorded":true,"detail":"awaiting approval"}}"#;
        let decoded = decode_msg(wire).expect("old-wire reply decodes");
        assert_eq!(
            decoded,
            LobbyMsg::JoinReply {
                recorded: true,
                detail: "awaiting approval".into(),
                cap: None,
                fatal: false,
            }
        );
    }
}
