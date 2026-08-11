//! account-model tests: creation, multi-scheme membership, the "any surviving
//! member authorizes" recovery path, replay/last-member guards, the valset
//! gate, and the client-ACL facet — all over the store-backed module (a
//! [`MemStore`] test double; the qmdb continuity proof lives in
//! `tests/sync_round_trip.rs`).

use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use commonware_cryptography::Signer as _;
use futures::executor::block_on;
use sdk_testkit::MemStore;
use sha2::{Digest, Sha256};

const CHAIN: &str = "test-chain";

// ---- a minimal Ctx (the shared sdk-testkit double) ----------------------

use sdk_testkit::TestCtx;

/// a valset-query responder over an optional member/resident set — the only
/// host-routed read identity's execute makes.
fn valset_reads(
    members: Option<Vec<Vec<u8>>>,
    residents: Option<Vec<Vec<u8>>>,
) -> impl FnMut(&[u8]) -> Result<Vec<u8>, Error> {
    move |req| {
        let q = valset::decode_query(req).map_err(Error::Module)?;
        match (q, &members, &residents) {
            (valset::ValsetQuery::Validators, Some(m), _) => Ok(valset::encode_reply(
                &valset::ValsetReply::Validators(m.clone()),
            )),
            (valset::ValsetQuery::Residents, _, Some(o)) => Ok(valset::encode_reply(
                &valset::ValsetReply::Residents(o.clone()),
            )),
            _ => Err(Error::QueryUnsupported),
        }
    }
}

fn ctx_with(
    origin: sdk::Origin,
    members: Option<Vec<Vec<u8>>>,
    residents: Option<Vec<Vec<u8>>>,
) -> TestCtx {
    TestCtx::with_env(sdk::Env {
        height: 0,
        consensus_time: 100,
        origin,
        me: "identity".into(),
    })
    .on_query("valset", valset_reads(members, residents))
}

fn ctx_external(key: &[u8]) -> TestCtx {
    ctx_with(sdk::Origin::External(key.to_vec()), None, None)
}
fn ctx_gated(key: &[u8], validators: Vec<Vec<u8>>, residents: Vec<Vec<u8>>) -> TestCtx {
    ctx_with(
        sdk::Origin::External(key.to_vec()),
        Some(validators),
        Some(residents),
    )
}
/// a governance-follow-up origin (module), the only origin allowed to move
/// client standing besides genesis.
fn ctx_module(name: &str) -> TestCtx {
    ctx_with(sdk::Origin::Module(name.into()), None, None)
}
fn ctx_system() -> TestCtx {
    ctx_with(sdk::Origin::System, None, None)
}

// ---- member builders (one per scheme) -----------------------------------

type Ed = commonware_cryptography::ed25519::PrivateKey;
type P256Native = commonware_cryptography::secp256r1::standard::PrivateKey;

fn ed(seed: u64) -> Ed {
    Ed::from_seed(seed)
}
fn ed_pub(k: &Ed) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}
fn ed_proof(k: &Ed, ns: &[u8], preimage: &[u8]) -> MemberProof {
    MemberProof::Signature {
        sig: k.sign(ns, preimage).as_ref().to_vec(),
    }
}
fn ed_auth(k: &Ed, ns: &[u8], preimage: &[u8]) -> MemberAuth {
    MemberAuth {
        key: ed_pub(k),
        kind: KeyKind::Ed25519,
        proof: ed_proof(k, ns, preimage),
    }
}

fn p256(seed: u64) -> P256Native {
    P256Native::from_seed(seed)
}
fn p256_pub(k: &P256Native) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}
fn p256_auth(k: &P256Native, ns: &[u8], preimage: &[u8]) -> MemberAuth {
    MemberAuth {
        key: p256_pub(k),
        kind: KeyKind::P256,
        proof: MemberProof::Signature {
            sig: k.sign(ns, preimage).as_ref().to_vec(),
        },
    }
}

