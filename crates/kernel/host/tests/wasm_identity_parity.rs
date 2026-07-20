//! the adapter-port equivalence proof for the identity cutover: the
//! `identity-wasm` component (the NATIVE `identity` crate compiled to wasm
//! behind `guest-adapter`) and the native `Identity` module answer the SAME op
//! sequence with IDENTICAL query replies, and their roots move in lockstep
//! (move on commit, hold on no-ops and abort). the roots THEMSELVES differ —
//! the port persists the native canonical snapshot as one host-KV value, a
//! declared state-schema break (revision 2) — and this proof pins that
//! difference so it can never be mistaken for accidental compatibility.
//!
//! identity is the first ported module whose native constructor takes a
//! PER-NETWORK parameter: the chain id every certificate preimage folds in. a
//! wasm component is fixed bytes, so the id travels as GENESIS CONFIG — a
//! host-installed `__config` store entry (`sdk::genesis_config`) adopted at
//! construction. this proof pins the two consequences: the config is IN the
//! root (two networks' genesis roots differ — honestly, they are different
//! consensus states), and the config GOVERNS the guest (a certificate minted
//! for one chain id verifies only on the host configured with it).
//!
//! the wasm host carries a REAL genesis-seeded `valset::Valset` sibling, so
//! the member gate on `BindNode` resolves through the runtime's memoized
//! replay under real dispatch. the WebAuthn (passkey) and multi-scheme member
//! verifies run IN the guest — deterministic pure-Rust p256 on wasm32 — and
//! must answer byte-identically to the native module.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use commonware_cryptography::ed25519::PrivateKey;
use commonware_cryptography::Signer as _;
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use identity::{
    add_member_preimage, bind_preimage, encode_msg, encode_query, unbind_preimage,
    remove_member_preimage, Identity, IdentityMsg, IdentityQuery, KeyKind, MemberAuth,
    MemberProof, IDENTITY_ADD_MEMBER_NS, IDENTITY_BIND_NS, IDENTITY_REMOVE_MEMBER_NS,
    IDENTITY_UNBIND_NS, MAX_QUERY_LIMIT,
};
use sdk::{Error, Module as _, Msg, Origin, StateRoot};
use sha2::{Digest as _, Sha256};
use valset::{encode_msg as valset_encode_msg, Valset, ValsetMsg};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/identity-wasm` by the
/// module build target; committed so this proof is self-contained.
const IDENTITY_WASM: &[u8] = include_bytes!("fixtures/identity.component.wasm");

/// the chain id BOTH runtimes are constructed with — natively as a constructor
/// argument, on the wasm side as the host-installed genesis config.
const CHAIN_ID: &str = "test-chain";

/// the wasm identity at a given chain id: component + the host-computed
/// initial store carrying the `__config` genesis parameters — exactly the
/// production construction (`bin/node/src/host_state.rs`).
fn wasm_identity_with_chain(chain_id: &str) -> WasmModule {
    let mut module = WasmModule::from_bytes("identity", IDENTITY_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the identity
        // canonical state.
        .with_state_schema_revision(2);
    let config = sdk::genesis_config::encode_config(&[("chain_id", chain_id.as_bytes())]);
    let (bytes, root) = wasm_host::initial_state(&[(sdk::genesis_config::CONFIG_KEY, &config)]);
    module.install(&bytes, root).expect("install genesis config");
    module
}

fn wasm_identity() -> WasmModule {
    wasm_identity_with_chain(CHAIN_ID)
}

/// the production wiring, verbatim: identity is valset-gated under this
/// network's chain id.
fn native_identity() -> Identity {
    Identity::new("identity", Some("valset".into()), CHAIN_ID.to_string())
}

fn seeded_valset(validators: &[Vec<u8>]) -> Valset {
    let mut valset = Valset::new("valset");
    for v in validators {
        valset.insert(v.clone());
    }
    valset
}

fn native_host(validators: &[Vec<u8>]) -> Host {
    Host::genesis(vec![
        Box::new(native_identity()),
        Box::new(seeded_valset(validators)),
    ])
    .expect("genesis")
}

fn wasm_host_(validators: &[Vec<u8>]) -> Host {
    Host::genesis(vec![
        Box::new(wasm_identity()),
        Box::new(seeded_valset(validators)),
    ])
    .expect("genesis")
}

// ---- member builders (the shapes identity's own tests use) -----------------

type Ed = PrivateKey;

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

// a WebAuthn passkey, synthesized exactly as an authenticator would produce
// it (identity's own test recipe). p256 signing is RFC-6979 deterministic, so
// the proof bytes are identical on every run — no OS randomness in this proof.
fn wa_key(seed: u8) -> p256::ecdsa::SigningKey {
    p256::ecdsa::SigningKey::from_slice(&[seed; 32]).expect("valid scalar")
}
fn wa_pub(k: &p256::ecdsa::SigningKey) -> Vec<u8> {
    k.verifying_key().to_sec1_bytes().to_vec()
}
fn wa_proof(k: &p256::ecdsa::SigningKey, rp_id: &str, ns: &[u8], preimage: &[u8]) -> MemberProof {
    use p256::ecdsa::{signature::Signer as _, Signature};
    // challenge = SHA256(namespace ‖ preimage), mirroring identity's scheme.
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

// ---- op + query helpers -----------------------------------------------------

fn msg(m: &IdentityMsg) -> Msg {
    Msg {
        target: "identity".into(),
        payload: encode_msg(m),
    }
}

fn bind(founder: &Ed, node: &[u8], nonce: u64) -> Msg {
    msg(&IdentityMsg::BindNode {
        authorizer: ed_auth(founder, IDENTITY_BIND_NS, &bind_preimage(CHAIN_ID, node, nonce)),
    })
}

fn grant(key: &[u8]) -> Msg {
    Msg {
        target: "valset".into(),
        payload: valset_encode_msg(&ValsetMsg::Grant { key: key.to_vec() }),
    }
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
        protocol_version: 0,
    }
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("identity").expect("identity registered")
}

/// the read matrix: every query family — the paginated listing (full, a
/// window, an empty window), per-account gets (present + absent), and both
/// index resolvers over every key the test knows about.
async fn replies(
    h: &Host,
    accounts: &[Vec<u8>],
    nodes: &[Vec<u8>],
    members: &[Vec<u8>],
) -> Vec<Vec<u8>> {
    let mut queries = vec![
        encode_query(&IdentityQuery::All {
            from: 0,
            limit: MAX_QUERY_LIMIT,
        }),
        encode_query(&IdentityQuery::All { from: 1, limit: 1 }),
        encode_query(&IdentityQuery::All { from: 0, limit: 0 }),
        encode_query(&IdentityQuery::Get {
            account_id: b"absent".to_vec(),
        }),
        encode_query(&IdentityQuery::OfNode {
            node_key: b"absent".to_vec(),
        }),
        encode_query(&IdentityQuery::OfMember {
            member_key: b"absent".to_vec(),
        }),
    ];
    for a in accounts {
        queries.push(encode_query(&IdentityQuery::Get {
            account_id: a.clone(),
        }));
    }
    for n in nodes {
        queries.push(encode_query(&IdentityQuery::OfNode {
            node_key: n.clone(),
        }));
    }
    for m in members {
        queries.push(encode_query(&IdentityQuery::OfMember {
            member_key: m.clone(),
        }));
    }
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("identity", q).await.expect("query"));
    }
    out
}

/// the fixed keyset one world uses; shared by the matrix helpers.
struct World {
    node_a: Vec<u8>,
    node_b: Vec<u8>,
    node_c: Vec<u8>,
    founder_a: Ed,
    founder_b: Ed,
    joiner: Ed,
    passkey: p256::ecdsa::SigningKey,
}

impl World {
    fn new() -> Self {
        Self {
            node_a: ed_pub(&ed(1)),
            node_b: ed_pub(&ed(2)),
            node_c: ed_pub(&ed(3)),
            founder_a: ed(11),
            founder_b: ed(12),
            joiner: ed(21),
            passkey: wa_key(0x42),
        }
    }

    fn accounts(&self) -> Vec<Vec<u8>> {
        vec![ed_pub(&self.founder_a), ed_pub(&self.founder_b)]
    }
    fn nodes(&self) -> Vec<Vec<u8>> {
        vec![self.node_a.clone(), self.node_b.clone(), self.node_c.clone()]
    }
    fn members(&self) -> Vec<Vec<u8>> {
        vec![
            ed_pub(&self.founder_a),
            ed_pub(&self.founder_b),
            ed_pub(&self.joiner),
            wa_pub(&self.passkey),
        ]
    }
}

/// submit one accepted op to BOTH hosts and assert the parity invariants:
/// identical replies, lockstep root movement, and the pinned schema break.
async fn roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    w: &World,
    height: u64,
    origin: Origin,
    m: Msg,
    moves: bool,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect("native submit");
    wasm.submit_at(block(height, origin), m)
        .await
        .expect("wasm submit");
    assert_eq!(
        replies(native, &w.accounts(), &w.nodes(), &w.members()).await,
        replies(wasm, &w.accounts(), &w.nodes(), &w.members()).await,
        "replies diverge after block {height}"
    );
    assert_eq!(native.module_root("valset"), wasm.module_root("valset"));
    if moves {
        assert_ne!(root_of(native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(wasm), w_before, "wasm root stuck at {height}");
    } else {
        assert_eq!(root_of(native), n_before, "native root moved at {height}");
        assert_eq!(root_of(wasm), w_before, "wasm root moved at {height}");
    }
    assert_ne!(root_of(native), root_of(wasm));
}

#[test]
fn same_ops_same_replies_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let w = World::new();
    let (a_id, rp) = (ed_pub(&w.founder_a), "ducktape");
    let validators = vec![w.node_a.clone()];

    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    // the schema break is visible from GENESIS, and asymmetrically: the native
    // empty registry is the ZERO sentinel, while the wasm root commits to the
    // host-KV store already carrying the genesis config.
    assert_eq!(root_of(&native), StateRoot::ZERO, "native genesis sentinel");
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must differ — the port is a DECLARED schema break"
    );
    assert_eq!(
        replies(&native, &w.accounts(), &w.nodes(), &w.members()).await,
        replies(&wasm, &w.accounts(), &w.nodes(), &w.members()).await,
        "empty registries answer identically"
    );

    // sibling-only blocks (valset grants for the resident tier) hold the
    // identity roots on both runtimes.
    roundtrip(&mut native, &mut wasm, &w, 1, Origin::System, grant(&w.node_b), false).await;
    roundtrip(&mut native, &mut wasm, &w, 2, Origin::System, grant(&w.node_c), false).await;

    // h3: a VALIDATOR founds account A (valset gate resolves through the wasm
    // runtime's memoized replay; the bind cert verifies IN the guest).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        3,
        Origin::External(w.node_a.clone()),
        bind(&w.founder_a, &w.node_a.clone(), 0),
        true,
    )
    .await;

    // h4: the SAME bind again is declaratively idempotent — no nonce bump, no
    // staged write, the root holds on both runtimes.
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        4,
        Origin::External(w.node_a.clone()),
        bind(&w.founder_a, &w.node_a.clone(), 0),
        false,
    )
    .await;

    // h5: a RESIDENT founds account B (the union arm admits the second tier).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        5,
        Origin::External(w.node_b.clone()),
        bind(&w.founder_b, &w.node_b.clone(), 0),
        true,
    )
    .await;

    // h6: founder A admits a second ed25519 key (consent + possession over one
    // preimage, both verified in the guest). A's nonce: 1.
    let preimage = add_member_preimage(CHAIN_ID, &a_id, &ed_pub(&w.joiner), KeyKind::Ed25519, 1);
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        6,
        Origin::External(w.node_a.clone()),
        msg(&IdentityMsg::AddMemberKey {
            new_key: ed_pub(&w.joiner),
            new_kind: KeyKind::Ed25519,
            new_label: Some("laptop".into()),
            possession: ed_proof(&w.joiner, IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&w.founder_a, IDENTITY_ADD_MEMBER_NS, &preimage),
        }),
        true,
    )
    .await;

    // h7: founder A admits a WebAuthn PASSKEY — the raw ECDSA-P256 assertion
    // envelope (authData ‖ SHA256(clientDataJSON)) verifies inside the wasm
    // guest, byte-identically to the native module. A's nonce: 2.
    let preimage =
        add_member_preimage(CHAIN_ID, &a_id, &wa_pub(&w.passkey), KeyKind::WebauthnP256, 2);
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        7,
        Origin::External(w.node_a.clone()),
        msg(&IdentityMsg::AddMemberKey {
            new_key: wa_pub(&w.passkey),
            new_kind: KeyKind::WebauthnP256,
            new_label: Some("phone".into()),
            possession: wa_proof(&w.passkey, rp, IDENTITY_ADD_MEMBER_NS, &preimage),
            authorizer: ed_auth(&w.founder_a, IDENTITY_ADD_MEMBER_NS, &preimage),
        }),
        true,
    )
    .await;

    // h8: the SECOND member (not the founder) binds another node to A —
    // membership, not founding, is the authority. A's nonce: 3.
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        8,
        Origin::External(w.node_c.clone()),
        msg(&IdentityMsg::BindNode {
            authorizer: ed_auth(
                &w.joiner,
                IDENTITY_BIND_NS,
                &bind_preimage(CHAIN_ID, &w.node_c, 3),
            ),
        }),
        true,
    )
    .await;

    // h9: the PASSKEY authorizes evicting that node (the recovery path, with a
    // fresh WebAuthn assertion verified in the guest). A's nonce: 4.
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        9,
        Origin::External(w.node_a.clone()),
        msg(&IdentityMsg::UnbindNode {
            node_key: w.node_c.clone(),
            authorizer: wa_auth(
                &w.passkey,
                rp,
                IDENTITY_UNBIND_NS,
                &unbind_preimage(CHAIN_ID, &w.node_c, 4),
            ),
        }),
        true,
    )
    .await;

    // h10: the passkey evicts the second ed25519 key. A's nonce: 5.
    let preimage = remove_member_preimage(CHAIN_ID, &a_id, &ed_pub(&w.joiner), 5);
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        10,
        Origin::External(w.node_a.clone()),
        msg(&IdentityMsg::RemoveMemberKey {
            target_key: ed_pub(&w.joiner),
            authorizer: wa_auth(&w.passkey, rp, IDENTITY_REMOVE_MEMBER_NS, &preimage),
        }),
        true,
    )
    .await;

    // h11: a bound node names its account (origin-gated, no signature, no
    // nonce bump — but updated_at moves, so the root moves).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        11,
        Origin::External(w.node_a.clone()),
        msg(&IdentityMsg::SetAccountName {
            display_name: "  Kim  ".into(),
        }),
        true,
    )
    .await;

    // decoded spot check on the wasm side: the surviving membership is the
    // founder + the passkey, the name trimmed, node C evicted.
    let reply = wasm
        .query(
            "identity",
            &encode_query(&IdentityQuery::Get {
                account_id: a_id.clone(),
            }),
        )
        .await
        .expect("get A");
    let identity::IdentityReply::Account(Some(acc)) =
        identity::decode_reply(&reply).expect("decode")
    else {
        panic!("account A must exist");
    };
    assert_eq!(acc.display_name.as_deref(), Some("Kim"));
    assert_eq!(acc.nonce, 6);
    assert_eq!(acc.member_keys.len(), 2, "founder + passkey survive");
    assert_eq!(
        acc.nodes,
        vec![identity::NodeView {
            node_key: w.node_a.clone(),
            label: None,
        }]
    );

    // error-shaped queries reject identically too (needle containment — the
    // wasm runtime wraps the native reason in its wit-error rendering).
    let junk = b"definitely-not-json".to_vec();
    let n_err = native.query("identity", &junk).await.expect_err("native rejects");
    let w_err = wasm.query("identity", &junk).await.expect_err("wasm rejects");
    let Error::Module(n_msg) = n_err else {
        panic!("native query error shape: {n_err:?}");
    };
    let Error::Module(w_msg) = w_err else {
        panic!("wasm query error shape: {w_err:?}");
    };
    assert!(n_msg.contains("expected value"), "native reason: {n_msg}");
    assert!(w_msg.contains("expected value"), "wasm reason: {w_msg}");

    // queries are read-only on the wasm side too.
    let settled = root_of(&wasm);
    let _ = replies(&wasm, &w.accounts(), &w.nodes(), &w.members()).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let w = World::new();
    let a_id = ed_pub(&w.founder_a);
    let outsider = ed_pub(&ed(99));
    let validators = vec![w.node_a.clone()];

    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    // seed both worlds identically: node B gains resident standing, A is
    // founded and bound to node A.
    for host in [&mut native, &mut wasm] {
        host.submit_at(block(1, Origin::System), grant(&w.node_b))
            .await
            .expect("grant resident");
        host.submit_at(
            block(2, Origin::External(w.node_a.clone())),
            bind(&w.founder_a, &w.node_a.clone(), 0),
        )
        .await
        .expect("bind node A");
    }

    // the rejection matrix: every distinct refusal family — the valset gate
    // (decided by sibling reads inside the wasm runtime), certificate
    // verification (stale nonce), the registered-kind pin, single-ownership,
    // membership invariants, origin shapes, and the decode seam. each rejected
    // block must leave BOTH roots byte-identical (abort: no trace).
    let stale_bind = bind(&w.founder_a, &w.node_b.clone(), 0); // A's nonce is 1 now
    let mut forged_kind = ed_auth(
        &w.founder_a,
        IDENTITY_BIND_NS,
        &bind_preimage(CHAIN_ID, &w.node_b, 1),
    );
    forged_kind.kind = KeyKind::P256;
    let already_preimage =
        add_member_preimage(CHAIN_ID, &a_id, &ed_pub(&w.founder_a), KeyKind::Ed25519, 1);
    let remove_last_preimage = remove_member_preimage(CHAIN_ID, &a_id, &ed_pub(&w.founder_a), 1);

    let rejects: Vec<(Origin, Msg, &str)> = vec![
        // valset-gated: a key with no standing — the STANDING read resolves
        // through the wasm runtime and rejects.
        (
            Origin::External(outsider.clone()),
            bind(&ed(31), &outsider.clone(), 0),
            "not a network member",
        ),
        // a stale certificate: minted at nonce 0, the account is at 1.
        (
            Origin::External(w.node_b.clone()),
            stale_bind,
            "does not verify",
        ),
        // the registered-kind pin: the founder's real signature presented
        // under a forged kind.
        (
            Origin::External(w.node_b.clone()),
            msg(&IdentityMsg::BindNode {
                authorizer: forged_kind,
            }),
            "does not match its registered kind",
        ),
        // a malformed founding key for its kind (from a standing origin).
        (
            Origin::External(w.node_b.clone()),
            msg(&IdentityMsg::BindNode {
                authorizer: MemberAuth {
                    key: vec![7; 5],
                    kind: KeyKind::Ed25519,
                    proof: MemberProof::Signature { sig: vec![0; 64] },
                },
            }),
            "founding key is malformed",
        ),
        // single ownership: node A is already bound to A; founder B cannot
        // steal it.
        (
            Origin::External(w.node_a.clone()),
            bind(&w.founder_b, &w.node_a.clone(), 0),
            "already bound to another account",
        ),
        // membership invariant: the founding key is already a member.
        (
            Origin::External(w.node_a.clone()),
            msg(&IdentityMsg::AddMemberKey {
                new_key: ed_pub(&w.founder_a),
                new_kind: KeyKind::Ed25519,
                new_label: None,
                possession: MemberProof::Signature { sig: vec![0; 64] },
                authorizer: ed_auth(&w.founder_a, IDENTITY_ADD_MEMBER_NS, &already_preimage),
            }),
            "already a member",
        ),
        // membership invariant: the last key can never be removed.
        (
            Origin::External(w.node_a.clone()),
            msg(&IdentityMsg::RemoveMemberKey {
                target_key: ed_pub(&w.founder_a),
                authorizer: ed_auth(
                    &w.founder_a,
                    IDENTITY_REMOVE_MEMBER_NS,
                    &remove_last_preimage,
                ),
            }),
            "last member",
        ),
        // unbinding a node nobody bound.
        (
            Origin::External(w.node_a.clone()),
            msg(&IdentityMsg::UnbindNode {
                node_key: b"never-bound".to_vec(),
                authorizer: ed_auth(&w.founder_a, IDENTITY_UNBIND_NS, b"irrelevant"),
            }),
            "not bound",
        ),
        // naming from an unbound node.
        (
            Origin::External(w.node_b.clone()),
            msg(&IdentityMsg::SetAccountName {
                display_name: "x".into(),
            }),
            "not bound to an account",
        ),
        // an over-limit display name from a bound node.
        (
            Origin::External(w.node_a.clone()),
            msg(&IdentityMsg::SetAccountName {
                display_name: "x".repeat(65),
            }),
            "exceeds the 64-byte limit",
        ),
        // origin shapes: system and empty-external submitters.
        (
            Origin::System,
            bind(&w.founder_a, &w.node_b.clone(), 1),
            "origin-gated to external submitters",
        ),
        (
            Origin::External(Vec::new()),
            bind(&w.founder_a, &w.node_b.clone(), 1),
            "non-empty submitter",
        ),
        // the decode seam (from a fully-authorized origin).
        (
            Origin::External(w.node_a.clone()),
            Msg {
                target: "identity".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (height, (origin, m, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 3;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, origin.clone()), m.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), m)
            .await
            .expect_err("wasm must reject");

        let SubmitError::Rejected(Error::Module(n_msg)) = n_err else {
            panic!("native rejection shape: {n_err:?}");
        };
        let SubmitError::Rejected(Error::Module(w_msg)) = w_err else {
            panic!("wasm rejection shape: {w_err:?}");
        };
        assert!(n_msg.contains(needle), "native reason: {n_msg}");
        assert!(
            w_msg.contains(needle),
            "wasm reason must carry the native reason: {w_msg}"
        );

        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_eq!(
            replies(&native, &w.accounts(), &w.nodes(), &w.members()).await,
            replies(&wasm, &w.accounts(), &w.nodes(), &w.members()).await
        );
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let w = World::new();
    let outsider = ed_pub(&ed(99));
    let validators = vec![w.node_a.clone(), w.node_b.clone()];

    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    // ONE block, two ops: the bind founds account B, and the SetAccountName
    // from the SAME node reads that bind STAGED IN THIS BLOCK — on the wasm
    // side dispatch 2 reloads dispatch 1's staged `__state` (the
    // read-your-writes seam the adapter relies on), while dispatch 1's member
    // gate resolves through memoized replay.
    let batch = vec![
        (
            Origin::External(w.node_b.clone()),
            bind(&w.founder_b, &w.node_b.clone(), 0),
        ),
        (
            Origin::External(w.node_b.clone()),
            msg(&IdentityMsg::SetAccountName {
                display_name: "quack".into(),
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(1, Origin::External(w.node_b.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, Origin::External(w.node_b.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "both members must apply: {:?}",
            out.members
        );
    }
    assert_eq!(
        replies(&native, &w.accounts(), &w.nodes(), &w.members()).await,
        replies(&wasm, &w.accounts(), &w.nodes(), &w.members()).await
    );

    // ONE block where the SECOND member rejects (the standing gate — the
    // sibling read itself is what rejects): the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal
    // the accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (
            Origin::External(w.node_a.clone()),
            bind(&w.founder_a, &w.node_a.clone(), 0),
        ),
        (
            Origin::External(outsider.clone()),
            bind(&ed(31), &outsider.clone(), 0),
        ),
    ];
    let n_out = native
        .submit_block(block(2, Origin::External(w.node_a.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, Origin::External(w.node_a.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(
        replies(&native, &w.accounts(), &w.nodes(), &w.members()).await,
        replies(&wasm, &w.accounts(), &w.nodes(), &w.members()).await
    );
}

#[test]
fn genesis_config_is_consensus_state_and_governs_the_guest() {
    futures::executor::block_on(genesis_config_inner());
}

async fn genesis_config_inner() {
    // the config is IN the root: two networks' identity tenants differ at
    // GENESIS (the config is part of the root preimage), and the same network
    // constructs the identical root — per-network genesis roots, honestly.
    let here = wasm_identity_with_chain(CHAIN_ID);
    let same = wasm_identity_with_chain(CHAIN_ID);
    let other = wasm_identity_with_chain("other-chain");
    assert_eq!(here.root(), same.root(), "same config, same genesis root");
    assert_ne!(
        here.root(),
        other.root(),
        "a different chain id IS a different genesis consensus state"
    );

    // and the config GOVERNS the guest: the same bind certificate (minted for
    // CHAIN_ID) is accepted by the tenant configured with CHAIN_ID and
    // deterministically refused by the tenant configured with another chain —
    // proof the parameter actually reaches the ported logic, not just the root.
    let w = World::new();
    let validators = vec![w.node_a.clone()];
    let mut wasm = wasm_host_(&validators);
    let mut other_host = Host::genesis(vec![
        Box::new(wasm_identity_with_chain("other-chain")),
        Box::new(seeded_valset(&validators)),
    ])
    .expect("genesis");

    let m = bind(&w.founder_a, &w.node_a.clone(), 0);
    wasm.submit_at(block(1, Origin::External(w.node_a.clone())), m.clone())
        .await
        .expect("the configured chain accepts its own certificate");
    let err = other_host
        .submit_at(block(1, Origin::External(w.node_a.clone())), m)
        .await
        .expect_err("a foreign chain id must refuse the certificate");
    let SubmitError::Rejected(Error::Module(reason)) = err else {
        panic!("rejection shape: {err:?}");
    };
    assert!(
        reason.contains("does not verify"),
        "the chain-scoped preimage is what refuses: {reason}"
    );
}
