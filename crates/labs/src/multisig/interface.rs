//! the multisig module's public wire surface — types only.
//!
//! ## trust model, read carefully
//!
//! An owner is a 20-byte **Ethereum address**, not a Ducktape account. That is
//! not a shortcut: the Safe contract itself knows only addresses, and it will
//! only ever accept a secp256k1 signature, so a passkey-only member could never
//! approve regardless of how we modelled it. Mirroring the Safe's own owner set
//! makes authorization uniformly "ecrecover ∈ owners" and keeps the identity
//! module out of the approval path entirely.
//!
//! [`MultisigMsg::BindOwnerEoa`] maps an owner address back to a Ducktape
//! account for ATTRIBUTION ("Alice approved"). It is an audit convenience and
//! authorizes nothing — deleting every binding would not change which
//! transactions can execute.
//!
//! Byte fields are `Vec<u8>` and length-validated on decode: 20 for an address,
//! 32 for a hash or a `uint256`, 65 for a signature.

use serde::{Deserialize, Serialize};

/// Which signing backend produces the finished transaction. THE MPC SEAM:
/// the ops above are already backend-agnostic — propose an intent,
/// collect contributions, emit a finished transaction. Under `ThresholdEcdsa` a
/// "contribution" becomes a signing round rather than a signature, and the
/// finished transaction carries one signature instead of M.
///
/// Deliberately an enum field and not a `trait VaultSigner`: an interface with
/// one implementor abstracts over nothing.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    #[default]
    Safe,
    ThresholdEcdsa,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MultisigMsg {
    /// Record an existing Safe. Deploying one is an ordinary chain transaction,
    /// not a consensus op.
    ///
    /// `signature` is a possession proof over [`register_preimage`] and must
    /// recover to one of the declared `owners` — otherwise anyone could spam
    /// vault records pointing at Safes they have nothing to do with.
    RegisterVault {
        vault_id: String,
        chain_id: u64,
        safe_address: Vec<u8>,
        owners: Vec<Vec<u8>>,
        threshold: u8,
        signature: Vec<u8>,
    },

    /// Attribution only (see the module docs): bind an owner address to the
    /// submitting account. `possession` proves the address is ours; the ordered
    /// lane's frame signature proves who is submitting.
    BindOwnerEoa {
        vault_id: String,
        address: Vec<u8>,
        possession: Vec<u8>,
    },

    /// Propose a transaction AND cast the proposer's own approval in one op:
    /// the signature over the SafeTx hash is what proves the proposer is an
    /// owner, so a separate authorization would be redundant.
    ///
    /// `nonce` is chosen by the proposer (not allocated by consensus) because
    /// the proposer must know it in order to sign. Two proposals MAY share a
    /// nonce — that is how a Safe transaction is replaced or cancelled, and
    /// executing either one invalidates the rest at that nonce.
    ProposeTx {
        vault_id: String,
        nonce: u64,
        to: Vec<u8>,
        value: Vec<u8>,
        data: Vec<u8>,
        signature: Vec<u8>,
    },

    /// Add an owner signature over an existing proposal's SafeTx hash.
    Approve {
        vault_id: String,
        safe_tx_hash: Vec<u8>,
        signature: Vec<u8>,
    },

    /// Chain facts, submitted by the oracle. Validator-gated.
    ///
    /// `owners`/`threshold` are compared against the mirror: any divergence
    /// marks the vault DRIFTED and freezes new proposals. The Safe is
    /// authoritative and we fail closed rather than silently reconcile a money
    /// path.
    RecordChainState {
        vault_id: String,
        nonce: u64,
        owners: Vec<Vec<u8>>,
        threshold: u8,
    },

    /// The outcome of a broadcast. Validator-gated.
    RecordExecution {
        vault_id: String,
        safe_tx_hash: Vec<u8>,
        chain_tx_hash: Vec<u8>,
        success: bool,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MultisigQuery {
    Vaults,
    Vault { vault_id: String },
    Proposals { vault_id: String },
    /// The finished `execTransaction` calldata for a proposal that has reached
    /// its threshold — what the oracle broadcasts.
    Executable { vault_id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VaultView {
    pub vault_id: String,
    pub chain_id: u64,
    pub safe_address: Vec<u8>,
    pub owners: Vec<Vec<u8>>,
    pub threshold: u8,
    pub backend: Backend,
    pub chain_nonce: u64,
    /// The mirror disagrees with the chain: no new proposals are accepted until
    /// it is re-registered. See [`MultisigMsg::RecordChainState`].
    pub drifted: bool,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProposalView {
    pub safe_tx_hash: Vec<u8>,
    pub vault_id: String,
    pub nonce: u64,
    pub to: Vec<u8>,
    pub value: Vec<u8>,
    pub data: Vec<u8>,
    pub proposer: Vec<u8>,
    /// Owner addresses that have signed, ascending.
    pub approvals: Vec<Vec<u8>>,
    pub threshold: u8,
    pub executed: Option<ExecutionView>,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExecutionView {
    pub chain_tx_hash: Vec<u8>,
    pub success: bool,
}

/// A proposal that has reached threshold: the exact calldata to broadcast.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExecutableView {
    pub safe_tx_hash: Vec<u8>,
    pub vault_id: String,
    pub chain_id: u64,
    pub safe_address: Vec<u8>,
    pub nonce: u64,
    pub calldata: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MultisigReply {
    Vaults(Vec<VaultView>),
    Vault(Option<VaultView>),
    Proposals(Vec<ProposalView>),
    Executable(Vec<ExecutableView>),
}

/// The event emitted the moment a proposal reaches its threshold — the oracle's
/// cue to broadcast.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MultisigEvent {
    Executable(ExecutableView),
}

/// The bytes an owner signs to register a vault. Domain-separated so a
/// registration proof can never be replayed as an approval (or onto another
/// chain, Safe, or vault id).
pub fn register_preimage(
    vault_id: &str,
    chain_id: u64,
    safe_address: &[u8],
    owners: &[Vec<u8>],
    threshold: u8,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"ducktape-multisig-register-v1");
    out.extend_from_slice(&(vault_id.len() as u64).to_le_bytes());
    out.extend_from_slice(vault_id.as_bytes());
    out.extend_from_slice(&chain_id.to_le_bytes());
    out.extend_from_slice(safe_address);
    out.extend_from_slice(&(owners.len() as u64).to_le_bytes());
    for owner in owners {
        out.extend_from_slice(owner);
    }
    out.push(threshold);
    out
}

/// The bytes an owner signs to bind their address to their account.
pub fn bind_preimage(vault_id: &str, address: &[u8], submitter: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"ducktape-multisig-bind-eoa-v1");
    out.extend_from_slice(&(vault_id.len() as u64).to_le_bytes());
    out.extend_from_slice(vault_id.as_bytes());
    out.extend_from_slice(address);
    out.extend_from_slice(submitter);
    out
}

pub fn encode_msg(msg: &MultisigMsg) -> Vec<u8> {
    serde_json::to_vec(msg).expect("MultisigMsg is always serializable")
}

pub fn decode_msg(bytes: &[u8]) -> Result<MultisigMsg, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

pub fn encode_query(query: &MultisigQuery) -> Vec<u8> {
    serde_json::to_vec(query).expect("MultisigQuery is always serializable")
}

pub fn decode_query(bytes: &[u8]) -> Result<MultisigQuery, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

pub fn encode_reply(reply: &MultisigReply) -> Vec<u8> {
    serde_json::to_vec(reply).expect("MultisigReply is always serializable")
}

pub fn decode_reply(bytes: &[u8]) -> Result<MultisigReply, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

pub fn encode_event(event: &MultisigEvent) -> Vec<u8> {
    serde_json::to_vec(event).expect("MultisigEvent is always serializable")
}

pub fn decode_event(bytes: &[u8]) -> Result<MultisigEvent, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
