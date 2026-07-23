//! the adapter-port equivalence proof for the MERGED gateway module: the
//! `gateway` guest component (the NATIVE `gateway` crate compiled to wasm
//! behind `guest-adapter`) and the native `Gateway` module answer the SAME op
//! sequence with IDENTICAL query replies, and their roots move in lockstep
//! (move on commit, hold on no-ops and abort). the roots THEMSELVES differ —
//! the port persists the native canonical snapshot as one host-KV value, a
//! declared state-schema break (revision 3) — and this proof pins that
//! difference so it can never be mistaken for accidental compatibility.
//!
//! gateway now owns the WHOLE `.duck` name → AccountId → route pipeline: BOTH
//! the route plane AND the `.duck` handle plane absorbed from the retired
//! `duckdns` module (which had its own `duckdns` guest + parity proof —
//! both merged here). the route-plane tests below cover SetRoute; the
//! handle-plane test covers SetHandle / Resolve on the SAME merged tenant.
//!
//! like identity, gateway's constructor takes the PER-NETWORK chain id, which
//! travels as GENESIS CONFIG (a host-installed `__config` store entry —
//! `sdk::genesis_config`); the config-in-the-root and config-governs-the-guest
//! pins ride at the end of this file.
//!
//! every gateway execute depends on SIBLING reads: the valset standing gate
//! (validators ∪ residents) and the identity `OfNode` account derivation plus
//! the current-member signer check. both hosts therefore carry the REAL native
//! siblings — a genesis-seeded `valset::Valset` and an `identity::Identity`
//! wired exactly as production — so on the wasm side every acceptance and
//! every gating rejection resolves through the runtime's memoized replay. the
//! WASM host carries NATIVE identity + valset: each parity proof isolates ONE
//! wasm tenant.

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use gateway::{
    DuckDnsName, GATEWAY_ROUTE_NS, Gateway, GatewayMsg, GatewayQuery, GatewayReply,
    MAX_QUERY_LIMIT, MemberAuthorization, ResolvedAccount, RouteAudience, RouteDefinition,
    RouteMethod, RouteName, RoutePolicy, RouteStatement, RouteTarget, decode_reply, encode_msg,
    encode_query, route_signing_preimage,
};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use identity::{
    IDENTITY_BIND_NS, Identity, IdentityMsg, KeyKind, MemberAuth, MemberProof, bind_preimage,
};
use sdk::{Error, Module as _, Msg, Origin, StateRoot};
use valset::{Valset, ValsetMsg, encode_msg as valset_encode_msg};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `gateway` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const GATEWAY_WASM: &[u8] = include_bytes!("fixtures/gateway.component.wasm");

/// the chain id BOTH runtimes are constructed with — natively as a constructor
/// argument, on the wasm side as the host-installed genesis config. the
/// identity sibling binds under the same id (one network, one chain id).
const CHAIN_ID: &str = "test-chain";

/// the wasm gateway at a given chain id: component + the host-computed initial
/// store carrying the `__config` genesis parameters — exactly the production
/// construction (`bin/node/src/host_state.rs`).
fn wasm_gateway_with_chain(chain_id: &str) -> WasmModule {
    let mut module = WasmModule::from_bytes("gateway", GATEWAY_WASM).expect("load component");
    let config = sdk::genesis_config::encode_config(&[("chain_id", chain_id.as_bytes())]);
    let (bytes, root) = wasm_host::initial_state(&[(sdk::genesis_config::CONFIG_KEY, &config)]);
    module
        .install(&bytes, root)
        .expect("install genesis config");
    module
}

fn wasm_gateway() -> WasmModule {
    wasm_gateway_with_chain(CHAIN_ID)
}

/// the production wiring, verbatim (`bin/node/src/host_state.rs`).
fn native_gateway() -> Gateway {
    Gateway::new("gateway", "identity", Some("valset".into()), CHAIN_ID)
}

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
        Box::new(native_gateway()),
        Box::new(native_identity()),
        Box::new(seeded_valset(validators)),
    ])
    .expect("genesis")
}

