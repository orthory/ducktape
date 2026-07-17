//! wiring-semantics tests for the inviter-side intro handler
//! (`reachability_plane::handle_intro`): install-before-ack ordering and the
//! no-install/no-ack refusal of junk. the intro codec is covered by
//! `lobby::tests` and the sealed-envelope crypto by `reachability::seal::tests`
//! — so these inject an IDENTITY `open` (the doorbell's open is DI) and drive
//! the post-open verify/install/ack path with plaintext bundles.

/// the injected opener for these unit tests: the seal roundtrip is proven in
/// `reachability::seal`, so here the "sealed" bytes ARE the plaintext bundle.
fn open_identity(sealed: &[u8]) -> Result<Vec<u8>, String> {
    Ok(sealed.to_vec())
}

use std::sync::{Arc, Mutex};

use commonware_cryptography::{Signer as _, ed25519};

use crate::config::mint_invite_token;
use crate::lobby;
use crate::reachability_plane::{IntroPath, handle_intro};

const BINDING: &[u8] = b"net#00000000@feedface";

fn src() -> std::net::SocketAddr {
    "127.0.0.1:4242".parse().unwrap()
}

/// a real joiner-side WireGuard keypair: post-verify acks come back SEALED to
/// the intro's announced WG key, so the tests must hold its secret to read
/// them.
fn joiner_wg_keypair() -> reachability::WireGuardKeypair {
    let dir = tempfile::tempdir().unwrap();
    reachability::WireGuardKeypair::load_or_generate(&dir.path().join("wg.key"))
        .unwrap()
        .0
}

/// a valid encoded intro plus the joiner identity and WG keypair it binds.
fn intro_bytes() -> (Vec<u8>, ed25519::PublicKey, reachability::WireGuardKeypair) {
    let issuer = ed25519::PrivateKey::from_seed(1);
    let joiner = ed25519::PrivateKey::from_seed(2);
    let token = mint_invite_token(&issuer, BINDING, crate::config::InviteRole::Resident, u64::MAX);
    let wg = joiner_wg_keypair();
    let msg = lobby::intro_request(&joiner, BINDING, &token, wg.public_key().0);
    (lobby::encode_intro(&msg), joiner.public_key(), wg)
}

/// open a post-verify (SEALED) ack with the joiner's WG secret and decode it.
fn open_sealed_ack(wg: &reachability::WireGuardKeypair, sealed: &[u8]) -> lobby::IntroAck {
    let opened = wg.open_sealed(sealed).expect("sealed to the joiner's WG key");
    lobby::decode_intro_ack(&opened).expect("a decodable ack")
}

#[tokio::test]
async fn a_valid_intro_installs_before_acking() {
    let (bytes, joiner_pk, wg) = intro_bytes();
    let wg_pub = wg.public_key().0;
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
                assert_eq!(wireguard_public_key.0, wg_pub);
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
        open_identity,
        None,
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
    // the post-install ack is SEALED to the joiner's WG key — no gate hook
    // (`None`) means the reply is `Installed`.
    let ack = open_sealed_ack(&wg, &acked[0]);
    assert!(matches!(ack.reply, lobby::IntroReply::Installed));
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
        open_identity,
        None,
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
        let alive = handle_intro(&bytes, src(), b"other-net", "test", path, &weak, open_identity, None, |b| {
            store.lock().unwrap().push(b);
            async {}
        })
        .await;
        assert!(alive);
        assert!(cmd_rx.try_recv().is_err(), "no install on failed verify");
        let acked = acked.lock().unwrap();
        if acks_back {
            // a PRE-VERIFY refusal goes out in the CLEAR (no joiner key
            // trusted yet) — it decodes without any seal.
            let ack = lobby::decode_intro_ack(&acked[0]).expect("direct path acks the refusal");
            assert!(matches!(ack.reply, lobby::IntroReply::Refused { .. }));
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
        crate::config::InviteRole::Resident,
        1, // 1970 — long expired
    );
    let wg = joiner_wg_keypair();
    let bytes =
        lobby::encode_intro(&lobby::intro_request(&joiner, BINDING, &token, wg.public_key().0));

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
    let weak = cmd_tx.downgrade();
    let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let store = acked.clone();
    let alive = handle_intro(&bytes, src(), BINDING, "test", IntroPath::Direct, &weak, open_identity, None, |b| {
        store.lock().unwrap().push(b);
        async {}
    })
    .await;
    assert!(alive);
    assert!(cmd_rx.try_recv().is_err(), "no tunnel install for an expired token");
    let acked = acked.lock().unwrap();
    // the expiry gate runs POST-verify, so its refusal is sealed.
    let ack = open_sealed_ack(&wg, &acked[0]);
    let lobby::IntroReply::Refused { detail } = ack.reply else {
        panic!("expected Refused, got {:?}", ack.reply);
    };
    assert!(detail.contains("expired"), "{detail}");
}

