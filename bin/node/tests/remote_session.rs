//! Two-node directed interactive session over the peer mesh.
//!
//! A guest node directs its host peer to spawn a sandboxed interactive session
//! running a scripted child (`cat`, standing in for a provider so the test needs
//! no real API), forwards a keystroke line over the INPUT lane, and observes the
//! echoed bytes fan back onto the guest node's own `term:<id>` topic. Then the
//! guest closes the session and the host tears the VM down.
//!
//! Every wait is on the system's own events — a committed-state query, a log
//! marker, or the next ws frame — never a fixed sleep.
//!
//! Requires a bootable microVM (the interactive plane exists only on a
//! sandboxed node): `/dev/kvm` open to this process and the guest images built.
//! The test SKIPS loudly when they are absent, exactly like the crate's other
//! live sandbox integration tests. The credential/airlock swap is proven by the
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
use common::{Cluster, create_account, hex, sandbox_toml, skip_unless_sandboxed, submit_frame};
use commonware_cryptography::{Signer as _, ed25519};
use futures::{SinkExt as _, StreamExt as _};
use gateway::{
    CredentialKind, CredentialRecord, DuckDnsName, GatewayMsg, GatewayQuery, GatewayReply,
    MemberAuthorization, SetCredentialStatement, set_credential_preimage,
};
use serde_json::json;
use tokio::runtime::Runtime;
use tokio_tungstenite::tungstenite::Message;

const READY: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
/// the child echo must fan all the way back to the guest topic within this
/// bound (container cold-start + the mesh hop); a deadline, not a poll.
const ECHO: Duration = Duration::from_secs(120);