fn wasm_host_(validators: &[Vec<u8>]) -> Host {
    Host::genesis(vec![
        Box::new(wasm_gateway()),
        Box::new(native_identity()),
        Box::new(seeded_valset(validators)),
    ])
    .expect("genesis")
}

type Ed = PrivateKey;

fn ed(seed: u64) -> Ed {
    Ed::from_seed(seed)
}
fn ed_pub(k: &Ed) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}

/// an identity BindNode op founding `founder`'s account from the submitting
/// node — the certificate flow identity's tests drive, here to seed the REAL
/// identity sibling both gateways read through.
fn bind(founder: &Ed, node: &[u8]) -> Msg {
    let auth = MemberAuth {
        key: ed_pub(founder),
        kind: KeyKind::Ed25519,
        proof: MemberProof::Signature {
            sig: founder
                .sign(IDENTITY_BIND_NS, &bind_preimage(CHAIN_ID, node, 0))
                .as_ref()
                .to_vec(),
        },
    };
    Msg {
        target: "identity".into(),
        payload: identity::encode_msg(&IdentityMsg::BindNode { authorizer: auth }),
    }
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
    }
}

// ---- route builders ---------------------------------------------------------

/// a 64-char lowercase hex manifest digest, distinct per seed.
fn manifest_hex(seed: u8) -> String {
    format!("{seed:02x}").repeat(32)
}

/// a static-content route: GET+HEAD, bodyless, bounded response — the shape
/// `validate_route` demands of DuckFS targets.
fn content_route(seed: u8) -> RouteDefinition {
    RouteDefinition {
        target: RouteTarget::DuckFs {
            manifest_sha256: manifest_hex(seed),
        },
        policy: RoutePolicy {
            audience: RouteAudience::Network,
            methods: vec![RouteMethod::Get, RouteMethod::Head],
            max_request_bytes: 0,
            max_response_bytes: 1024 * 1024,
            allow_authorization: false,
            allow_upgrade: false,
        },
    }
}

/// a loopback-HTTP route with a body-bearing method, streaming responses and
/// websocket upgrade — the other target family.
fn loopback_route() -> RouteDefinition {
    RouteDefinition {
        target: RouteTarget::LoopbackHttp,
        policy: RoutePolicy {
            audience: RouteAudience::Owner,
            methods: vec![RouteMethod::Get, RouteMethod::Post],
            max_request_bytes: 1024,
            max_response_bytes: 0,
            allow_authorization: false,
            allow_upgrade: true,
        },
    }
}

fn statement(
    account_id: &[u8],
    label: Option<&str>,
    publisher: &[u8],
    revision: u64,
    route: Option<RouteDefinition>,
) -> RouteStatement {
    RouteStatement {
        version: 1,
        chain_id: CHAIN_ID.into(),
        account_id: account_id.to_vec(),
        name: match label {
            Some(l) => RouteName::named(l),
            None => RouteName::apex(),
        },
        publisher_node: publisher.to_vec(),
        revision,
        route,
    }
}

/// a SetRoute msg whose statement is signed for real by `signer` (an Ed25519
/// account member) over the canonical preimage.
fn set_route(st: RouteStatement, signer: &Ed) -> Msg {
    let preimage = route_signing_preimage(&st).expect("statement validates");
    let authorization = MemberAuthorization {
        signer: ed_pub(signer),
        signature: signer.sign(GATEWAY_ROUTE_NS, &preimage).as_ref().to_vec(),
    };
    Msg {
        target: "gateway".into(),
        payload: encode_msg(&GatewayMsg::SetRoute {
            statement: st,
            authorization,
        }),
    }
}

/// a SetRoute msg with an EXPLICIT authorization — for statements the
/// canonical preimage refuses to encode (bad label, revision 0, a policy the
/// target forbids), where the module's own validation is the thing under test.
fn set_route_raw(st: RouteStatement, signer: Vec<u8>, signature: Vec<u8>) -> Msg {
    Msg {
        target: "gateway".into(),
        payload: encode_msg(&GatewayMsg::SetRoute {
            statement: st,
            authorization: MemberAuthorization { signer, signature },
        }),
    }
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("gateway").expect("gateway registered")
}

