//! Two-node directed interactive session over the peer mesh.
//!
//! A guest node directs its host peer to spawn a sandboxed interactive session
//! running a scripted child (`cat`, standing in for a provider so the test needs
//! no real API), forwards a keystroke line over the INPUT lane, and observes the
//! echoed bytes fan back onto the guest node's own `term:<id>` topic. Then the
//! guest closes the session and the host reaps the container.
//!
//! Every wait is on the system's own events — a committed-state query, a log
//! marker, or the next ws frame — never a fixed sleep.
//!
//! Requires a working Podman (the interactive plane exists only on a sandboxed node):
//! the test SKIPS loudly when podman is absent, exactly like the crate's other
//! live-podman integration tests. The credential/airlock swap is proven by the
//! airlock e2e; the echo provider declares no broker, so the seeded credential
//! here only exercises the host's admission gate, not a live upstream.
//!
//! The creator gate (a forwarded INPUT frame is written only from the creating
//! node) is not exercised here: forging an INPUT frame from a non-creator peer
//! requires speaking the overlay stream protocol directly, which no public node
//! surface exposes — a guest node only ever forwards input for a session IT
//! created. That gate is covered by the pure `input_permitted` /
//! `input_frame_is_accepted_only_from_the_creator_node` unit tests in
//! `term_plane.rs`.

mod common;

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use common::{Cluster, hex, poll_until, sandbox_toml, serial, skip_unless_sandboxed};
use commonware_cryptography::{Signer as _, ed25519};
use futures::{SinkExt as _, StreamExt as _};
use gateway::{
    CredentialKind, CredentialRecord, DuckDnsName, GatewayMsg, GatewayQuery, GatewayReply,
    MemberAuthorization, SetCredentialStatement, set_credential_preimage,
};
use identity::{AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth};
use serde_json::json;
use tokio::runtime::Runtime;
use tokio_tungstenite::tungstenite::Message;

const READY: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
/// the child echo must fan all the way back to the guest topic within this
/// bound (container cold-start + the mesh hop); a deadline, not a poll.
const ECHO: Duration = Duration::from_secs(120);

/// the image the scripted echo provider runs in: this suite's child is driven
/// through a pty, so it keeps the fuller `node` base rather than the harness
/// default.
const SANDBOX_IMAGE: &str = "docker.io/library/node:22-slim";

fn bind_auth(member: &ed25519::PrivateKey, chain: &str, node: &[u8]) -> MemberAuth {
    identity::testkit::ed_bind_auth(member, &identity::bind_preimage(chain, node, 0))
}

fn account_of_node(cluster: &Cluster, reader: usize, node: &[u8]) -> Option<AccountView> {
    let bytes = cluster.query(
        reader,
        "identity",
        &identity::encode_query(&IdentityQuery::OfNode {
            node_key: node.to_vec(),
        }),
    )?;
    match identity::decode_reply(&bytes).ok()? {
        IdentityReply::Account(account) => account,
        IdentityReply::Accounts(_) | IdentityReply::Clients(_) => None,
    }
}

fn resolve_handle(cluster: &Cluster, reader: usize, handle: &str) -> Option<Vec<u8>> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Resolve {
            name: DuckDnsName {
                handle: handle.into(),
            },
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Resolved(Some(account)) => Some(account.account_id),
        _ => None,
    }
}

fn credential_present(cluster: &Cluster, reader: usize, name: &str) -> Option<()> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Credential { name: name.into() }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Credential(Some(_)) => Some(()),
        _ => None,
    }
}

