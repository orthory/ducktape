//! invite redemption end-to-end through a REAL host: minting is the admission
//! decision, so a `GovMsg::Redeem` carrying a member-minted token plus the
//! joiner's proof-of-possession grants full-node (observer) standing with no
//! ballot — and the redeemed-nonce set makes every token single-use.
//!
//! ops are driven through `Host::submit_at` with `Origin::External(...)`,
//! exactly the shape the ordered lane hands the host after VERIFYING a frame
//! signature — so what these tests pin is the authorization model the live
//! network runs.

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use governance::invite::{INVITE_GRANT_NAMESPACE, INVITE_NONCE_LEN, InviteToken, sign_join_proof};
use governance::{
    GovMsg, GovQuery, GovReply, Governance, decode_reply as gov_decode, encode_msg as gov_encode,
    encode_query as gov_query,
};
use host::{BlockContext, Host, SubmitError};
use sdk::{Error, Msg, Origin};
use valset::Valset;
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode, encode_query as valset_query,
};

const BINDING: &[u8] = b"testnet#00000000@feedface";

fn keypair(seed: u8) -> PrivateKey {
    let seed = [seed; 32];
    PrivateKey::decode(&seed[..]).expect("any 32 bytes is a valid seed")
}

fn key_bytes(k: &PrivateKey) -> Vec<u8> {
    k.public_key().as_ref().to_vec()
}

/// mint a token as `issuer` with a fixed nonce (tests need determinism, so no
/// OS randomness here — the nonce value is arbitrary).
fn mint(issuer: &PrivateKey, nonce_byte: u8) -> InviteToken {
    let nonce = [nonce_byte; INVITE_NONCE_LEN];
    let msg = [BINDING, nonce.as_slice()].concat();
    InviteToken {
        issuer: issuer.public_key(),
        nonce,
        sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
    }
}

fn redeem_msg(token: &InviteToken, joiner: &PrivateKey) -> Vec<u8> {
    let proof = sign_join_proof(joiner, BINDING, token);
    gov_encode(&GovMsg::Redeem {
        issuer: token.issuer.as_ref().to_vec(),
        nonce: token.nonce.to_vec(),
        token_sig: token.sig.encode().as_ref().to_vec(),
        joiner: key_bytes(joiner),
        proof: proof.encode().as_ref().to_vec(),
    })
}

/// a host with governance (invite-wired) gating a valset seeded with members
/// 1 and 2.
fn gov_host() -> Host {
    let mut valset = Valset::new("valset");
    valset.insert(key_bytes(&keypair(1)));
    valset.insert(key_bytes(&keypair(2)));
    Host::genesis(vec![
        Box::new(valset),
        Box::new(
            Governance::new("governance", "valset", "upgrade", "identity")
                .with_invite_binding(BINDING),
        ),
    ])
    .expect("genesis")
}

async fn submit_as(
    host: &mut Host,
    who: &[u8],
    at: u64,
    payload: Vec<u8>,
) -> Result<(), SubmitError> {
    host.submit_at(
        BlockContext {
            protocol_version: 0,
            height: at,
            consensus_time: at,
            origin: Origin::External(who.to_vec()),
        },
        Msg {
            target: "governance".into(),
            payload,
        },
    )
    .await
    .map(|_| ())
}

async fn residents(host: &Host) -> Vec<Vec<u8>> {
    let reply = host
        .query("valset", &valset_query(&ValsetQuery::Residents))
        .await
        .expect("valset query");
    match valset_decode(&reply).expect("decode") {
        ValsetReply::Residents(v) => v,
        other => panic!("expected Observers, got {other:?}"),
    }
}

async fn redemptions(host: &Host) -> Vec<governance::RedemptionView> {
    let reply = host
        .query("governance", &gov_query(&GovQuery::Redemptions))
        .await
        .expect("gov query");
    match gov_decode(&reply).expect("decode") {
        GovReply::Redemptions(r) => r,
        other => panic!("expected Redemptions, got {other:?}"),
    }
}

