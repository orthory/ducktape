//! invite redemption end-to-end through a REAL host: minting is the admission
//! decision, so a `GovMsg::Redeem` carrying a member-minted token plus the
//! joiner's proof-of-possession grants standing with no ballot — and the
//! redeemed-nonce set makes every token single-use.
//!
//! the join protocol: EVERY invite is bearer (무기명). There is no target lock;
//! whoever presents a valid join proof for the nonce first wins the grant, and
//! single-use bounds it. The role (`Resident`/`Client`) selects which standing
//! plane the grant lands in.
//!
//! ops are driven through `Host::submit_at` with `Origin::External(...)`,
//! exactly the shape the ordered lane hands the host after VERIFYING a frame
//! signature — so what these tests pin is the authorization model the live
//! network runs.

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;
use governance::invite::{
    INVITE_GRANT_NAMESPACE, INVITE_NONCE_LEN, InviteRole, InviteToken, sign_join_proof,
};
use governance::{
    GovMsg, GovQuery, GovReply, Governance, decode_reply as gov_decode, encode_msg as gov_encode,
    encode_query as gov_query,
};
use identity::Identity;
use identity::{
    IdentityQuery, IdentityReply, decode_reply as identity_decode, encode_query as identity_query,
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

/// re-state the invite-grant preimage here rather than reach into
/// `governance::invite`: a preimage drift in the crate then FAILS these tests
/// loudly instead of silently signing a stale shape. v2: `binding ‖ nonce ‖
/// role ‖ expiry` — no kind byte, no target.
fn grant_preimage_for_tests(binding: &[u8], nonce: &[u8], role: InviteRole, expires: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(binding);
    out.extend_from_slice(nonce);
    out.push(role.as_u8());
    out.extend_from_slice(&expires.to_le_bytes());
    out
}

/// mint a BEARER token as `issuer` with explicit role and expiry (fixed nonce —
/// tests need determinism). There is no target: any key that presents a valid
/// proof for this nonce may redeem.
fn mint(issuer: &PrivateKey, nonce_byte: u8, role: InviteRole, expires: u64) -> InviteToken {
    let nonce = [nonce_byte; INVITE_NONCE_LEN];
    let msg = grant_preimage_for_tests(BINDING, &nonce, role, expires);
    InviteToken {
        issuer: issuer.public_key(),
        nonce,
        role,
        expires_unix_secs: expires,
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
        role: token.role.as_u8(),
        expires_unix_secs: token.expires_unix_secs,
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
        Box::new(Identity::new("identity", None, "testnet".into())),
        Box::new(
            Governance::new("governance", "valset", "identity")
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
        other => panic!("expected Residents, got {other:?}"),
    }
}

async fn clients(host: &Host) -> Vec<Vec<u8>> {
    let reply = host
        .query("identity", &identity_query(&IdentityQuery::Clients))
        .await
        .expect("identity query");
    match identity_decode(&reply).expect("decode") {
        IdentityReply::Clients(v) => v,
        other => panic!("expected Clients, got {other:?}"),
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
        let token = mint(&member, 7, InviteRole::Resident, u64::MAX);

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
        let (member, joiner) = (keypair(1), keypair(9));
        let token = mint(&member, 7, InviteRole::Resident, u64::MAX);

        submit_as(
            &mut host,
            &key_bytes(&member),
            1,
            redeem_msg(&token, &joiner),
        )
        .await
        .expect("first redemption");

        // the joiner replaying its OWN nonce: a deterministic double-admit
        // reject. (for a same-key replay the resident-standing guard fires
        // before the nonce guard — both enforce single-use, so accept either.)
        let err = submit_as(
            &mut host,
            &key_bytes(&member),
            2,
            redeem_msg(&token, &joiner),
        )
        .await
        .expect_err("second redemption must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m))
                if m.contains("already redeemed") || m.contains("already holds resident standing")),
            "a replay must be refused as a double-admit, got {err:?}"
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
        let mut rebuilt = Governance::new("governance", "valset", "identity")
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
        let foreign = mint(&outsider, 3, InviteRole::Resident, u64::MAX);
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

        // the join proof binds the REDEEMING key: a frame that claims joiner
        // keypair(10) but carries a proof signed by keypair(9) fails
        // proof-of-possession — a bearer token is not a blank cheque, the
        // redeemer must hold the key it names.
        let claimed = keypair(10);
        let token = mint(&member, 4, InviteRole::Resident, u64::MAX);
        let proof = sign_join_proof(&joiner, BINDING, &token); // signed by 9, not 10
        let forged = gov_encode(&GovMsg::Redeem {
            issuer: token.issuer.as_ref().to_vec(),
            nonce: token.nonce.to_vec(),
            token_sig: token.sig.encode().as_ref().to_vec(),
            joiner: key_bytes(&claimed),
            proof: proof.encode().as_ref().to_vec(),
            role: token.role.as_u8(),
            expires_unix_secs: token.expires_unix_secs,
        });
        let err = submit_as(&mut host, &key_bytes(&member), 2, forged)
            .await
            .expect_err("mismatched proof must be refused");
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
                "identity",
            )),
        ])
        .expect("genesis");

        let (member, joiner) = (keypair(1), keypair(9));
        let token = mint(&member, 5, InviteRole::Resident, u64::MAX);
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

