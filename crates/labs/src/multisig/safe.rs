//! Gnosis Safe wire format: the EIP-712 SafeTx hash, and `execTransaction`
//! calldata.
//!
//! ## why this file is its own thing
//!
//! This is the ONLY Safe-aware code in the module (the MPC seam is
//! this file boundary plus the `backend` field — not a trait with one impl).
//! Everything else in `multisig` is backend-agnostic: propose an intent,
//! collect contributions, emit a finished transaction.
//!
//! ## why the type hashes are COMPUTED, not pasted
//!
//! A wrong SafeTx hash is the worst failure this module has: every owner signs
//! it, consensus accepts every signature as valid (they *are* valid — over the
//! wrong bytes), and the transaction reverts on-chain forever. Nothing
//! upstream catches it. So the hashes are derived from their EIP-712 type
//! STRINGS, which are self-evidently checkable against the Safe contract, and
//! the tests pin them to Safe's published constants. A pasted hex constant
//! would be a single typo away from silently unspendable approvals.
//!
//! Targets Safe >= 1.3.0 (the domain separator binds `chainId`; 1.1.1 did not,
//! and is not supported).

use alloy_primitives::{Address, B256, Signature, U256, keccak256};

/// `keccak256("EIP712Domain(uint256 chainId,address verifyingContract)")`
fn domain_typehash() -> B256 {
    keccak256(b"EIP712Domain(uint256 chainId,address verifyingContract)")
}

/// `keccak256("SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)")`
fn safe_tx_typehash() -> B256 {
    keccak256(
        b"SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,\
uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)",
    )
}

/// The `execTransaction` 4-byte selector, derived from its signature.
fn exec_transaction_selector() -> [u8; 4] {
    let h = keccak256(
        b"execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
    );
    [h[0], h[1], h[2], h[3]]
}

/// A Safe `operation`. `DelegateCall` executes foreign code against the Safe's
/// OWN storage — it can rewrite the owner set and the threshold, i.e. steal the
/// vault in one transaction. The module refuses to propose one (see
/// [`SafeTx::validate`]); the variant exists only so the encoding is complete
/// and a future policy-gated use has somewhere to live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Call = 0,
    DelegateCall = 1,
}

/// The exact tuple the Safe hashes and the chain executes.
///
/// The gas fields are carried (they are part of the signed hash, so they cannot
/// be omitted) but the module pins them to zero — see [`SafeTx::validate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeTx {
    pub to: Address,
    pub value: U256,
    pub data: Vec<u8>,
    pub operation: Operation,
    pub safe_tx_gas: U256,
    pub base_gas: U256,
    pub gas_price: U256,
    pub gas_token: Address,
    pub refund_receiver: Address,
    pub nonce: U256,
}

impl SafeTx {
    /// A plain value/call transfer with every gas-refund parameter zeroed.
    pub fn call(to: Address, value: U256, data: Vec<u8>, nonce: u64) -> Self {
        Self {
            to,
            value,
            data,
            operation: Operation::Call,
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            gas_token: Address::ZERO,
            refund_receiver: Address::ZERO,
            nonce: U256::from(nonce),
        }
    }

    /// Refuse the two shapes that let a proposal steal the vault outright.
    ///
    /// - `DelegateCall` runs arbitrary code against the Safe's own storage, so
    ///   one approved proposal could rewrite owners and threshold.
    /// - A non-zero `gasPrice`/`gasToken`/`refundReceiver` makes the Safe pay
    ///   the executor an attacker-chosen amount out of vault funds — the
    ///   documented Safe gas-refund griefing vector. The executor pays their
    ///   own gas precisely so these can stay pinned at zero.
    pub fn validate(&self) -> Result<(), String> {
        if self.operation != Operation::Call {
            return Err("multisig: DELEGATECALL proposals are refused (it can rewrite the Safe's own owners and threshold)".into());
        }
        if !self.safe_tx_gas.is_zero()
            || !self.base_gas.is_zero()
            || !self.gas_price.is_zero()
            || !self.gas_token.is_zero()
            || !self.refund_receiver.is_zero()
        {
            return Err(
                "multisig: gas-refund parameters must be zero (the executor pays their own gas)"
                    .into(),
            );
        }
        Ok(())
    }

    /// `keccak256(abi.encode(SAFE_TX_TYPEHASH, ...))` — the EIP-712 struct hash.
    fn struct_hash(&self) -> B256 {
        let mut buf = Vec::with_capacity(32 * 11);
        buf.extend_from_slice(safe_tx_typehash().as_slice());
        buf.extend_from_slice(word_address(self.to).as_slice());
        buf.extend_from_slice(&self.value.to_be_bytes::<32>());
        // a dynamic `bytes` member hashes to keccak256(contents) inside abi.encode
        buf.extend_from_slice(keccak256(&self.data).as_slice());
        buf.extend_from_slice(word_u8(self.operation as u8).as_slice());
        buf.extend_from_slice(&self.safe_tx_gas.to_be_bytes::<32>());
        buf.extend_from_slice(&self.base_gas.to_be_bytes::<32>());
        buf.extend_from_slice(&self.gas_price.to_be_bytes::<32>());
        buf.extend_from_slice(word_address(self.gas_token).as_slice());
        buf.extend_from_slice(word_address(self.refund_receiver).as_slice());
        buf.extend_from_slice(&self.nonce.to_be_bytes::<32>());
        keccak256(buf)
    }
}

