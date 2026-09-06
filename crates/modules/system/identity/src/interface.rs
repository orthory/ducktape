//! the identity module's public wire surface -- types only.
//!
//! an ACCOUNT is an abstract principal identified by a NUMBER (monotonic from
//! 1), owning an ASSOCIATION of keys of mixed [`KeyScheme`]s (an ed25519 device
//! key, an Ethereum wallet, a WebAuthn passkey). the frame ORIGIN is the acting
//! key for every op: [`IdentityMsg::Create`] founds an account for the origin,
//! [`IdentityMsg::AddKey`] admits the origin into an existing member's account
//! (that member proves consent over [`add_key_preimage`] at the origin's
//! CURRENT generation, naming the ACCOUNT it admits into and the consensus
//! time it dies at, so the proof is single-use, account-bound and short-lived),
//! and
//! `RemoveKey`/`SetName`/`SetProfile` need nothing but the origin's membership.
//! no account is ever keyed by a key, and no node is ever bound to an account:
//! attribution comes only from a user-signed origin, resolved through
//! [`IdentityQuery::OfKey`].

use serde::{Deserialize, Serialize};

pub use keyscheme::KeyScheme;

/// the account id: monotonic from 1. `0` is never an account.
pub type AccountNumber = u64;

/// signing domain for add-key consents -- namespace-separated from every other
/// signed artifact (frames, invites, coord caps, endpoint records).
pub const IDENTITY_ADD_KEY_NS: &[u8] = b"ducktape-identity-add-key-v1";

/// max account name length, in bytes. a name is display text, not an id.
pub const MAX_NAME_LEN: usize = 64;
/// max key label length, in bytes (e.g. "Kim's phone").
pub const MAX_LABEL_LEN: usize = 64;
/// max account bio/status length, in bytes (a short status line).
pub const MAX_BIO_LEN: usize = 280;
/// max avatar reference length, in bytes -- a duckfs path
/// (`/shared/attachments/avatars/<sha16>.<ext>`); the bytes live in the files
/// module, identity stores only the reference.
pub const MAX_AVATAR_REF_LEN: usize = 512;
/// query pagination ceiling -- [`IdentityQuery::All`] clamps `limit` to this.
pub const MAX_QUERY_LIMIT: u64 = 256;
/// how far past the block executing it a consent's `expires_at` may reach.
/// `consensus_time` is a block height on a validator network heartbeating at
/// about one block a second, so this is roughly seven days -- long enough for
/// a device pairing that waits on a person, short enough that a mis-issued
/// ticket is a mistake rather than a permanent bearer credential to the
/// account. a consent naming a later expiry is refused at execution.
pub const MAX_CONSENT_TTL: u64 = 604_800;

/// one key of an association as queries expose it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KeyView {
    pub scheme: KeyScheme,
    pub pubkey: Vec<u8>,
    pub label: Option<String>,
    pub added_at: u64,
}

/// one account as queries expose it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AccountView {
    pub number: AccountNumber,
    /// display text; NOT unique -- the same person across workspaces is the
    /// same key in both associations, never the same name.
    pub name: String,
    /// the association, ascending by public key. never empty for a live account.
    pub keys: Vec<KeyView>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
    pub updated_at: u64,
}