fn resolve_handle(cluster: &Cluster, reader: usize, handle: &str) -> Option<u64> {
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

/// An operator capability dir whose sole provider, `echo`, runs a bare `cat` on
/// the pty — a scripted child that echoes stdin, no real provider or credential
/// needed. Its private executor directory carries the actual Linux binary
/// mounted at `/opt/duck/bin/cat` in the host's session microVM.
fn echo_fixture() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("echo provider tempdir");
    let specs = dir.path().join("specs");
    let executors = dir.path().join("executors");
    std::fs::create_dir_all(&specs).expect("echo spec directory");
    std::fs::create_dir_all(&executors).expect("echo executor directory");
    let source = workspace_config::executor_dir()
        .expect("the configured guest-compatible executor directory")
        .join("cat");
    std::fs::copy(&source, executors.join("cat")).unwrap_or_else(|error| {
        panic!(
            "install a guest-compatible Linux cat in {} before running remote_session: {error}",
            source.display()
        )
    });
    std::fs::write(
        specs.join("echo.toml"),
        // An empty interactive argv launches the installed cat bare; it copies
        // its pty stdin straight back to stdout.
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
async fn drive_and_observe_echo(port: u16, session_id: &str, topic: &str, secret: &str) -> bool {
    let url = format!("ws://127.0.0.1:{port}/v1/ws");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .expect("guest ws connect");

    let subscribe = json!({ "op": "subscribe", "topics": [topic], "token": secret }).to_string();
    ws.send(Message::Text(subscribe))
        .await
        .expect("ws subscribe");

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

/// Wait for the guest node's own `term:<id>` topic to report the session OVER.
///
/// This is the frame `agent pty` breaks its attach loop on, and the guest node
/// can only send it once ITS ring is flagged ended — which happens only if the
/// host forwarded the terminal grain across the mesh. A subscriber arriving
/// after the close drains the ring and then receives it, so this needs no
/// timing: it waits on the frame itself.
async fn await_term_ended(port: u16, topic: &str, secret: &str) -> bool {
    let url = format!("ws://127.0.0.1:{port}/v1/ws");
    let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .expect("guest ws connect");
    let subscribe = json!({ "op": "subscribe", "topics": [topic], "token": secret }).to_string();
    ws.send(Message::Text(subscribe))
        .await
        .expect("ws subscribe");

    while let Some(frame) = ws.next().await {
        let Ok(Message::Text(text)) = frame else {
            continue;
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let is_our_terminator = value["type"] == "term_ended" && value["topic"] == topic;
        if is_our_terminator {
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
    if skip_unless_sandboxed("guest_drives_a_scripted_child_on_the_host_over_the_forwarded_lane")
        .is_some()
    {
        return;
    }
    let rt = Runtime::new().unwrap();

    // the host's sole provider is the scripted `cat`; keep the dir alive for the
    // node's whole lifetime.
    let fixture = echo_fixture();
    let empty = fixture.path().join("empty");
    std::fs::create_dir_all(&empty).expect("empty guest provider directory");

    // two real WireGuard nodes: guest (0) directs, host (1) sandboxes. Both run
    // a terminal plane; only the host carries the echo provider.
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.wireguard = true;
    cluster.extra_toml = sandbox_toml();
    cluster.env[0] = vec![
        (
            "DUCKTAPE_CAPABILITY_DIR".into(),
            empty.display().to_string(),
        ),
        ("DUCKTAPE_EXECUTOR_DIR".into(), empty.display().to_string()),
    ];
    cluster.env[1] = vec![
        (
            "DUCKTAPE_CAPABILITY_DIR".into(),
            fixture.path().join("specs").display().to_string(),
        ),
        (
            "DUCKTAPE_EXECUTOR_DIR".into(),
            fixture.path().join("executors").display().to_string(),
        ),
    ];
    for index in 0..2 {
        cluster.spawn(index);
    }
    for index in 0..2 {
        cluster.wait_marker(index, "rpc listening on", READY);
        cluster.wait_marker(index, "converged root_hash=", READY);
        cluster.wait_marker(index, "peer handshake COMPLETE", READY);
        cluster.wait_marker(index, "term_session_plane_bound", READY);
    }

    // THE HOST NEEDS AN AGENT DAEMON, not just a `[sandbox]` table.
    //
    // `has_sandbox()` is `link().is_some()` — whether an agent service has
    // ATTACHED — so a node with a perfectly good sandbox config refuses every
    // create until one runs beside it. The refusal says "no configured sandbox
    // image", which is NOT the condition it tested; the sibling arm words the
    // same fact correctly ("no agent service attached"). That wording is what
    // made this look like a config problem.
    //
    // AFTER the readiness markers: the daemon reads the node's identity out of
    // the running node, so starting it beside a node that has not bound yet
    // dies with `could not read this node's identity`.
    cluster.spawn_service(1, "agent");

    let guest = ed25519::PrivateKey::from_seed(42);
    let guest_node = Cluster::identity(0);
    let host_node = Cluster::identity(1);

    // the guest USER founds an account through the guest node. The HOST derives
    // no creator account at all — whether the credential may be drawn on is the
    // lender's decision, made against the work a session points at. What this
    // account still buys is the owner the gateway module ties the record below
    // to, and the `.duck` handle the host resolves as the airlock authority.
    //
    // So this test proves no admission decision about the guest's ACCOUNT. What
    // it does prove is the half that survives: a peer-created session spawns,
    // streams, and is input-gated to its creator node.
    let guest_account = create_account(&cluster, 0, &guest, "guest");
    cluster.await_committed(1, "the guest account to reach the host", FINALIZE, || {
        common::account_of_key(&cluster, 1, guest.public_key().as_ref())
    });

    // the guest registers a `.duck` handle — the owner-airlock authority the host
    // resolves for the credential (unused by the broker-less echo provider, but
    // the admission path resolves it).
    submit_frame(
        &cluster,
        0,
        &guest,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("guest".into()),
        }),
    );
    cluster.await_committed(1, "guest handle resolves", FINALIZE, || {
        resolve_handle(&cluster, 1, "guest").filter(|account| *account == guest_account)
    });

    // seed a credential the guest's account owns on committed gateway state —
    // a user-signed frame, so its origin is a member of the owner account.
    let record = CredentialRecord {
        name: "guest-fable-1".into(),
        owner_account: guest_account,
        publisher_node: guest_node.clone(),
        kind: CredentialKind::Claude,
        seal_pk: [9; 32],
        grants: Default::default(),
    };
    let credential = signed_set_credential(&guest, &cluster.namespace, record);
    submit_frame(
        &cluster,
        0,
        &guest,
        "gateway",
        &gateway::encode_msg(&credential),
    );
    cluster.await_committed(1, "credential is committed", FINALIZE, || {
        credential_present(&cluster, 1, "guest-fable-1")
    });

    // THE HOST DECIDES WHOSE WORK IT RUNS — the other half of the handshake.
    //
    // The credential above is the GUEST saying what may be drawn on. This is
    // node 1 saying it will run node 0's work at all. Work admission landed in
    // "a host decides whose work it runs" (2026-07-26), which taught
    // `sched_pinned_run` to write this policy but not this suite — and this
    // suite could not notice, because it was skipped for want of the sandbox's
    // host prerequisites and libtest counts a skip as `ok`. Without it the host
    // answers 502 `work_not_admitted`.
    //
    // A directed create arrives over the MESH, vouched for as a peer NODE and
    // carrying no user proof — and a node is never an account, so no account
    // list admits it: only `anyone` does. Written as the FILE the operator (and
    // `ducktape node work admit anyone`) produces, not through the writer, and
    // re-read on every decision — so it lands with no restart.
    std::fs::write(
        cluster.workspace(1).join("work-admit.toml"),
        "admit = [\"anyone\"]\n",
    )
    .expect("write work-admit.toml");

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
    // `session_ended` is the AGENT DAEMON's line, not the node's: the plane
    // lives in the service process, and the node's `TermEnded` handler logs
    // nothing. Waiting on the node's log here would time out after a fully
    // successful close.
    let ended = cluster.wait_service_marker(1, "agent", "session_ended", READY);
    assert!(
        ended.contains(&session_id),
        "the host reaps the closed session {session_id}: {ended:?}"
    );

    // ...and the END has to reach the GUEST, which is a different claim: the
    // client that blocks on this topic runs here, not on the host. Without the
    // forwarded terminal grain the host reaps cleanly (asserted above) while
    // every cross-node `agent pty` stays attached to a dead session forever.
    let guest_saw_the_end = rt.block_on(async {
        tokio::time::timeout(
            ECHO,
            await_term_ended(cluster.http_ports[0], &topic, &secret),
        )
        .await
        .unwrap_or(false)
    });
    assert!(
        guest_saw_the_end,
        "the guest's term topic must report the session over;\n{}",
        cluster.all_log_tails(60)
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
    let mut cluster = common::NetworkShapeCluster::new();
    let chain_id = cluster.init_founder("term-parked-joiner");
    assert!(
        !chain_id.is_empty(),
        "init should print the founded chain id"
    );
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", READY);

    let invite = cluster.invite();
    let friend_key_hex = cluster.join_friend(&invite);
    assert_eq!(
        friend_key_hex.len(),
        64,
        "join prints the friend's pubkey hex"
    );
    cluster.spawn(1);
    // the regression: pre-fix, a joiner never wired the plane, so this marker
    // never appeared and every create answered the plane-missing 503.
    cluster.wait_marker(1, "terminal_plane_ready", READY);

    // creating a session MUTATES this node, so the request carries the node's
    // own operator credential — the harness spawned it, so it is its operator.
    let (status, body) = cluster.http(
        1,
        "POST",
        "/v1/term/sessions",
        Some(&json!({ "agent": "echo" })),
    );
    assert_eq!(
        status, 503,
        "no agent service is attached, so the create refuses: {body}"
    );
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .starts_with("interactive sessions require an agent service"),
        "the refusal must be the no-daemon one — the plane itself is present: {body}"
    );
}
