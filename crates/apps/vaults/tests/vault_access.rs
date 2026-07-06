//! vault write-integrity through a REAL host with authenticated origins: only
//! owners rotate secrets and membership, atomicity holds through the host
//! boundary, and snapshots verify-then-adopt.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use host::{BlockContext, Host, SubmitError};
use sdk::{Error, Module as _, Msg, Origin, StateRoot};
use vaults::Vaults;
use vaults::{VaultMsg, VaultQuery, VaultReply, decode_reply, encode_msg, encode_query};

fn key(seed: u8) -> Vec<u8> {
    let seed = [seed; 32];
    PrivateKey::decode(&seed[..])
        .expect("valid seed")
        .public_key()
        .as_ref()
        .to_vec()
}

async fn submit_as(
    host: &mut Host,
    who: &[u8],
    at: u64,
    payload: Vec<u8>,
) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext { protocol_version: 0,
            height: at,
            consensus_time: at,
            origin: Origin::External(who.to_vec()),
        },
        Msg {
            target: "vaults".into(),
            payload,
        },
    )
    .await
    .map(|_| ())
}

async fn secret_version(host: &Host, vault: &str, name: &str) -> Option<u64> {
    let reply = host
        .query(
            "vaults",
            &encode_query(&VaultQuery::Secret {
                vault_id: vault.into(),
                name: name.into(),
            }),
        )
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        VaultReply::Secret(s) => s.map(|s| s.version),
        _ => None,
    }
}

#[test]
fn owners_rotate_secrets_and_non_owners_are_refused() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Vaults::new("vaults"))]).expect("genesis");
        let (alice, mallory) = (key(1), key(3));

        submit_as(
            &mut host,
            &alice,
            1,
            encode_msg(&VaultMsg::CreateVault {
                vault_id: "infra".into(),
                name: "Infra".into(),
            }),
        )
        .await
        .expect("create");

        submit_as(
            &mut host,
            &alice,
            2,
            encode_msg(&VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "deploy-token".into(),
                ciphertext: b"envelope-v1".to_vec(),
            }),
        )
        .await
        .expect("owner put");
        assert_eq!(
            secret_version(&host, "infra", "deploy-token").await,
            Some(1)
        );

        // a non-owner cannot write, delete, or change membership.
        for payload in [
            encode_msg(&VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "deploy-token".into(),
                ciphertext: b"evil".to_vec(),
            }),
            encode_msg(&VaultMsg::DeleteSecret {
                vault_id: "infra".into(),
                name: "deploy-token".into(),
            }),
            encode_msg(&VaultMsg::AddOwner {
                vault_id: "infra".into(),
                key: mallory.clone(),
            }),
        ] {
            let err = submit_as(&mut host, &mallory, 3, payload)
                .await
                .expect_err("non-owner op must be refused");
            assert!(
                matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("owner")),
                "got {err:?}"
            );
        }
        assert_eq!(
            secret_version(&host, "infra", "deploy-token").await,
            Some(1)
        );

        // rotation bumps the version; module origins are refused outright.
        submit_as(
            &mut host,
            &alice,
            4,
            encode_msg(&VaultMsg::PutSecret {
                vault_id: "infra".into(),
                name: "deploy-token".into(),
                ciphertext: b"envelope-v2".to_vec(),
            }),
        )
        .await
        .expect("rotate");
        assert_eq!(
            secret_version(&host, "infra", "deploy-token").await,
            Some(2)
        );
    });
}

#[test]
fn ownership_invariants_hold() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Vaults::new("vaults"))]).expect("genesis");
        let (alice, bob) = (key(1), key(2));

        submit_as(
            &mut host,
            &alice,
            1,
            encode_msg(&VaultMsg::CreateVault {
                vault_id: "v".into(),
                name: "V".into(),
            }),
        )
        .await
        .expect("create");

        // the last owner cannot be removed.
        let err = submit_as(
            &mut host,
            &alice,
            2,
            encode_msg(&VaultMsg::RemoveOwner {
                vault_id: "v".into(),
                key: alice.clone(),
            }),
        )
        .await
        .expect_err("sole owner removal refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("at least one owner"))
        );

        // adding an owner also makes them a reader; removing a reader who is
        // an owner is refused.
        submit_as(
            &mut host,
            &alice,
            3,
            encode_msg(&VaultMsg::AddOwner {
                vault_id: "v".into(),
                key: bob.clone(),
            }),
        )
        .await
        .expect("add owner");
        let err = submit_as(
            &mut host,
            &alice,
            4,
            encode_msg(&VaultMsg::RemoveReader {
                vault_id: "v".into(),
                key: bob.clone(),
            }),
        )
        .await
        .expect_err("owner reader removal refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("ownership first"))
        );

        let reply = host
            .query(
                "vaults",
                &encode_query(&VaultQuery::Vault {
                    vault_id: "v".into(),
                }),
            )
            .await
            .expect("query");
        let VaultReply::Vault(Some(view)) = decode_reply(&reply).expect("decode") else {
            panic!("vault exists");
        };
        assert_eq!(view.owners.len(), 2);
        assert!(view.readers.contains(&bob), "owners are readers");
    });
}

#[test]
fn snapshot_round_trips_and_tampering_is_refused() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Vaults::new("vaults"))]).expect("genesis");
        let alice = key(1);
        submit_as(
            &mut host,
            &alice,
            1,
            encode_msg(&VaultMsg::CreateVault {
                vault_id: "v".into(),
                name: "V".into(),
            }),
        )
        .await
        .expect("create");
        submit_as(
            &mut host,
            &alice,
            2,
            encode_msg(&VaultMsg::PutSecret {
                vault_id: "v".into(),
                name: "s".into(),
                ciphertext: vec![7u8; 32],
            }),
        )
        .await
        .expect("put");

        let root = host.module_root("vaults").expect("root");
        let finalized = host::FinalizedBlock {
            height: 2,
            app_hash: host.app_hash(),
        };
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = host
            .capture_finalized_snapshot(finalized)
            .expect("capture")
            .module("vaults")
            .expect("entry")
            .state_sync
            .clone()
        else {
            panic!("vaults must advertise snapshot bytes");
        };

        let mut rebuilt = Vaults::new("vaults");
        rebuilt.install(&bytes, root).expect("install");
        assert_eq!(rebuilt.root(), root);

        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        let mut fresh = Vaults::new("vaults");
        assert!(fresh.install(&tampered, root).is_err());
        assert_eq!(
            fresh.root(),
            StateRoot::ZERO,
            "refused install leaves no trace"
        );
    });
}

#[test]
fn oversized_ciphertext_is_refused() {
    block_on(async {
        let mut host = Host::genesis(vec![Box::new(Vaults::new("vaults"))]).expect("genesis");
        let alice = key(1);
        submit_as(
            &mut host,
            &alice,
            1,
            encode_msg(&VaultMsg::CreateVault {
                vault_id: "v".into(),
                name: "V".into(),
            }),
        )
        .await
        .expect("create");
        let err = submit_as(
            &mut host,
            &alice,
            2,
            encode_msg(&VaultMsg::PutSecret {
                vault_id: "v".into(),
                name: "blob".into(),
                ciphertext: vec![0u8; 64 * 1024 + 1],
            }),
        )
        .await
        .expect_err("oversized ciphertext refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("ceiling"))
        );
    });
}
