use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};

use crate::host_reads::joiner_epoch_mesh;

fn pubkey_bytes(seed: &[u8; 32]) -> Vec<u8> {
    ed25519::PrivateKey::decode(seed.as_slice())
        .unwrap()
        .public_key()
        .as_ref()
        .to_vec()
}

// regression: a parked joiner must track the SAME epoch mesh as every
// member — `descriptor_mesh ∪ participants ∪ residents`. discovery kills a
// peer whose bit-vector length disagrees at a shared index, so a joiner that
// drops the manifest's residents (its own grant included) tracks a shorter
// set and is torn down on every gossip round — the churn the sentry +
// coordinator resident hit (`bit vector length mismatch expected=2 actual=3`).
#[test]
fn joiner_epoch_mesh_folds_members_and_residents() {
    let founder = ed25519::PrivateKey::decode([1u8; 32].as_slice())
        .unwrap()
        .public_key();
    let lobby = ed25519::PrivateKey::decode([2u8; 32].as_slice())
        .unwrap()
        .public_key();
    let resident = ed25519::PrivateKey::decode([3u8; 32].as_slice())
        .unwrap()
        .public_key();
    // the descriptor mesh every member carries: founder + derived lobby key.
    let descriptor_mesh = vec![founder, lobby];
    // the manifest a member serves once the resident's grant has committed:
    // participants = validators, residents = the granted resident (itself).
    let participants = vec![pubkey_bytes(&[1u8; 32])]; // founder
    let residents = vec![pubkey_bytes(&[3u8; 32])]; // the resident

    let set = joiner_epoch_mesh(&descriptor_mesh, &participants, &residents);

    assert!(
        set.position(&resident).is_some(),
        "joiner dropped the manifest resident — discovery will kill the link \
             on a bit-vector length mismatch"
    );
    assert_eq!(
        set.len(),
        3,
        "every member tracks 3 (founder, lobby, resident); a shorter joiner \
             set is torn down every discovery round"
    );
}