/// `keccak256(abi.encode(DOMAIN_TYPEHASH, chainId, safe))`.
pub fn domain_separator(chain_id: u64, safe: Address) -> B256 {
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(domain_typehash().as_slice());
    buf.extend_from_slice(&U256::from(chain_id).to_be_bytes::<32>());
    buf.extend_from_slice(word_address(safe).as_slice());
    keccak256(buf)
}

/// The digest every owner signs: `keccak256(0x19 ‖ 0x01 ‖ domainSeparator ‖ structHash)`.
///
/// Computed in consensus from the proposal's own fields, never taken from the
/// proposer — that is what stops a proposer from showing other owners one
/// transaction and having them sign another.
pub fn safe_tx_hash(chain_id: u64, safe: Address, tx: &SafeTx) -> B256 {
    let mut buf = Vec::with_capacity(66);
    buf.extend_from_slice(&[0x19, 0x01]);
    buf.extend_from_slice(domain_separator(chain_id, safe).as_slice());
    buf.extend_from_slice(tx.struct_hash().as_slice());
    keccak256(buf)
}

/// Recover the owner address that produced `sig` over `hash`.
///
/// Ethereum's `ecrecover` is the verifier of record for Safe owner signatures,
/// so this is the whole of our signature checking — no curve is
/// re-implemented anywhere in the tree.
///
/// The signature is rejected unless S is in the lower half of the curve order
/// (EIP-2). Without that check, `(r, s, v)` and `(r, -s, v^1)` both recover the
/// same owner, so one approval could be replayed as a second, distinct-looking
/// approval and a 2-of-3 vault could be executed by ONE owner.
pub fn recover_owner(hash: B256, sig: &[u8; 65]) -> Result<Address, String> {
    let signature =
        Signature::from_raw_array(sig).map_err(|e| format!("malformed signature: {e}"))?;
    if signature.normalize_s().is_some() {
        return Err("signature S is not normalized (EIP-2 low-S required)".into());
    }
    signature
        .recover_address_from_prehash(&hash)
        .map_err(|e| format!("signature does not recover: {e}"))
}

/// Pack owner signatures for `execTransaction`.
///
/// Safe's `checkNSignatures` walks the blob in ASCENDING OWNER ADDRESS order
/// and rejects anything else, so the sort is not cosmetic — an unsorted blob
/// reverts on-chain. `v` is emitted as 27/28 (the ecrecover convention Safe
/// expects for an EOA signature).
pub fn pack_signatures(mut approvals: Vec<(Address, [u8; 65])>) -> Vec<u8> {
    approvals.sort_by_key(|(owner, _)| *owner);
    let mut out = Vec::with_capacity(approvals.len() * 65);
    for (_, sig) in approvals {
        out.extend_from_slice(&sig[..64]);
        // from_raw_array accepted 0/1 or 27/28; normalize back to 27/28.
        out.push(if sig[64] >= 27 { sig[64] } else { sig[64] + 27 });
    }
    out
}

/// ABI-encoded `execTransaction(...)` calldata — the exact bytes broadcast.
///
/// Built in consensus so the finished transaction is committed state: the
/// oracle that broadcasts it transmits bytes it did not choose.
pub fn exec_transaction_calldata(tx: &SafeTx, signatures: &[u8]) -> Vec<u8> {
    // 10 head words; `data` and `signatures` are dynamic and live in the tail.
    const HEAD_WORDS: usize = 10;
    let head_len = 32 * HEAD_WORDS;
    let data_offset = head_len;
    let signatures_offset = data_offset + 32 + pad32(tx.data.len());

    let mut out = Vec::new();
    out.extend_from_slice(&exec_transaction_selector());
    out.extend_from_slice(word_address(tx.to).as_slice());
    out.extend_from_slice(&tx.value.to_be_bytes::<32>());
    out.extend_from_slice(&U256::from(data_offset).to_be_bytes::<32>());
    out.extend_from_slice(word_u8(tx.operation as u8).as_slice());
    out.extend_from_slice(&tx.safe_tx_gas.to_be_bytes::<32>());
    out.extend_from_slice(&tx.base_gas.to_be_bytes::<32>());
    out.extend_from_slice(&tx.gas_price.to_be_bytes::<32>());
    out.extend_from_slice(word_address(tx.gas_token).as_slice());
    out.extend_from_slice(word_address(tx.refund_receiver).as_slice());
    out.extend_from_slice(&U256::from(signatures_offset).to_be_bytes::<32>());
    push_dynamic_bytes(&mut out, &tx.data);
    push_dynamic_bytes(&mut out, signatures);
    out
}

// ---- ABI word helpers -------------------------------------------------------

fn pad32(len: usize) -> usize {
    len.div_ceil(32) * 32
}

/// length word, then the bytes right-padded to a 32-byte boundary.
fn push_dynamic_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&U256::from(bytes.len()).to_be_bytes::<32>());
    out.extend_from_slice(bytes);
    out.extend(std::iter::repeat_n(0u8, pad32(bytes.len()) - bytes.len()));
}

/// an `address` as a left-padded 32-byte word.
fn word_address(a: Address) -> B256 {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a.as_slice());
    B256::from(w)
}

/// a `uint8` as a left-padded 32-byte word.
fn word_u8(v: u8) -> B256 {
    let mut w = [0u8; 32];
    w[31] = v;
    B256::from(w)
}