/// an existing member's consent to admit the frame origin: which key, which
/// ACCOUNT it admits into, when the consent dies, and its scheme-owned proof
/// bytes over [`add_key_preimage`] at the origin's current generation.
///
/// `account` and `expires_at` are not spoofable: both are under the
/// authorizer's signature, and `account` is cross-checked against that key's
/// membership at execution. that pairing is the point -- the signature alone
/// would let a payload name an account nobody consented to, and the membership
/// lookup alone would follow the authorizer onto whatever account it sits on
/// when the consent is finally spent.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Authorizer {
    pub key: Vec<u8>,
    /// the account this consent admits into -- the authorizer's own, at
    /// signing time and at execution time both.
    pub account: AccountNumber,
    /// consensus time after which the consent is refused; at most
    /// [`MAX_CONSENT_TTL`] past the block that executes it.
    pub expires_at: u64,
    pub proof: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IdentityMsg {
    /// found an account for the ORIGIN key (of `scheme`; the frame signature
    /// is the possession proof). the origin must belong to no account.
    Create { name: String, scheme: KeyScheme },
    /// admit the ORIGIN key (of `scheme`) into `authorizer.account`, which the
    /// authorizer key must still belong to. `authorizer.proof` is that
    /// member's proof over [`add_key_preimage`] at the origin's CURRENT
    /// generation; on success the origin's generation advances, so the proof
    /// never verifies again. the origin must belong to no account.
    AddKey {
        scheme: KeyScheme,
        label: Option<String>,
        authorizer: Authorizer,
    },
    /// drop `key` from the origin's account. a member removes ITSELF, or a key
    /// admitted no earlier than it was -- never the last one, and never a key
    /// senior to the remover. writes nothing but the record: a removed key may
    /// be re-admitted later, at its next generation.
    RemoveKey { key: Vec<u8> },
    /// rename the origin's account. empty trims reject; over
    /// [`MAX_NAME_LEN`] bytes rejects.
    SetName { name: String },
    /// set the avatar reference and/or bio of the origin's account. each field
    /// empty-trims to cleared, over its byte cap rejects.
    SetProfile {
        avatar: Option<String>,
        bio: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IdentityQuery {
    /// accounts numbered `from..`, at most `limit` (clamped to
    /// [`MAX_QUERY_LIMIT`]). `from: 0` reads from 1. no account is ever
    /// deleted, so the numbering has no gaps.
    All { from: u64, limit: u64 },
    Get { number: AccountNumber },
    /// the account `key` belongs to, if any -- the ONE resolver every consumer
    /// reads through.
    OfKey { key: Vec<u8> },
    /// how many times `key` has been admitted anywhere (absent = 0) -- the
    /// generation an add-key consent must sign.
    KeyGen { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IdentityReply {
    Accounts(Vec<AccountView>),
    Account(Option<AccountView>),
    Gen(u64),
}

/// the signed preimage of an add-key consent: chain id ‖ scheme tag ‖ new key
/// ‖ generation ‖ account ‖ expiry. the generation makes it single-use, the
/// account pins WHICH association it admits into (a consent minted on one
/// account never follows its author onto another), and the expiry bounds how
/// long an unspent one stays live -- there is no revoke op, so the clock is
/// the revocation.
pub fn add_key_preimage(
    chain_id: &str,
    scheme: KeyScheme,
    new_key: &[u8],
    generation: u64,
    account: AccountNumber,
    expires_at: u64,
) -> Vec<u8> {
    let mut out = Vec::new();
    sdk::codec::push_bytes(&mut out, chain_id.as_bytes());
    out.push(scheme.tag());
    sdk::codec::push_bytes(&mut out, new_key);
    out.extend_from_slice(&generation.to_le_bytes());
    out.extend_from_slice(&account.to_le_bytes());
    out.extend_from_slice(&expires_at.to_le_bytes());
    out
}

/// the byte principal other modules (governance ballots, forge owners) use
/// for an ACCOUNT: 8 bytes LE -- length-disjoint from every key scheme's
/// public key, so a principal is either an account or a key by length alone.
pub fn account_principal(number: AccountNumber) -> Vec<u8> {
    number.to_le_bytes().to_vec()
}

/// the inverse of [`account_principal`], for renderers: `Some(n)` iff `bytes`
/// is an 8-byte account principal.
pub fn principal_account(bytes: &[u8]) -> Option<AccountNumber> {
    <[u8; 8]>::try_from(bytes).ok().map(u64::from_le_bytes)
}

pub fn encode_msg(m: &IdentityMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<IdentityMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &IdentityQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<IdentityQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &IdentityReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<IdentityReply, String> {
    sdk::wire::decode(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_key_preimage_is_deterministic_and_every_field_moves_it() {
        let base = add_key_preimage("net-a", KeyScheme::Ed25519, &[2u8; 32], 0, 1, 500);
        assert_eq!(
            base,
            add_key_preimage("net-a", KeyScheme::Ed25519, &[2u8; 32], 0, 1, 500)
        );
        assert_ne!(
            base,
            add_key_preimage("net-b", KeyScheme::Ed25519, &[2u8; 32], 0, 1, 500),
            "chain moves it"
        );
        assert_ne!(
            add_key_preimage("n", KeyScheme::Secp256k1, &[2u8; 33], 0, 1, 500),
            add_key_preimage("n", KeyScheme::Secp256r1, &[2u8; 33], 0, 1, 500),
            "scheme moves it"
        );
        assert_ne!(
            base,
            add_key_preimage("net-a", KeyScheme::Ed25519, &[3u8; 32], 0, 1, 500),
            "key moves it"
        );
        assert_ne!(
            base,
            add_key_preimage("net-a", KeyScheme::Ed25519, &[2u8; 32], 1, 1, 500),
            "generation moves it"
        );
        assert_ne!(
            base,
            add_key_preimage("net-a", KeyScheme::Ed25519, &[2u8; 32], 0, 2, 500),
            "account moves it"
        );
        assert_ne!(
            base,
            add_key_preimage("net-a", KeyScheme::Ed25519, &[2u8; 32], 0, 1, 501),
            "expiry moves it"
        );
    }

    #[test]
    fn account_principal_round_trips_and_never_looks_like_a_key() {
        assert_eq!(principal_account(&account_principal(7)), Some(7));
        assert_eq!(account_principal(1).len(), 8);
        assert_eq!(principal_account(&[0u8; 32]), None);
        assert_eq!(principal_account(&[0u8; 33]), None);
    }

    #[test]
    fn msg_codec_roundtrips() {
        let authorizer = Authorizer {
            key: vec![7; 32],
            account: 4,
            expires_at: 900,
            proof: vec![9; 64],
        };
        for m in [
            IdentityMsg::Create {
                name: "alice".into(),
                scheme: KeyScheme::Ed25519,
            },
            IdentityMsg::AddKey {
                scheme: KeyScheme::Secp256r1,
                label: Some("phone".into()),
                authorizer: authorizer.clone(),
            },
            IdentityMsg::RemoveKey { key: vec![4; 32] },
            IdentityMsg::SetName {
                name: "alice".into(),
            },
            IdentityMsg::SetProfile {
                avatar: Some("/shared/attachments/avatars/0123456789abcdef.png".into()),
                bio: None,
            },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
        for q in [
            IdentityQuery::All { from: 0, limit: 16 },
            IdentityQuery::Get { number: 3 },
            IdentityQuery::OfKey { key: vec![3; 32] },
            IdentityQuery::KeyGen { key: vec![3; 32] },
        ] {
            assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        }
        for r in [
            IdentityReply::Account(None),
            IdentityReply::Gen(4),
            IdentityReply::Accounts(vec![AccountView {
                number: 1,
                name: "a".into(),
                keys: vec![KeyView {
                    scheme: KeyScheme::Ed25519,
                    pubkey: vec![1; 32],
                    label: None,
                    added_at: 0,
                }],
                avatar: None,
                bio: None,
                updated_at: 0,
            }]),
        ] {
            assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
        }
    }
}