/// the read matrix: exact-name gets (present, tombstoned, absent) and the
/// management listings for both accounts plus an absent one.
async fn replies(h: &Host, accounts: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut queries = Vec::new();
    for a in accounts {
        for label in [None, Some("api"), Some("web"), Some("multi"), Some("iso")] {
            queries.push(encode_query(&GatewayQuery::Get {
                account_id: a.clone(),
                name: match label {
                    Some(l) => RouteName::named(l),
                    None => RouteName::apex(),
                },
            }));
        }
        queries.push(encode_query(&GatewayQuery::List {
            account_id: a.clone(),
        }));
    }
    queries.push(encode_query(&GatewayQuery::Get {
        account_id: b"absent".to_vec(),
        name: RouteName::apex(),
    }));
    queries.push(encode_query(&GatewayQuery::List {
        account_id: b"absent".to_vec(),
    }));
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("gateway", q).await.expect("query"));
    }
    out
}

/// the world both tests stand up: node A a validator bound to account A, node
/// B a resident bound to account B, node C a resident bound to NO account
/// (the seam between the two sibling gates).
struct World {
    node_a: Vec<u8>,
    node_b: Vec<u8>,
    node_c: Vec<u8>,
    founder_a: Ed,
    founder_b: Ed,
}

impl World {
    fn new() -> Self {
        Self {
            node_a: ed_pub(&ed(1)),
            node_b: ed_pub(&ed(2)),
            node_c: ed_pub(&ed(3)),
            founder_a: ed(11),
            founder_b: ed(12),
        }
    }
    fn a_id(&self) -> Vec<u8> {
        ed_pub(&self.founder_a)
    }
    fn b_id(&self) -> Vec<u8> {
        ed_pub(&self.founder_b)
    }
    fn accounts(&self) -> Vec<Vec<u8>> {
        vec![self.a_id(), self.b_id()]
    }

    /// stand up the shared siblings on one host. these blocks touch only the
    /// NATIVE siblings — the gateway root must hold through all of them.
    async fn seed(&self, host: &mut Host) {
        host.submit_at(block(1, Origin::System), grant(&self.node_b))
            .await
            .expect("grant resident B");
        host.submit_at(block(2, Origin::System), grant(&self.node_c))
            .await
            .expect("grant resident C");
        host.submit_at(
            block(3, Origin::External(self.node_a.clone())),
            bind(&self.founder_a, &self.node_a),
        )
        .await
        .expect("bind node A");
        host.submit_at(
            block(4, Origin::External(self.node_b.clone())),
            bind(&self.founder_b, &self.node_b),
        )
        .await
        .expect("bind node B");
    }
}

