//! the adapter-port equivalence proof for vaults — the first
//! `snapshot_guest!` tenant: the `vaults` guest component (the NATIVE
//! `vaults` crate compiled to wasm behind `guest-adapter`) and the native
//! `Vaults` answer the SAME op sequence with IDENTICAL query replies, accept
//! and reject identically, and their roots move in lockstep. the roots
//! THEMSELVES differ (the runs/hello shape): the port persists the native
//! canonical snapshot as one host-KV value under the adapter's reserved
//! keys, so the wasm root commits to that map, not to the native preimage.
//! vaults has no committed state anywhere (it is in no genesis selection),
//! so there is nothing for a root to be continuous WITH — the wasm module's
//! own snapshot/install round-trip below is the restore/state-sync claim.
//!
//! vaults' every gate is keyed on `env().origin` (authenticated external
//! submitter, owner checks against it) and `env().consensus_time` (created/
//! updated stamps) — exactly the inputs that cross the WIT boundary — so the
//! rejection matrix pins each gate on both sides.

use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Error, Module as _, Msg, Origin, StateRoot, StateSyncHandle};
use vaults::{VaultMsg, VaultQuery, Vaults, encode_msg, encode_query};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `vaults` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const VAULTS_WASM: &[u8] = include_bytes!("fixtures/vaults.component.wasm");

/// the module caps one secret's ciphertext (a vault holds credentials, not
/// blobs). the native crate keeps the constant private; the matrix pins the
/// OBSERVABLE bound, so a native change to it fails here on both runtimes.
const MAX_CIPHERTEXT_LEN: usize = 64 * 1024;

fn wasm_vaults() -> WasmModule {
    WasmModule::from_bytes("vaults", VAULTS_WASM).expect("load component")
}

fn native_host() -> Host {
    Host::genesis(vec![Box::new(Vaults::new("vaults"))]).expect("genesis")
}

fn wasm_host_() -> Host {
    Host::genesis(vec![Box::new(wasm_vaults())]).expect("genesis")
}

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn external(tag: u8) -> Origin {
    Origin::External(key(tag))
}

fn op(m: &VaultMsg) -> Msg {
    Msg {
        target: "vaults".into(),
        payload: encode_msg(m),
    }
}

fn put(vault_id: &str, name: &str, ciphertext: &[u8]) -> Msg {
    op(&VaultMsg::PutSecret {
        vault_id: vault_id.into(),
        name: name.into(),
        ciphertext: ciphertext.to_vec(),
    })
}

/// one block's agreed context: both runtimes must see the identical env
/// (vaults stamps `consensus_time` into created/updated fields, so a env
/// divergence would surface as a root divergence).
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("vaults").expect("vaults registered")
}