fn signed_set_credential(
    signer: &ed25519::PrivateKey,
    chain_id: &str,
    record: CredentialRecord,
) -> GatewayMsg {
    let statement = SetCredentialStatement {
        chain_id: chain_id.into(),
        record,
    };
    let preimage = set_credential_preimage(&statement).unwrap();
    let signature = signer
        .sign(gateway::GATEWAY_CREDENTIAL_NS, &preimage)
        .as_ref()
        .to_vec();
    GatewayMsg::SetCredential {
        statement,
        authorization: MemberAuthorization {
            signer: signer.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

/// an operator capability dir whose sole provider, `echo`, runs a bare `cat` on
/// the pty — a scripted child that echoes stdin, no real provider or credential
/// needed. Written to a tempdir handed to the HOST node via
/// `DUCKTAPE_CAPABILITY_DIR`.
fn echo_spec_dir() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("capability spec tempdir");
    std::fs::write(
        dir.path().join("echo.toml"),
        // detect.bin = cat (present on the host PATH, mounted into the sandbox);
        // an empty [interactive] argv launches it bare — `cat` copies its pty
        // stdin straight back to stdout.
        r#"spec = 1
[capability]
tag = "echo"
description = "scripted echo child (cat) for the remote-session e2e"
[detect]
bin = "cat"
[invoke]
prompt = "stdin"
[output]
format = "text"
[interactive]
args = []
"#,
    )
    .expect("write echo spec");
    dir
}

/// subscribe the guest ws to the session topic, forward `ping\n`, and return
/// whether the echoed bytes land back on that topic. Event-driven throughout:
/// it awaits the `subscribed` frame before sending input (the admission gate),
/// then awaits ws frames until the echo arrives — bounded only by an overall
/// deadline so a broken lane fails the test instead of hanging.
///
/// `secret` is the guest node's own 0600 service-link token: `term:` is a
/// workspace-gated topic, so without it the subscribe is refused and this
/// connection has nothing to send a keystroke on.
async fn drive_and_observe_echo(
    port: u16,
    session_id: &str,
    topic: &str,
    secret: &str,
) -> bool {
    let url = format!("ws://127.0.0.1:{port}/v1/ws");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .expect("guest ws connect");

    let subscribe =
        json!({ "op": "subscribe", "topics": [topic], "token": secret }).to_string();
    ws.send(Message::Text(subscribe)).await.expect("ws subscribe");

    // the input handler needs the ADMITTED handle registered, so wait for the
    // ack before forwarding a keystroke.
    loop {
        let frame = ws.next().await.expect("ws stays open").expect("ws frame");
        if let Message::Text(text) = frame {
            let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            if value["type"] == "subscribed" {
                break;
            }
        }
    }

    let data = STANDARD.encode(b"ping\n");
    let input = json!({ "op": "term_input", "session": session_id, "data": data }).to_string();
    ws.send(Message::Text(input)).await.expect("ws term input");

    while let Some(frame) = ws.next().await {
        let Ok(Message::Text(text)) = frame else {
            continue;
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        // a TermChunk rides `type:"event"` on the term topic with a base64 `item`
        // (and no `op`, unlike a module event).
        let is_our_chunk = value["topic"] == topic && value.get("item").is_some();
        if !is_our_chunk {
            continue;
        }
        let item = value["item"].as_str().unwrap_or_default();
        let bytes = STANDARD.decode(item).unwrap_or_default();
        if bytes.windows(4).any(|window| window == b"ping") {
            return true;
        }
    }
    false
}

/// The whole Phase 2 data + control path on two real nodes: directed create from
/// the guest to the host, a scripted child on the host's sandbox, a keystroke
/// forwarded over the INPUT lane, the echo fanned back to the guest's own term
/// topic, then close + host-side reap.
#[test]
fn guest_drives_a_scripted_child_on_the_host_over_the_forwarded_lane() {
    let _serial = serial();
    if skip_unless_sandboxed("guest_drives_a_scripted_child_on_the_host_over_the_forwarded_lane")
        .is_some()
    {
        return;
    }
    let rt = Runtime::new().unwrap();

    // the host's sole provider is the scripted `cat`; keep the dir alive for the
    // node's whole lifetime.
    let spec_dir = echo_spec_dir();

    // two real WireGuard nodes: guest (0) directs, host (1) sandboxes. Both run a
    // Podman terminal plane; only the host carries the echo provider.
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.wireguard = true;
    cluster.extra_toml = sandbox_toml(SANDBOX_IMAGE);
    cluster.env[1] = vec![(
        "DUCKTAPE_CAPABILITY_DIR".into(),
        spec_dir.path().display().to_string(),
    )];
    for index in 0..2 {
        cluster.spawn(index);
    }
    for index in 0..2 {
        cluster.wait_marker(index, "rpc listening on", READY);
        cluster.wait_marker(index, "converged root_hash=", READY);
        cluster.wait_marker(index, "peer handshake COMPLETE", READY);
        cluster.wait_marker(index, "term_session_plane_bound", READY);
    }

    let guest = ed25519::PrivateKey::from_seed(42);
    let guest_node = Cluster::identity(0);
    let host_node = Cluster::identity(1);

    // bind the guest node to the guest account. The HOST no longer derives a
    // creator account at all — whether the credential may be drawn on is the
    // lender's decision, made against the account the lender's node stamps on the
    // gateway hop, which is the HOST's. What this binding still buys is the
    // publisher→owner tie the gateway module needs for the record below, and the
    // `.duck` handle the host resolves as the airlock authority.
    //
    // So this test no longer proves an admission decision about the guest. What
    // it does prove is the half that survives: a peer-created session spawns,
    // streams, and is input-gated to its creator node.
    cluster.submit(
        0,
        "identity",
        &identity::encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(&guest, &cluster.namespace, &guest_node),
        }),
    );
    poll_until("guest identity binding", FINALIZE, || {
        account_of_node(&cluster, 1, &guest_node)
            .filter(|account| account.account_id == guest.public_key().as_ref())
    });

    // the guest registers a `.duck` handle — the owner-airlock authority the host
    // resolves for the credential (unused by the broker-less echo provider, but
    // the admission path resolves it).
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("guest".into()),
        }),
    );
    poll_until("guest handle resolves", FINALIZE, || {
        resolve_handle(&cluster, 1, "guest")
            .filter(|account| account.as_slice() == guest.public_key().as_ref())
    });

    // seed a credential the guest owns on committed gateway state — submitted via
    // the guest node so its origin is the guest node (the publisher the module
    // ties to the owner account).
    let record = CredentialRecord {
        name: "guest-fable-1".into(),
        owner_account: guest.public_key().as_ref().to_vec(),
        publisher_node: guest_node.clone(),
        kind: CredentialKind::Claude,
        seal_pk: [9; 32],
        grants: Default::default(),
    };
    let credential = signed_set_credential(&guest, &cluster.namespace, record);
    cluster.submit(0, "gateway", &gateway::encode_msg(&credential));
    poll_until("credential is committed", FINALIZE, || {
        credential_present(&cluster, 1, "guest-fable-1")
    });

    // the guest creates a session ON the host, naming its owned credential.
    let (status, body) = cluster.http(
        0,
        "POST",
        "/v1/term/sessions",
        Some(&json!({
            "agent": "echo",
            "mode": "single",
            "node": hex(&host_node),
            "cred": "guest-fable-1",
            "cpu": 1,
            "mem_gb": 1,
        })),
    );
    assert_eq!(status, 200, "directed remote create failed: {body}");
    let session_id = body["session_id"]
        .as_str()
        .unwrap_or_else(|| panic!("create reply carries a session id: {body}"))
        .to_string();
    let topic = body["topic"]
        .as_str()
        .expect("create reply carries a topic")
        .to_string();

    // the guest node's own 0600 service-link token, minted at boot beside its
    // node.toml. `term:` is workspace-gated on the ws surface, so reading this
    // file is what admits the attach below — the same proof the agent daemon
    // gives to take the interactive plane.
    let secret = noded::services::read_link_token(&cluster.workspace(0))
        .expect("the guest node minted its service-link token");

    // the guest forwards a keystroke over the INPUT lane; the host writes it to
    // the child's pty, the child echoes, and the output fans back to the guest's
    // OWN term topic.
    let echoed = rt.block_on(async {
        tokio::time::timeout(
            ECHO,
            drive_and_observe_echo(cluster.http_ports[0], &session_id, &topic, &secret),
        )
        .await
        .unwrap_or(false)
    });
    assert!(
        echoed,
        "the forwarded keystroke must echo back on the guest's term topic;\n{}",
        cluster.all_log_tails(60)
    );

    // close reaps the host session (and its container); the host logs the
    // session's end — an event-driven signal, not a timer.
    let (status, _body) = cluster.http(
        0,
        "POST",
        &format!("/v1/term/sessions/{session_id}/close"),
        None,
    );
    assert_eq!(status, 204, "close is a 204 no-op");
    let ended = cluster.wait_marker(1, "session_ended", READY);
    assert!(
        ended.contains(&session_id),
        "the host reaps the closed session {session_id}: {ended:?}"
    );
}