// a WebAuthn passkey, synthesized exactly as an authenticator would produce it.
fn wa_key(seed: u8) -> p256::ecdsa::SigningKey {
    p256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
}
fn wa_pub(k: &p256::ecdsa::SigningKey) -> Vec<u8> {
    k.verifying_key().to_sec1_bytes().to_vec()
}
fn wa_proof(k: &p256::ecdsa::SigningKey, rp_id: &str, ns: &[u8], preimage: &[u8]) -> MemberProof {
    use p256::ecdsa::{Signature, signature::Signer as _};
    // challenge = SHA256(namespace ‖ preimage), mirroring scheme::webauthn_challenge.
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
fn wa_auth(k: &p256::ecdsa::SigningKey, rp_id: &str, ns: &[u8], preimage: &[u8]) -> MemberAuth {
    MemberAuth {
        key: wa_pub(k),
        kind: KeyKind::WebauthnP256,
        proof: wa_proof(k, rp_id, ns, preimage),
    }
}

// ---- harness ------------------------------------------------------------

fn new_identity() -> Identity {
    Identity::new("identity", Box::new(MemStore::new()), None, CHAIN.to_string())
}
fn new_gated_identity() -> Identity {
    Identity::new(
        "identity",
        Box::new(MemStore::new()),
        Some("valset".into()),
        CHAIN.to_string(),
    )
}

/// the root of a store that never committed anything — the store-backed twin
/// of the old ZERO sentinel (a MemStore hash of the empty map, not ZERO).
fn empty_root() -> StateRoot {
    new_identity().root()
}

/// execute a message from `origin`, then commit the block.
fn apply(id: &mut Identity, origin: &[u8], msg: IdentityMsg) -> Result<(), Error> {
    let mut ctx = ctx_external(origin);
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

fn get_account(id: &Identity, account_id: &[u8]) -> Option<AccountView> {
    let reply = block_on(id.query(&encode_query(&IdentityQuery::Get {
        account_id: account_id.to_vec(),
    })))
    .unwrap();
    match decode_reply(&reply).unwrap() {
        IdentityReply::Account(a) => a,
        other => panic!("expected Account, got {other:?}"),
    }
}

/// found an account by binding `node` with founding ed25519 key `founder`.
/// returns the account id.
fn found_account(id: &mut Identity, founder: &Ed, node: &[u8]) -> Vec<u8> {
    let account_id = ed_pub(founder);
    let auth = ed_auth(founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node, 0));
    apply(id, node, IdentityMsg::BindNode { authorizer: auth }).expect("found account");
    account_id
}

// ---- tests --------------------------------------------------------------

#[test]
fn bind_creates_a_single_member_account() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);

    let acc = get_account(&id, &account_id).expect("account exists");
    assert_eq!(acc.account_id, ed_pub(&founder));
    assert_eq!(acc.nonce, 1, "a founding bind bumps the nonce to 1");
    assert_eq!(acc.member_keys.len(), 1);
    assert_eq!(acc.member_keys[0].pubkey, ed_pub(&founder));
    assert_eq!(acc.member_keys[0].kind, KeyKind::Ed25519);
    assert_eq!(
        acc.nodes,
        vec![NodeView {
            node_key: node.to_vec(),
            label: None,
        }]
    );

    // resolvable by node and by member.
    assert_eq!(account_of_node(&id, node).unwrap().account_id, account_id);
    assert_eq!(
        account_of_member(&id, &ed_pub(&founder))
            .unwrap()
            .account_id,
        account_id
    );
}

