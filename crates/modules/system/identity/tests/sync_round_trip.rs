//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `Identity` around the injected store — the same
//! discriminating property chat, pages, agent, automations, and governance
//! prove, over the account + key-index + generation-counter layout.
//!
//! the source SEEDS the `__config` chain-id record exactly the way the
//! production genesis path does (`bin/node/src/host_state.rs`
//! `seed_store_config`), then founds an account, admits a WebAuthn passkey
//! (the consent verifies in-module), REMOVES it again (the op log carries an
//! index DELETE, not just inserts, and the generation counter survives), and
//! sets the profile (record overwrites). only a real sync that ships the
//! ACTUAL proven op range lands on the same root — and the config record
//! arrives with it, which is what lets a joiner's wasm guest read its chain id
//! from the synced store.

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use identity::{
    IDENTITY_ADD_KEY_NS, Authorizer, Identity, IdentityMsg, IdentityQuery, IdentityReply,
    KeyScheme, add_key_preimage, decode_reply, encode_msg, encode_query,
};
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

const CHAIN: &str = "sync-chain";

type Ed = PrivateKey;

fn ed(seed: u64) -> Ed {
    Ed::from_seed(seed)
}

fn ed_pub(k: &Ed) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}

/// the founder's consent to admit `new_key` (of `scheme`) at `generation`.
fn ed_consent(member: &Ed, scheme: KeyScheme, new_key: &[u8], generation: u64) -> Authorizer {
    let preimage = add_key_preimage(CHAIN, scheme, new_key, generation);
    Authorizer {
        key: ed_pub(member),
        proof: keyscheme::testkit::ed25519_proof(member, IDENTITY_ADD_KEY_NS, &preimage),
    }
}

fn identity_msg(m: &IdentityMsg) -> Msg {
    Msg {
        target: "identity".into(),
        payload: encode_msg(m),
    }
}

fn ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "identity".into(),
    })
}

// drive one op through the REAL module path: execute + commit_block (one op
// per block-height), so the committed op log is what a validator produces.
async fn apply_commit(m: &mut Identity, height: u64, origin: Origin, op: Msg) {
    let mut c = ctx(height, origin);
    m.execute(&mut c, &op).await.unwrap();
    m.commit_block().await.unwrap();
}

/// the read matrix compared source-vs-joiner: the listing, the account point
/// read, the key resolver for a live key and a removed one, and the removed
/// key's surviving generation counter.
const QUERIES: [&str; 5] = ["all", "get", "of-key-founder", "of-key-removed", "gen-removed"];

