//! the pluggable member-key verifier -- an account is an umbrella over member
//! keys of different KINDS, and this module is the one place that knows how to
//! turn "(kind, public key, proof)" into a yes/no over a chain-scoped preimage.
//!
//! membership operations (add/remove a member key, bind/unbind a node) are
//! scheme-INDEPENDENT: each hashes a chain-and-nonce-scoped preimage and asks
//! this module "does `proof` demonstrate that the holder of `pubkey` (read as
//! `kind`) authorized `preimage` under `namespace`?". the account-state logic
//! in `lib.rs` never touches a curve; it speaks only [`verify_authority`].
//!
//! ## determinism (the whole job)
//!
//! every verify here is a pure boolean over bytes -- no clock, no RNG, no
//! float, no ambient I/O -- so every validator reaches the same verdict or the
//! chain forks. the two native kinds ride commonware's vetted `Verifier`
//! (namespace-prefixed ECDSA/EdDSA); the WebAuthn kind uses the pure-Rust
//! `p256` verify on every architecture (never the arch-gated native backend),
//! so its accept/reject is byte-identical fleet-wide.
//!
//! ## the kind set is CLOSED and versioned
//!
//! a validator must recognize EVERY kind it might see, or two honest nodes
//! disagree on an op's validity. so kinds are a compiled enum, never a runtime
//! plugin table: adding one is a coordinated protocol change (every node ships
//! the new verify), and that is exactly the safe reading of "support any
//! scheme". a future `Secp256k1`/`Bls12381` is a new variant plus one verify
//! arm here -- see the note on [`KeyKind`].
//!
//! ## why WebAuthn is its own kind, not "P256 again"
//!
//! a passkey never signs our bytes. it signs the fixed WebAuthn structure
//! `authenticatorData ‖ SHA-256(clientDataJSON)`, and the ONLY field we control
//! is `clientDataJSON.challenge`. so our preimage can't be handed to it
//! directly: it is hashed into the challenge, and verification is an ENVELOPE
//! check (parse clientDataJSON, match the challenge, check the user-presence
//! flag and the RP id hash, then verify a raw ECDSA-P256 signature over the
//! reconstructed signed bytes). the signature is raw `R‖S` here: the native
//! FIDO2 transport normalizes the authenticator's ASN.1/DER down to 64 bytes
//! before it ever reaches consensus, so this verifier stays DER-free.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// the closed, versioned set of member-key schemes. adding a variant is a
/// coordinated protocol change (every validator must ship its verify arm), so
/// this is an enum, not a plugin registry.
///
/// future variants slot in behind the same [`verify_authority`] dispatch:
/// `Secp256k1` (needs the `k256` crate AND a shim replicating commonware's
/// `union_unique` signing preimage, so the native signer stays uniform) and
/// `Bls12381` (commonware ships it, but as a multisig primitive -- a single-sig
/// verify path has to be lifted out first). both are deliberately out of this
/// PR: hand-rolling a curve's signing-preimage format inside a consensus
/// module is a correctness footgun, so we ship the kinds commonware already
/// covers uniformly, plus the inherently-bespoke WebAuthn envelope.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
    /// ed25519 -- the founding/desktop seed key, and the only kind a v1
    /// account can be CREATED with (a passkey can only JOIN, since its
    /// ceremony runs off-device). verified via commonware's namespaced EdDSA.
    Ed25519,
    /// a native NIST P-256 (secp256r1) key that signs with commonware's
    /// discipline -- e.g. a hardware key used as a raw signer. verified via
    /// commonware's namespaced ECDSA.
    P256,
    /// a WebAuthn/FIDO2 passkey on P-256. NOT a plain signer: verification is
    /// the WebAuthn assertion envelope (see the module docs). the stored
    /// public key is the raw SEC1 point the transport lifted out of the COSE
    /// credential, so this module never parses CBOR.
    WebauthnP256,
}

