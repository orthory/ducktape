//! account-model tests: founding by number, multi-scheme associations, the
//! single-use add-key consent (per-key generation), re-linking a removed key,
//! the last-key guard, member gating of rename/profile, the cap, and abort —
//! all over the store-backed module (a [`MemStore`] test double; the qmdb
//! continuity proof lives in `tests/sync_round_trip.rs`).

use super::*;
use commonware_cryptography::Signer as _;
use futures::executor::block_on;
use sdk_testkit::{MemStore, TestCtx};

const CHAIN: &str = "test-chain";
/// the consensus time every op runs at unless a test names another.
const NOW: u64 = 100;
/// the expiry every consent carries unless a test names another -- inside
/// [`MAX_CONSENT_TTL`] of [`NOW`].
const EXPIRES: u64 = 1_000;

// ---- a minimal Ctx (the shared sdk-testkit double) ----------------------

fn ctx_at(key: &[u8], consensus_time: u64) -> TestCtx {
    TestCtx::with_env(sdk::Env {
        height: 0,
        consensus_time,
        origin: sdk::Origin::External(key.to_vec()),
        me: "identity".into(),
    })
}

fn ctx_external(key: &[u8]) -> TestCtx {
    ctx_at(key, NOW)
}

// ---- key builders (one per scheme) -----------------------------------------

type Ed = commonware_cryptography::ed25519::PrivateKey;

fn ed(seed: u64) -> Ed {
    Ed::from_seed(seed)
}
fn ed_pub(k: &Ed) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}
/// an ed25519 member's consent to admit `new_key` into `account` at `gen`.
fn ed_consent(
    member: &Ed,
    scheme: KeyScheme,
    new_key: &[u8],
    generation: u64,
    account: u64,
) -> Authorizer {
    ed_consent_until(member, scheme, new_key, generation, account, EXPIRES)
}

fn ed_consent_until(
    member: &Ed,
    scheme: KeyScheme,
    new_key: &[u8],
    generation: u64,
    account: u64,
    expires_at: u64,
) -> Authorizer {
    let preimage = add_key_preimage(CHAIN, scheme, new_key, generation, account, expires_at);
    Authorizer {
        key: ed_pub(member),
        account,
        expires_at,
        proof: keyscheme::testkit::ed25519_proof(member, IDENTITY_ADD_KEY_NS, &preimage),
    }
}

fn wallet(seed: u8) -> k256::ecdsa::SigningKey {
    keyscheme::testkit::eth_key(seed)
}
fn wallet_pub(k: &k256::ecdsa::SigningKey) -> Vec<u8> {
    keyscheme::testkit::eth_pubkey(k)
}
fn wallet_consent(
    member: &k256::ecdsa::SigningKey,
    scheme: KeyScheme,
    new_key: &[u8],
    generation: u64,
    account: u64,
) -> Authorizer {
    let preimage = add_key_preimage(CHAIN, scheme, new_key, generation, account, EXPIRES);
    Authorizer {
        key: wallet_pub(member),
        account,
        expires_at: EXPIRES,
        proof: keyscheme::testkit::eth_proof(member, IDENTITY_ADD_KEY_NS, &preimage),
    }
}

fn passkey(seed: u8) -> p256::ecdsa::SigningKey {
    keyscheme::testkit::passkey(seed)
}
fn passkey_pub(k: &p256::ecdsa::SigningKey) -> Vec<u8> {
    keyscheme::testkit::passkey_pubkey(k)
}
fn passkey_consent(
    member: &p256::ecdsa::SigningKey,
    scheme: KeyScheme,
    new_key: &[u8],
    generation: u64,
    account: u64,
) -> Authorizer {
    let preimage = add_key_preimage(CHAIN, scheme, new_key, generation, account, EXPIRES);
    Authorizer {
        key: passkey_pub(member),
        account,
        expires_at: EXPIRES,
        proof: keyscheme::testkit::passkey_proof(
            member,
            "ducktape",
            IDENTITY_ADD_KEY_NS,
            &preimage,
            true,
        ),
    }
}

// ---- harness ------------------------------------------------------------

fn new_identity() -> Identity {
    Identity::new("identity", Box::new(MemStore::new()), CHAIN.to_string())
}

/// execute a message from `origin`, then commit (or abort) the block.
fn apply(id: &mut Identity, origin: &[u8], msg: IdentityMsg) -> Result<(), Error> {
    apply_at(id, origin, msg, NOW)
}

