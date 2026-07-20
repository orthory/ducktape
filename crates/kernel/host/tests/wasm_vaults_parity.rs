//! the adapter-port equivalence proof for the vaults cutover: the `vaults-wasm`
//! component (the NATIVE `vaults` crate compiled to wasm behind `guest-adapter`)
//! and the native `Vaults` module answer the SAME op sequence with IDENTICAL
//! query replies, and their roots move in lockstep (move on commit, hold on
//! abort). the roots THEMSELVES differ — the port persists the native canonical
//! snapshot as one host-KV value, a declared state-schema break (revision 2) —
//! and this proof pins that difference so it can never be mistaken for
//! accidental compatibility.

use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Error, Msg, Origin, StateRoot};
use vaults::{
    encode_msg, encode_query, decode_reply, VaultMsg, VaultQuery, VaultReply, Vaults,
};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/vaults-wasm` by the module
/// build target; committed so this proof is self-contained.
const VAULTS_WASM: &[u8] = include_bytes!("fixtures/vaults.component.wasm");

fn wasm_vaults() -> WasmModule {
    WasmModule::from_bytes("vaults", VAULTS_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the vaults
        // canonical state — the same declaration bin/node makes.
        .with_state_schema_revision(2)
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

fn op(m: &VaultMsg) -> Msg {
    Msg {
        target: "vaults".into(),
        payload: encode_msg(m),
    }
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, who: &[u8]) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin: Origin::External(who.to_vec()),
        protocol_version: 0,
    }
}

/// the read matrix: every query family, including the `None` shapes.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&VaultQuery::Vaults),
        encode_query(&VaultQuery::Vault {
            vault_id: "infra".into(),
        }),
        encode_query(&VaultQuery::Vault {
            vault_id: "absent".into(),
        }),
        encode_query(&VaultQuery::Secret {
            vault_id: "infra".into(),
            name: "db-pass".into(),
        }),
        encode_query(&VaultQuery::Secret {
            vault_id: "infra".into(),
            name: "gone".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("vaults", q).await.expect("query"));
    }
    out
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("vaults").expect("vaults registered")
}

#[test]
fn same_ops_same_replies_roots_in_lockstep_schema_break_pinned() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

    // the schema break is visible from genesis: the native empty root is the
    // ZERO sentinel, the wasm root commits to the (empty) host-KV store.
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots must differ — the port is a DECLARED schema break"
    );

    // every op family, in one deterministic sequence (owner-gating exercised
    // through both the creator and a co-owner; a rotate bumps the version).
    let ops: Vec<(Vec<u8>, VaultMsg)> = vec![
        (
            alice.clone(),
            VaultMsg::CreateVault {
                vault_id: "infra".into(),
                name: "Infra".into(),
            },
        ),
        (
            alice.clone(),
            VaultMsg::AddOwner {
                vault_id: "infra".into(),
                key: bob.clone(),
            },
        ),
        (
            alice.clone(),
            VaultMsg::AddReader {
                vault_id: "infra".into(),
                key: carol.clone(),
            },
        ),
        (
            bob.clone(),
            VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "db-pass".into(),
                ciphertext: b"ct-v1".to_vec(),
            },
        ),
        (
            alice.clone(),
            VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "db-pass".into(),
                ciphertext: b"ct-v2-rotated".to_vec(),
            },
        ),
        (
            alice.clone(),
            VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "api-key".into(),
                ciphertext: b"ct-api".to_vec(),
            },
        ),
        (
            bob.clone(),
            VaultMsg::DeleteSecret {
                vault_id: "infra".into(),
                name: "api-key".into(),
            },
        ),
        (
            alice.clone(),
            VaultMsg::RemoveReader {
                vault_id: "infra".into(),
                key: carol.clone(),
            },
        ),
        (
            alice.clone(),
            VaultMsg::RemoveOwner {
                vault_id: "infra".into(),
                key: bob.clone(),
            },
        ),
    ];

    for (height, (who, msg)) in ops.into_iter().enumerate() {
        let height = height as u64 + 1;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        native
            .submit_at(block(height, &who), op(&msg))
            .await
            .expect("native submit");
        wasm.submit_at(block(height, &who), op(&msg))
            .await
            .expect("wasm submit");

        // replies identical after every block (the whole read matrix).
        assert_eq!(
            replies(&native).await,
            replies(&wasm).await,
            "replies diverge after block {height}"
        );
        // roots move in LOCKSTEP: every accepted vault op changes state, so
        // both commit boundaries must move their module root...
        assert_ne!(root_of(&native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(&wasm), w_before, "wasm root stuck at {height}");
        // ...to values that differ from each other (the pinned schema break).
        assert_ne!(root_of(&native), root_of(&wasm));
    }

    // decoded spot check: the rotate landed as version 2 with rotated bytes,
    // created_at pinned to the first put's block and updated_at to the rotate's.
    let reply = wasm
        .query(
            "vaults",
            &encode_query(&VaultQuery::Secret {
                vault_id: "infra".into(),
                name: "db-pass".into(),
            }),
        )
        .await
        .expect("secret query");
    let VaultReply::Secret(Some(secret)) = decode_reply(&reply).expect("decode") else {
        panic!("expected a stored secret");
    };
    assert_eq!(secret.version, 2);
    assert_eq!(secret.ciphertext, b"ct-v2-rotated");
    assert_eq!(secret.created_at, 1_004);
    assert_eq!(secret.updated_at, 1_005);

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
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, carol) = (key(0xA1), key(0xC3));

    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, &alice),
            op(&VaultMsg::CreateVault {
                vault_id: "infra".into(),
                name: "Infra".into(),
            }),
        )
        .await
        .expect("create");
    }

    // the rejection matrix: every distinct refusal family the native module
    // implements. each rejected block must leave BOTH roots byte-identical
    // (the abort path: staged writes discarded, no trace).
    let rejects: Vec<(Vec<u8>, VaultMsg, &str)> = vec![
        (
            carol.clone(),
            VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "stolen".into(),
                ciphertext: b"ct".to_vec(),
            },
            "not a vault owner",
        ),
        (
            alice.clone(),
            VaultMsg::RemoveOwner {
                vault_id: "infra".into(),
                key: alice.clone(),
            },
            "at least one owner",
        ),
        (
            alice.clone(),
            VaultMsg::CreateVault {
                vault_id: "infra".into(),
                name: "Duplicate".into(),
            },
            "already exists",
        ),
        (
            alice.clone(),
            VaultMsg::AddOwner {
                vault_id: "ghost".into(),
                key: carol.clone(),
            },
            "no such vault",
        ),
        (
            Vec::new(),
            VaultMsg::CreateVault {
                vault_id: "anon".into(),
                name: "Anon".into(),
            },
            "non-empty external submitter",
        ),
    ];

    for (height, (who, msg, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 2;
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));

        let n_err = native
            .submit_at(block(height, &who), op(&msg))
            .await
            .expect_err("native must reject");
        let w_err = wasm
            .submit_at(block(height, &who), op(&msg))
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
    let mut native = native_host();
    let mut wasm = wasm_host_();
    let (alice, carol) = (key(0xA1), key(0xC3));

    // ONE block, two ops: the second op's owner check READS the first op's
    // staged write (the vault only exists in this block's overlay). on the
    // wasm side that is the outer staged `__state` being reloaded by the
    // second dispatch — the read-your-writes seam the adapter relies on.
    let batch = vec![
        (
            Origin::External(alice.clone()),
            op(&VaultMsg::CreateVault {
                vault_id: "infra".into(),
                name: "Infra".into(),
            }),
        ),
        (
            Origin::External(alice.clone()),
            op(&VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "db-pass".into(),
                ciphertext: b"ct-v1".to_vec(),
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(1, &alice), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, &alice), batch)
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

    // ONE block where the SECOND member rejects: the runtime aborts the staged
    // overlay and replays the accepted member — committed state must equal the
    // accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (
            Origin::External(alice.clone()),
            op(&VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "api-key".into(),
                ciphertext: b"ct-api".to_vec(),
            }),
        ),
        (
            Origin::External(carol.clone()),
            op(&VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "stolen".into(),
                ciphertext: b"ct".to_vec(),
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(2, &alice), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, &alice), batch)
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
        let reply = host
            .query(
                "vaults",
                &encode_query(&VaultQuery::Secret {
                    vault_id: "infra".into(),
                    name: "stolen".into(),
                }),
            )
            .await
            .expect("query");
        assert_eq!(
            decode_reply(&reply).expect("decode"),
            VaultReply::Secret(None),
            "a rejected member must leave no trace"
        );
    }
}