#[test]
fn a_second_ed25519_key_joins_and_can_bind_its_own_node() {
    let mut id = new_identity();
    let founder = ed(1);
    let node1 = b"node-1";
    let account_id = found_account(&mut id, &founder, node1);

    // founder admits a second key; the new key proves possession, founder consents.
    let joiner = ed(2);
    let nonce = get_account(&id, &account_id).unwrap().nonce; // 1
    let preimage = add_member_preimage(
        CHAIN,
        &account_id,
        &ed_pub(&joiner),
        KeyKind::Ed25519,
        nonce,
    );
    apply(
        &mut id,
        node1,
        IdentityMsg::AddMemberKey {
            new_key: ed_pub(&joiner),
            new_kind: KeyKind::Ed25519,
            new_label: Some("laptop".into()),
            possession: ed_proof(&joiner, IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&founder, IDENTITY_ADD_MEMBER_NS, &preimage),
        },
    )
    .expect("add second member");

    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(acc.member_keys.len(), 2);
    assert_eq!(acc.nonce, 2);

    // the joiner (NOT the founder) now binds a second node -- both resolve to
    // the same account.
    let node2 = b"node-2";
    let nonce = acc.nonce; // 2
    apply(
        &mut id,
        node2,
        IdentityMsg::BindNode {
            authorizer: ed_auth(
                &joiner,
                IDENTITY_BIND_NS,
                &bind_preimage(CHAIN, node2, nonce),
            ),
        },
    )
    .expect("joiner binds node2");

    assert_eq!(account_of_node(&id, node1).unwrap().account_id, account_id);
    assert_eq!(account_of_node(&id, node2).unwrap().account_id, account_id);
}

#[test]
fn any_surviving_member_can_evict_a_lost_key_but_not_the_last() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);

    // add a second key.
    let joiner = ed(2);
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    let preimage = add_member_preimage(
        CHAIN,
        &account_id,
        &ed_pub(&joiner),
        KeyKind::Ed25519,
        nonce,
    );
    apply(
        &mut id,
        node,
        IdentityMsg::AddMemberKey {
            new_key: ed_pub(&joiner),
            new_kind: KeyKind::Ed25519,
            new_label: None,
            possession: ed_proof(&joiner, IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&founder, IDENTITY_ADD_MEMBER_NS, &preimage),
        },
    )
    .unwrap();

    // the joiner evicts the FOUNDER (the recovery path -- a surviving device
    // removes a lost one). the account id persists even though its founding key
    // is gone.
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    let preimage = remove_member_preimage(CHAIN, &account_id, &ed_pub(&founder), nonce);
    apply(
        &mut id,
        node,
        IdentityMsg::RemoveMemberKey {
            target_key: ed_pub(&founder),
            authorizer: ed_auth(&joiner, IDENTITY_REMOVE_MEMBER_NS, &preimage),
        },
    )
    .expect("joiner evicts founder");

    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(acc.member_keys.len(), 1);
    assert_eq!(acc.member_keys[0].pubkey, ed_pub(&joiner));
    assert_eq!(acc.account_id, ed_pub(&founder), "account id is stable");
    // the evicted key's ownership-index entry is gone with it.
    assert!(account_of_member(&id, &ed_pub(&founder)).is_none());

    // removing the LAST member is refused.
    let nonce = acc.nonce;
    let preimage = remove_member_preimage(CHAIN, &account_id, &ed_pub(&joiner), nonce);
    let err = apply(
        &mut id,
        node,
        IdentityMsg::RemoveMemberKey {
            target_key: ed_pub(&joiner),
            authorizer: ed_auth(&joiner, IDENTITY_REMOVE_MEMBER_NS, &preimage),
        },
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("last member"), "got {err:?}");
}