/// The terminal plane must exist on a PARKED joiner — the credential-lending
/// guest shape is a resident laptop directing a pty to a compute host, so the
/// plane cannot be validator-gated. Boots the product join flow (founder +
/// parked joiner, no promotion), then asserts the joiner's node wired the
/// plane: the boot marker fires, and a local create refuses for the RIGHT
/// reason (no sandbox configured) — never the plane-missing "terminal sessions
/// are not enabled" 503 that a joiner used to answer.
#[test]
fn a_parked_joiner_serves_the_terminal_plane() {
    let _serial = serial();
    let mut cluster = common::NetworkShapeCluster::new();
    let chain_id = cluster.init_founder("term-parked-joiner");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", READY);

    let invite = cluster.invite();
    let friend_key_hex = cluster.join_friend(&invite);
    assert_eq!(friend_key_hex.len(), 64, "join prints the friend's pubkey hex");
    cluster.spawn(1);
    // the regression: pre-fix, a joiner never wired the plane, so this marker
    // never appeared and every create answered the plane-missing 503.
    cluster.wait_marker(1, "terminal_plane_ready", READY);

    let (status, body) = common::http_request(
        cluster.http_ports[1],
        "POST",
        "/v1/term/sessions",
        Some(&json!({ "agent": "echo" })),
    );
    assert_eq!(status, 503, "no agent service is attached, so the create refuses: {body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .starts_with("interactive sessions require an agent service"),
        "the refusal must be the no-daemon one — the plane itself is present: {body}"
    );
}
