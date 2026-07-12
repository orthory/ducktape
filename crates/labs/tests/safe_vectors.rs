//! Pins the Safe wire format to the contract's published constants.
//!
//! Everything else in the module can be wrong and something will notice: a bad
//! signature fails to recover, a bad nonce reverts, a bad owner is refused. A
//! wrong SafeTx hash is the one mistake that is SILENT — every owner signs it,
//! consensus accepts every signature as genuinely valid (they are: over the
//! wrong bytes), and the transaction is simply unspendable forever. So the
//! derived-from-type-string hashes are pinned here against the values the
//! deployed Safe contract uses.

use alloy_primitives::{Address, U256, address, b256, hex, keccak256};
use labs::multisig::safe::{
    Operation, SafeTx, domain_separator, exec_transaction_calldata, pack_signatures, recover_owner,
    safe_tx_hash,
};

#[test]
fn type_hashes_match_the_deployed_safe_contract() {
    // Safe >= 1.3.0, `DOMAIN_SEPARATOR_TYPEHASH`.
    assert_eq!(
        keccak256(b"EIP712Domain(uint256 chainId,address verifyingContract)"),
        b256!("47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218"),
    );
    // Safe >= 1.0.0, `SAFE_TX_TYPEHASH`.
    assert_eq!(
        keccak256(
            b"SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,\
uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)"
        ),
        b256!("bb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8"),
    );
}

#[test]
fn exec_transaction_selector_is_0x6a761202() {
    let h = keccak256(
        b"execTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes)",
    );
    assert_eq!(&h[..4], &hex!("6a761202"));
}

/// The domain separator binds BOTH the chain and the Safe. If it did not, an
/// approval collected for a vault on one chain would be replayable against the
/// same Safe address on another (Safe addresses are frequently identical across
/// chains — that is the whole point of CREATE2 deployment).
#[test]
fn domain_separator_binds_chain_and_safe() {
    let safe = address!("1111111111111111111111111111111111111111");
    let other = address!("2222222222222222222222222222222222222222");
    assert_ne!(domain_separator(1, safe), domain_separator(8453, safe));
    assert_ne!(domain_separator(1, safe), domain_separator(1, other));
}

#[test]
fn safe_tx_hash_binds_every_field() {
    let safe = address!("1111111111111111111111111111111111111111");
    let to = address!("3333333333333333333333333333333333333333");
    let base = SafeTx::call(to, U256::from(1_000u64), vec![0xde, 0xad], 7);
    let h = safe_tx_hash(1, safe, &base);

    // value, data, nonce, recipient and chain each move the digest.
    let mut v = base.clone();
    v.value = U256::from(1_001u64);
    assert_ne!(h, safe_tx_hash(1, safe, &v));

    let mut d = base.clone();
    d.data = vec![0xde, 0xae];
    assert_ne!(h, safe_tx_hash(1, safe, &d));

    let mut n = base.clone();
    n.nonce = U256::from(8u64);
    assert_ne!(h, safe_tx_hash(1, safe, &n));

    let mut t = base.clone();
    t.to = address!("4444444444444444444444444444444444444444");
    assert_ne!(h, safe_tx_hash(1, safe, &t));

    assert_ne!(h, safe_tx_hash(8453, safe, &base));
}

#[test]
fn delegatecall_and_gas_refunds_are_refused() {
    let to = address!("3333333333333333333333333333333333333333");
    let mut tx = SafeTx::call(to, U256::ZERO, vec![], 0);
    assert!(tx.validate().is_ok());

    // DELEGATECALL runs foreign code against the Safe's own storage: one
    // approved proposal could rewrite the owner set and steal the vault.
    tx.operation = Operation::DelegateCall;
    assert!(tx.validate().is_err());

    // A non-zero refund makes the Safe pay the executor out of vault funds.
    let mut tx = SafeTx::call(to, U256::ZERO, vec![], 0);
    tx.gas_price = U256::from(1u64);
    assert!(tx.validate().is_err());

    let mut tx = SafeTx::call(to, U256::ZERO, vec![], 0);
    tx.refund_receiver = address!("5555555555555555555555555555555555555555");
    assert!(tx.validate().is_err());
}

/// Safe's `checkNSignatures` walks the blob in ascending owner order and
/// reverts on anything else, so this sort is load-bearing, not cosmetic.
#[test]
fn packed_signatures_are_sorted_by_owner_ascending() {
    let hi = (address!("ffffffffffffffffffffffffffffffffffffffff"), [0x11u8; 65]);
    let lo = (address!("0000000000000000000000000000000000000001"), [0x22u8; 65]);
    let packed = pack_signatures(vec![hi, lo]);
    assert_eq!(packed.len(), 130);
    // the low owner's signature bytes come first
    assert_eq!(packed[0], 0x22);
    assert_eq!(packed[65], 0x11);
}

#[test]
fn packed_signature_v_is_27_or_28() {
    let owner = address!("0000000000000000000000000000000000000001");
    let mut sig = [0x33u8; 65];
    sig[64] = 0; // parity form
    assert_eq!(pack_signatures(vec![(owner, sig)])[64], 27);
    sig[64] = 1;
    assert_eq!(pack_signatures(vec![(owner, sig)])[64], 28);
    sig[64] = 28; // already in ecrecover form
    assert_eq!(pack_signatures(vec![(owner, sig)])[64], 28);
}