/// [`apply`] at a chosen consensus time — how a test orders admissions.
fn apply_at(id: &mut Identity, origin: &[u8], msg: IdentityMsg, at: u64) -> Result<(), Error> {
    let mut ctx = ctx_at(origin, at);
    let m = Msg {
        target: "identity".into(),
        payload: encode_msg(&msg),
    };
    let r = block_on(id.execute(&mut ctx, &m));
    if r.is_ok() {
        block_on(id.commit_block()).unwrap();
    } else {
        block_on(id.abort_block()).unwrap();
    }
    r
}

fn create(id: &mut Identity, origin: &[u8], name: &str, scheme: KeyScheme) -> Result<(), Error> {
    apply(
        id,
        origin,
        IdentityMsg::Create {
            name: name.into(),
            scheme,
        },
    )
}

fn add_key(id: &mut Identity, origin: &[u8], scheme: KeyScheme, authorizer: Authorizer) -> Result<(), Error> {
    add_key_at(id, origin, scheme, authorizer, NOW)
}

fn add_key_at(
    id: &mut Identity,
    origin: &[u8],
    scheme: KeyScheme,
    authorizer: Authorizer,
    at: u64,
) -> Result<(), Error> {
    apply_at(
        id,
        origin,
        IdentityMsg::AddKey {
            scheme,
            label: None,
            authorizer,
        },
        at,
    )
}

fn reply(id: &Identity, q: IdentityQuery) -> IdentityReply {
    decode_reply(&block_on(id.query(&encode_query(&q))).unwrap()).unwrap()
}

fn get(id: &Identity, number: u64) -> Option<AccountView> {
    match reply(id, IdentityQuery::Get { number }) {
        IdentityReply::Account(a) => a,
        other => panic!("expected Account, got {other:?}"),
    }
}

fn of_key(id: &Identity, key: &[u8]) -> Option<AccountView> {
    match reply(id, IdentityQuery::OfKey { key: key.to_vec() }) {
        IdentityReply::Account(a) => a,
        other => panic!("expected Account, got {other:?}"),
    }
}

fn key_gen(id: &Identity, key: &[u8]) -> u64 {
    match reply(id, IdentityQuery::KeyGen { key: key.to_vec() }) {
        IdentityReply::Gen(g) => g,
        other => panic!("expected Gen, got {other:?}"),
    }
}

fn all(id: &Identity, from: u64, limit: u64) -> Vec<u64> {
    match reply(id, IdentityQuery::All { from, limit }) {
        IdentityReply::Accounts(a) => a.into_iter().map(|a| a.number).collect(),
        other => panic!("expected Accounts, got {other:?}"),
    }
}

fn refused(err: Error, needle: &str) {
    assert!(
        format!("{err:?}").contains(needle),
        "expected {needle:?}, got {err:?}"
    );
}

// ---- tests --------------------------------------------------------------

#[test]
fn create_founds_account_one_then_two() {
    let mut id = new_identity();
    let (a, b) = (ed(1), ed(2));
    create(&mut id, &ed_pub(&a), "alice", KeyScheme::Ed25519).unwrap();
    create(&mut id, &ed_pub(&b), "  bob ", KeyScheme::Ed25519).unwrap();

    let alice = get(&id, 1).expect("account 1");
    assert_eq!(alice.number, 1);
    assert_eq!(alice.name, "alice");
    assert_eq!(alice.keys.len(), 1);
    assert_eq!(alice.keys[0].pubkey, ed_pub(&a));
    assert_eq!(alice.keys[0].scheme, KeyScheme::Ed25519);
    assert_eq!(get(&id, 2).unwrap().name, "bob", "names are trimmed");
    assert!(get(&id, 3).is_none());
    assert_eq!(of_key(&id, &ed_pub(&b)).unwrap().number, 2);
    assert!(of_key(&id, &ed_pub(&ed(3))).is_none());
    assert_eq!(all(&id, 0, 16), vec![1, 2], "from 0 reads from 1");
    assert_eq!(key_gen(&id, &ed_pub(&ed(9))), 0, "a fresh key is at generation 0");
}

#[test]
fn a_key_founds_at_most_one_account_and_names_are_not_unique() {
    let mut id = new_identity();
    let a = ed(1);
    create(&mut id, &ed_pub(&a), "eddy", KeyScheme::Ed25519).unwrap();
    refused(
        create(&mut id, &ed_pub(&a), "again", KeyScheme::Ed25519).unwrap_err(),
        "already belongs to an account",
    );
    // display-only names: a second account may pick the same one.
    create(&mut id, &ed_pub(&ed(2)), "eddy", KeyScheme::Ed25519).unwrap();
    assert_eq!(all(&id, 0, 16), vec![1, 2]);
}

