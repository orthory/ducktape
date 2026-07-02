//! the V2 certificate SHAPE — the whole reason V2Bls exists.
//!
//! a V1 (ed25519) certificate is a COLLECTION of signatures: every quorum signer
//! contributes its own 64-byte signature, so certificates grow linearly with the
//! validator set. a V2 (bls multisig) certificate is ONE aggregated bls signature
//! plus a signer-index bitmap: the aggregate is CONSTANT-size no matter how many
//! validators signed, and only the bitmap (1 bit per participant) grows. this
//! test pins that shape down over the REAL scheme surface — dev-seeded dual-key
//! schemes (the exact construction bin/node's "bls-multisig" selector uses), a
//! real simplex `Subject::Finalize`, `sign` -> `assemble` -> `verify_certificate`
//! — by assembling finalization certificates at N=4 and N=40 and asserting:
//!
//!   1. the certificate still ATTRIBUTES: exactly the quorum's signer indices ride
//!      along in the bitmap (per-validator liveness/fault evidence keeps working),
//!   2. the aggregated-signature portion is byte-for-byte the SAME SIZE at both N
//!      (one bls signature, not a collection),
//!   3. the whole encoded certificate stays essentially FLAT (10x the validators
//!      adds only bitmap bytes), while the V1 twin grows by 64 bytes per extra
//!      quorum signer, and
//!   4. a verifier-only scheme accepts the assembled certificate (it is a REAL
//!      certificate, not just a plausibly-shaped struct).

use commonware_codec::EncodeSize as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::simplex::types::{Proposal, Subject};
use commonware_consensus::types::{Epoch, Round, View};
use commonware_cryptography::certificate::Scheme as _;
use commonware_cryptography::{Signer as _, ed25519};
use commonware_parallel::Sequential;
use commonware_utils::{
    Faults as _, N3f1, TryCollect as _, ordered::BiMap, ordered::Set, test_rng,
};

use consensus::{BlsCertificate, BlsScheme, bls_dev_scheme, digest_of};

const NAMESPACE: &[u8] = b"cert-shape";

/// the one finalized proposal every certificate in this test covers.
fn proposal() -> Proposal<consensus::Digest> {
    Proposal::new(
        Round::new(Epoch::new(0), View::new(1)),
        View::new(0),
        digest_of(b"the finalized frame"),
    )
}

/// assemble a V2 finalization certificate over an n-validator dev set: quorum
/// signers sign `Subject::Finalize`, one of them aggregates. returns the quorum
/// and the certificate.
fn bls_finalization_certificate(n: u64) -> (usize, BlsCertificate) {
    let seeds: Vec<u64> = (0..n).collect();
    let schemes: Vec<BlsScheme> = seeds
        .iter()
        .map(|s| bls_dev_scheme(NAMESPACE, &seeds, *s).expect("dev key in the set"))
        .collect();
    let proposal = proposal();
    let quorum = N3f1::quorum(n as u32) as usize;
    let attestations: Vec<_> = schemes
        .iter()
        .take(quorum)
        .map(|s| {
            s.sign(Subject::Finalize {
                proposal: &proposal,
            })
            .expect("signer signs")
        })
        .collect();
    let certificate = schemes[0]
        .assemble::<_, N3f1>(attestations, &Sequential)
        .expect("quorum assembles");
    (quorum, certificate)
}

/// the V1 contrast: the encoded size of an ed25519 finalization certificate over
/// an n-validator dev set (a COLLECTION of quorum signatures).
fn ed25519_certificate_size(n: u64) -> (usize, usize) {
    let keys: Vec<ed25519::PrivateKey> = (0..n).map(ed25519::PrivateKey::from_seed).collect();
    let participants: Set<ed25519::PublicKey> =
        Set::try_from(keys.iter().map(|k| k.public_key()).collect::<Vec<_>>())
            .expect("distinct dev keys");
    let schemes: Vec<simplex_ed25519::Scheme> = keys
        .iter()
        .map(|k| {
            simplex_ed25519::Scheme::signer(NAMESPACE, participants.clone(), k.clone())
                .expect("dev key in the set")
        })
        .collect();
    let proposal = proposal();
    let quorum = N3f1::quorum(n as u32) as usize;
    let attestations: Vec<_> = schemes
        .iter()
        .take(quorum)
        .map(|s| {
            s.sign(Subject::Finalize {
                proposal: &proposal,
            })
            .expect("signer signs")
        })
        .collect();
    let certificate = schemes[0]
        .assemble::<_, N3f1>(attestations, &Sequential)
        .expect("quorum assembles");
    (quorum, certificate.encode_size())
}

#[test]
fn a_bls_certificate_is_one_aggregated_signature_plus_signer_indices() {
    let (quorum_small, small) = bls_finalization_certificate(4);
    let (quorum_large, large) = bls_finalization_certificate(40);

    // 1. attribution survives aggregation: exactly the quorum's indices are set.
    assert_eq!(
        small.signers.count(),
        quorum_small,
        "small cert carries quorum signer indices"
    );
    assert_eq!(
        large.signers.count(),
        quorum_large,
        "large cert carries quorum signer indices"
    );

    // 2. ONE aggregated signature: its encoding does not grow with the set — 27
    //    signers aggregate into exactly as many bytes as 3 did.
    assert_eq!(
        small.signature.encode_size(),
        large.signature.encode_size(),
        "the aggregate signature is constant-size at any validator count"
    );

    // 3. the certificate stays FLAT: 10x the validators (4 -> 40, quorum 3 -> 27)
    //    adds only signer-bitmap bytes (1 bit per participant), never signatures.
    let growth = large.encode_size() - small.encode_size();
    assert!(
        growth <= 16,
        "10x validators must only add bitmap bytes, grew by {growth}"
    );

    // ... while the V1 collection-of-signatures certificate grows by 64 bytes per
    // additional quorum signer — the linear cost V2 exists to delete.
    let (ed_quorum_small, ed_small) = ed25519_certificate_size(4);
    let (ed_quorum_large, ed_large) = ed25519_certificate_size(40);
    let ed_growth = ed_large - ed_small;
    assert!(
        ed_growth >= 64 * (ed_quorum_large - ed_quorum_small),
        "the V1 contrast grows a full signature per extra quorum signer, grew by {ed_growth}"
    );
}

#[test]
fn an_assembled_bls_certificate_verifies_against_a_verifier_only_scheme() {
    // the shape assertions above mean nothing unless what was assembled IS a
    // certificate — a verifier holding only the (identity -> bls) participant map
    // (no secret) must accept it for the same subject.
    let n = 4u64;
    let (_, certificate) = bls_finalization_certificate(n);
    let seeds: Vec<u64> = (0..n).collect();
    // any member's scheme doubles as a verifier, but build the REAL verifier-only
    // instance to prove no signing key is needed to check a certificate.
    let participants: BiMap<ed25519::PublicKey, consensus::BlsPublicKey> = seeds
        .iter()
        .map(|s| {
            (
                ed25519::PrivateKey::from_seed(*s).public_key(),
                consensus::bls_dev_public(*s),
            )
        })
        .try_collect()
        .expect("distinct dev keys");
    let verifier = BlsScheme::verifier(NAMESPACE, participants);
    let proposal = proposal();
    assert!(
        verifier.verify_certificate::<_, _, N3f1>(
            &mut test_rng(),
            Subject::Finalize {
                proposal: &proposal
            },
            &certificate,
            &Sequential,
        ),
        "a verifier-only scheme accepts the aggregated certificate"
    );
}