impl KeyKind {
    /// the stable one-byte wire tag for this kind. folded into signing
    /// preimages (so a consent is bound to the exact kind) and the canonical
    /// snapshot encoding (so `root()` commits to it). NEVER renumber an
    /// existing tag -- it would silently reinterpret committed state and fork
    /// the chain; only append.
    pub fn tag(self) -> u8 {
        match self {
            KeyKind::Ed25519 => 0,
            KeyKind::P256 => 1,
            KeyKind::WebauthnP256 => 2,
        }
    }

    /// the inverse of [`KeyKind::tag`]; `None` for an unknown tag (a byzantine
    /// snapshot or a newer node's kind this build doesn't recognize -- either
    /// way, reject rather than guess).
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(KeyKind::Ed25519),
            1 => Some(KeyKind::P256),
            2 => Some(KeyKind::WebauthnP256),
            _ => None,
        }
    }

    /// whether a member of this kind carries an RP-id-hash pin (WebAuthn only).
    /// the canonical encoding and its strict decoder enforce this 1:1, so a
    /// snapshot can't smuggle a pin onto a native key or drop it from a passkey.
    pub fn expects_rp_id_hash(self) -> bool {
        matches!(self, KeyKind::WebauthnP256)
    }

    /// a fast, allocation-free well-formedness check on a public key's bytes
    /// for this kind -- used to reject a malformed `new_key` at the top of an
    /// add before any signature work. it is NOT a substitute for the proof
    /// check; it only rules out bytes that could never be a key of this kind.
    pub fn pubkey_wellformed(self, pubkey: &[u8]) -> bool {
        match self {
            // commonware ed25519 public keys are a fixed 32 bytes.
            KeyKind::Ed25519 => pubkey.len() == 32,
            // both native and WebAuthn P-256 keys are stored as the 33-byte
            // compressed SEC1 point; accept an uncompressed 65-byte point too,
            // since some authenticators emit that and it round-trips 1:1.
            KeyKind::P256 | KeyKind::WebauthnP256 => {
                p256::ecdsa::VerifyingKey::from_sec1_bytes(pubkey).is_ok()
            }
        }
    }
}

/// a proof that the holder of a member key authorized a specific preimage --
/// scheme-tagged so a native curve carries a bare signature and a passkey
/// carries its whole assertion envelope.
///
/// the enum is untagged-by-shape at the type level but tagged on the wire
/// (`serde` external tagging); a `(kind, proof)` shape mismatch (e.g. a
/// `Signature` proof for a `WebauthnP256` key) never verifies -- see
/// [`verify_authority`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemberProof {
    /// a commonware namespace-scoped signature over the raw preimage, for the
    /// native kinds (`Ed25519` / `P256`).
    Signature { sig: Vec<u8> },
    /// a WebAuthn assertion: the passkey signed `authenticator_data ‖
    /// SHA-256(client_data_json)`, and `client_data_json.challenge` carries our
    /// preimage hash. `signature` is raw `R‖S` (DER already normalized away by
    /// the transport).
    Webauthn {
        authenticator_data: Vec<u8>,
        client_data_json: Vec<u8>,
        signature: Vec<u8>,
    },
}

/// the `type` a WebAuthn *assertion* (`navigator.credentials.get`) stamps into
/// clientDataJSON. a registration ("webauthn.create") assertion is rejected:
/// possession proofs must come from a `get` over our challenge.
const WEBAUTHN_GET_TYPE: &str = "webauthn.get";

/// authenticatorData is at minimum rpIdHash(32) ‖ flags(1) ‖ signCount(4).
const AUTH_DATA_MIN_LEN: usize = 37;
/// flags bit 0: User Present (a human interacted with the authenticator).
const FLAG_USER_PRESENT: u8 = 0x01;