#[test]
fn add_key_admits_with_member_consent_and_the_consent_is_single_use() {
    let mut id = new_identity();
    let (founder, joiner) = (ed(1), ed(2));
    create(&mut id, &ed_pub(&founder), "alice", KeyScheme::Ed25519).unwrap();

    let consent = ed_consent(&founder, KeyScheme::Ed25519, &ed_pub(&joiner), 0, 1);
    apply(
        &mut id,
        &ed_pub(&joiner),
        IdentityMsg::AddKey {
            scheme: KeyScheme::Ed25519,
            label: Some(" laptop ".into()),
            authorizer: consent.clone(),
        },
    )
    .expect("joiner admitted");
    let acc = get(&id, 1).unwrap();
    assert_eq!(acc.keys.len(), 2);
    let joined = acc.keys.iter().find(|k| k.pubkey == ed_pub(&joiner)).unwrap();
    assert_eq!(joined.label.as_deref(), Some("laptop"));
    assert_eq!(of_key(&id, &ed_pub(&joiner)).unwrap().number, 1);
    assert_eq!(key_gen(&id, &ed_pub(&joiner)), 1, "admission advances the generation");

    // a member cannot be admitted twice ...
    refused(
        add_key(&mut id, &ed_pub(&joiner), KeyScheme::Ed25519, consent.clone()).unwrap_err(),
        "already belongs to an account",
    );
    // ... and the spent consent never verifies again, even for a fresh account.
    apply(&mut id, &ed_pub(&joiner), IdentityMsg::RemoveKey { key: ed_pub(&joiner) })
        .expect("joiner leaves");
    refused(
        add_key(&mut id, &ed_pub(&joiner), KeyScheme::Ed25519, consent).unwrap_err(),
        "consent does not verify",
    );
}

#[test]
fn a_removed_key_relinks_anywhere_at_its_next_generation() {
    let mut id = new_identity();
    let (a, b, k) = (ed(1), ed(2), ed(3));
    create(&mut id, &ed_pub(&a), "alice", KeyScheme::Ed25519).unwrap();
    create(&mut id, &ed_pub(&b), "bob", KeyScheme::Ed25519).unwrap();

    add_key(&mut id, &ed_pub(&k), KeyScheme::Ed25519, ed_consent(&a, KeyScheme::Ed25519, &ed_pub(&k), 0, 1)).unwrap();
    apply(&mut id, &ed_pub(&a), IdentityMsg::RemoveKey { key: ed_pub(&k) }).unwrap();
    assert!(of_key(&id, &ed_pub(&k)).is_none());
    assert_eq!(key_gen(&id, &ed_pub(&k)), 1, "removal keeps the counter");

    // bob's consent at the OLD generation is a forgery ...
    refused(
        add_key(&mut id, &ed_pub(&k), KeyScheme::Ed25519, ed_consent(&b, KeyScheme::Ed25519, &ed_pub(&k), 0, 2)).unwrap_err(),
        "consent does not verify",
    );
    // ... at the current one it relinks the key to bob's account.
    add_key(&mut id, &ed_pub(&k), KeyScheme::Ed25519, ed_consent(&b, KeyScheme::Ed25519, &ed_pub(&k), 1, 2)).unwrap();
    assert_eq!(of_key(&id, &ed_pub(&k)).unwrap().number, 2);
    assert_eq!(key_gen(&id, &ed_pub(&k)), 2);
    assert_eq!(get(&id, 1).unwrap().keys.len(), 1);
}

#[test]
fn a_removed_authorizer_cannot_consent() {
    let mut id = new_identity();
    let (a, b, k) = (ed(1), ed(2), ed(3));
    create(&mut id, &ed_pub(&a), "alice", KeyScheme::Ed25519).unwrap();
    add_key(&mut id, &ed_pub(&b), KeyScheme::Ed25519, ed_consent(&a, KeyScheme::Ed25519, &ed_pub(&b), 0, 1)).unwrap();
    // b mints a consent for k, then a evicts b.
    let stale = ed_consent(&b, KeyScheme::Ed25519, &ed_pub(&k), 0, 1);
    apply(&mut id, &ed_pub(&a), IdentityMsg::RemoveKey { key: ed_pub(&b) }).unwrap();
    refused(
        add_key(&mut id, &ed_pub(&k), KeyScheme::Ed25519, stale).unwrap_err(),
        "authorizer belongs to no account",
    );
}