#[test]
fn a_passkey_joins_and_then_authorizes_an_unbind() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);

    // founder admits a WebAuthn passkey (the phone). possession is a webauthn
    // assertion; founder's ed25519 signature consents.
    let passkey = wa_key(0x42);
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    let preimage = add_member_preimage(
        CHAIN,
        &account_id,
        &wa_pub(&passkey),
        KeyKind::WebauthnP256,
        nonce,
    );
    apply(
        &mut id,
        node,
        IdentityMsg::AddMemberKey {
            new_key: wa_pub(&passkey),
            new_kind: KeyKind::WebauthnP256,
            new_label: Some("phone".into()),
            possession: wa_proof(&passkey, "ducktape", IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&founder, IDENTITY_ADD_MEMBER_NS, &preimage),
        },
    )
    .expect("passkey joins");

    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(acc.member_keys.len(), 2);
    let pk_view = acc
        .member_keys
        .iter()
        .find(|m| m.pubkey == wa_pub(&passkey))
        .unwrap();
    assert_eq!(pk_view.kind, KeyKind::WebauthnP256);
    assert_eq!(pk_view.label.as_deref(), Some("phone"));

    // the passkey -- a first-class member -- now authorizes evicting the node
    // (e.g. the laptop was lost; the phone cleans up). it signs a fresh
    // webauthn assertion over the unbind preimage.
    let nonce = acc.nonce;
    apply(
        &mut id,
        node,
        IdentityMsg::UnbindNode {
            node_key: node.to_vec(),
            authorizer: wa_auth(
                &passkey,
                "ducktape",
                IDENTITY_UNBIND_NS,
                &unbind_preimage(CHAIN, node, nonce),
            ),
        },
    )
    .expect("passkey authorizes unbind");

    assert!(account_of_node(&id, node).is_none(), "node evicted");
    // the account survives with its members intact.
    assert_eq!(get_account(&id, &account_id).unwrap().member_keys.len(), 2);
}

#[test]
fn a_native_p256_key_can_found_and_authorize() {
    // proves the abstraction is genuinely multi-curve, not ed25519-shaped: a
    // native P-256 key founds an account and binds a node.
    let mut id = new_identity();
    let founder = p256(3);
    let node = b"node-1";
    let account_id = p256_pub(&founder);
    apply(
        &mut id,
        node,
        IdentityMsg::BindNode {
            authorizer: p256_auth(&founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node, 0)),
        },
    )
    .expect("p256 founds account");
    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(acc.member_keys[0].kind, KeyKind::P256);
}

#[test]
fn a_stale_certificate_is_rejected_after_the_nonce_advances() {
    let mut id = new_identity();
    let founder = ed(1);
    let node1 = b"node-1";
    let account_id = found_account(&mut id, &founder, node1);
    // account nonce is now 1. a cert minted at nonce 0 (the founding preimage)
    // can't be replayed to bind another node.
    let stale = ed_auth(
        &founder,
        IDENTITY_BIND_NS,
        &bind_preimage(CHAIN, b"node-2", 0),
    );
    let err = apply(
        &mut id,
        b"node-2",
        IdentityMsg::BindNode { authorizer: stale },
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("does not verify"),
        "got {err:?}"
    );
    // signing the CURRENT nonce works.
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    let fresh = ed_auth(
        &founder,
        IDENTITY_BIND_NS,
        &bind_preimage(CHAIN, b"node-2", nonce),
    );
    apply(
        &mut id,
        b"node-2",
        IdentityMsg::BindNode { authorizer: fresh },
    )
    .expect("fresh cert");
}

#[test]
fn a_forged_authorizer_kind_is_rejected() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);
    let nonce = get_account(&id, &account_id).unwrap().nonce;

    // claim the founder is a P256 member while presenting its real ed25519 sig:
    // the registered kind (Ed25519) does not match the asserted kind (P256).
    let mut auth = ed_auth(
        &founder,
        IDENTITY_BIND_NS,
        &bind_preimage(CHAIN, b"node-2", nonce),
    );
    auth.kind = KeyKind::P256;
    let err = apply(
        &mut id,
        b"node-2",
        IdentityMsg::BindNode { authorizer: auth },
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("kind does not match"),
        "got {err:?}"
    );
}

