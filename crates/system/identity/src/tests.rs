//! account-model tests: creation, multi-scheme membership, the "any surviving
//! member authorizes" recovery path, replay/last-member guards, the valset
//! gate, and byzantine-resistant snapshot/install.

use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use commonware_cryptography::Signer as _;
use futures::executor::block_on;

const CHAIN: &str = "test-chain";

// ---- a minimal Ctx ------------------------------------------------------

struct TestCtx {
    env: sdk::Env,
    members: Option<Vec<Vec<u8>>>,
    residents: Option<Vec<Vec<u8>>>,
}
impl TestCtx {
    fn external(key: &[u8]) -> Self {
        Self {
            env: sdk::Env {
                protocol_version: 0,
                height: 0,
                consensus_time: 100,
                origin: sdk::Origin::External(key.to_vec()),
                me: "identity".into(),
            },
            members: None,
            residents: None,
        }
    }
    fn gated(key: &[u8], validators: Vec<Vec<u8>>, residents: Vec<Vec<u8>>) -> Self {
        let mut ctx = Self::external(key);
        ctx.members = Some(validators);
        ctx.residents = Some(residents);
        ctx
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, t: &str, r: &[u8]) -> Result<Vec<u8>, Error> {
        if t != "valset" {
            return Err(Error::QueryUnsupported);
        }
        let q = valset::decode_query(r).map_err(Error::Module)?;
        match (q, &self.members, &self.residents) {
            (valset::ValsetQuery::Validators, Some(m), _) => Ok(valset::encode_reply(
                &valset::ValsetReply::Validators(m.clone()),
            )),
            (valset::ValsetQuery::Residents, _, Some(o)) => Ok(valset::encode_reply(
                &valset::ValsetReply::Residents(o.clone()),
            )),
            _ => Err(Error::QueryUnsupported),
        }
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
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
    Identity::new("identity", None, CHAIN.to_string())
}
fn new_gated_identity() -> Identity {
    Identity::new("identity", Some("valset".into()), CHAIN.to_string())
}

/// execute a message from `origin`, then commit the block.
fn apply(id: &mut Identity, origin: &[u8], msg: IdentityMsg) -> Result<(), Error> {
    let mut ctx = TestCtx::external(origin);
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
    assert_eq!(acc.nodes, vec![node.to_vec()]);

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
fn bind_is_valset_gated_when_configured() {
    let mut id = new_gated_identity();
    let founder = ed(1);
    let node = b"node-1";
    let auth = ed_auth(&founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN, node, 0));
    let msg = IdentityMsg::BindNode { authorizer: auth };

    // origin not in validators/residents -> rejected.
    let mut ctx = TestCtx::gated(node, vec![b"someone-else".to_vec()], vec![]);
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
    let mut ctx = TestCtx::gated(node, vec![], vec![node.to_vec()]);
    block_on(id.execute(&mut ctx, &m)).expect("resident may bind");
}

#[test]
fn snapshot_install_roundtrips_mixed_scheme_membership() {
    // build an account with an ed25519 founder + a webauthn passkey + a node.
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

    // a fresh module installs the snapshot against the served root ...
    let bytes = id.snapshot();
    let root = id.root();
    let mut joiner = new_identity();
    joiner
        .install(&bytes, root)
        .expect("install verifies + adopts");

    // ... and now answers queries identically, including the passkey member.
    assert_eq!(joiner.root(), root);
    assert_eq!(
        get_account(&joiner, &account_id),
        get_account(&id, &account_id)
    );
    assert_eq!(
        account_of_member(&joiner, &wa_pub(&passkey))
            .unwrap()
            .account_id,
        account_id
    );

    // a tampered snapshot (root won't match) is refused, state untouched.
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 0xff;
    let mut victim = new_identity();
    assert!(victim.install(&corrupt, root).is_err());
    assert_eq!(victim.root(), StateRoot::ZERO);
}

#[test]
fn byzantine_snapshots_are_rejected() {
    // a legitimately-decoding baseline we then corrupt in targeted ways.
    let mut id = new_identity();
    let founder = ed(1);
    found_account(&mut id, &founder, b"node-1");
    let good = id.snapshot();
    assert!(Identity::decode_snapshot(&good).is_ok());

    // trailing byte.
    let mut trailing = good.clone();
    trailing.push(0);
    assert!(Identity::decode_snapshot(&trailing).is_err());

    // an account with zero members: hand-encode one and confirm it rejects.
    // account count 1, id-len 1 + id byte, name flag 0, nonce 0, member count 0.
    let mut zero_members = Vec::new();
    zero_members.extend_from_slice(&1u64.to_le_bytes()); // account count
    zero_members.extend_from_slice(&1u64.to_le_bytes()); // id len
    zero_members.push(0xAA); // id
    zero_members.push(0u8); // name flag
    zero_members.extend_from_slice(&0u64.to_le_bytes()); // nonce
    zero_members.extend_from_slice(&0u64.to_le_bytes()); // member count == 0
    zero_members.extend_from_slice(&0u64.to_le_bytes()); // node count
    zero_members.extend_from_slice(&0u64.to_le_bytes()); // updated_at
    let err = Identity::decode_snapshot(&zero_members).unwrap_err();
    assert!(format!("{err:?}").contains("no member keys"), "got {err:?}");
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