#[test]
fn a_bearer_resident_token_is_first_wins_single_use() {
    block_on(async {
        let mut host = gov_host();
        let issuer = keypair(1);
        // ONE bearer Resident token; two different keys race to redeem it.
        let token = mint(&issuer, 1, InviteRole::Resident, u64::MAX);

        // key A — any key at all, no target lock — wins resident standing.
        let a = keypair(50);
        submit_as(&mut host, &key_bytes(&a), 10, redeem_msg(&token, &a))
            .await
            .expect("A redeems the bearer resident invite");
        assert!(residents(&host).await.contains(&key_bytes(&a)), "A holds resident standing");

        // key B presents the SAME token with its OWN valid proof: the nonce is
        // spent — single-use first-wins is the whole bearer containment story.
        let b = keypair(51);
        let err = submit_as(&mut host, &key_bytes(&b), 11, redeem_msg(&token, &b))
            .await
            .expect_err("second redemption of a spent bearer token");
        assert!(format!("{err:?}").contains("already redeemed"), "{err:?}");
        assert!(!residents(&host).await.contains(&key_bytes(&b)), "B gained nothing");

        // expiry is deliberately NOT consensus-enforced: consensus_time is
        // block height on this chain (no deterministic wall clock exists
        // in-consensus), so a wall-clock-expired token still settles here.
        // enforcement lives at the joiner's decode and at every gating member's
        // wall clock before Redeem submission (the sealed intro doorbell);
        // single-use bounds any residual window. this pins the ABSENCE of the
        // check so nobody re-adds a vacuous height-vs-seconds comparison.
        let c = keypair(52);
        let stale = mint(&issuer, 2, InviteRole::Resident, 1_000);
        submit_as(&mut host, &key_bytes(&c), 1_000_000, redeem_msg(&stale, &c))
            .await
            .expect("an expired token is not rejected in-consensus");
        assert!(residents(&host).await.contains(&key_bytes(&c)));
    });
}

#[test]
fn a_client_role_token_grants_client_standing_not_residency() {
    block_on(async {
        let mut host = gov_host();
        let (member, client) = (keypair(1), keypair(9));
        let token = mint(&member, 3, InviteRole::Client, u64::MAX);

        submit_as(
            &mut host,
            &key_bytes(&client),
            10,
            redeem_msg(&token, &client),
        )
        .await
        .expect("client redeems");

        // client standing is granted — and it is CLIENT standing, a distinct
        // tier: the joiner is in the clients set and NOT in residents/validators.
        assert_eq!(
            clients(&host).await,
            vec![key_bytes(&client)],
            "the joiner holds client standing"
        );
        assert!(
            residents(&host).await.is_empty(),
            "a Client redeem grants NO resident standing — the tiers are distinct"
        );
        // the admission is still audited in the shared redemption set.
        let audit = redemptions(&host).await;
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].joiner, key_bytes(&client));
    });
}

#[test]
fn a_client_token_is_single_use() {
    block_on(async {
        let mut host = gov_host();
        let (member, client) = (keypair(1), keypair(9));
        let token = mint(&member, 4, InviteRole::Client, u64::MAX);

        submit_as(
            &mut host,
            &key_bytes(&client),
            10,
            redeem_msg(&token, &client),
        )
        .await
        .expect("first client redemption");

        // the same Client nonce cannot redeem twice — either the shared nonce
        // gate or the already-a-client dedup fires; both enforce single-use.
        let err = submit_as(
            &mut host,
            &key_bytes(&client),
            11,
            redeem_msg(&token, &client),
        )
        .await
        .expect_err("second client redemption must be refused");
        assert!(
            matches!(err, SubmitError::Rejected(Error::Module(ref m))
                if m.contains("already redeemed") || m.contains("already holds client standing")),
            "got {err:?}"
        );
        assert_eq!(clients(&host).await, vec![key_bytes(&client)], "still one client");
    });
}

#[test]
fn the_join_proof_is_enforced_for_a_client_token_too() {
    block_on(async {
        let mut host = gov_host();
        let issuer = keypair(1);

        // claim keypair(10) as the joiner but sign the proof with keypair(8):
        // proof-of-possession fails for a Client token exactly as for Resident.
        let claimed = keypair(10);
        let thief = keypair(8);
        let token = mint(&issuer, 2, InviteRole::Client, u64::MAX);
        let proof = sign_join_proof(&thief, BINDING, &token); // signed by 8, not 10
        let forged = gov_encode(&GovMsg::Redeem {
            issuer: token.issuer.as_ref().to_vec(),
            nonce: token.nonce.to_vec(),
            token_sig: token.sig.encode().as_ref().to_vec(),
            joiner: key_bytes(&claimed),
            proof: proof.encode().as_ref().to_vec(),
            role: token.role.as_u8(),
            expires_unix_secs: token.expires_unix_secs,
        });
        let err = submit_as(&mut host, &key_bytes(&claimed), 11, forged)
            .await
            .expect_err("bad join proof");
        assert!(format!("{err:?}").contains("proof-of-possession"), "{err:?}");

        assert!(clients(&host).await.is_empty(), "nothing was granted");
    });
}
