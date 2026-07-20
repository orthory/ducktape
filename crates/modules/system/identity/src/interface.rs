//! the identity module's public wire surface -- types only.
//!
//! an ACCOUNT is the umbrella a person owns: it is keyed by its FOUNDING key
//! (`account_id` = the first member key), collects many MEMBER KEYS of
//! different schemes (an ed25519 seed key, a WebAuthn passkey, ...), shares one
//! display name across all of them, and owns many NODES (each a workspace's
//! mesh/valset identity). binding a node resolves to the ACCOUNT, so any
//! verified submit origin (a node key) maps to the human who owns it.
//!
//! writes go via [`IdentityMsg`]; reads via [`IdentityQuery`] ->
//! [`IdentityReply`]. every state-changing op is authorized by a MEMBER KEY --
//! captured as a [`MemberAuth`] (which key, which scheme, and the proof over
//! the op's chain-and-nonce-scoped preimage) -- so a certificate can never
//! replay across networks or after the account nonce advances. adding/removing
//! a member key needs TWO consents: an existing member authorizes, and (for an
//! add) the new key proves possession, both over the same preimage.

use serde::{Deserialize, Serialize};

pub use crate::scheme::{KeyKind, MemberProof};

/// signing domain for node-bind certificates -- namespace-separated from every
/// other signed artifact (frames, invites, coord caps, endpoint records).
pub const IDENTITY_BIND_NS: &[u8] = b"ducktape-identity-bind-v1";
/// signing domain for node-unbind certificates.
pub const IDENTITY_UNBIND_NS: &[u8] = b"ducktape-identity-unbind-v1";
/// signing domain for add-member-key certificates.
pub const IDENTITY_ADD_MEMBER_NS: &[u8] = b"ducktape-identity-add-member-v1";
/// signing domain for remove-member-key certificates.
pub const IDENTITY_REMOVE_MEMBER_NS: &[u8] = b"ducktape-identity-remove-member-v1";

/// max account display-name length, in bytes.
pub const MAX_NAME_LEN: usize = 64;
/// max member-key label length, in bytes (e.g. "Kim's phone").
pub const MAX_LABEL_LEN: usize = 64;
/// max account bio/status length, in bytes (a short status line).
pub const MAX_BIO_LEN: usize = 280;
/// max avatar reference length, in bytes -- a duckfs path
/// (`/shared/attachments/avatars/<sha16>.<ext>`); the bytes live in the files
/// module, identity stores only the reference.
pub const MAX_AVATAR_REF_LEN: usize = 512;
/// query pagination ceiling -- [`IdentityQuery::All`] clamps `limit` to this.
pub const MAX_QUERY_LIMIT: u64 = 256;

/// one member key as queries expose it: its public key, scheme, optional
/// human label, and the block timestamp it was admitted at.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MemberKeyView {
    pub pubkey: Vec<u8>,
    pub kind: KeyKind,
    pub label: Option<String>,
    pub added_at: u64,
}

/// one bound node as queries expose it: its node (mesh/valset) key and the
/// optional human label a bound device set for it (`SetNodeLabel`). The label
/// is a per-network on-chain fact, so a device's name shows on the user's other
/// devices connected to the same network.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    pub node_key: Vec<u8>,
    pub label: Option<String>,
}

/// one account record as queries expose it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AccountView {
    /// the account's stable id -- the founding member key's bytes. it need not
    /// still be a current member (the founder can be removed while others
    /// remain); it is an identifier, not a live authority.
    pub account_id: Vec<u8>,
    pub display_name: Option<String>,
    /// account avatar reference: a duckfs path the app resolves against the
    /// files module (`None` when unset). Global to the account; propagated to
    /// each network by the app's reconcile-on-connect pass.
    pub avatar: Option<String>,
    /// account bio/status line (`None` when unset). Global to the account.
    pub bio: Option<String>,
    /// replay guard: every accepted member-signed op signs the CURRENT nonce,
    /// and acceptance bumps it. shared by the whole account.
    pub nonce: u64,
    /// the collected keys, ascending by public key. always non-empty for a
    /// live account.
    pub member_keys: Vec<MemberKeyView>,
    /// the bound nodes, ascending by node key, each with its optional label.
    pub nodes: Vec<NodeView>,
    pub updated_at: u64,
}