/// the read matrix: the merged listing, one vault's view (and the None
/// shape), and a secret (and the None shape).
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = vec![
        encode_query(&VaultQuery::Vaults),
        encode_query(&VaultQuery::Vault {
            vault_id: "team".into(),
        }),
        encode_query(&VaultQuery::Vault {
            vault_id: "absent".into(),
        }),
        encode_query(&VaultQuery::Secret {
            vault_id: "team".into(),
            name: "api-key".into(),
        }),
        encode_query(&VaultQuery::Secret {
            vault_id: "team".into(),
            name: "absent".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in queries {
        out.push(h.query("vaults", &q).await.expect("vaults query"));
    }
    out
}

#[test]
fn same_ops_same_replies_roots_in_lockstep() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();

    // the genesis replies agree (both serve the empty vault map); the roots
    // differ by design from block zero (native: `StateRoot::ZERO` on empty,
    // wasm: the empty host-KV map's encoding).
    assert_eq!(replies(&native).await, replies(&wasm).await);

    // every op family, in one deterministic sequence — alice owns "team",
    // bob joins as owner, carol passes through as reader. every op below
    // changes committed state, so the roots must MOVE in lockstep too.
    let alice = external(0xA1);
    let bob = external(0xB2);
    let ops: Vec<(Origin, Msg)> = vec![
        (
            alice.clone(),
            op(&VaultMsg::CreateVault {
                vault_id: "team".into(),
                name: "Team Vault".into(),
            }),
        ),
        (
            alice.clone(),
            op(&VaultMsg::AddOwner {
                vault_id: "team".into(),
                key: key(0xB2),
            }),
        ),
        (alice.clone(), put("team", "api-key", b"ciphertext-v1")),
        // an overwrite bumps the version and keeps created_at.
        (bob.clone(), put("team", "api-key", b"ciphertext-v2")),
        (
            alice.clone(),
            op(&VaultMsg::AddReader {
                vault_id: "team".into(),
                key: key(0xC3),
            }),
        ),
        (
            bob.clone(),
            op(&VaultMsg::RemoveReader {
                vault_id: "team".into(),
                key: key(0xC3),
            }),
        ),
        (alice.clone(), put("team", "db-pass", b"other-secret")),
        (
            bob.clone(),
            op(&VaultMsg::DeleteSecret {
                vault_id: "team".into(),
                name: "db-pass".into(),
            }),
        ),
        (
            bob.clone(),
            op(&VaultMsg::RemoveOwner {
                vault_id: "team".into(),
                key: key(0xA1),
            }),
        ),
    ];
    for (height, (origin, msg)) in ops.into_iter().enumerate() {
        let height = height as u64 + 1;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect("native submit");
        wasm.submit_at(block(height, origin), msg)
            .await
            .expect("wasm submit");
        // roots move in LOCKSTEP (every op above changes committed state);
        // their values differ by design, so the equivalence claim is the
        // reply matrix.
        assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "vault replies diverge after block {height}"
        );
    }
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = external(0xA1);

    // seed one committed vault + secret so the owner gates are reachable.
    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, alice.clone()),
            op(&VaultMsg::CreateVault {
                vault_id: "team".into(),
                name: "Team Vault".into(),
            }),
        )
        .await
        .expect("seed create");
        host.submit_at(block(2, alice.clone()), put("team", "api-key", b"v1"))
            .await
            .expect("seed put");
    }

    // the rejection matrix: every distinct refusal family — the external-
    // origin gate (module/system/empty-key), the owner gate, membership
    // invariants, the ciphertext bounds, duplicate/absent shapes, and
    // undecodable bytes. each rejected block must leave BOTH roots
    // byte-identical (the abort path: staged writes discarded).
    let stranger = external(0xD4);
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        (
            Origin::Module("chat".into()),
            put("team", "api-key", b"x"),
            "external submitter",
        ),
        (
            Origin::System,
            put("team", "api-key", b"x"),
            "external submitter",
        ),
        (
            Origin::External(Vec::new()),
            put("team", "api-key", b"x"),
            "non-empty external submitter",
        ),
        (
            stranger.clone(),
            put("team", "api-key", b"x"),
            "not a vault owner",
        ),
        (
            alice.clone(),
            op(&VaultMsg::CreateVault {
                vault_id: "team".into(),
                name: "again".into(),
            }),
            "already exists",
        ),
        (
            alice.clone(),
            op(&VaultMsg::RemoveOwner {
                vault_id: "team".into(),
                key: key(0xA1),
            }),
            "at least one owner",
        ),
        (
            alice.clone(),
            op(&VaultMsg::RemoveReader {
                vault_id: "team".into(),
                key: key(0xA1),
            }),
            "remove ownership first",
        ),
        (
            alice.clone(),
            put("team", "empty", b""),
            "must not be empty",
        ),
        (
            alice.clone(),
            put("team", "huge", &vec![0u8; MAX_CIPHERTEXT_LEN + 1]),
            "ceiling",
        ),
        (
            alice.clone(),
            op(&VaultMsg::DeleteSecret {
                vault_id: "team".into(),
                name: "absent".into(),
            }),
            "no such secret",
        ),
        (
            alice.clone(),
            Msg {
                target: "vaults".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];
    for (i, (origin, msg, needle)) in rejects.into_iter().enumerate() {
        let height = i as u64 + 3;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        let n_err = native
            .submit_at(block(height, origin.clone()), msg.clone())
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, origin), msg)
            .await
            .expect_err("wasm must reject");
        // both reject DETERMINISTICALLY with the native module's reason (the
        // wasm side wraps it in its wit-error rendering — containment, not
        // string equality).
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
        assert_eq!(replies(&native).await, replies(&wasm).await);
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let alice = external(0xA1);

    // ONE block, two ops: the put only lands because the owner check reads the
    // vault the first op staged (it exists only in this block's overlay) — on
    // the wasm side that is the outer staged `__state` being reloaded by the
    // second dispatch.
    let batch = vec![
        (
            alice.clone(),
            op(&VaultMsg::CreateVault {
                vault_id: "hot".into(),
                name: "Hot".into(),
            }),
        ),
        (alice.clone(), put("hot", "s", b"v")),
    ];
    let n_out = native
        .submit_block(block(1, alice.clone()), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, alice.clone()), batch)
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
    assert_eq!(replies(&native).await, replies(&wasm).await);

    // ONE block where the SECOND member rejects (a stranger's put): the
    // runtime aborts the staged overlay and replays the accepted member —
    // committed state must equal the accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (alice.clone(), put("hot", "s2", b"v2")),
        (external(0xD4), put("hot", "s", b"stolen")),
    ];
    let n_out = native
        .submit_block(block(2, alice.clone()), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, alice), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    assert_ne!(root_of(&native), n_before, "the accepted member landed");
    assert_ne!(root_of(&wasm), w_before, "the accepted member landed");
    assert_eq!(replies(&native).await, replies(&wasm).await);
}