async fn replies(m: &Identity, founder: &[u8], removed: &[u8]) -> Vec<IdentityReply> {
    let queries = [
        encode_query(&IdentityQuery::All { from: 0, limit: 16 }),
        encode_query(&IdentityQuery::Get { number: 1 }),
        encode_query(&IdentityQuery::OfKey {
            key: founder.to_vec(),
        }),
        encode_query(&IdentityQuery::OfKey {
            key: removed.to_vec(),
        }),
        encode_query(&IdentityQuery::KeyGen {
            key: removed.to_vec(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(decode_reply(&m.query(q).await.unwrap()).unwrap());
    }
    out
}

/// the production wiring shape.
fn identity_over(store: Box<dyn sdk::MerkleStore>) -> Identity {
    Identity::new("identity", store, CHAIN.to_string())
}

#[test]
fn synced_store_reconstructs_source_root_accounts_and_indexes() {
    deterministic::Runner::default().start(|context| async move {
        let founder = ed(1);
        let passkey = keyscheme::testkit::passkey(0x42);
        let passkey_pub = keyscheme::testkit::passkey_pubkey(&passkey);

        // SOURCE: seed the genesis-config record the way the production
        // genesis path does, THEN wrap the module — the config is committed
        // store state under the shared `sdk::store_key` convention, part of
        // the root from block zero.
        let mut src_store = QmdbStore::init(context.child("src"), "src").await;
        let config = sdk::genesis_config::encode_config(&[("chain_id", CHAIN.as_bytes())]);
        src_store
            .commit_batch(vec![(
                sdk::store_key(sdk::genesis_config::CONFIG_KEY),
                Some(config.clone()),
            )])
            .await
            .expect("seed genesis config");
        let config_root = src_store.root();
        assert_ne!(config_root, StateRoot::ZERO, "config alone moves the root");
        let mut src = identity_over(Box::new(src_store));

        // found account 1 from the founder key.
        apply_commit(
            &mut src,
            1,
            Origin::External(ed_pub(&founder)),
            identity_msg(&IdentityMsg::Create {
                name: "alice".into(),
                scheme: KeyScheme::Ed25519,
            }),
        )
        .await;
        // admit the passkey: origin = the passkey, consent = the founder.
        apply_commit(
            &mut src,
            2,
            Origin::External(passkey_pub.clone()),
            identity_msg(&IdentityMsg::AddKey {
                scheme: KeyScheme::Secp256r1,
                label: Some("phone".into()),
                authorizer: ed_consent(&founder, KeyScheme::Secp256r1, &passkey_pub, 0),
            }),
        )
        .await;
        // remove it again: the op log carries a key-index DELETE, and the
        // generation counter stays at 1.
        apply_commit(
            &mut src,
            3,
            Origin::External(ed_pub(&founder)),
            identity_msg(&IdentityMsg::RemoveKey {
                key: passkey_pub.clone(),
            }),
        )
        .await;
        // profile writes: record overwrites.
        apply_commit(
            &mut src,
            4,
            Origin::External(ed_pub(&founder)),
            identity_msg(&IdentityMsg::SetProfile {
                avatar: Some("/shared/attachments/avatars/cafe.png".into()),
                bio: Some("syncing ducks".into()),
            }),
        )
        .await;
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, config_root, "the ops moved the root");
        let src_replies = replies(&src, &ed_pub(&founder), &passkey_pub).await;
        let IdentityReply::Accounts(listed) = &src_replies[0] else {
            panic!("expected the listing");
        };
        assert_eq!(listed.len(), 1, "one account is numbered");
        assert_eq!(src_replies[4], IdentityReply::Gen(1), "removal keeps the counter");

        // the module consumed its store, so REOPEN the committed partitions
        // as a bare store for the handoff (drop first — one owner at a time).
        drop(src);
        let src_store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(
            src_store.root(),
            src_root,
            "reopened store must recover the committed root"
        );

        // describe the target (root + op range), THEN hand the source off as
        // the sync resolver (consumes it — order matters).
        let target = src_store.sync_boundary_target().await;
        let resolver = src_store.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from
        // the resolver, then wrap the module around the injected store — the
        // exact shape a joining host uses. no ops are applied in application
        // order on this side.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");

        // the genesis-config record ARRIVED with the op range: this is what a
        // joiner's wasm guest reads its chain id from.
        assert_eq!(
            store
                .get(&sdk::store_key(sdk::genesis_config::CONFIG_KEY))
                .await
                .expect("config read"),
            Some(config),
            "the __config record rides the sync"
        );
        let synced = identity_over(Box::new(store));

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // account record, key index and generation counter synced together:
        // the joiner answers every read exactly like the source (including
        // the ABSENT index entry for the removed key).
        let synced_replies = replies(&synced, &ed_pub(&founder), &passkey_pub).await;
        for (name, (a, b)) in QUERIES.iter().zip(src_replies.iter().zip(&synced_replies)) {
            assert_eq!(a, b, "the {name} reply diverged");
        }
        let IdentityReply::Account(Some(account)) = &synced_replies[1] else {
            panic!("the account record must be present on the joiner");
        };
        assert_eq!(account.keys.len(), 1, "only the founder survives");
        let IdentityReply::Account(None) = &synced_replies[3] else {
            panic!("the removed key must stay unlinked on the joiner");
        };
    });
}
