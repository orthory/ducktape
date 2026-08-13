//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `Identity` around the injected store — the same
//! discriminating property chat, pages, agent, automations, and governance
//! prove, over the account + ownership-index + roster layout.
//!
//! the source SEEDS the `__config` chain-id record exactly the way the
//! production genesis path does (`bin/node/src/host_state.rs`
//! `seed_store_config`), then founds an account, admits a WebAuthn passkey
//! member (the rp-pinned meta must survive the trip), binds and UNBINDS a
//! second node (the op log carries an index DELETE, not just inserts), and
//! sets the profile (record overwrites). only a real sync that ships the
//! ACTUAL proven op range lands on the same root — and the config record arrives with it,
//! which is what lets a joiner's wasm guest read its chain id from the
//! synced store.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use identity::{
    IDENTITY_ADD_MEMBER_NS, IDENTITY_BIND_NS, IDENTITY_UNBIND_NS, Identity, IdentityMsg,
    IdentityQuery, IdentityReply, KeyKind, MemberAuth, MemberProof, add_member_preimage,
    bind_preimage, decode_reply, encode_msg, encode_query, unbind_preimage,
};
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use sha2::{Digest as _, Sha256};
use statesync::qmdb::QmdbStore;

const CHAIN: &str = "sync-chain";

type Ed = PrivateKey;

fn ed(seed: u64) -> Ed {
    Ed::from_seed(seed)
}

fn ed_pub(k: &Ed) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}

fn ed_auth(k: &Ed, ns: &[u8], preimage: &[u8]) -> MemberAuth {
    MemberAuth {
        key: ed_pub(k),
        kind: KeyKind::Ed25519,
        proof: MemberProof::Signature {
            sig: k.sign(ns, preimage).as_ref().to_vec(),
        },
    }
}