#[test]
fn a_valid_redemption_grants_full_node_standing_without_a_ballot() {
    block_on(async {
        let mut host = gov_host();
        let (member, joiner) = (keypair(1), keypair(9));
        let token = mint(&member, 7);

        submit_as(
            &mut host,
            &key_bytes(&member),
            1,
            redeem_msg(&token, &joiner),
        )
        .await
        .expect("redeem");

        assert_eq!(
            residents(&host).await,
            vec![key_bytes(&joiner)],
            "the joiner holds resident standing in the same block"
        );
        let audit = redemptions(&host).await;
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].joiner, key_bytes(&joiner));
        assert_eq!(audit[0].issuer, key_bytes(&member));
        assert_eq!(audit[0].height, 1);
    });
}

#[test]
fn a_token_is_single_use_and_survives_snapshot_round_trip() {
    block_on(async {
        let mut host = gov_host();
        let (member, joiner, second) = (keypair(1), keypair(9), keypair(10));
        let token = mint(&member, 7);

        submit_as(
            &mut host,
            &key_bytes(&member),
            1,
            redeem_msg(&token, &joiner),
        )
        .await
        .expect("first redemption");

        // the same token under a DIFFERENT key: single-use, deterministic reject.
        let err = submit_as(
            &mut host,
            &key_bytes(&member),
            2,
            redeem_msg(&token, &second),
        )
        .await
        .expect_err("second redemption must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("already redeemed")),
            "got {err:?}"
        );

        // the redeemed set is state: it rides the snapshot, and a rebuilt
        // instance reproduces the exact root.
        use sdk::Module as _;
        let expected_root = host.module_root("governance").expect("gov root");
        let sdk::StateSyncHandle::SnapshotBytes(bytes) = ({
            let finalized = host::FinalizedBlock {
                height: 2,
                app_hash: host.app_hash(),
            };
            host.capture_finalized_snapshot(finalized)
                .expect("capture")
                .module("governance")
                .expect("gov entry")
                .state_sync
                .clone()
        }) else {
            panic!("governance must advertise snapshot bytes");
        };
        let mut rebuilt = Governance::new("governance", "valset", "upgrade", "identity")
            .with_invite_binding(BINDING);
        rebuilt.install(&bytes, expected_root).expect("install");
        assert_eq!(rebuilt.root(), expected_root, "round-trip root");
    });
}

#[test]
fn forged_or_unauthorized_redemptions_are_refused() {
    block_on(async {
        let mut host = gov_host();
        let (member, outsider, joiner) = (keypair(1), keypair(8), keypair(9));

        // a token minted by a NON-member verifies cryptographically but fails
        // the membership check — an outsider cannot admit anyone.
        let foreign = mint(&outsider, 3);
        let err = submit_as(
            &mut host,
            &key_bytes(&member),
            1,
            redeem_msg(&foreign, &joiner),
        )
        .await
        .expect_err("non-member token must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("no longer part")),
            "got {err:?}"
        );

        // a substituted joiner key fails the proof-of-possession.
        let token = mint(&member, 4);
        let proof = sign_join_proof(&joiner, BINDING, &token);
        let forged = gov_encode(&GovMsg::Redeem {
            issuer: token.issuer.as_ref().to_vec(),
            nonce: token.nonce.to_vec(),
            token_sig: token.sig.encode().as_ref().to_vec(),
            joiner: key_bytes(&keypair(10)), // not the key that signed the proof
            proof: proof.encode().as_ref().to_vec(),
        });
        let err = submit_as(&mut host, &key_bytes(&member), 2, forged)
            .await
            .expect_err("substituted joiner must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("proof-of-possession")),
            "got {err:?}"
        );

        assert!(residents(&host).await.is_empty(), "nothing was admitted");
    });
}

#[test]
fn a_network_without_a_binding_refuses_redemption() {
    block_on(async {
        let mut valset = Valset::new("valset");
        valset.insert(key_bytes(&keypair(1)));
        let mut host = Host::genesis(vec![
            Box::new(valset),
            // no with_invite_binding — the dev-seed shape.
            Box::new(Governance::new(
                "governance",
                "valset",
                "upgrade",
                "identity",
            )),
        ])
        .expect("genesis");

        let (member, joiner) = (keypair(1), keypair(9));
        let token = mint(&member, 5);
        let err = submit_as(
            &mut host,
            &key_bytes(&member),
            1,
            redeem_msg(&token, &joiner),
        )
        .await
        .expect_err("no binding — refuse");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("not wired")),
            "got {err:?}"
        );
    });
}