#[test]
fn set_account_name_is_origin_gated_to_a_bound_node() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);

    apply(
        &mut id,
        node,
        IdentityMsg::SetAccountName {
            display_name: "  Kim  ".into(),
        },
    )
    .expect("bound node names its account");
    assert_eq!(
        get_account(&id, &account_id)
            .unwrap()
            .display_name
            .as_deref(),
        Some("Kim")
    );

    // an unbound node cannot name any account.
    let err = apply(
        &mut id,
        b"stranger",
        IdentityMsg::SetAccountName {
            display_name: "x".into(),
        },
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("not bound"), "got {err:?}");
    // naming does not bump the nonce.
    assert_eq!(get_account(&id, &account_id).unwrap().nonce, 1);
}

#[test]
fn set_node_label_is_origin_gated_to_the_accounts_own_nodes() {
    let mut id = new_identity();
    let founder = ed(1);
    let node1 = b"node-1";
    let account_id = found_account(&mut id, &founder, node1);

    // a second key joins and binds node2, so the account owns two nodes.
    let joiner = ed(2);
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    let preimage =
        add_member_preimage(CHAIN, &account_id, &ed_pub(&joiner), KeyKind::Ed25519, nonce);
    apply(
        &mut id,
        node1,
        IdentityMsg::AddMemberKey {
            new_key: ed_pub(&joiner),
            new_kind: KeyKind::Ed25519,
            new_label: None,
            possession: ed_proof(&joiner, IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&founder, IDENTITY_ADD_MEMBER_NS, &preimage),
        },
    )
    .unwrap();
    let node2 = b"node-2";
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    apply(
        &mut id,
        node2,
        IdentityMsg::BindNode {
            authorizer: ed_auth(&joiner, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node2, nonce)),
        },
    )
    .unwrap();

    // node1 (a bound origin) labels its sibling node2 -> the label is trimmed
    // and visible via a plain Get, from any device on this network.
    apply(
        &mut id,
        node1,
        IdentityMsg::SetNodeLabel {
            node_key: node2.to_vec(),
            label: Some("  Kim's laptop  ".into()),
        },
    )
    .expect("bound node labels a sibling");
    assert_eq!(
        node_label(&id, &account_id, node2).as_deref(),
        Some("Kim's laptop")
    );
    // labeling is cosmetic: it does NOT bump the account nonce.
    let before = get_account(&id, &account_id).unwrap().nonce;

    // clearing (empty trim) drops the label back to None, still no nonce bump.
    apply(
        &mut id,
        node2,
        IdentityMsg::SetNodeLabel {
            node_key: node2.to_vec(),
            label: Some("   ".into()),
        },
    )
    .expect("empty label clears");
    assert_eq!(node_label(&id, &account_id, node2), None);
    assert_eq!(get_account(&id, &account_id).unwrap().nonce, before);

    // an UNBOUND node cannot label anything.
    let err = apply(
        &mut id,
        b"stranger",
        IdentityMsg::SetNodeLabel {
            node_key: node1.to_vec(),
            label: Some("x".into()),
        },
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("not bound to an account"),
        "got {err:?}"
    );

    // a bound node in a DIFFERENT account cannot label this account's node.
    let other = ed(9);
    let other_node = b"other-node";
    found_account(&mut id, &other, other_node);
    let err = apply(
        &mut id,
        other_node,
        IdentityMsg::SetNodeLabel {
            node_key: node1.to_vec(),
            label: Some("theft".into()),
        },
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("not bound to the origin's account"),
        "got {err:?}"
    );
}