#[tokio::test]
async fn a_client_role_intro_neither_installs_nor_tunnels() {
    // a cryptographically VALID intro whose token grants the `Client` role:
    // verify passes (role is signature-covered), but only `Resident` is
    // redeemable this generation, so the intro gate (ADR §3.1 V8) must refuse
    // the tunnel — a doomed join never obtains one.
    let issuer = ed25519::PrivateKey::from_seed(1);
    let joiner = ed25519::PrivateKey::from_seed(2);
    let token = mint_invite_token(&issuer, BINDING, crate::config::InviteRole::Client, u64::MAX);
    let wg = joiner_wg_keypair();
    let bytes =
        lobby::encode_intro(&lobby::intro_request(&joiner, BINDING, &token, wg.public_key().0));

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<reachability::ReachabilityCommand>(8);
    let weak = cmd_tx.downgrade();
    let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let store = acked.clone();
    let alive = handle_intro(&bytes, src(), BINDING, "test", IntroPath::Direct, &weak, open_identity, None, |b| {
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
    // the role gate runs POST-verify, so its refusal is sealed too.
    let ack = open_sealed_ack(&wg, &acked[0]);
    let lobby::IntroReply::Refused { detail } = ack.reply else {
        panic!("expected Refused, got {:?}", ack.reply);
    };
    assert!(detail.contains("not redeemable"), "{detail}");
}

#[test]
fn a_sealed_intro_hides_the_token_and_opens_only_for_the_member() {
    // item 5 mule test: the joiner seals its first-contact intro to the
    // member's WireGuard X25519 key. An on-path observer on café wifi sees only
    // opaque bytes; only the member holding the matching secret can open it, and
    // the opened bundle verifies end to end. A DIFFERENT member cannot open it.
    let dir = tempfile::tempdir().unwrap();
    let member =
        reachability::WireGuardKeypair::load_or_generate(&dir.path().join("member.key")).unwrap().0;
    let attacker =
        reachability::WireGuardKeypair::load_or_generate(&dir.path().join("attacker.key")).unwrap().0;

    let (plaintext, joiner_pk, wg) = intro_bytes();
    let wg_pub = wg.public_key().0;
    let sealed = reachability::seal(&member.public_key().0, &plaintext);

    // confidentiality: the bearer token never appears in the clear — the sealed
    // datagram does not decode as an intro, and a different key cannot open it.
    assert!(
        lobby::decode_intro(&sealed).is_err(),
        "the sealed datagram must not be a cleartext intro an observer can read"
    );
    assert!(
        attacker.open_sealed(&sealed).is_err(),
        "only the member the intro was sealed to may open it"
    );

    // the member opens it and the bundle verifies exactly as before.
    let opened = member.open_sealed(&sealed).expect("the member opens its own sealed intro");
    let msg = lobby::decode_intro(&opened).expect("decodes after open");
    let verified = lobby::verify_intro(&msg, BINDING).expect("verifies end to end");
    assert_eq!(verified.joiner, joiner_pk);
    assert_eq!(verified.wg_public_key, wg_pub);
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
        open_identity,
        None,
        |_| async {},
    )
    .await;
    assert!(!alive, "a gone command channel tells the caller to exit");
}

#[tokio::test]
async fn a_gated_intro_forwards_once_and_answers_settled_outcomes() {
    // the member-side gate seam (Join v2 §4): the FIRST verified intro
    // forwards a GateForward to the run loop and acks sealed `Installed`;
    // once the loop writes an outcome into the shared map, a retransmit is
    // answered with THAT (sealed), without forwarding again.
    let (bytes, joiner_pk, wg) = intro_bytes();
    let (fwd_tx, mut fwd_rx) = tokio::sync::mpsc::channel::<lobby::GateForward>(8);
    let outcomes: crate::reachability_plane::GateOutcomes = Default::default();
    let hook = crate::reachability_plane::GateHook {
        forward: fwd_tx,
        outcomes: outcomes.clone(),
    };

    // a fake plane that settles every install immediately.
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(8);
    let weak = cmd_tx.downgrade();
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            if let reachability::ReachabilityCommand::InstallInvitePeer { reply, .. } = cmd {
                let _ = reply.0.send(Ok(()));
            }
        }
    });

    let acked: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let ring = |acked: Arc<Mutex<Vec<Vec<u8>>>>| {
        let bytes = bytes.clone();
        let hook = hook.clone();
        let weak = weak.clone();
        async move {
            handle_intro(
                &bytes,
                src(),
                BINDING,
                "test",
                IntroPath::Direct,
                &weak,
                open_identity,
                Some(&hook),
                |b| {
                    acked.lock().unwrap().push(b);
                    async {}
                },
            )
            .await
        }
    };

    // round 1: forwarded + sealed Installed.
    assert!(ring(acked.clone()).await);
    let fwd = fwd_rx.try_recv().expect("the gate request is forwarded");
    assert_eq!(fwd.joiner, joiner_pk.as_ref().to_vec());
    let ack = open_sealed_ack(&wg, &acked.lock().unwrap()[0]);
    assert!(matches!(ack.reply, lobby::IntroReply::Installed));

    // the loop settles the gate: the retransmit reads the outcome — sealed —
    // and does NOT forward again.
    outcomes.lock().unwrap().insert(
        joiner_pk.as_ref().to_vec(),
        lobby::IntroReply::Admitted {
            height: 12,
            cap: None,
        },
    );
    assert!(ring(acked.clone()).await);
    assert!(
        fwd_rx.try_recv().is_err(),
        "a settled joiner is answered from the map, never re-forwarded"
    );
    let ack = open_sealed_ack(&wg, &acked.lock().unwrap()[1]);
    assert!(matches!(
        ack.reply,
        lobby::IntroReply::Admitted { height: 12, cap: None }
    ));
}