#[test]
fn wasm_snapshot_round_trips_into_a_fresh_wasm_module() {
    futures::executor::block_on(round_trip_inner());
}

fn snapshot_bytes(m: &dyn sdk::Module) -> Vec<u8> {
    match m.state_sync_handle().expect("handle") {
        StateSyncHandle::SnapshotBytes(b) => b,
        other => panic!("expected snapshot bytes, got {other:?}"),
    }
}

/// the restore/state-sync claim for this tenant: the wasm module's OWN
/// snapshot (the host-KV map encoding the manifest lane ships) installs into
/// a fresh wasm module at the same root and serves the same replies — the
/// exact path a rebooted or joining node takes for a snapshot-lane module.
async fn round_trip_inner() {
    let mut wasm = wasm_vaults();
    let mut ctx = sdk_testkit::TestCtx::with_env(sdk::Env {
        height: 1,
        consensus_time: 7_000,
        origin: external(0xA1),
        me: "vaults".into(),
    });
    for m in [
        op(&VaultMsg::CreateVault {
            vault_id: "team".into(),
            name: "Team Vault".into(),
        }),
        put("team", "api-key", b"ciphertext-v1"),
    ] {
        wasm.execute(&mut ctx, &m).await.expect("wasm execute");
    }
    wasm.commit_block().await.expect("wasm commit");
    let root = wasm.root();
    let snapshot = snapshot_bytes(&wasm);

    let mut fresh = wasm_vaults();
    fresh
        .install(&snapshot, root)
        .expect("wasm snapshot installs into a fresh wasm module");
    assert_eq!(fresh.root(), root, "installed root must match");
    let q = encode_query(&VaultQuery::Secret {
        vault_id: "team".into(),
        name: "api-key".into(),
    });
    assert_eq!(
        wasm.query(&q).await.expect("wasm query"),
        fresh.query(&q).await.expect("fresh query"),
        "replies diverge over the installed snapshot"
    );
}