/// the consent NAMES its account, so it dies with the authorizer's membership
/// of THAT account — it never follows the authorizer onto the next one.
#[test]
fn a_consent_never_follows_its_authorizer_to_another_account() {
    let mut id = new_identity();
    let (a, b, c, k) = (ed(1), ed(2), ed(3), ed(4));
    create(&mut id, &ed_pub(&a), "alice", KeyScheme::Ed25519).unwrap();
    create(&mut id, &ed_pub(&c), "carol", KeyScheme::Ed25519).unwrap();
    add_key(&mut id, &ed_pub(&b), KeyScheme::Ed25519, ed_consent(&a, KeyScheme::Ed25519, &ed_pub(&b), 0, 1)).unwrap();

    // b mints a ticket for k on account 1 and k sits on it, unspent.
    let stale = ed_consent(&b, KeyScheme::Ed25519, &ed_pub(&k), 0, 1);
    // b leaves account 1 and relinks to carol's account 2.
    apply(&mut id, &ed_pub(&a), IdentityMsg::RemoveKey { key: ed_pub(&b) }).unwrap();
    add_key(&mut id, &ed_pub(&b), KeyScheme::Ed25519, ed_consent(&c, KeyScheme::Ed25519, &ed_pub(&b), 1, 2)).unwrap();
    assert_eq!(of_key(&id, &ed_pub(&b)).unwrap().number, 2);

    // the untouched ticket does NOT admit its holder into account 2 ...
    refused(
        add_key(&mut id, &ed_pub(&k), KeyScheme::Ed25519, stale).unwrap_err(),
        "consent names account 1, its authorizer is on account 2",
    );
    // ... and naming account 2 in the payload is not a shortcut either: the
    // signature covers the account too.
    let mut forged = ed_consent(&b, KeyScheme::Ed25519, &ed_pub(&k), 0, 1);
    forged.account = 2;
    refused(
        add_key(&mut id, &ed_pub(&k), KeyScheme::Ed25519, forged).unwrap_err(),
        "consent does not verify",
    );
    assert!(of_key(&id, &ed_pub(&k)).is_none());
}

/// an unspent consent is not a permanent bearer credential: it dies on the
/// clock, and one minted to outlive the ceiling never verifies at all.
#[test]
fn a_consent_expires_and_cannot_outlive_the_ceiling() {
    let mut id = new_identity();
    let (a, k) = (ed(1), ed(2));
    create(&mut id, &ed_pub(&a), "alice", KeyScheme::Ed25519).unwrap();

    let consent = ed_consent_until(&a, KeyScheme::Ed25519, &ed_pub(&k), 0, 1, NOW + 10);
    refused(
        add_key_at(&mut id, &ed_pub(&k), KeyScheme::Ed25519, consent.clone(), NOW + 11).unwrap_err(),
        "consent expired at 110",
    );
    refused(
        add_key(
            &mut id,
            &ed_pub(&k),
            KeyScheme::Ed25519,
            ed_consent_until(&a, KeyScheme::Ed25519, &ed_pub(&k), 0, 1, NOW + MAX_CONSENT_TTL + 1),
        )
        .unwrap_err(),
        "outlives the",
    );
    // at its own consensus time it still admits.
    add_key_at(&mut id, &ed_pub(&k), KeyScheme::Ed25519, consent, NOW + 10).unwrap();
    assert_eq!(of_key(&id, &ed_pub(&k)).unwrap().number, 1);
}

#[test]
fn a_wallet_founds_and_a_passkey_and_a_wallet_consent() {
    let mut id = new_identity();
    let w = wallet(3);
    create(&mut id, &wallet_pub(&w), "wallet-first", KeyScheme::Secp256k1).unwrap();
    assert_eq!(get(&id, 1).unwrap().keys[0].scheme, KeyScheme::Secp256k1);

    // the wallet admits a passkey ...
    let p = passkey(0x42);
    add_key(&mut id, &passkey_pub(&p), KeyScheme::Secp256r1, wallet_consent(&w, KeyScheme::Secp256r1, &passkey_pub(&p), 0, 1)).unwrap();
    // ... the passkey admits an ed25519 device key (WebAuthn assertion consent).
    let d = ed(7);
    add_key(&mut id, &ed_pub(&d), KeyScheme::Ed25519, passkey_consent(&p, KeyScheme::Ed25519, &ed_pub(&d), 0, 1)).unwrap();
    let acc = get(&id, 1).unwrap();
    assert_eq!(acc.keys.len(), 3);
    assert_eq!(of_key(&id, &ed_pub(&d)).unwrap().number, 1);

    // a consent minted for a different scheme of the same bytes is a forgery.
    let e = ed(8);
    refused(
        add_key(&mut id, &ed_pub(&e), KeyScheme::Ed25519, passkey_consent(&p, KeyScheme::Secp256r1, &ed_pub(&e), 0, 1)).unwrap_err(),
        "consent does not verify",
    );
}