#[test]
fn set_profile_is_origin_gated_trims_caps_and_clears() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);

    // a bound node sets avatar + bio; both trim.
    apply(
        &mut id,
        node,
        IdentityMsg::SetProfile {
            avatar: Some("  /shared/attachments/avatars/abc.png  ".into()),
            bio: Some("  hi there  ".into()),
        },
    )
    .expect("bound node sets its profile");
    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(acc.avatar.as_deref(), Some("/shared/attachments/avatars/abc.png"));
    assert_eq!(acc.bio.as_deref(), Some("hi there"));
    // no signature is consumed: the nonce stays put (still 1 from the bind).
    assert_eq!(acc.nonce, 1);

    // empty-trim clears each field independently.
    apply(
        &mut id,
        node,
        IdentityMsg::SetProfile {
            avatar: Some("   ".into()),
            bio: None,
        },
    )
    .expect("empty avatar clears");
    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(acc.avatar, None, "whitespace avatar clears");
    assert_eq!(acc.bio, None, "None bio clears");

    // over-cap bio rejects, state untouched.
    let err = apply(
        &mut id,
        node,
        IdentityMsg::SetProfile {
            avatar: None,
            bio: Some("x".repeat(MAX_BIO_LEN + 1)),
        },
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("bio exceeds"), "got {err:?}");

    // an unbound node cannot set any account's profile.
    let err = apply(
        &mut id,
        b"stranger",
        IdentityMsg::SetProfile {
            avatar: Some("/shared/attachments/avatars/evil.png".into()),
            bio: None,
        },
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("not bound"), "got {err:?}");
}

#[test]
fn node_label_sets_and_drops_with_its_node_on_unbind() {
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);

    apply(
        &mut id,
        node,
        IdentityMsg::SetNodeLabel {
            node_key: node.to_vec(),
            label: Some("my box".into()),
        },
    )
    .unwrap();
    assert_eq!(node_label(&id, &account_id, node).as_deref(), Some("my box"));

    // unbinding the node drops it -- and its label -- from the account, and
    // clears the node-ownership index.
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    apply(
        &mut id,
        node,
        IdentityMsg::UnbindNode {
            node_key: node.to_vec(),
            authorizer: ed_auth(
                &founder,
                IDENTITY_UNBIND_NS,
                &unbind_preimage(CHAIN, node, nonce),
            ),
        },
    )
    .expect("unbind");
    assert!(get_account(&id, &account_id).unwrap().nodes.is_empty());
    assert!(account_of_node(&id, node).is_none(), "index entry dropped");
}

#[test]
fn bind_is_valset_gated_when_configured() {
    let mut id = new_gated_identity();
    let founder = ed(1);
    let node = b"node-1";
    let auth = ed_auth(&founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node, 0));
    let msg = IdentityMsg::BindNode { authorizer: auth };

    // origin not in validators/residents -> rejected.
    let mut ctx = ctx_gated(node, vec![b"someone-else".to_vec()], vec![]);
    let m = Msg {
        target: "identity".into(),
        payload: encode_msg(&msg),
    };
    let err = block_on(id.execute(&mut ctx, &m)).unwrap_err();
    assert!(
        format!("{err:?}").contains("not a network member"),
        "got {err:?}"
    );

    // origin present as a resident -> admitted.
    let mut ctx = ctx_gated(node, vec![], vec![node.to_vec()]);
    block_on(id.execute(&mut ctx, &m)).expect("resident may bind");
}

