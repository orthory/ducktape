//! wiring-semantics tests for the inviter-side intro handler
//! (`reachability_plane::handle_intro`): install-before-ack ordering and the
//! no-install/no-ack refusal of junk. the intro codec/crypto itself is
//! covered by `lobby::tests` — not re-tested here.

use std::sync::{Arc, Mutex};

use commonware_cryptography::{Signer as _, ed25519};

use crate::config::mint_invite_token;
use crate::lobby;
use crate::reachability_plane::{IntroPath, handle_intro};

const BINDING: &[u8] = b"net#00000000@feedface";

fn src() -> std::net::SocketAddr {
    "127.0.0.1:4242".parse().unwrap()
}

/// a valid encoded intro plus the joiner identity and WireGuard key it binds.
fn intro_bytes() -> (Vec<u8>, ed25519::PublicKey, [u8; 32]) {
    let issuer = ed25519::PrivateKey::from_seed(1);
    let joiner = ed25519::PrivateKey::from_seed(2);
    let token = mint_invite_token(
        &issuer,
        BINDING,
        &joiner.public_key(),
        crate::config::InviteRole::Resident,
        u64::MAX,
    );
    let wg = [9u8; 32];
    let msg = lobby::intro_request(&joiner, BINDING, &token, wg);
    (lobby::encode_intro(&msg), joiner.public_key(), wg)
}

#[tokio::test]
async fn a_valid_intro_installs_before_acking() {
    let (bytes, joiner_pk, wg) = intro_bytes();
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(8);
    let weak = cmd_tx.downgrade();
    // one shared step log: the fake plane pushes "install" when the command
    // arrives, the ack closure pushes "ack" — order proves ack never outruns
    // the settled install.
    let log: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();

    let plane_log = log.clone();
    let plane = tokio::spawn(async move {
        match cmd_rx.recv().await.expect("install command sent") {
            reachability::ReachabilityCommand::InstallInvitePeer {
                peer,
                wireguard_public_key,
                endpoint,
                reply,
            } => {
                assert_eq!(peer, joiner_pk);
                assert_eq!(wireguard_public_key.0, wg);
                assert_eq!(endpoint, src());
                plane_log.lock().unwrap().push("install");
                reply.0.send(Ok(())).unwrap();
            }
            other => panic!("expected InstallInvitePeer, got {other:?}"),
        }
    });

    let ack_log = log.clone();
    let ack_store = acked.clone();
    let alive = handle_intro(
        &bytes,
        src(),
        BINDING,
        "test",
        IntroPath::Direct,
        &weak,
        |bytes| {
            ack_log.lock().unwrap().push("ack");
            ack_store.lock().unwrap().push(bytes);
            async {}
        },
    )
    .await;
    assert!(alive);
    plane.await.unwrap();

    assert_eq!(
        *log.lock().unwrap(),
        ["install", "ack"],
        "the install command lands (and settles) before the ack"
    );
    let acked = acked.lock().unwrap();
    let ack = lobby::decode_intro_ack(&acked[0]).expect("a decodable ack");
    assert!(ack.installed);
    assert_eq!(ack.detail, "tunnel installed");
}

#[tokio::test]
async fn junk_neither_installs_nor_acks() {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
    let weak = cmd_tx.downgrade();
    let acks = Arc::new(Mutex::new(0usize));
    let n = acks.clone();
    let alive = handle_intro(
        b"not an intro",
        src(),
        BINDING,
        "test",
        IntroPath::Direct,
        &weak,
        |_| {
            *n.lock().unwrap() += 1;
            async {}
        },
    )
    .await;
    assert!(alive, "junk keeps the loop running");
    assert!(cmd_rx.try_recv().is_err(), "no install for junk");
    assert_eq!(*acks.lock().unwrap(), 0, "no ack for junk");
}

