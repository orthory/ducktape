//! the identity module's public wire surface -- types only.
//!
//! a USER is an ed25519 keypair held by the person (in the app), a NODE is a
//! workspace's mesh/valset identity. this module binds nodes to users so any
//! verified submit origin (a node key) resolves to the human who owns it.
//! writes go via [`IdentityMsg`]; reads via [`IdentityQuery`] ->
//! [`IdentityReply`]. bind/unbind carry USER-KEY SIGNATURES over
//! chain-and-nonce-scoped preimages so a certificate can never replay across
//! networks or after an unbind.

use serde::{Deserialize, Serialize};

/// signing domain for bind certificates -- namespace-separated from every
/// other signed artifact (frames, invites, coord caps, endpoint records).
pub const IDENTITY_BIND_NS: &[u8] = b"ducktape-identity-bind-v1";
/// signing domain for unbind certificates.
pub const IDENTITY_UNBIND_NS: &[u8] = b"ducktape-identity-unbind-v1";

/// max user display-name length, in bytes (profiles' exact limit).
pub const MAX_NAME_LEN: usize = 64;
/// query pagination ceiling -- [`IdentityQuery::All`] clamps `limit` to this.
pub const MAX_QUERY_LIMIT: u64 = 256;

/// one user record as queries expose it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UserView {
    pub user_key: Vec<u8>,
    pub display_name: Option<String>,
    /// replay guard: every accepted user-signed op must sign the CURRENT
    /// nonce, and acceptance bumps it.
    pub nonce: u64,
    pub nodes: Vec<Vec<u8>>,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityMsg {
    /// bind the SUBMITTING NODE (the verified origin -- never a payload field)
    /// to `user_key`. `user_sig` is the user key's signature over
    /// [`bind_preimage`] with the user's current nonce (0 when the user record
    /// does not exist yet). both consents ride one op: the node consents by
    /// being the origin, the user by the signature.
    BindNode { user_key: Vec<u8>, user_sig: Vec<u8> },
    /// remove `node_key` from its user's set. user-signed over
    /// [`unbind_preimage`]; accepted from ANY external origin so a surviving
    /// device can evict a lost one. bumps the nonce, killing stale bind certs.
    UnbindNode { node_key: Vec<u8>, user_sig: Vec<u8> },
    /// set the display name of the user the SUBMITTING NODE is bound to.
    /// origin-gated (a bound node is user-trusted hardware); empty trim
    /// clears, over [`MAX_NAME_LEN`] bytes rejects.
    SetUserName { display_name: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityQuery {
    /// every user, ascending by user key, offset+limit paginated.
    All { from: u64, limit: u64 },
    /// one user by user key.
    Get { user_key: Vec<u8> },
    /// the user owning `node_key`, if bound -- the resolver other modules and
    /// the app read through.
    UserOf { node_key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityReply {
    Users(Vec<UserView>),
    User(Option<UserView>),
}

/// the signed preimage of a bind certificate: length-prefixed chain id +
/// node key + nonce, so no field boundary ambiguity exists and a cert minted
/// for one network can never bind a node on another.
pub fn bind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    preimage(chain_id, node_key, nonce)
}

/// the signed preimage of an unbind certificate (same shape, different
/// namespace at signing time).
pub fn unbind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    preimage(chain_id, node_key, nonce)
}

fn preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    let chain = chain_id.as_bytes();
    let mut out = Vec::with_capacity(16 + chain.len() + node_key.len() + 8);
    out.extend_from_slice(&(chain.len() as u64).to_le_bytes());
    out.extend_from_slice(chain);
    out.extend_from_slice(&(node_key.len() as u64).to_le_bytes());
    out.extend_from_slice(node_key);
    out.extend_from_slice(&nonce.to_le_bytes());
    out
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
    fn preimage_is_length_prefixed_and_deterministic() {
        let a = bind_preimage("net-a", &[1u8; 32], 0);
        let b = bind_preimage("net-a", &[1u8; 32], 0);
        assert_eq!(a, b);
        // chain id and node key cannot bleed into each other
        assert_ne!(bind_preimage("ab", &[1, 2, 3], 0), bind_preimage("a", &[98, 1, 2, 3], 0));
        // nonce moves the preimage
        assert_ne!(bind_preimage("n", &[1u8; 32], 0), bind_preimage("n", &[1u8; 32], 1));
    }

    #[test]
    fn msg_codec_roundtrips() {
        for m in [
            IdentityMsg::BindNode { user_key: vec![7; 32], user_sig: vec![9; 64] },
            IdentityMsg::UnbindNode { node_key: vec![1; 32], user_sig: vec![2; 64] },
            IdentityMsg::SetUserName { display_name: "eddy".into() },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
        let q = IdentityQuery::UserOf { node_key: vec![3; 32] };
        assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        let r = IdentityReply::User(None);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }
}