/// the WebAuthn challenge our chain demands for `(namespace, preimage)`:
/// `SHA-256(namespace ‖ preimage)`. folding the namespace in gives a passkey
/// the SAME domain separation commonware's `verify` gives the native kinds,
/// so an assertion minted to "add member X" can never be replayed as
/// "unbind node Y".
///
/// PUBLIC so the ENROLLMENT side (the node verb that tells the phone what to
/// sign) computes the challenge from the exact same bytes the verifier checks
/// against — one source of truth, no drift between "what was signed" and "what
/// is accepted".
pub fn webauthn_challenge(namespace: &[u8], preimage: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(namespace);
    h.update(preimage);
    h.finalize().into()
}

/// does `proof` demonstrate that the holder of `pubkey` (read as `kind`)
/// authorized `preimage` under `namespace`?
///
/// `rp_id_hash` matters only for `WebauthnP256`: `Some(h)` ENFORCES that the
/// assertion's authenticatorData carries rpIdHash `h` (used once a passkey is a
/// stored member, pinning it to the RP it enrolled under); `None` skips that
/// pin (used at enrollment, where the proof itself establishes the RP -- the
/// caller then reads it back via [`webauthn_rp_id_hash`] and stores it).
pub fn verify_authority(
    kind: KeyKind,
    pubkey: &[u8],
    rp_id_hash: Option<&[u8; 32]>,
    namespace: &[u8],
    preimage: &[u8],
    proof: &MemberProof,
) -> bool {
    match (kind, proof) {
        (KeyKind::Ed25519, MemberProof::Signature { sig }) => {
            verify_ed25519(pubkey, namespace, preimage, sig)
        }
        (KeyKind::P256, MemberProof::Signature { sig }) => {
            verify_p256_native(pubkey, namespace, preimage, sig)
        }
        (
            KeyKind::WebauthnP256,
            MemberProof::Webauthn {
                authenticator_data,
                client_data_json,
                signature,
            },
        ) => verify_webauthn_p256(
            pubkey,
            rp_id_hash,
            namespace,
            preimage,
            authenticator_data,
            client_data_json,
            signature,
        ),
        // any (kind, proof-shape) mismatch is a categorical no: a native key
        // must present a `Signature`, a passkey a `Webauthn` envelope.
        _ => false,
    }
}

/// read the RP id hash (authenticatorData[0..32]) out of a WebAuthn proof, so
/// the caller can pin a freshly enrolled passkey to its RP. `None` for a
/// non-WebAuthn proof or a truncated envelope.
pub fn webauthn_rp_id_hash(proof: &MemberProof) -> Option<[u8; 32]> {
    match proof {
        MemberProof::Webauthn {
            authenticator_data, ..
        } if authenticator_data.len() >= 32 => {
            Some(authenticator_data[..32].try_into().expect("32 bytes"))
        }
        _ => None,
    }
}

// ---- per-kind verifiers -------------------------------------------------

/// commonware namespaced EdDSA over the raw preimage.
fn verify_ed25519(pubkey: &[u8], namespace: &[u8], preimage: &[u8], sig: &[u8]) -> bool {
    use commonware_codec::DecodeExt as _;
    use commonware_cryptography::{
        Verifier as _,
        ed25519::{PublicKey, Signature},
    };
    let (Ok(pk), Ok(sig)) = (PublicKey::decode(pubkey), Signature::decode(sig)) else {
        return false;
    };
    pk.verify(namespace, preimage, &sig)
}

/// commonware namespaced ECDSA-P256 over the raw preimage.
fn verify_p256_native(pubkey: &[u8], namespace: &[u8], preimage: &[u8], sig: &[u8]) -> bool {
    use commonware_codec::DecodeExt as _;
    use commonware_cryptography::{
        Verifier as _,
        secp256r1::standard::{PublicKey, Signature},
    };
    let (Ok(pk), Ok(sig)) = (PublicKey::decode(pubkey), Signature::decode(sig)) else {
        return false;
    };
    pk.verify(namespace, preimage, &sig)
}