#[tokio::test]
async fn a_failed_verification_acks_direct_and_stays_silent_coordinated() {
    // a well-formed intro minted for ANOTHER network: decodes, fails verify.
    let (bytes, _, _) = intro_bytes();
    for (path, acks_back) in [(IntroPath::Direct, true), (IntroPath::Coordinated, false)] {
        let (cmd_tx, mut cmd_rx) =
            tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
        let weak = cmd_tx.downgrade();
        let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
        let store = acked.clone();
        let alive = handle_intro(&bytes, src(), b"other-net", "test", path, &weak, |b| {
            store.lock().unwrap().push(b);
            async {}
        })
        .await;
        assert!(alive);
        assert!(cmd_rx.try_recv().is_err(), "no install on failed verify");
        let acked = acked.lock().unwrap();
        if acks_back {
            let ack = lobby::decode_intro_ack(&acked[0]).expect("direct path acks the refusal");
            assert!(!ack.installed);
        } else {
            assert!(acked.is_empty(), "the coordinated path stays silent");
        }
    }
}

#[tokio::test]
async fn an_expired_token_neither_installs_nor_tunnels() {
    // a cryptographically VALID intro whose token expired: verify passes (the
    // expiry is signature-covered, not signature-breaking), but the member's
    // wall-clock gate must refuse the tunnel — this is the enforcement point,
    // since consensus_time is block height and the joiner's decode check only
    // stops honest joiners.
    let issuer = ed25519::PrivateKey::from_seed(1);
    let joiner = ed25519::PrivateKey::from_seed(2);
    let token = mint_invite_token(
        &issuer,
        BINDING,
        &joiner.public_key(),
        crate::config::InviteRole::Resident,
        1, // 1970 — long expired
    );
    let bytes = lobby::encode_intro(&lobby::intro_request(&joiner, BINDING, &token, [9u8; 32]));

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
    let weak = cmd_tx.downgrade();
    let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let store = acked.clone();
    let alive = handle_intro(&bytes, src(), BINDING, "test", IntroPath::Direct, &weak, |b| {
        store.lock().unwrap().push(b);
        async {}
    })
    .await;
    assert!(alive);
    assert!(cmd_rx.try_recv().is_err(), "no tunnel install for an expired token");
    let acked = acked.lock().unwrap();
    let ack = lobby::decode_intro_ack(&acked[0]).expect("the refusal is acked");
    assert!(!ack.installed);
    assert!(ack.detail.contains("expired"), "{}", ack.detail);
}

#[tokio::test]
async fn a_client_role_intro_neither_installs_nor_tunnels() {
    // a cryptographically VALID intro whose token grants the `Client` role:
    // verify passes (role is signature-covered), but only `Resident` is
    // redeemable this generation, so the intro gate (ADR §3.1 V8) must refuse
    // the tunnel — a doomed join never obtains one.
    let issuer = ed25519::PrivateKey::from_seed(1);
    let joiner = ed25519::PrivateKey::from_seed(2);
    let token = mint_invite_token(
        &issuer,
        BINDING,
        &joiner.public_key(),
        crate::config::InviteRole::Client,
        u64::MAX,
    );
    let bytes = lobby::encode_intro(&lobby::intro_request(&joiner, BINDING, &token, [9u8; 32]));

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
    let weak = cmd_tx.downgrade();
    let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let store = acked.clone();
    let alive = handle_intro(&bytes, src(), BINDING, "test", IntroPath::Direct, &weak, |b| {
        store.lock().unwrap().push(b);
        async {}
    })
    .await;
    assert!(alive);
    assert!(
        cmd_rx.try_recv().is_err(),
        "no tunnel install for a non-Resident role"
    );
    let acked = acked.lock().unwrap();
    let ack = lobby::decode_intro_ack(&acked[0]).expect("the refusal is acked");
    assert!(!ack.installed);
    assert!(ack.detail.contains("not redeemable"), "{}", ack.detail);
}

#[tokio::test]
async fn a_dead_plane_channel_stops_the_loop() {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
    let weak = cmd_tx.downgrade();
    drop(cmd_tx);
    drop(cmd_rx);
    let (bytes, _, _) = intro_bytes();
    let alive = handle_intro(
        &bytes,
        src(),
        BINDING,
        "test",
        IntroPath::Direct,
        &weak,
        |_| async {},
    )
    .await;
    assert!(!alive, "a gone command channel tells the caller to exit");
}