#[test]
fn mixed_scheme_membership_round_trips_the_store_records() {
    // build an account with an ed25519 founder + a webauthn passkey + a node,
    // set the global profile, and confirm every read model (record, roster
    // listing, both ownership indexes) serves the mixed-scheme membership.
    let mut id = new_identity();
    let founder = ed(1);
    let node = b"node-1";
    let account_id = found_account(&mut id, &founder, node);
    let passkey = wa_key(0x51);
    let nonce = get_account(&id, &account_id).unwrap().nonce;
    let preimage = add_member_preimage(
        CHAIN,
        &account_id,
        &wa_pub(&passkey),
        KeyKind::WebauthnP256,
        nonce,
    );
    apply(
        &mut id,
        node,
        IdentityMsg::AddMemberKey {
            new_key: wa_pub(&passkey),
            new_kind: KeyKind::WebauthnP256,
            new_label: Some("phone".into()),
            possession: wa_proof(&passkey, "ducktape", IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&founder, IDENTITY_ADD_MEMBER_NS, &preimage),
        },
    )
    .unwrap();

    apply(
        &mut id,
        node,
        IdentityMsg::SetProfile {
            avatar: Some("/shared/attachments/avatars/0123456789abcdef.png".into()),
            bio: Some("building ducks".into()),
        },
    )
    .unwrap();
    let acc = get_account(&id, &account_id).unwrap();
    assert_eq!(
        acc.avatar.as_deref(),
        Some("/shared/attachments/avatars/0123456789abcdef.png")
    );
    assert_eq!(acc.bio.as_deref(), Some("building ducks"));

    // the member index resolves the passkey (a webauthn key with its rp pin)
    // to the same account the roster listing serves.
    assert_eq!(
        account_of_member(&id, &wa_pub(&passkey))
            .unwrap()
            .account_id,
        account_id
    );
    let reply = block_on(id.query(&encode_query(&IdentityQuery::All { from: 0, limit: 10 })))
        .unwrap();
    let IdentityReply::Accounts(listed) = decode_reply(&reply).unwrap() else {
        panic!("expected Accounts");
    };
    assert_eq!(listed, vec![get_account(&id, &account_id).unwrap()]);
}

fn account_of_node(id: &Identity, node: &[u8]) -> Option<AccountView> {
    let reply = block_on(id.query(&encode_query(&IdentityQuery::OfNode {
        node_key: node.to_vec(),
    })))
    .unwrap();
    match decode_reply(&reply).unwrap() {
        IdentityReply::Account(a) => a,
        other => panic!("expected Account, got {other:?}"),
    }
}
/// the label of `node` under `account_id`, panicking if the node is not bound
/// -- a read helper for the `SetNodeLabel` tests.
fn node_label(id: &Identity, account_id: &[u8], node: &[u8]) -> Option<String> {
    get_account(id, account_id)
        .unwrap()
        .nodes
        .into_iter()
        .find(|n| n.node_key == node)
        .expect("node is bound to the account")
        .label
}

fn account_of_member(id: &Identity, member: &[u8]) -> Option<AccountView> {
    let reply = block_on(id.query(&encode_query(&IdentityQuery::OfMember {
        member_key: member.to_vec(),
    })))
    .unwrap();
    match decode_reply(&reply).unwrap() {
        IdentityReply::Account(a) => a,
        other => panic!("expected Account, got {other:?}"),
    }
}

// ---- client standing (the submit-door ACL facet) ------------------------

fn client_set(id: &Identity) -> Vec<Vec<u8>> {
    let reply = block_on(id.query(&encode_query(&IdentityQuery::Clients))).unwrap();
    match decode_reply(&reply).unwrap() {
        IdentityReply::Clients(v) => v,
        other => panic!("expected Clients, got {other:?}"),
    }
}

/// run one client op from `ctx`, committing on success (mirrors `apply` but
/// keeps the caller's chosen origin — module/system/external).
fn run_client(id: &mut Identity, ctx: &mut TestCtx, msg: IdentityMsg) -> Result<(), Error> {
    let m = Msg {
        target: "identity".into(),
        payload: encode_msg(&msg),
    };
    let r = block_on(id.execute(ctx, &m));
    if r.is_ok() {
        block_on(id.commit_block()).unwrap();
    } else {
        block_on(id.abort_block()).unwrap();
    }
    r
}

