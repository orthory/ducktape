//! the adapter-port equivalence proof for the duckdns cutover: the
//! `duckdns-wasm` component (the NATIVE `duckdns` crate compiled to wasm behind
//! `guest-adapter`) and the native `DuckDns` module answer the SAME op sequence
//! with IDENTICAL query replies, and their roots move in lockstep (move on
//! commit, hold on no-ops and abort). the roots THEMSELVES differ — the port
//! persists the native canonical snapshot as one host-KV value, a declared
//! state-schema break (revision 2) — and this proof pins that difference so it
//! can never be mistaken for accidental compatibility.
//!
//! duckdns is the first ported module whose EVERY execute depends on SIBLING
//! reads: the valset standing gate (validators ∪ residents) and the identity
//! `OfNode` account derivation. both hosts therefore carry the REAL native
//! siblings — a genesis-seeded `valset::Valset` and an `identity::Identity`
//! wired exactly as production (`bin/node/src/host_state.rs`) — so on the wasm
//! side every acceptance and every gating rejection resolves through the
//! runtime's memoized-replay machinery (`query-module` pauses, the host
//! resolves through `Ctx`, the pure guest re-runs with the answer memoized).

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use duckdns::{
    decode_reply, encode_msg, encode_query, DuckDns, DuckDnsMsg, DuckDnsName, DuckDnsQuery,
    DuckDnsReply, ResolvedAccount, MAX_QUERY_LIMIT,
};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use identity::{
    bind_preimage, Identity, IdentityMsg, KeyKind, MemberAuth, MemberProof, IDENTITY_BIND_NS,
};
use sdk::{Error, Msg, Origin, StateRoot};
use valset::{Valset, ValsetMsg};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/examples/duckdns-wasm` (see that
/// crate's Cargo.toml for the componentize recipe); committed so this proof is
/// self-contained.
const DUCKDNS_WASM: &[u8] = include_bytes!("fixtures/duckdns.component.wasm");

/// the chain id the identity sibling is constructed with — the same string the
/// identity module's own tests bind under (`crates/system/identity/src/tests.rs`).
const CHAIN_ID: &str = "test-chain";

fn wasm_duckdns() -> WasmModule {
    WasmModule::from_bytes("duckdns", DUCKDNS_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the duckdns
        // canonical state.
        .with_state_schema_revision(2)
}

/// a real ed25519 keypair: node keys double as valset members AND as the
/// 32-byte external origins duckdns authenticates; founder keys sign the
/// identity bind certificates.
fn signer(seed: u64) -> PrivateKey {
    PrivateKey::from_seed(seed)
}

fn pubkey(k: &PrivateKey) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}

/// the production wiring, verbatim (`bin/node/src/host_state.rs`): duckdns
/// derives accounts through "identity" and gates mutations through "valset";
/// identity itself is valset-gated under this network's chain id.
fn native_duckdns() -> DuckDns {
    DuckDns::new("duckdns", "identity", Some("valset".into()))
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
        Box::new(native_duckdns()),
        Box::new(native_identity()),
        Box::new(seeded_valset(validators)),
    ])
    .expect("genesis")
}

fn wasm_host_(validators: &[Vec<u8>]) -> Host {
    Host::genesis(vec![
        Box::new(wasm_duckdns()),
        Box::new(native_identity()),
        Box::new(seeded_valset(validators)),
    ])
    .expect("genesis")
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

fn set_handle(handle: Option<&str>) -> Msg {
    Msg {
        target: "duckdns".into(),
        payload: encode_msg(&DuckDnsMsg::SetHandle {
            handle: handle.map(Into::into),
        }),
    }
}

/// an identity BindNode op: `founder` consents (at the fresh account's nonce 0)
/// to binding the submitting node — the certificate flow identity's own tests
/// drive, here against the REAL registered module on both hosts.
fn bind(node_pub: &[u8], founder: &PrivateKey) -> Msg {
    let auth = MemberAuth {
        key: pubkey(founder),
        kind: KeyKind::Ed25519,
        proof: MemberProof::Signature {
            sig: founder
                .sign(IDENTITY_BIND_NS, &bind_preimage(CHAIN_ID, node_pub, 0))
                .as_ref()
                .to_vec(),
        },
    };
    Msg {
        target: "identity".into(),
        payload: identity::encode_msg(&IdentityMsg::BindNode { authorizer: auth }),
    }
}

/// grant RESIDENT standing (mesh tier, no quorum seat) — duckdns admits the
/// validators ∪ residents union, so the matrix covers both classes.
fn grant(node_pub: &[u8]) -> Msg {
    Msg {
        target: "valset".into(),
        payload: valset::encode_msg(&ValsetMsg::Grant {
            key: node_pub.to_vec(),
        }),
    }
}

fn resolve_query(handle: &str) -> Vec<u8> {
    encode_query(&DuckDnsQuery::Resolve {
        name: DuckDnsName {
            handle: handle.into(),
        },
    })
}

/// the read matrix: every query family — present/absent resolves, the full
/// registration listing, a pagination window, and an empty window.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        resolve_query("orthory"),
        resolve_query("renamed"),
        resolve_query("quack-2"),
        resolve_query("absent"),
        encode_query(&DuckDnsQuery::Registrations {
            from: 0,
            limit: MAX_QUERY_LIMIT,
        }),
        encode_query(&DuckDnsQuery::Registrations { from: 1, limit: 1 }),
        encode_query(&DuckDnsQuery::Registrations { from: 0, limit: 0 }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("duckdns", q).await.expect("query"));
    }
    out
}

async fn resolved(h: &Host, handle: &str) -> Option<ResolvedAccount> {
    let reply = h
        .query("duckdns", &resolve_query(handle))
        .await
        .expect("resolve query");
    match decode_reply(&reply).expect("decode") {
        DuckDnsReply::Resolved(r) => r,
        other => panic!("expected Resolved, got {other:?}"),
    }
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("duckdns").expect("duckdns registered")
}

/// stand up the shared world on one host: nodeB gains resident standing, then
/// both nodes bind to their (fresh, founder-keyed) identity accounts. these
/// blocks touch only the NATIVE siblings — the duckdns root must hold through
/// all of them on both runtimes.
async fn seed(host: &mut Host, node_a: &[u8], node_b: &[u8], founder_a: &PrivateKey, founder_b: &PrivateKey) {
    host.submit_at(block(1, Origin::System), grant(node_b))
        .await
        .expect("grant resident");
    host.submit_at(block(2, Origin::External(node_a.to_vec())), bind(node_a, founder_a))
        .await
        .expect("bind node A");
    host.submit_at(block(3, Origin::External(node_b.to_vec())), bind(node_b, founder_b))
        .await
        .expect("bind node B");
}

#[test]
fn same_ops_same_replies_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let (node_a, node_b) = (signer(1), signer(2));
    let (founder_a, founder_b) = (signer(11), signer(12));
    let (a_pub, b_pub) = (pubkey(&node_a), pubkey(&node_b));
    let validators = vec![a_pub.clone()];

    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    // the schema break is visible from genesis: the native empty root is the
    // ZERO sentinel, the wasm root commits to the (empty) host-KV store.
    assert_eq!(root_of(&native), StateRoot::ZERO, "native genesis sentinel");
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must differ — the port is a DECLARED schema break"
    );

    // sibling-only blocks (valset grant + identity binds) leave duckdns alone.
    let (n0, w0) = (root_of(&native), root_of(&wasm));
    seed(&mut native, &a_pub, &b_pub, &founder_a, &founder_b).await;
    seed(&mut wasm, &a_pub, &b_pub, &founder_a, &founder_b).await;
    assert_eq!(root_of(&native), n0, "sibling blocks hold the native root");
    assert_eq!(root_of(&wasm), w0, "sibling blocks hold the wasm root");

    // every op family, in one deterministic sequence. EVERY acceptance here
    // depends on sibling reads resolving through the wasm runtime: the valset
    // standing gate (a validator origin, then a resident origin) and the
    // identity OfNode account derivation. `moves` says whether the op changes
    // committed state — root movement must agree on BOTH sides.
    let ops: Vec<(Vec<u8>, Option<&str>, bool)> = vec![
        // a VALIDATOR registers (valset + identity gates both pass).
        (a_pub.clone(), Some("orthory"), true),
        // a RESIDENT registers (the union read admits the second tier).
        (b_pub.clone(), Some("quack-2"), true),
        // declaratively idempotent: the same handle again is a no-op.
        (a_pub.clone(), Some("orthory"), false),
        // an atomic rename frees the old name in the same op.
        (a_pub.clone(), Some("renamed"), true),
        // None unregisters without touching identity.
        (b_pub.clone(), None, true),
        // the freed name is claimable by ANOTHER account.
        (b_pub.clone(), Some("orthory"), true),
    ];

    for (height, (who, handle, moves)) in ops.into_iter().enumerate() {
        let height = height as u64 + 4;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, Origin::External(who.clone())), set_handle(handle))
            .await
            .expect("native submit");
        wasm.submit_at(block(height, Origin::External(who)), set_handle(handle))
            .await
            .expect("wasm submit");

        // replies identical after every block (the whole read matrix).
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "replies diverge after block {height}"
        );
        // roots move in LOCKSTEP: a state-changing op moves both commit
        // boundaries, a no-op holds both...
        if moves {
            assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
            assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        } else {
            assert_eq!(root_of(&native), n_before, "native root moved at {height}");
            assert_eq!(root_of(&wasm), w_before, "wasm root moved at {height}");
        }
        // ...and the roots themselves always differ (the pinned schema break).
        assert_ne!(root_of(&native), root_of(&wasm));
    }

    // decoded spot check on the wasm side: resolution stops at the stable
    // AccountId (the founding key), never a node key.
    assert_eq!(
        resolved(&wasm, "renamed").await,
        Some(ResolvedAccount {
            account_id: pubkey(&founder_a),
        }),
        "A's rename resolves to A's account id"
    );
    assert_eq!(
        resolved(&wasm, "orthory").await,
        Some(ResolvedAccount {
            account_id: pubkey(&founder_b),
        }),
        "the freed handle now belongs to B's account"
    );
    assert_eq!(resolved(&wasm, "quack-2").await, None, "B's old name is gone");

    // error-shaped queries reject identically too (the wasm runtime wraps —
    // and Debug-escapes — the native reason, so the parity claim is needle
    // containment on both sides, not string equality).
    for (q, needle) in [
        // reserved root label: refused at lookup.
        (resolve_query("net"), "reserved"),
        (
            encode_query(&DuckDnsQuery::Registrations {
                from: 0,
                limit: MAX_QUERY_LIMIT + 1,
            }),
            "exceeds",
        ),
    ] {
        let n_err = native.query("duckdns", &q).await.expect_err("native query rejects");
        let w_err = wasm.query("duckdns", &q).await.expect_err("wasm query rejects");
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

    // queries are read-only on the wasm side too: the root is STABLE across
    // the whole read matrix.
    let settled = root_of(&wasm);
    let _ = replies(&wasm).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let (node_a, node_b, node_c) = (signer(1), signer(2), signer(3));
    let (founder_a, founder_b) = (signer(11), signer(12));
    let (a_pub, b_pub, c_pub) = (pubkey(&node_a), pubkey(&node_b), pubkey(&node_c));
    // a well-formed 32-byte node key with NO standing anywhere.
    let outsider = pubkey(&signer(99));
    let validators = vec![a_pub.clone()];

    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);

    for host in [&mut native, &mut wasm] {
        seed(host, &a_pub, &b_pub, &founder_a, &founder_b).await;
        // nodeC: resident standing but NO identity account — the seam between
        // the two gates.
        host.submit_at(block(4, Origin::System), grant(&c_pub))
            .await
            .expect("grant C");
        host.submit_at(block(5, Origin::External(a_pub.clone())), set_handle(Some("orthory")))
            .await
            .expect("register orthory");
    }

    // the rejection matrix: every distinct refusal family the native module
    // implements — BOTH sibling gates (valset standing, identity account),
    // both origin shapes, admission policy, handle grammar, ownership, and the
    // decode seam. each rejected block must leave BOTH roots byte-identical
    // (the abort path: staged writes discarded, no trace).
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        // valset-gated: a key with no standing — the STANDING read resolves
        // through the wasm runtime and rejects.
        (
            Origin::External(outsider),
            set_handle(Some("outsider")),
            "not a validator or admitted resident",
        ),
        // identity-gated: standing but unbound — the ACCOUNT read resolves
        // through the wasm runtime and rejects.
        (
            Origin::External(c_pub.clone()),
            set_handle(Some("unbound")),
            "not bound to an identity account",
        ),
        // admission policy: reserved root labels never admit.
        (
            Origin::External(a_pub.clone()),
            set_handle(Some("net")),
            "reserved",
        ),
        (
            Origin::External(a_pub.clone()),
            set_handle(Some("agents")),
            "reserved",
        ),
        // handle grammar.
        (
            Origin::External(a_pub.clone()),
            set_handle(Some("Bad_Handle")),
            "invalid characters",
        ),
        (
            Origin::External(a_pub.clone()),
            set_handle(Some("-lead")),
            "must not start or end with a hyphen",
        ),
        // ownership: one handle, one account.
        (
            Origin::External(b_pub.clone()),
            set_handle(Some("orthory")),
            "already claimed by another account",
        ),
        // origin shape: a non-32-byte external key.
        (
            Origin::External(vec![7; 16]),
            set_handle(Some("short")),
            "32-byte node key",
        ),
        // origin shape: no external submitter at all.
        (
            Origin::System,
            set_handle(Some("sys")),
            "requires an external node origin",
        ),
        // the decode seam (from a fully-authorized origin, so the decode is
        // what rejects).
        (
            Origin::External(a_pub.clone()),
            Msg {
                target: "duckdns".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (height, (origin, msg, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 6;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), msg)
            .await
            .expect_err("wasm must reject");

        // both reject DETERMINISTICALLY with the native module's reason. the
        // wasm runtime wraps the reason in its wit-error rendering, so the
        // parity claim is containment, not string equality.
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

        // abort leaves no trace: both roots byte-identical to pre-block.
        assert_eq!(root_of(&native), n_before, "native root moved on reject");
        assert_eq!(root_of(&wasm), w_before, "wasm root moved on reject");
        assert_eq!(replies(&native).await, replies(&wasm).await);
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let (node_a, node_b) = (signer(1), signer(2));
    let (founder_a, founder_b) = (signer(11), signer(12));
    let (a_pub, b_pub) = (pubkey(&node_a), pubkey(&node_b));
    let outsider = pubkey(&signer(99));
    let validators = vec![a_pub.clone()];

    let mut native = native_host(&validators);
    let mut wasm = wasm_host_(&validators);
    seed(&mut native, &a_pub, &b_pub, &founder_a, &founder_b).await;
    seed(&mut wasm, &a_pub, &b_pub, &founder_a, &founder_b).await;

    // ONE block, three ops: the second op's declarative rename READS the first
    // op's staged registration, and the third op claims the handle the rename
    // freed IN THIS BLOCK — on the wasm side that is the outer staged `__state`
    // being reloaded by each later dispatch (the read-your-writes seam the
    // adapter relies on), with the sibling gates resolving through memoized
    // replay on every one of the three dispatches.
    let batch = vec![
        (Origin::External(a_pub.clone()), set_handle(Some("first"))),
        (Origin::External(a_pub.clone()), set_handle(Some("second"))),
        (Origin::External(b_pub.clone()), set_handle(Some("first"))),
    ];
    let n_out = native
        .submit_block(block(4, Origin::External(a_pub.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(4, Origin::External(a_pub.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "all members must apply: {:?}",
            out.members
        );
    }
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        assert_eq!(
            resolved(host, "first").await,
            Some(ResolvedAccount {
                account_id: pubkey(&founder_b),
            }),
            "the third dispatch claimed the handle the staged rename freed"
        );
        assert_eq!(
            resolved(host, "second").await,
            Some(ResolvedAccount {
                account_id: pubkey(&founder_a),
            })
        );
    }

    // ONE block where the SECOND member rejects (a standing-gate refusal — the
    // sibling read itself is what rejects): the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal the
    // accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (Origin::External(a_pub.clone()), set_handle(Some("prime"))),
        (Origin::External(outsider), set_handle(Some("steal"))),
    ];
    let n_out = native
        .submit_block(block(5, Origin::External(a_pub.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(5, Origin::External(a_pub)), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    // the accepted member landed (roots moved), the rejected one left nothing.
    assert_ne!(root_of(&native), n_before);
    assert_ne!(root_of(&wasm), w_before);
    assert_eq!(replies(&native).await, replies(&wasm).await);
    for host in [&native, &wasm] {
        assert_eq!(
            resolved(host, "prime").await,
            Some(ResolvedAccount {
                account_id: pubkey(&founder_a),
            }),
            "the accepted member landed"
        );
        assert_eq!(
            resolved(host, "steal").await,
            None,
            "a rejected member must leave no trace"
        );
        assert_eq!(
            resolved(host, "first").await,
            Some(ResolvedAccount {
                account_id: pubkey(&founder_b),
            }),
            "B's registration survives the replay"
        );
    }
}
