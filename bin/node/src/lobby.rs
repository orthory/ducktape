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
    JoinReply { recorded: bool, detail: String },
    /// "this STANDBY key is up — activate it": the key's own height-windowed
    /// signature under the valset online-proof domain. any member relays it
    /// into the ordered lane as `ValsetMsg::Online`; the module re-verifies
    /// the same proof deterministically, so the relayer attests nothing.
    OnlineAnnounce {
        key: Vec<u8>,
        signed_height: u64,
        signature: Vec<u8>,
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

/// build the online announce for a standby node: a fresh proof of possession
/// signed over `key || signed_height` under the valset online-proof domain.
/// `signed_height` should be the freshest committed height the node has seen
/// (the manifest height while parked) — the proof expires
/// `ONLINE_PROOF_TTL_BLOCKS` past it, and re-announces mint fresh proofs.
pub fn online_announce(key: &ed25519::PrivateKey, signed_height: u64) -> LobbyMsg {
    use commonware_cryptography::Signer as _;
    let key_bytes = key.public_key().as_ref().to_vec();
    let signature = key.sign(
        valset_interface::ONLINE_PROOF_NS,
        &valset_interface::online_proof_message(&key_bytes, signed_height),
    );
    LobbyMsg::OnlineAnnounce {
        key: key_bytes,
        signed_height,
        signature: signature.as_ref().to_vec(),
    }
}

/// verify an online announce's proof of possession — crypto only; standby
/// membership and the height window are the valset module's deterministic
/// checks (a member relay only pre-filters junk, it attests nothing).
pub fn verify_online_announce(msg: &LobbyMsg) -> Result<ed25519::PublicKey, String> {
    use commonware_cryptography::Verifier as _;
    let LobbyMsg::OnlineAnnounce {
        key,
        signed_height,
        signature,
    } = msg
    else {
        return Err("not an online announce".into());
    };
    let pk =
        ed25519::PublicKey::decode(key.as_slice()).map_err(|e| format!("standby key: {e}"))?;
    let sig = ed25519::Signature::decode(signature.as_slice())
        .map_err(|e| format!("online proof signature: {e}"))?;
    let payload = valset_interface::online_proof_message(key, *signed_height);
    if !pk.verify(valset_interface::ONLINE_PROOF_NS, &payload, &sig) {
        return Err("online proof does not verify against the announced key".into());
    }
    Ok(pk)
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
    fn an_online_announce_roundtrips_and_verifies_only_its_own_key() {
        let standby = ed25519::PrivateKey::from_seed(5);
        let msg = online_announce(&standby, 42);
        let decoded = decode_msg(&encode_msg(&msg)).expect("roundtrip");
        assert_eq!(decoded, msg);
        assert_eq!(
            verify_online_announce(&decoded).expect("verifies"),
            standby.public_key()
        );

        // a substituted key fails the proof.
        let LobbyMsg::OnlineAnnounce {
            signed_height,
            signature,
            ..
        } = msg
        else {
            unreachable!()
        };
        let forged = LobbyMsg::OnlineAnnounce {
            key: ed25519::PrivateKey::from_seed(6)
                .public_key()
                .as_ref()
                .to_vec(),
            signed_height,
            signature,
        };
        assert!(verify_online_announce(&forged).is_err());
    }

    #[test]
    fn a_reply_roundtrips() {
        let msg = LobbyMsg::JoinReply {
            recorded: true,
            detail: "awaiting approval".into(),
        };
        assert_eq!(decode_msg(&encode_msg(&msg)).expect("roundtrip"), msg);
    }
}