/// the WebAuthn assertion envelope (see the module docs).
#[allow(clippy::too_many_arguments)]
fn verify_webauthn_p256(
    pubkey: &[u8],
    rp_id_hash: Option<&[u8; 32]>,
    namespace: &[u8],
    preimage: &[u8],
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &[u8],
) -> bool {
    use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier as _};

    // 1. authenticatorData shape + user-presence. (signCount replay is not
    //    tracked: our preimage carries the account nonce, so a replayed
    //    assertion fails the nonce check in `lib.rs` regardless.)
    if authenticator_data.len() < AUTH_DATA_MIN_LEN {
        return false;
    }
    if authenticator_data[32] & FLAG_USER_PRESENT == 0 {
        return false;
    }
    // 2. RP pin (only once the member is stored): the assertion must come from
    //    the same RP the passkey enrolled under.
    if let Some(expected) = rp_id_hash
        && &authenticator_data[..32] != expected.as_slice()
    {
        return false;
    }

    // 3. clientDataJSON: a `get` assertion whose challenge is exactly ours.
    #[derive(Deserialize)]
    struct ClientData {
        #[serde(rename = "type")]
        type_: String,
        challenge: String,
    }
    let Ok(client) = serde_json::from_slice::<ClientData>(client_data_json) else {
        return false;
    };
    if client.type_ != WEBAUTHN_GET_TYPE {
        return false;
    }
    let Ok(challenge) = URL_SAFE_NO_PAD.decode(client.challenge.as_bytes()) else {
        return false;
    };
    if challenge != webauthn_challenge(namespace, preimage) {
        return false;
    }

    // 4. raw ECDSA-P256-SHA256 over `authenticatorData ‖ SHA-256(clientDataJSON)`.
    let (Ok(vk), Ok(sig)) = (
        VerifyingKey::from_sec1_bytes(pubkey),
        Signature::from_slice(signature),
    ) else {
        return false;
    };
    let mut signed = Vec::with_capacity(authenticator_data.len() + 32);
    signed.extend_from_slice(authenticator_data);
    signed.extend_from_slice(&Sha256::digest(client_data_json));
    vk.verify(&signed, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    const NS: &[u8] = b"ducktape-identity-add-member-v1";
    const OTHER_NS: &[u8] = b"ducktape-identity-remove-member-v1";

    // ---- native kinds ----------------------------------------------------

    #[test]
    fn ed25519_roundtrips_and_is_namespace_and_preimage_bound() {
        let signer = commonware_cryptography::ed25519::PrivateKey::from_seed(7);
        let pubkey = signer.public_key();
        let preimage = b"chain|account|newkey|nonce";
        let sig = signer.sign(NS, preimage);
        let proof = MemberProof::Signature {
            sig: sig.as_ref().to_vec(),
        };
        let pk = pubkey.as_ref();

        assert!(verify_authority(
            KeyKind::Ed25519,
            pk,
            None,
            NS,
            preimage,
            &proof
        ));
        // wrong namespace, wrong preimage, and wrong kind all fail.
        assert!(!verify_authority(
            KeyKind::Ed25519,
            pk,
            None,
            OTHER_NS,
            preimage,
            &proof
        ));
        assert!(!verify_authority(
            KeyKind::Ed25519,
            pk,
            None,
            NS,
            b"different",
            &proof
        ));
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            pk,
            None,
            NS,
            preimage,
            &proof
        ));
    }

    #[test]
    fn p256_native_roundtrips() {
        let signer = commonware_cryptography::secp256r1::standard::PrivateKey::from_seed(9);
        let pubkey = signer.public_key();
        let preimage = b"chain|account|newkey|nonce";
        let sig = signer.sign(NS, preimage);
        let proof = MemberProof::Signature {
            sig: sig.as_ref().to_vec(),
        };
        assert!(verify_authority(
            KeyKind::P256,
            pubkey.as_ref(),
            None,
            NS,
            preimage,
            &proof
        ));
        // a native-P256 signature is NOT a valid WebAuthn envelope.
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            pubkey.as_ref(),
            None,
            NS,
            preimage,
            &proof
        ));
    }

    #[test]
    fn p256_raw_ecdsa_over_the_signing_payload_verifies() {
        // The in-app LAN enrollment mints a real P256 member from a phone using
        // a pure-JS signer (@noble/curves): it signs `add_member_signing_payload`
        // with RAW ECDSA-P256-SHA256 (low-S), NOT commonware's Signer. Prove
        // that still verifies as KeyKind::P256 — i.e. the node's payload bytes +
        // a raw ECDSA sig round-trip through the on-chain verifier.
        use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

        let sk = SigningKey::from_slice(&[0x33u8; 32]).expect("valid scalar");
        // compressed SEC1 — the form commonware's secp256r1 PublicKey decodes.
        let new_key = sk.verifying_key().to_encoded_point(true).as_bytes().to_vec();
        let account_id = [0xaau8; 33];

        let payload =
            crate::add_member_signing_payload("chain-a", &account_id, &new_key, KeyKind::P256, 7);
        let sig: Signature = sk.sign(&payload);
        let proof = MemberProof::Signature { sig: sig.to_bytes().to_vec() };

        let preimage =
            crate::add_member_preimage("chain-a", &account_id, &new_key, KeyKind::P256, 7);
        assert!(verify_authority(
            KeyKind::P256,
            &new_key,
            None,
            crate::IDENTITY_ADD_MEMBER_NS,
            &preimage,
            &proof,
        ));

        // a signature over a different payload (nonce 8) must not verify at 7.
        let stale: Signature = sk.sign(&crate::add_member_signing_payload(
            "chain-a",
            &account_id,
            &new_key,
            KeyKind::P256,
            8,
        ));
        let stale_proof = MemberProof::Signature { sig: stale.to_bytes().to_vec() };
        assert!(!verify_authority(
            KeyKind::P256,
            &new_key,
            None,
            crate::IDENTITY_ADD_MEMBER_NS,
            &preimage,
            &stale_proof,
        ));
    }

    #[test]
    fn wellformed_rejects_wrong_lengths() {
        assert!(KeyKind::Ed25519.pubkey_wellformed(&[0u8; 32]));
        assert!(!KeyKind::Ed25519.pubkey_wellformed(&[0u8; 33]));
        // a genuine compressed P-256 point is well-formed; random 33 bytes are
        // overwhelmingly not on-curve.
        let vk = p256_signing_key(0x11).verifying_key().to_sec1_bytes();
        assert!(KeyKind::P256.pubkey_wellformed(&vk));
        assert!(!KeyKind::P256.pubkey_wellformed(&[0u8; 33]));
    }

    // ---- WebAuthn envelope ----------------------------------------------

    fn p256_signing_key(seed: u8) -> p256::ecdsa::SigningKey {
        p256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
    }

    /// build a self-consistent WebAuthn assertion for `(namespace, preimage)`:
    /// mirrors exactly what an authenticator produces, so a passing verify
    /// proves our envelope reconstruction matches real signing.
    fn make_webauthn_proof(
        signing_key: &p256::ecdsa::SigningKey,
        rp_id: &str,
        namespace: &[u8],
        preimage: &[u8],
        user_present: bool,
    ) -> (Vec<u8>, MemberProof) {
        use p256::ecdsa::{Signature, signature::Signer as _};

        let challenge = webauthn_challenge(namespace, preimage);
        let client_data_json = format!(
            r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://ducktape.local"}}"#,
            URL_SAFE_NO_PAD.encode(challenge)
        )
        .into_bytes();

        let mut authenticator_data = Vec::new();
        authenticator_data.extend_from_slice(&Sha256::digest(rp_id.as_bytes()));
        authenticator_data.push(if user_present { FLAG_USER_PRESENT } else { 0 });
        authenticator_data.extend_from_slice(&0u32.to_be_bytes()); // signCount

        let mut signed = authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&client_data_json));
        // RustCrypto signs deterministically (RFC6979) and low-S normalized;
        // `.to_bytes()` is the raw R‖S the transport would hand us.
        let sig: Signature = signing_key.sign(&signed);

        let pubkey = signing_key.verifying_key().to_sec1_bytes().to_vec();
        let proof = MemberProof::Webauthn {
            authenticator_data,
            client_data_json,
            signature: sig.to_bytes().to_vec(),
        };
        (pubkey, proof)
    }

    #[test]
    fn webauthn_assertion_verifies_and_binds_challenge() {
        let sk = p256_signing_key(0x21);
        let preimage = b"chain|account|newkey|nonce-0";
        let (pubkey, proof) = make_webauthn_proof(&sk, "ducktape", NS, preimage, true);

        // enrollment-time: no RP pin yet, but everything else must check out.
        assert!(verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            None,
            NS,
            preimage,
            &proof
        ));
        // the challenge is bound: a different namespace or preimage fails,
        // because the signed challenge no longer matches.
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            None,
            OTHER_NS,
            preimage,
            &proof
        ));
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            None,
            NS,
            b"chain|account|newkey|nonce-1",
            &proof
        ));
    }

    #[test]
    fn webauthn_requires_user_presence() {
        let sk = p256_signing_key(0x22);
        let preimage = b"up-flag-test";
        let (pubkey, proof) = make_webauthn_proof(&sk, "ducktape", NS, preimage, false);
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            None,
            NS,
            preimage,
            &proof
        ));
    }

    #[test]
    fn webauthn_rp_id_pin_enforced_when_stored() {
        let sk = p256_signing_key(0x23);
        let preimage = b"rp-pin-test";
        let (pubkey, proof) = make_webauthn_proof(&sk, "ducktape", NS, preimage, true);

        let good = webauthn_rp_id_hash(&proof).expect("webauthn proof carries an rp id hash");
        assert_eq!(good, Sha256::digest(b"ducktape").as_slice());
        // the pin the enrollment established passes; a different RP's hash fails
        // even though the signature itself is valid.
        assert!(verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            Some(&good),
            NS,
            preimage,
            &proof
        ));
        let wrong: [u8; 32] = Sha256::digest(b"evil.example").into();
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            Some(&wrong),
            NS,
            preimage,
            &proof
        ));
    }

    #[test]
    fn webauthn_rejects_tampered_signature() {
        let sk = p256_signing_key(0x24);
        let preimage = b"tamper-test";
        let (pubkey, mut proof) = make_webauthn_proof(&sk, "ducktape", NS, preimage, true);
        if let MemberProof::Webauthn { signature, .. } = &mut proof {
            signature[0] ^= 0xff;
        }
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            None,
            NS,
            preimage,
            &proof
        ));
    }

    #[test]
    fn webauthn_rejects_registration_type() {
        // a `webauthn.create` clientData must not pass as a possession proof.
        let sk = p256_signing_key(0x25);
        let preimage = b"type-test";
        let challenge = webauthn_challenge(NS, preimage);
        let client_data_json = format!(
            r#"{{"type":"webauthn.create","challenge":"{}","origin":"x"}}"#,
            URL_SAFE_NO_PAD.encode(challenge)
        )
        .into_bytes();
        let mut authenticator_data = Vec::new();
        authenticator_data.extend_from_slice(&Sha256::digest(b"ducktape"));
        authenticator_data.push(FLAG_USER_PRESENT);
        authenticator_data.extend_from_slice(&0u32.to_be_bytes());
        let mut signed = authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&client_data_json));
        use p256::ecdsa::{Signature, signature::Signer as _};
        let sig: Signature = sk.sign(&signed);
        let proof = MemberProof::Webauthn {
            authenticator_data,
            client_data_json,
            signature: sig.to_bytes().to_vec(),
        };
        let pubkey = sk.verifying_key().to_sec1_bytes().to_vec();
        assert!(!verify_authority(
            KeyKind::WebauthnP256,
            &pubkey,
            None,
            NS,
            preimage,
            &proof
        ));
    }
}