/// an authorization by one member key: which key, its scheme, and its proof
/// over the operation's preimage. the account it speaks for is resolved from
/// this key's membership -- never carried as a spoofable payload field.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MemberAuth {
    pub key: Vec<u8>,
    pub kind: KeyKind,
    pub proof: MemberProof,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityMsg {
    /// bind the SUBMITTING NODE (the verified origin -- never a payload field)
    /// to the account that `authorizer` is a member of, consented to by that
    /// member's signature over [`bind_preimage`] at the account's current
    /// nonce. if `authorizer.key` belongs to no account yet, this CREATES a
    /// new account founded by that key (account_id = the key, nonce 0) -- the
    /// path the desktop's auto-bind takes on first connect.
    BindNode { authorizer: MemberAuth },
    /// remove `node_key` from its account. authorized by ANY member of that
    /// account over [`unbind_preimage`], so a surviving device can evict a
    /// lost one. bumps the nonce, killing stale certs.
    UnbindNode {
        node_key: Vec<u8>,
        authorizer: MemberAuth,
    },
    /// add `new_key` (of `new_kind`, optional `new_label`) to the account
    /// `authorizer` belongs to. TWO consents over [`add_member_preimage`]: the
    /// existing member `authorizer` admits it, and `possession` proves the new
    /// key holds itself. bumps the nonce.
    AddMemberKey {
        new_key: Vec<u8>,
        new_kind: KeyKind,
        new_label: Option<String>,
        possession: MemberProof,
        authorizer: MemberAuth,
    },
    /// remove `target_key` from the account `authorizer` belongs to, over
    /// [`remove_member_preimage`]. any member may remove any member (including
    /// itself) -- the "surviving device evicts a lost one" recovery path at
    /// key granularity -- EXCEPT the last remaining member (an account keeps
    /// at least one live key). bumps the nonce.
    RemoveMemberKey {
        target_key: Vec<u8>,
        authorizer: MemberAuth,
    },
    /// set the display name of the account the SUBMITTING NODE is bound to.
    /// origin-gated (a bound node is user-trusted hardware); empty trim
    /// clears, over [`MAX_NAME_LEN`] bytes rejects. no signature, no nonce bump.
    SetAccountName { display_name: String },
    /// set the avatar reference and/or bio of the account the SUBMITTING NODE
    /// is bound to. origin-gated exactly like [`IdentityMsg::SetAccountName`]
    /// (a bound node is user-trusted hardware); each field empty-trims to
    /// cleared, over its byte cap ([`MAX_AVATAR_REF_LEN`]/[`MAX_BIO_LEN`])
    /// rejects. no signature, no nonce bump. the app's per-network reconcile
    /// pass pushes the account's global avatar/bio through this.
    SetProfile {
        avatar: Option<String>,
        bio: Option<String>,
    },
    /// set (or clear, on empty trim) the human label of `node_key`. ORIGIN-GATED
    /// exactly like [`IdentityMsg::SetAccountName`]: accepted only from a node
    /// bound to the account, and `node_key` must be bound to that SAME account
    /// (you label your own devices). a device label is cosmetic display
    /// metadata, not a capability — so it rides the origin gate a bound node
    /// already carries, with no member signature and no nonce bump. over
    /// [`MAX_LABEL_LEN`] bytes rejects.
    SetNodeLabel {
        node_key: Vec<u8>,
        label: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityQuery {
    /// every account, ascending by account id, offset+limit paginated.
    All { from: u64, limit: u64 },
    /// one account by its id (its founding key).
    Get { account_id: Vec<u8> },
    /// the account owning `node_key`, if bound -- the resolver other modules
    /// and the app read through.
    OfNode { node_key: Vec<u8> },
    /// the account a `member_key` belongs to, if any -- how a device finds its
    /// own account from whatever member key it holds locally.
    OfMember { member_key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityReply {
    Accounts(Vec<AccountView>),
    Account(Option<AccountView>),
}

// ---- signing preimages --------------------------------------------------
//
// each is length-prefixed so no field boundary is ambiguous, and each folds
// in the chain id so a certificate minted for one network can never act on
// another. the account nonce (folded in) makes each single-use.

/// the signed preimage of a node-bind certificate: chain id + node key + nonce.
pub fn bind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    let mut out = Vec::new();
    push_len(&mut out, chain_id.as_bytes());
    push_len(&mut out, node_key);
    out.extend_from_slice(&nonce.to_le_bytes());
    out
}

/// the signed preimage of a node-unbind certificate (same shape as bind, but
/// signed under [`IDENTITY_UNBIND_NS`]).
pub fn unbind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    bind_preimage(chain_id, node_key, nonce)
}

/// the signed preimage of an add-member-key certificate, over (in order) the
/// chain id, account id, new key, its one-byte kind tag, and the nonce. the
/// kind tag binds the consent to the exact scheme being admitted, so it can't
/// be swapped after the fact.
pub fn add_member_preimage(
    chain_id: &str,
    account_id: &[u8],
    new_key: &[u8],
    new_kind: KeyKind,
    nonce: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_len(&mut out, chain_id.as_bytes());
    push_len(&mut out, account_id);
    push_len(&mut out, new_key);
    out.push(new_kind.tag());
    out.extend_from_slice(&nonce.to_le_bytes());
    out
}

/// the signed preimage of a remove-member-key certificate: chain id + account
/// id + target key + nonce.
pub fn remove_member_preimage(
    chain_id: &str,
    account_id: &[u8],
    target_key: &[u8],
    nonce: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_len(&mut out, chain_id.as_bytes());
    push_len(&mut out, account_id);
    push_len(&mut out, target_key);
    out.extend_from_slice(&nonce.to_le_bytes());
    out
}

fn push_len(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// the exact bytes a NATIVE key (`Ed25519`/`P256`) signs to prove possession
/// when joining an account: commonware's namespaced signing preimage
/// `union_unique(IDENTITY_ADD_MEMBER_NS, add_member_preimage(...))`, which is
/// what `commonware_cryptography`'s `verify` reconstructs internally.
///
/// exposed for the ENROLLMENT side: the node hands a joining device these exact
/// bytes to ECDSA/EdDSA-sign, so the phone page never reconstructs the preimage
/// or the namespace framing -- one source of truth with the on-chain verifier.
/// (a WebAuthn passkey does NOT use this; its challenge is the hashed form --
/// see [`crate::webauthn_challenge`].)
pub fn add_member_signing_payload(
    chain_id: &str,
    account_id: &[u8],
    new_key: &[u8],
    new_kind: KeyKind,
    nonce: u64,
) -> Vec<u8> {
    commonware_utils::union_unique(
        IDENTITY_ADD_MEMBER_NS,
        &add_member_preimage(chain_id, account_id, new_key, new_kind, nonce),
    )
}

pub fn encode_msg(m: &IdentityMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<IdentityMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &IdentityQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<IdentityQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &IdentityReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<IdentityReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preimages_are_length_prefixed_and_deterministic() {
        let a = add_member_preimage("net-a", &[1u8; 32], &[2u8; 33], KeyKind::WebauthnP256, 0);
        let b = add_member_preimage("net-a", &[1u8; 32], &[2u8; 33], KeyKind::WebauthnP256, 0);
        assert_eq!(a, b);
        // account id and new key cannot bleed into each other.
        assert_ne!(
            add_member_preimage("n", &[1, 2, 3], &[4], KeyKind::Ed25519, 0),
            add_member_preimage("n", &[1], &[2, 3, 4], KeyKind::Ed25519, 0),
        );
        // the kind tag moves the preimage: same key admitted as a different
        // scheme is a different consent.
        assert_ne!(
            add_member_preimage("n", &[1u8; 32], &[2u8; 33], KeyKind::P256, 0),
            add_member_preimage("n", &[1u8; 32], &[2u8; 33], KeyKind::WebauthnP256, 0),
        );
        // nonce moves the preimage.
        assert_ne!(
            remove_member_preimage("n", &[1u8; 32], &[2u8; 32], 0),
            remove_member_preimage("n", &[1u8; 32], &[2u8; 32], 1),
        );
    }

    #[test]
    fn msg_codec_roundtrips() {
        let auth = MemberAuth {
            key: vec![7; 32],
            kind: KeyKind::Ed25519,
            proof: MemberProof::Signature { sig: vec![9; 64] },
        };
        for m in [
            IdentityMsg::BindNode {
                authorizer: auth.clone(),
            },
            IdentityMsg::UnbindNode {
                node_key: vec![1; 32],
                authorizer: auth.clone(),
            },
            IdentityMsg::AddMemberKey {
                new_key: vec![2; 33],
                new_kind: KeyKind::WebauthnP256,
                new_label: Some("phone".into()),
                possession: MemberProof::Webauthn {
                    authenticator_data: vec![0; 37],
                    client_data_json: b"{}".to_vec(),
                    signature: vec![3; 64],
                },
                authorizer: auth.clone(),
            },
            IdentityMsg::RemoveMemberKey {
                target_key: vec![4; 32],
                authorizer: auth.clone(),
            },
            IdentityMsg::SetAccountName {
                display_name: "eddy".into(),
            },
            IdentityMsg::SetProfile {
                avatar: Some("/shared/attachments/avatars/0123456789abcdef.png".into()),
                bio: None,
            },
            IdentityMsg::SetProfile {
                avatar: None,
                bio: Some("building ducks".into()),
            },
            IdentityMsg::SetNodeLabel {
                node_key: vec![5; 32],
                label: Some("Kim's laptop".into()),
            },
            IdentityMsg::SetNodeLabel {
                node_key: vec![5; 32],
                label: None,
            },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
        let q = IdentityQuery::OfMember {
            member_key: vec![3; 32],
        };
        assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        let r = IdentityReply::Account(None);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }
}