#[test]
fn client_grant_from_module_origin_moves_root_and_reads_back() {
    let mut id = new_identity();
    let empty = empty_root();
    assert_eq!(id.root(), empty, "nothing committed yet");
    let key = ed_pub(&ed(1));

    // staged: read-your-writes sees it before commit; root reflects committed.
    let mut ctx = ctx_module("governance");
    let m = Msg {
        target: "identity".into(),
        payload: encode_msg(&IdentityMsg::GrantClient { key: key.clone() }),
    };
    block_on(id.execute(&mut ctx, &m)).unwrap();
    assert_eq!(id.root(), empty, "root reflects committed only");
    assert_eq!(client_set(&id), vec![key.clone()], "read-your-writes");
    block_on(id.commit_block()).unwrap();
    assert_ne!(id.root(), empty, "a committed grant moves the root");
    assert_eq!(client_set(&id), vec![key.clone()]);

    // re-granting a key that already holds standing stages NOTHING: the root
    // is byte-identical after the duplicate commits.
    let granted = id.root();
    let mut ctx = ctx_module("governance");
    run_client(&mut id, &mut ctx, IdentityMsg::GrantClient { key }).unwrap();
    assert_eq!(id.root(), granted, "a duplicate grant is a staged no-op");
}

#[test]
fn client_grant_from_external_origin_is_refused() {
    let mut id = new_identity();
    let mut ctx = ctx_external(&ed_pub(&ed(9)));
    let err = run_client(
        &mut id,
        &mut ctx,
        IdentityMsg::GrantClient { key: ed_pub(&ed(1)) },
    )
    .unwrap_err();
    assert!(
        matches!(err, Error::Module(m) if m.contains("only via governance")),
        "external self-grant must be refused",
    );
    assert!(client_set(&id).is_empty());
}

#[test]
fn client_revoke_restores_the_empty_plane_root() {
    let mut id = new_identity();
    let empty = id.root();
    let key = ed_pub(&ed(2));
    let mut sys = ctx_system();
    run_client(&mut id, &mut sys, IdentityMsg::GrantClient { key: key.clone() }).unwrap();
    assert_eq!(client_set(&id), vec![key.clone()]);
    run_client(&mut id, &mut sys, IdentityMsg::RevokeClient { key: key.clone() }).unwrap();
    assert!(client_set(&id).is_empty(), "revoke removed it");
    // revoking the last client DELETES the record: the store returns to its
    // never-granted shape, so the root is the empty root again.
    assert_eq!(id.root(), empty, "the last revoke restores the empty root");
    // revoking a key that holds no standing stages nothing (still Ok).
    run_client(&mut id, &mut sys, IdentityMsg::RevokeClient { key }).unwrap();
    assert_eq!(id.root(), empty);
}

#[test]
fn client_grant_rejects_a_malformed_key() {
    let mut id = new_identity();
    let mut sys = ctx_system();
    let err = run_client(&mut id, &mut sys, IdentityMsg::GrantClient { key: vec![0u8; 16] }).unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    assert!(client_set(&id).is_empty());
}

#[test]
fn abort_block_drops_staged_accounts_and_clients_together() {
    let mut id = new_identity();
    let empty = id.root();
    let founder = ed(3);
    let node = b"node-x";

    // stage a founding bind AND a client grant in one block, then abort: no
    // record, no roster entry, no index entry, no client survives, and the
    // root never moved.
    let auth = ed_auth(&founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node, 0));
    let mut ctx = ctx_external(node);
    let m = Msg {
        target: "identity".into(),
        payload: encode_msg(&IdentityMsg::BindNode { authorizer: auth }),
    };
    block_on(id.execute(&mut ctx, &m)).unwrap();
    let mut sys = ctx_system();
    let grant = Msg {
        target: "identity".into(),
        payload: encode_msg(&IdentityMsg::GrantClient {
            key: ed_pub(&ed(7)),
        }),
    };
    block_on(id.execute(&mut sys, &grant)).unwrap();
    assert!(get_account(&id, &ed_pub(&founder)).is_some(), "staged read");
    assert_eq!(client_set(&id).len(), 1, "staged read");

    block_on(id.abort_block()).unwrap();
    assert!(get_account(&id, &ed_pub(&founder)).is_none());
    assert!(account_of_node(&id, node).is_none());
    assert!(account_of_member(&id, &ed_pub(&founder)).is_none());
    assert!(client_set(&id).is_empty());
    assert_eq!(id.root(), empty, "an aborted block leaves no trace");
}