/// submit one accepted op to BOTH hosts and assert the parity invariants.
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
        replies(native, &w.accounts()).await,
        replies(wasm, &w.accounts()).await,
        "replies diverge after block {height}"
    );
    for sibling in ["identity", "valset"] {
        assert_eq!(
            native.module_root(sibling),
            wasm.module_root(sibling),
            "the native {sibling} sibling diverged"
        );
    }
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
    let validators = vec![w.node_a.clone()];
    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    // the schema break is visible from GENESIS, asymmetrically: the native
    // empty registry is the ZERO sentinel, the wasm root commits to the
    // host-KV store already carrying the genesis config.
    assert_eq!(root_of(&native), StateRoot::ZERO, "native genesis sentinel");
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must differ — the port is a DECLARED schema break"
    );

    // sibling-only seeding (grants + binds) holds the gateway roots.
    let (n0, w0) = (root_of(&native), root_of(&wasm));
    w.seed(&mut native).await;
    w.seed(&mut wasm).await;
    assert_eq!(root_of(&native), n0, "sibling blocks hold the native root");
    assert_eq!(root_of(&wasm), w0, "sibling blocks hold the wasm root");

    // h5: a VALIDATOR publishes its apex content route (both sibling gates —
    // valset standing, identity OfNode + signer membership — resolve through
    // the wasm runtime's memoized replay; the route signature verifies IN the
    // guest).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        5,
        Origin::External(w.node_a.clone()),
        set_route(
            statement(&w.a_id(), None, &w.node_a, 1, Some(content_route(0x11))),
            &w.founder_a,
        ),
        true,
    )
    .await;

    // h6: a RESIDENT publishes a labeled loopback route (the union arm).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        6,
        Origin::External(w.node_b.clone()),
        set_route(
            statement(&w.b_id(), Some("api"), &w.node_b, 1, Some(loopback_route())),
            &w.founder_b,
        ),
        true,
    )
    .await;

    // h7: a monotonic apex update (revision 2, new manifest).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        7,
        Origin::External(w.node_a.clone()),
        set_route(
            statement(&w.a_id(), None, &w.node_a, 2, Some(content_route(0x22))),
            &w.founder_a,
        ),
        true,
    )
    .await;

    // h8: an authenticated TOMBSTONE (route None) — still advances the
    // revision stream, drops out of List, stays Get-able.
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        8,
        Origin::External(w.node_a.clone()),
        set_route(statement(&w.a_id(), None, &w.node_a, 3, None), &w.founder_a),
        true,
    )
    .await;

    // h9: republishing over the tombstone continues the same stream (rev 4).
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        9,
        Origin::External(w.node_a.clone()),
        set_route(
            statement(&w.a_id(), None, &w.node_a, 4, Some(loopback_route())),
            &w.founder_a,
        ),
        true,
    )
    .await;

    // h10: a second label under A lands beside the apex.
    roundtrip(
        &mut native,
        &mut wasm,
        &w,
        10,
        Origin::External(w.node_a.clone()),
        set_route(
            statement(
                &w.a_id(),
                Some("web"),
                &w.node_a,
                1,
                Some(content_route(0x33)),
            ),
            &w.founder_a,
        ),
        true,
    )
    .await;

    // decoded spot check on the wasm side: the apex is back live at rev 4 as
    // loopback, "web" serves content, and B's listing carries exactly "api".
    let reply = wasm
        .query(
            "gateway",
            &encode_query(&GatewayQuery::List {
                account_id: w.a_id(),
            }),
        )
        .await
        .expect("list A");
    let gateway::GatewayReply::Routes(routes) = gateway::decode_reply(&reply).expect("decode")
    else {
        panic!("expected Routes reply");
    };
    assert_eq!(routes.len(), 2, "apex + web are live");
    assert_eq!(routes[0].name, RouteName::apex());
    assert_eq!(routes[0].revision, 4);
    assert_eq!(routes[0].target, "loopback_http");
    assert_eq!(routes[1].name, RouteName::named("web"));

    // error-shaped queries reject identically (needle containment).
    for (q, needle) in [
        (
            encode_query(&GatewayQuery::List {
                account_id: Vec::new(),
            }),
            "account id must be",
        ),
        (b"definitely-not-json".to_vec(), "expected value"),
    ] {
        let n_err = native
            .query("gateway", &q)
            .await
            .expect_err("native rejects");
        let w_err = wasm.query("gateway", &q).await.expect_err("wasm rejects");
        let Error::Module(n_msg) = n_err else {
            panic!("native query error shape: {n_err:?}");
        };
        let Error::Module(w_msg) = w_err else {
            panic!("wasm query error shape: {w_err:?}");
        };
        assert!(n_msg.contains(needle), "native query reason: {n_msg}");
        assert!(
            w_msg.contains(needle),
            "wasm query reason must carry the native reason: {w_msg}"
        );
    }

    // queries are read-only on the wasm side too.
    let settled = root_of(&wasm);
    let _ = replies(&wasm, &w.accounts()).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let w = World::new();
    let outsider = ed_pub(&ed(99));
    let outsider_signer = ed(99);
    let validators = vec![w.node_a.clone()];
    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);
    for host in [&mut native, &mut wasm] {
        w.seed(host).await;
        // one committed route so the revision matrix has a stream to violate.
        host.submit_at(
            block(5, Origin::External(w.node_a.clone())),
            set_route(
                statement(&w.a_id(), None, &w.node_a, 1, Some(content_route(0x11))),
                &w.founder_a,
            ),
        )
        .await
        .expect("seed route");
    }

    // a statement whose label the canonical grammar refuses (module-side
    // validation is under test, so the authorization is explicit junk).
    let bad_label = statement(
        &w.a_id(),
        Some("Bad_Label"),
        &w.node_a,
        1,
        Some(loopback_route()),
    );
    let zero_revision = statement(&w.a_id(), Some("api"), &w.node_a, 0, Some(loopback_route()));
    // a content route violating the signed content-policy shape (POST).
    let mut bad_content = content_route(0x44);
    bad_content.policy.methods = vec![RouteMethod::Get, RouteMethod::Post];
    bad_content.policy.max_request_bytes = 64;
    let bad_content_st = statement(&w.a_id(), Some("api"), &w.node_a, 1, Some(bad_content));

    // the rejection matrix: both sibling gates (valset standing, identity
    // account), chain scoping, publisher/account/signer authority, signature
    // verification, the monotonic revision stream, statement grammar, origin
    // shapes, and the decode seam. each rejected block leaves BOTH roots
    // byte-identical.
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        // valset-gated: no standing anywhere — decided by the sibling reads.
        (
            Origin::External(outsider.clone()),
            set_route(
                statement(&outsider, None, &outsider, 1, Some(loopback_route())),
                &outsider_signer,
            ),
            "not a validator or admitted resident",
        ),
        // identity-gated: resident standing but NO bound account.
        (
            Origin::External(w.node_c.clone()),
            set_route(
                statement(&w.a_id(), None, &w.node_c, 2, Some(loopback_route())),
                &w.founder_a,
            ),
            "not bound to an Identity account",
        ),
        // chain scoping: a statement for another network.
        (
            Origin::External(w.node_a.clone()),
            set_route(
                RouteStatement {
                    chain_id: "other-chain".into(),
                    ..statement(&w.a_id(), None, &w.node_a, 2, Some(loopback_route()))
                },
                &w.founder_a,
            ),
            "belongs to another chain",
        ),
        // publisher authority: the signed publisher is not the origin.
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(&w.a_id(), None, &w.node_b, 2, Some(loopback_route())),
                &w.founder_a,
            ),
            "publisher does not match",
        ),
        // account authority: the origin node belongs to A, not B.
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(&w.b_id(), None, &w.node_a, 1, Some(loopback_route())),
                &w.founder_b,
            ),
            "does not own the publisher node",
        ),
        // signer authority: a key outside the account's member set.
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(&w.a_id(), None, &w.node_a, 2, Some(loopback_route())),
                &outsider_signer,
            ),
            "not a current Ed25519 account member",
        ),
        // signature verification: a member signer, a junk signature.
        (
            Origin::External(w.node_a.clone()),
            set_route_raw(
                statement(&w.a_id(), None, &w.node_a, 2, Some(loopback_route())),
                ed_pub(&w.founder_a),
                vec![9; 64],
            ),
            "signature does not verify",
        ),
        // the monotonic stream: a replayed revision...
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(&w.a_id(), None, &w.node_a, 1, Some(content_route(0x55))),
                &w.founder_a,
            ),
            "route revision must be",
        ),
        // ...and a skipped one.
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(&w.a_id(), None, &w.node_a, 7, Some(content_route(0x55))),
                &w.founder_a,
            ),
            "route revision must be",
        ),
        // statement grammar: label, zero revision, content-policy shape.
        (
            Origin::External(w.node_a.clone()),
            set_route_raw(bad_label, ed_pub(&w.founder_a), vec![9; 64]),
            "invalid route label",
        ),
        (
            Origin::External(w.node_a.clone()),
            set_route_raw(zero_revision, ed_pub(&w.founder_a), vec![9; 64]),
            "revision starts at 1",
        ),
        (
            Origin::External(w.node_a.clone()),
            set_route_raw(bad_content_st, ed_pub(&w.founder_a), vec![9; 64]),
            "content routes require",
        ),
        // origin shapes.
        (
            Origin::System,
            set_route(
                statement(&w.a_id(), None, &w.node_a, 2, Some(loopback_route())),
                &w.founder_a,
            ),
            "requires an external node origin",
        ),
        (
            Origin::External(vec![7; 16]),
            set_route(
                statement(&w.a_id(), None, &w.node_a, 2, Some(loopback_route())),
                &w.founder_a,
            ),
            "32-byte node key",
        ),
        // the decode seam (from a fully-authorized origin).
        (
            Origin::External(w.node_a.clone()),
            Msg {
                target: "gateway".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (height, (origin, m, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 6;
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
            replies(&native, &w.accounts()).await,
            replies(&wasm, &w.accounts()).await
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
    let outsider_signer = ed(99);
    let validators = vec![w.node_a.clone()];
    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);
    w.seed(&mut native).await;
    w.seed(&mut wasm).await;

    // ONE block, two revisions of the SAME route: the second dispatch's
    // monotonic check reads the first dispatch's STAGED revision — on the wasm
    // side that is the outer staged `__state` reloaded by dispatch 2 (the
    // read-your-writes seam), with both sibling gates resolving through
    // memoized replay on each dispatch.
    let batch = vec![
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(
                    &w.a_id(),
                    Some("multi"),
                    &w.node_a,
                    1,
                    Some(content_route(0x11)),
                ),
                &w.founder_a,
            ),
        ),
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(
                    &w.a_id(),
                    Some("multi"),
                    &w.node_a,
                    2,
                    Some(loopback_route()),
                ),
                &w.founder_a,
            ),
        ),
    ];
    let n_out = native
        .submit_block(block(5, Origin::External(w.node_a.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(5, Origin::External(w.node_a.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "both revisions must apply: {:?}",
            out.members
        );
    }
    assert_eq!(
        replies(&native, &w.accounts()).await,
        replies(&wasm, &w.accounts()).await
    );

    // ONE block, three members: an accepted publish, a REPLAYED revision the
    // staged stream rejects (the rejection is DECIDED by read-your-writes),
    // and an accepted publish from the other account — committed state must
    // equal the accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(
                    &w.a_id(),
                    Some("iso"),
                    &w.node_a,
                    1,
                    Some(content_route(0x22)),
                ),
                &w.founder_a,
            ),
        ),
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(
                    &w.a_id(),
                    Some("iso"),
                    &w.node_a,
                    1,
                    Some(content_route(0x33)),
                ),
                &w.founder_a,
            ),
        ),
        (
            Origin::External(w.node_b.clone()),
            set_route(
                statement(&w.b_id(), None, &w.node_b, 1, Some(loopback_route())),
                &w.founder_b,
            ),
        ),
    ];
    let n_out = native
        .submit_block(block(6, Origin::External(w.node_a.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(6, Origin::External(w.node_a.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(
            matches!(out.members[1], MemberOutcome::Rejected { .. }),
            "the replayed revision must reject against the SAME-BLOCK stage: {:?}",
            out.members
        );
        assert!(matches!(out.members[2], MemberOutcome::Applied { .. }));
    }
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(
        replies(&native, &w.accounts()).await,
        replies(&wasm, &w.accounts()).await
    );

    // one block where a member from an account WITHOUT authority rejects
    // between two acceptances — the outsider's member is gate-rejected by the
    // sibling reads, leaving the accepted subset only.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (
            Origin::External(w.node_a.clone()),
            set_route(
                statement(&w.a_id(), Some("api"), &w.node_a, 1, Some(loopback_route())),
                &w.founder_a,
            ),
        ),
        (
            Origin::External(outsider.clone()),
            set_route(
                statement(&outsider, None, &outsider, 1, Some(loopback_route())),
                &outsider_signer,
            ),
        ),
    ];
    let n_out = native
        .submit_block(block(7, Origin::External(w.node_a.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(7, Origin::External(w.node_a.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(
        replies(&native, &w.accounts()).await,
        replies(&wasm, &w.accounts()).await
    );
}

#[test]
fn genesis_config_is_consensus_state_and_governs_the_guest() {
    futures::executor::block_on(genesis_config_inner());
}

async fn genesis_config_inner() {
    // the config is IN the root: per-network genesis roots, honestly.
    let here = wasm_gateway_with_chain(CHAIN_ID);
    let same = wasm_gateway_with_chain(CHAIN_ID);
    let other = wasm_gateway_with_chain("other-chain");
    assert_eq!(here.root(), same.root(), "same config, same genesis root");
    assert_ne!(
        here.root(),
        other.root(),
        "a different chain id IS a different genesis consensus state"
    );

    // and the config GOVERNS the guest: the same signed route statement
    // (scoped to CHAIN_ID) is accepted by the tenant configured with CHAIN_ID
    // and deterministically refused as another chain's by the tenant
    // configured differently.
    let w = World::new();
    let validators = vec![w.node_a.clone()];
    let mut wasm = wasm_host_(&validators);
    let mut other_host = Host::genesis(vec![
        Box::new(wasm_gateway_with_chain("other-chain")),
        Box::new(native_identity()),
        Box::new(seeded_valset(&validators)),
    ])
    .expect("genesis");
    w.seed(&mut wasm).await;
    w.seed(&mut other_host).await;

    let m = set_route(
        statement(&w.a_id(), None, &w.node_a, 1, Some(content_route(0x11))),
        &w.founder_a,
    );
    wasm.submit_at(block(5, Origin::External(w.node_a.clone())), m.clone())
        .await
        .expect("the configured chain accepts its own statement");
    let err = other_host
        .submit_at(block(5, Origin::External(w.node_a.clone())), m)
        .await
        .expect_err("a foreign chain id must refuse the statement");
    let SubmitError::Rejected(Error::Module(reason)) = err else {
        panic!("rejection shape: {err:?}");
    };
    assert!(
        reason.contains("belongs to another chain"),
        "the chain scope is what refuses: {reason}"
    );
}

// ---- handle plane (absorbed from duckdns) -----------------------------------

/// a SetHandle msg on the MERGED gateway module — the `.duck` human-name facet.
fn set_handle(handle: Option<&str>) -> Msg {
    Msg {
        target: "gateway".into(),
        payload: encode_msg(&GatewayMsg::SetHandle {
            handle: handle.map(Into::into),
        }),
    }
}

fn resolve_query(handle: &str) -> Vec<u8> {
    encode_query(&GatewayQuery::Resolve {
        name: DuckDnsName {
            handle: handle.into(),
        },
    })
}

/// the handle-plane read matrix: present/renamed/freed/absent resolves plus the
/// full registration listing — the whole surface duckdns's own parity pinned.
async fn handle_replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        resolve_query("orthory"),
        resolve_query("renamed"),
        resolve_query("quack-2"),
        resolve_query("absent"),
        encode_query(&GatewayQuery::Registrations {
            from: 0,
            limit: MAX_QUERY_LIMIT,
        }),
        encode_query(&GatewayQuery::Registrations { from: 1, limit: 1 }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("gateway", q).await.expect("query"));
    }
    out
}

async fn resolved(h: &Host, handle: &str) -> Option<ResolvedAccount> {
    let reply = h
        .query("gateway", &resolve_query(handle))
        .await
        .expect("resolve");
    match decode_reply(&reply).expect("decode") {
        GatewayReply::Resolved(r) => r,
        other => panic!("expected Resolved, got {other:?}"),
    }
}

#[test]
fn handle_plane_ops_stay_in_lockstep_on_the_merged_tenant() {
    futures::executor::block_on(handle_plane_inner());
}

async fn handle_plane_inner() {
    let w = World::new();
    let validators = vec![w.node_a.clone()];
    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    // the schema break is visible from genesis (native ZERO sentinel vs the
    // wasm host-KV root that already commits to `__config`).
    assert_eq!(root_of(&native), StateRoot::ZERO, "native genesis sentinel");
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots differ (schema break)"
    );

    // sibling-only seed blocks leave the gateway root untouched on both sides.
    w.seed(&mut native).await;
    w.seed(&mut wasm).await;
    assert_eq!(
        root_of(&native),
        StateRoot::ZERO,
        "seed holds the native root"
    );

    // every handle op family in one deterministic sequence; `moves` says
    // whether committed state changes — root movement must agree on both sides.
    // node A is a validator bound to account A; node B a resident bound to B.
    let ops: Vec<(Vec<u8>, Option<&str>, bool)> = vec![
        (w.node_a.clone(), Some("orthory"), true),   // validator registers
        (w.node_b.clone(), Some("quack-2"), true),   // resident registers
        (w.node_a.clone(), Some("orthory"), false),  // idempotent no-op
        (w.node_a.clone(), Some("renamed"), true),   // atomic rename frees "orthory"
        (w.node_b.clone(), None, true),              // unregister "quack-2"
        (w.node_b.clone(), Some("orthory"), true),   // claim the freed name
    ];

    for (i, (who, handle, moves)) in ops.into_iter().enumerate() {
        let height = i as u64 + 5;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(
                block(height, Origin::External(who.clone())),
                set_handle(handle),
            )
            .await
            .expect("native submit");
        wasm.submit_at(block(height, Origin::External(who)), set_handle(handle))
            .await
            .expect("wasm submit");

        assert_eq!(
            handle_replies(&native).await,
            handle_replies(&wasm).await,
            "handle replies diverge after block {height}"
        );
        if moves {
            assert_ne!(root_of(&native), n_before, "native stuck at {height}");
            assert_ne!(root_of(&wasm), w_before, "wasm stuck at {height}");
        } else {
            assert_eq!(root_of(&native), n_before, "native moved at {height}");
            assert_eq!(root_of(&wasm), w_before, "wasm moved at {height}");
        }
        assert_ne!(root_of(&native), root_of(&wasm), "the pinned schema break");
    }

    // resolution stops at the stable AccountId (the founding key), never a node.
    assert_eq!(
        resolved(&wasm, "renamed").await,
        Some(ResolvedAccount {
            account_id: w.a_id()
        }),
        "A's rename resolves to A's account id"
    );
    assert_eq!(
        resolved(&wasm, "orthory").await,
        Some(ResolvedAccount {
            account_id: w.b_id()
        }),
        "the freed handle now belongs to B's account"
    );
    assert_eq!(
        resolved(&wasm, "quack-2").await,
        None,
        "B's old name is gone"
    );

    // a reserved root label is refused identically on both runtimes, and the
    // reject leaves BOTH roots byte-identical to pre-block (abort, no trace).
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let n_err = native
        .submit_at(
            block(20, Origin::External(w.node_a.clone())),
            set_handle(Some("net")),
        )
        .await
        .expect_err("native rejects reserved");
    let w_err = wasm
        .submit_at(
            block(20, Origin::External(w.node_a.clone())),
            set_handle(Some("net")),
        )
        .await
        .expect_err("wasm rejects reserved");
    for err in [n_err, w_err] {
        let SubmitError::Rejected(Error::Module(reason)) = err else {
            panic!("rejection shape: {err:?}");
        };
        assert!(reason.contains("reserved"), "reason: {reason}");
    }
    assert_eq!(root_of(&native), n_before, "native moved on reject");
    assert_eq!(root_of(&wasm), w_before, "wasm moved on reject");
}