// a WebAuthn passkey, synthesized exactly as an authenticator would produce
// it (identity's own test recipe; RFC-6979 p256 signing is deterministic).
fn wa_key(seed: u8) -> p256::ecdsa::SigningKey {
    p256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
}
fn wa_pub(k: &p256::ecdsa::SigningKey) -> Vec<u8> {
    k.verifying_key().to_sec1_bytes().to_vec()
}
fn wa_proof(k: &p256::ecdsa::SigningKey, rp_id: &str, ns: &[u8], preimage: &[u8]) -> MemberProof {
    use p256::ecdsa::{Signature, signature::Signer as _};
    let mut chal = Sha256::new();
    chal.update(ns);
    chal.update(preimage);
    let challenge = chal.finalize();
    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://ducktape.local"}}"#,
        URL_SAFE_NO_PAD.encode(challenge)
    )
    .into_bytes();
    let mut authenticator_data = Vec::new();
    authenticator_data.extend_from_slice(&Sha256::digest(rp_id.as_bytes()));
    authenticator_data.push(0x01); // User Present
    authenticator_data.extend_from_slice(&0u32.to_be_bytes());
    let mut signed = authenticator_data.clone();
    signed.extend_from_slice(&Sha256::digest(&client_data_json));
    let sig: Signature = k.sign(&signed);
    MemberProof::Webauthn {
        authenticator_data,
        client_data_json,
        signature: sig.to_bytes().to_vec(),
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

/// the read matrix compared source-vs-joiner: the roster-served listing, the
/// account point read, both ownership-index resolvers (a live node, the
/// unbound node, the passkey member).
const QUERIES: [&str; 6] = [
    "all",
    "get",
    "of-node-live",
    "of-node-unbound",
    "of-member-founder",
    "of-member-passkey",
];

async fn replies(m: &Identity, account_id: &[u8], nodes: [&[u8]; 2], passkey: &[u8]) -> Vec<IdentityReply> {
    let queries = [
        encode_query(&IdentityQuery::All { from: 0, limit: 16 }),
        encode_query(&IdentityQuery::Get {
            account_id: account_id.to_vec(),
        }),
        encode_query(&IdentityQuery::OfNode {
            node_key: nodes[0].to_vec(),
        }),
        encode_query(&IdentityQuery::OfNode {
            node_key: nodes[1].to_vec(),
        }),
        encode_query(&IdentityQuery::OfMember {
            member_key: account_id.to_vec(),
        }),
        encode_query(&IdentityQuery::OfMember {
            member_key: passkey.to_vec(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(decode_reply(&m.query(q).await.unwrap()).unwrap());
    }
    out
}

/// the production wiring shape, ungated (no valset — the round trip proves
/// the record layout, not the member gate, which the parity proof pins).
fn identity_over(store: Box<dyn sdk::MerkleStore>) -> Identity {
    Identity::new("identity", store, None, CHAIN.to_string())
}

#[test]
fn synced_store_reconstructs_source_root_accounts_and_indexes() {
    deterministic::Runner::default().start(|context| async move {
        let founder = ed(1);
        let account_id = ed_pub(&founder);
        let passkey = wa_key(0x42);
        let node_a = b"node-a".as_slice();
        let node_b = b"node-b".as_slice();

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

        // found the account: bind node A under the founding ed25519 key.
        apply_commit(
            &mut src,
            1,
            Origin::External(node_a.to_vec()),
            identity_msg(&IdentityMsg::BindNode {
                authorizer: ed_auth(&founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node_a, 0)),
            }),
        )
        .await;
        // admit the WebAuthn passkey (rp-pinned member meta rides the record).
        let preimage =
            add_member_preimage(CHAIN, &account_id, &wa_pub(&passkey), KeyKind::WebauthnP256, 1);
        apply_commit(
            &mut src,
            2,
            Origin::External(node_a.to_vec()),
            identity_msg(&IdentityMsg::AddMemberKey {
                new_key: wa_pub(&passkey),
                new_kind: KeyKind::WebauthnP256,
                new_label: Some("phone".into()),
                possession: wa_proof(&passkey, "ducktape", IDENTITY_ADD_MEMBER_NS, &preimage),
                authorizer: ed_auth(&founder, IDENTITY_ADD_MEMBER_NS, &preimage),
            }),
        )
        .await;
        // bind a second node, then UNBIND it: the op log carries an
        // ownership-index DELETE, not just inserts.
        apply_commit(
            &mut src,
            3,
            Origin::External(node_b.to_vec()),
            identity_msg(&IdentityMsg::BindNode {
                authorizer: ed_auth(&founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node_b, 2)),
            }),
        )
        .await;
        apply_commit(
            &mut src,
            4,
            Origin::External(node_a.to_vec()),
            identity_msg(&IdentityMsg::UnbindNode {
                node_key: node_b.to_vec(),
                authorizer: ed_auth(
                    &founder,
                    IDENTITY_UNBIND_NS,
                    &unbind_preimage(CHAIN, node_b, 3),
                ),
            }),
        )
        .await;
        // profile writes: record overwrites.
        apply_commit(
            &mut src,
            5,
            Origin::External(node_a.to_vec()),
            identity_msg(&IdentityMsg::SetProfile {
                avatar: Some("/shared/attachments/avatars/cafe.png".into()),
                bio: Some("syncing ducks".into()),
            }),
        )
        .await;
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, config_root, "the ops moved the root");
        let src_replies = replies(&src, &account_id, [node_a, node_b], &wa_pub(&passkey)).await;
        let IdentityReply::Accounts(listed) = &src_replies[0] else {
            panic!("expected the listing");
        };
        assert_eq!(listed.len(), 1, "the account is rostered");

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

        // account record, roster, and both ownership indexes synced together:
        // the joiner answers every read exactly like the source (including
        // the ABSENT index entry for the unbound node).
        let synced_replies =
            replies(&synced, &account_id, [node_a, node_b], &wa_pub(&passkey)).await;
        for (name, (a, b)) in QUERIES.iter().zip(src_replies.iter().zip(&synced_replies)) {
            assert_eq!(a, b, "the {name} reply diverged");
        }
        let IdentityReply::Account(Some(account)) = &synced_replies[1] else {
            panic!("the account record must be present on the joiner");
        };
        assert_eq!(account.member_keys.len(), 2, "founder + passkey survive");
        let IdentityReply::Account(None) = &synced_replies[3] else {
            panic!("the unbound node must stay unbound on the joiner");
        };
    });
}