#[test]
fn a_malformed_key_for_its_declared_scheme_is_refused() {
    let mut id = new_identity();
    refused(
        create(&mut id, &ed_pub(&ed(1)), "x", KeyScheme::Secp256k1).unwrap_err(),
        "malformed for its scheme",
    );
    refused(
        create(&mut id, &[7u8; 5], "x", KeyScheme::Ed25519).unwrap_err(),
        "malformed for its scheme",
    );
    create(&mut id, &ed_pub(&ed(1)), "x", KeyScheme::Ed25519).unwrap();
    refused(
        add_key(&mut id, &[7u8; 5], KeyScheme::Ed25519, ed_consent(&ed(1), KeyScheme::Ed25519, &[7u8; 5], 0, 1)).unwrap_err(),
        "malformed for its scheme",
    );
}

#[test]
fn set_name_and_profile_are_member_gated_trimmed_and_capped() {
    let mut id = new_identity();
    let a = ed(1);
    create(&mut id, &ed_pub(&a), "alice", KeyScheme::Ed25519).unwrap();
    refused(
        apply(&mut id, &ed_pub(&ed(2)), IdentityMsg::SetName { name: "x".into() }).unwrap_err(),
        "origin key belongs to no account",
    );
    refused(
        apply(&mut id, &ed_pub(&a), IdentityMsg::SetName { name: "   ".into() }).unwrap_err(),
        "account name is empty",
    );
    refused(
        apply(&mut id, &ed_pub(&a), IdentityMsg::SetName { name: "n".repeat(MAX_NAME_LEN + 1) }).unwrap_err(),
        "exceeds",
    );
    refused(
        create(&mut id, &ed_pub(&ed(3)), "", KeyScheme::Ed25519).unwrap_err(),
        "account name is empty",
    );
    apply(&mut id, &ed_pub(&a), IdentityMsg::SetName { name: " Alice ".into() }).unwrap();
    apply(
        &mut id,
        &ed_pub(&a),
        IdentityMsg::SetProfile {
            avatar: Some("/shared/attachments/avatars/cafe.png".into()),
            bio: Some("  ".into()),
        },
    )
    .unwrap();
    let acc = get(&id, 1).unwrap();
    assert_eq!(acc.name, "Alice");
    assert_eq!(acc.avatar.as_deref(), Some("/shared/attachments/avatars/cafe.png"));
    assert_eq!(acc.bio, None, "empty trims clear");
    refused(
        apply(
            &mut id,
            &ed_pub(&a),
            IdentityMsg::SetProfile {
                avatar: None,
                bio: Some("b".repeat(MAX_BIO_LEN + 1)),
            },
        )
        .unwrap_err(),
        "bio exceeds",
    );
}

#[test]
fn the_account_cap_refuses_founding() {
    let mut id = new_identity();
    // stage the numbering at the cap, the way 65 536 foundings would have.
    id.store(NEXT_NUMBER_KEY.to_vec(), &(MAX_ACCOUNTS + 1));
    block_on(id.commit_block()).unwrap();
    refused(
        create(&mut id, &ed_pub(&ed(1)), "late", KeyScheme::Ed25519).unwrap_err(),
        "account cap reached",
    );
}

#[test]
fn abort_block_drops_staged_accounts() {
    let mut id = new_identity();
    let a = ed(1);
    let mut ctx = ctx_external(&ed_pub(&a));
    let m = Msg {
        target: "identity".into(),
        payload: encode_msg(&IdentityMsg::Create {
            name: "alice".into(),
            scheme: KeyScheme::Ed25519,
        }),
    };
    block_on(id.execute(&mut ctx, &m)).unwrap();
    assert!(get(&id, 1).is_some(), "staged writes are read-your-writes");
    block_on(id.abort_block()).unwrap();
    assert!(get(&id, 1).is_none());
    assert!(of_key(&id, &ed_pub(&a)).is_none());
    assert_eq!(all(&id, 0, 16), Vec::<u64>::new());
}

#[test]
fn all_pages_by_number() {
    let mut id = new_identity();
    for seed in 1..=3 {
        create(&mut id, &ed_pub(&ed(seed)), "n", KeyScheme::Ed25519).unwrap();
    }
    assert_eq!(all(&id, 2, 1), vec![2]);
    assert_eq!(all(&id, 2, 16), vec![2, 3]);
    assert_eq!(all(&id, 0, 0), Vec::<u64>::new());
    assert_eq!(all(&id, 9, 16), Vec::<u64>::new());
}