/// A hand-checked ABI layout: selector, ten head words, then the two dynamic
/// tails. Getting the offsets wrong produces calldata that reverts on-chain, so
/// the encoding is asserted structurally rather than trusted.
#[test]
fn exec_transaction_calldata_abi_layout() {
    let to = address!("3333333333333333333333333333333333333333");
    let tx = SafeTx::call(to, U256::from(5u64), vec![0xaa, 0xbb, 0xcc], 1);
    let signatures = [0x77u8; 65];
    let cd = exec_transaction_calldata(&tx, &signatures);

    assert_eq!(&cd[..4], &hex!("6a761202"));
    let word = |i: usize| &cd[4 + 32 * i..4 + 32 * (i + 1)];

    // head[0] to, left-padded
    assert_eq!(&word(0)[12..], to.as_slice());
    // head[1] value
    assert_eq!(U256::from_be_slice(word(1)), U256::from(5u64));
    // head[2] offset to `data` == 10 words
    assert_eq!(U256::from_be_slice(word(2)), U256::from(320u64));
    // head[3] operation == CALL
    assert_eq!(U256::from_be_slice(word(3)), U256::ZERO);
    // head[9] offset to `signatures` == 320 + 32 (len) + 32 (3 bytes padded)
    assert_eq!(U256::from_be_slice(word(9)), U256::from(384u64));

    // data tail: length then right-padded content
    assert_eq!(U256::from_be_slice(word(10)), U256::from(3u64));
    assert_eq!(&cd[4 + 352..4 + 355], &[0xaa, 0xbb, 0xcc]);
    // signature tail: length then content
    assert_eq!(U256::from_be_slice(word(12)), U256::from(65u64));
    assert_eq!(&cd[4 + 416..4 + 416 + 65], &signatures[..]);
    // total length is 32-byte aligned after the selector
    assert_eq!((cd.len() - 4) % 32, 0);
}

// ---- ecrecover ------------------------------------------------------------

fn sign(key: &k256::ecdsa::SigningKey, digest: alloy_primitives::B256) -> [u8; 65] {
    let (sig, recid) = key
        .sign_prehash_recoverable(digest.as_slice())
        .expect("sign");
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&sig.to_bytes());
    out[64] = recid.to_byte();
    out
}

fn address_of(key: &k256::ecdsa::SigningKey) -> Address {
    let pk = key.verifying_key().to_encoded_point(false);
    // Ethereum address = last 20 bytes of keccak(uncompressed pubkey without the 0x04 tag)
    Address::from_slice(&keccak256(&pk.as_bytes()[1..])[12..])
}

#[test]
fn recover_owner_round_trips() {
    let key = k256::ecdsa::SigningKey::from_slice(&[0x42u8; 32]).unwrap();
    let safe = address!("1111111111111111111111111111111111111111");
    let tx = SafeTx::call(safe, U256::ZERO, vec![], 0);
    let hash = safe_tx_hash(1, safe, &tx);

    let sig = sign(&key, hash);
    assert_eq!(recover_owner(hash, &sig).unwrap(), address_of(&key));

    // a signature over a different digest recovers to someone else entirely —
    // which is exactly why the module compares the recovered address against
    // the owner set rather than "verifying" anything.
    let other = safe_tx_hash(1, safe, &SafeTx::call(safe, U256::from(1u64), vec![], 0));
    assert_ne!(recover_owner(other, &sig).unwrap(), address_of(&key));
}

/// Without the low-S check, (r, s, v) and (r, -s, v^1) recover the SAME owner.
/// A 2-of-3 vault could then be executed by one owner submitting both forms of
/// their single signature. `recover_owner` must refuse the malleated twin.
#[test]
fn high_s_signature_is_refused() {
    let key = k256::ecdsa::SigningKey::from_slice(&[0x42u8; 32]).unwrap();
    let safe = address!("1111111111111111111111111111111111111111");
    let hash = safe_tx_hash(1, safe, &SafeTx::call(safe, U256::ZERO, vec![], 0));
    let sig = sign(&key, hash);

    // k256 always emits low-S, so the honest signature is accepted...
    let owner = recover_owner(hash, &sig).expect("low-S signature is accepted");

    // ...and its malleated twin (s -> n - s, flipped recovery bit) must not be,
    // even though it recovers to the same address.
    let s = k256::NonZeroScalar::try_from(&sig[32..64]).unwrap();
    let neg_s = -s;
    let mut twin = [0u8; 65];
    twin[..32].copy_from_slice(&sig[..32]);
    twin[32..64].copy_from_slice(&neg_s.to_bytes());
    twin[64] = sig[64] ^ 1;

    let err = recover_owner(hash, &twin).expect_err("high-S twin must be refused");
    assert!(err.contains("low-S"), "unexpected error: {err}");

    // prove the twin really would have recovered to the same owner — i.e. the
    // check above is preventing a genuine double-count, not rejecting garbage.
    let recovered = alloy_primitives::Signature::from_raw_array(&twin)
        .unwrap()
        .recover_address_from_prehash(&hash)
        .unwrap();
    assert_eq!(recovered, owner);
}
