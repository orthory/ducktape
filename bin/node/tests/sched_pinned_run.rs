//! `ducktape agent sched` end to end on REAL `ducktape` validators: a durable,
//! PINNED headless run is a bare `SagaMsg::Trigger` carrying a credential NAME in
//! its run envelope — no agent, no channel, no anchor. The executing node
//! resolves that name against COMMITTED state (the gateway credential record,
//! the run's saga origin, the submitter's account) and either runs it or refuses
//! it, before any provider spawns.
//!
//! Two proofs, each on its most reliable footing:
//!
//! - `a_granted_scheduled_run_executes_against_the_mock_upstream`: one credential
//!   node OWNS + SERVES a self-host airlock credential (testkit-minted SNP quote,
//!   verified by the REAL chain verifier under the test enclave's roots; a mock
//!   Anthropic upstream stands in). It submits a `sched` trigger pinned to
//!   ITSELF, drawing on its own credential — the owner is always granted. The
//!   run's Anthropic broker draws the resolved self-host airlock, swaps the
//!   session token for the sealed credential, and the mock upstream's `PONG`
//!   crosses back: it lands live on the `run-output:<id>` ring AND commits into
//!   the saga's own result record.
//!
//! - `an_ungranted_scheduled_run_is_refused_at_resolve`: node A submits a `sched`
//!   trigger PINNED to node B, naming B's credential. A's account was never
//!   granted, so B refuses at resolve — `credential_not_granted` lands in the
//!   saga's error and the provider NEVER spawns (its exec log stays empty). The
//!   refusal is the whole point of making the credential name a committed,
//!   origin-gated resolution rather than an envelope secret.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test sched_pinned_run -- --nocapture

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Cluster, poll_until, serial};
use commonware_cryptography::{Signer as _, ed25519};

use airlock::attest::{self, Measurement};
use airlock::client::Gateway as AirlockClient;
use airlock::server::{self, AttestMode, GatewayConfig};
use airlock::wire::{CredentialKind as WireCredentialKind, CredentialPayload};

use gateway::{
    CredentialKind, CredentialRecord, DuckDnsName, GATEWAY_CREDENTIAL_NS, GATEWAY_ROUTE_NS,
    GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization, RouteAudience, RouteDefinition,
    RouteMethod, RouteName, RoutePolicy, RouteStatement, RouteTarget, SetCredentialStatement,
    set_credential_preimage,
};
use identity::{AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth};
use saga::{SagaMsg, SagaQuery, SagaReply, SagaStatus, SagaView};

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use futures::{SinkExt as _, StreamExt as _};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio_tungstenite::tungstenite::Message;

const CONVERGE: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
/// budget for a full pinned run: provider spawn + broker session + one mock
/// round trip + result commit.
const ROUND_TRIP: Duration = Duration::from_secs(120);

/// the credential every leg names — a self-host Claude credential.
const CRED_NAME: &str = "owner-claude-1";
/// the capability tag the pinned run requires; the executing node stages a
/// script provider under it.
const TAG: &str = "sched-claude";

// ===========================================================================
// mock Anthropic upstream + testkit airlock gateway (mirrors airlock_gateway_e2e)
// ===========================================================================

fn measurement_hex() -> String {
    "11".repeat(attest::MRTD_LEN)
}

/// ONE test enclave (measures `0x11`x48). Its minted SNP chain is verified
/// through the REAL `airlock::verify` path, but only under its own roots.
fn test_enclave() -> &'static Arc<airlock::testkit::SnpTestEnclave> {
    static ENCLAVE: std::sync::OnceLock<Arc<airlock::testkit::SnpTestEnclave>> =
        std::sync::OnceLock::new();
    ENCLAVE.get_or_init(|| {
        let m = Measurement::from_hex(&measurement_hex()).unwrap();
        Arc::new(airlock::testkit::SnpTestEnclave::new(&m).unwrap())
    })
}

/// A mock Anthropic upstream: `/oauth/token` mints `acc-N`; `/v1/messages`
/// accepts ONLY `Bearer acc-N` (so a 200 proves the gateway swapped the scoped
/// session token for the sealed credential) and replies with the `PONG` marker.
/// `messages_hits` counts accepted `/v1/messages` calls — the host-side
/// exactly-once evidence (one provider execution = one upstream call), counted
/// where the sandbox boundary can't hide it.
#[derive(Default)]
struct MockUpstream {
    n: Mutex<u64>,
    messages_hits: Mutex<u64>,
}

async fn mock_oauth(State(st): State<Arc<MockUpstream>>) -> Json<serde_json::Value> {
    let mut n = st.n.lock().unwrap();
    *n += 1;
    Json(json!({
        "access_token": format!("acc-{n}"),
        "refresh_token": format!("ref-{n}"),
        "expires_in": 3600,
    }))
}

async fn mock_messages(
    State(st): State<Arc<MockUpstream>>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    let want = format!("Bearer acc-{}", *st.n.lock().unwrap());
    let got = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if got != want {
        return (axum::http::StatusCode::UNAUTHORIZED, "bad upstream bearer").into_response();
    }
    *st.messages_hits.lock().unwrap() += 1;
    ([("content-type", "text/event-stream")], "data: PONG\n\n").into_response()
}

async fn bind_and_serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Boot the mock upstream and the testkit airlock gateway pointed at it.
/// Returns `(gateway_base_url, gateway_loopback_port, upstream_counters)`.
async fn boot_gateway_and_upstream() -> (String, u16, Arc<MockUpstream>) {
    let counters = Arc::new(MockUpstream::default());
    let upstream = bind_and_serve(
        Router::new()
            .route("/oauth/token", post(mock_oauth))
            .route("/v1/messages", post(mock_messages))
            .with_state(counters.clone()),
    )
    .await;

    let (app, vendor) = server::build_with_quoter(
        GatewayConfig {
            attest: AttestMode::Tsm("snp".into()),
            seal_keypair: None,
            anthropic_base: upstream.clone(),
            openai_base: String::new(),
            oauth_token_url: format!("{upstream}/oauth/token"),
            oauth_client_id: "test-client".into(),
            session_ttl_secs: 3600,
            max_requests: 100,
        },
        "snp",
        test_enclave().quoter(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(vendor, "snp");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://127.0.0.1:{port}"), port, counters)
}

/// Verify the gateway quote and seal a credential under `name` (the mock mints
/// access tokens from this refresh seed). Returns the attested seal key — the
/// on-chain anchor the resolver pins.
async fn seal_credential(gw_base: &str, name: &str) -> [u8; 32] {
    let gw = AirlockClient::local(gw_base.to_string());
    let (quote, _vendor) = gw.fetch_quote().await.unwrap();
    let expected = Measurement::from_hex(&measurement_hex()).unwrap();
    let rd = airlock::verify::verify_quote(&quote, &expected, &test_enclave().roots())
        .await
        .unwrap();
    let seal_pk = attest::split_report_data(&rd).0;
    gw.upload_sealed_credential(
        &seal_pk,
        name,
        WireCredentialKind::Claude,
        &CredentialPayload::Refresh {
            refresh_token: "seed".into(),
            access_token: String::new(),
            expires_at: 0,
        },
    )
    .await
    .unwrap();
    seal_pk
}

// ===========================================================================
// identity + gateway helpers (mirror airlock_gateway_e2e / gateway_e2e)
// ===========================================================================

/// providers spawn only inside a sandbox now, so both legs need a working
/// podman; skip loudly without one, exactly like `remote_session`.
fn podman_available() -> bool {
    std::process::Command::new("podman")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// the `[sandbox]` table each cluster node boots with — appended LAST to the
/// generated toml (nothing may follow a toml table header).
fn sandbox_toml() -> Vec<String> {
    vec![
        "[sandbox]".into(),
        "runtime = \"podman\"".into(),
        "image = \"docker.io/library/node:22-slim\"".into(),
        "cores = 0".into(),
        "mem_gb = 0".into(),
    ]
}

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

/// Bind node `idx` (key `node`) to member `member`'s account and wait for it to
/// commit — the account the credential grant is checked against.
fn bind_node(cluster: &Cluster, idx: usize, member: &ed25519::PrivateKey, node: &[u8]) {
    cluster.submit(
        idx,
        "identity",
        &identity::encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(member, &cluster.namespace, node),
        }),
    );
    poll_until("identity binding", FINALIZE, || {
        account_of_node(cluster, idx, node)
            .filter(|account| account.account_id == member.public_key().as_ref())
    });
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

/// A member-signed `airlock` LoopbackHttp route (GET/HEAD/POST, 4 MiB cap,
/// `allow_authorization` so the session-token bearer reaches the enclave).
fn signed_airlock_route(
    member: &ed25519::PrivateKey,
    chain: &str,
    publisher: &[u8],
    revision: u64,
) -> GatewayMsg {
    let statement = RouteStatement {
        version: 1,
        chain_id: chain.into(),
        account_id: member.public_key().as_ref().to_vec(),
        name: RouteName::named("airlock"),
        publisher_node: publisher.to_vec(),
        revision,
        route: Some(RouteDefinition {
            target: RouteTarget::LoopbackHttp,
            policy: RoutePolicy {
                audience: RouteAudience::Network,
                methods: vec![RouteMethod::Get, RouteMethod::Head, RouteMethod::Post],
                max_request_bytes: 1024 * 1024,
                max_response_bytes: 4 * 1024 * 1024,
                allow_authorization: true,
                allow_upgrade: false,
            },
        }),
    };
    let signature = member
        .sign(
            GATEWAY_ROUTE_NS,
            &gateway::route_signing_preimage(&statement).unwrap(),
        )
        .as_ref()
        .to_vec();
    GatewayMsg::SetRoute {
        statement,
        authorization: MemberAuthorization {
            signer: member.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn airlock_route_revision(cluster: &Cluster, reader: usize, account: &[u8]) -> Option<u64> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Get {
            account_id: account.to_vec(),
            name: RouteName::named("airlock"),
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Route(record) => record.as_ref().as_ref().map(|r| r.statement.revision),
        _ => None,
    }
}

/// Owner-signed `SetCredential` registering `CRED_NAME` (empty grants — grants
/// ride separate `GrantCredential` ops).
fn set_credential(
    owner: &ed25519::PrivateKey,
    chain: &str,
    publisher_node: &[u8],
    seal_pk: [u8; 32],
) -> GatewayMsg {
    let statement = SetCredentialStatement {
        chain_id: chain.into(),
        record: CredentialRecord {
            name: CRED_NAME.into(),
            owner_account: owner.public_key().as_ref().to_vec(),
            publisher_node: publisher_node.to_vec(),
            kind: CredentialKind::Claude,
            seal_pk,
            grants: BTreeSet::new(),
        },
    };
    let signature = owner
        .sign(GATEWAY_CREDENTIAL_NS, &set_credential_preimage(&statement).unwrap())
        .as_ref()
        .to_vec();
    GatewayMsg::SetCredential {
        statement,
        authorization: MemberAuthorization {
            signer: owner.public_key().as_ref().to_vec(),
            signature,
        },
    }
}

fn credential_record(cluster: &Cluster, reader: usize) -> Option<CredentialRecord> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Credential {
            name: CRED_NAME.into(),
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Credential(record) => record,
        _ => None,
    }
}

// ===========================================================================
// the sched trigger + its committed result
// ===========================================================================

/// Submit a bare `SagaMsg::Trigger` from node `idx` (its key stamps the origin),
/// pinned to `target`, whose v3 envelope carries `CRED_NAME`. No demands — the
/// smallest reliable execution shape (the cpu/mem → Podman limit-flag
/// dimension is not exercised here).
fn submit_sched(cluster: &Cluster, idx: usize, saga_id: &str, target: &[u8], max_attempts: u32) {
    let spec = dispatch::encode_work_spec(&dispatch::WorkSpec {
        kind: dispatch::WORK_SPEC_KIND.into(),
        dispatch_id: saga_id.rsplit('\u{1f}').next().unwrap().into(),
        capability: TAG.into(),
        payload: compute_service::envelope::compose_headless(saga_id, "PING", Some(CRED_NAME))
            .into_bytes(),
        demands: BTreeMap::new(),
        admission: dispatch::AdmissionPolicy::Queue,
    });
    let trigger = SagaMsg::Trigger {
        saga_id: saga_id.into(),
        spec,
        reply_to: None,
        reply_payload: Vec::new(),
        deadline: None,
        max_attempts,
        lease_views: None,
        capability: Some(TAG.into()),
        demands: BTreeMap::new(),
        pinned_assignee: Some(target.to_vec()),
    };
    cluster.submit(idx, "saga", &saga::encode_msg(&trigger));
}

fn saga_view(cluster: &Cluster, reader: usize, saga_id: &str) -> Option<SagaView> {
    let bytes = cluster.query(
        reader,
        "saga",
        &saga::encode_query(&SagaQuery::Get {
            saga_id: saga_id.into(),
        }),
    )?;
    match saga::decode_reply(&bytes).ok()? {
        SagaReply::Saga(view) => view,
        _ => None,
    }
}

/// Poll committed state until the pinned saga reaches a terminal status.
fn wait_terminal(cluster: &Cluster, reader: usize, saga_id: &str, budget: Duration) -> SagaView {
    poll_until("the pinned saga to reach a terminal status", budget, || {
        saga_view(cluster, reader, saga_id).filter(|v| v.status.is_terminal())
    })
}

/// Read the buffered `run-output:<id>` ring off node `port`'s ws surface until a
/// line carries `marker`. Called AFTER the run committed, so the ring already
/// holds the line — the read is deterministic (event-driven on the marker, with
/// `budget` only as a failsafe), never a sleep.
///
/// `secret` is the node's own 0600 service-link token: `run-output:` carries
/// provider stdout, so it is a workspace-gated topic and an un-tokened subscribe
/// is refused.
async fn run_output_has(
    port: u16,
    id: &str,
    marker: &str,
    secret: &str,
    budget: Duration,
) -> bool {
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/v1/ws"))
        .await
        .expect("open run-output ws");
    socket
        .send(Message::Text(
            json!({
                "op": "subscribe",
                "topics": [format!("run-output:{id}")],
                "token": secret,
            })
            .to_string(),
        ))
        .await
        .expect("subscribe run-output");
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        let frame = match tokio::time::timeout(remaining, socket.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => text,
            Ok(Some(Ok(_))) => continue,
            _ => return false,
        };
        // `TailItem` is `#[serde(untagged)]`, so a run-output tail frame inlines
        // `{ "stream": …, "line": … }` directly under `item`.
        let carries_marker = serde_json::from_str::<serde_json::Value>(&frame)
            .ok()
            .and_then(|v| v["item"]["line"].as_str().map(str::to_string))
            .is_some_and(|line| line.contains(marker));
        if carries_marker {
            return true;
        }
    }
}

// ===========================================================================
// script provider staged on disk (mirrors dispatch_e2e's ScriptProvider)
// ===========================================================================

/// One script-backed provider for the `sched-claude` tag. `broker` picks the
/// isolation: the anthropic-messages broker (the executing node's credential
/// source rides `ANTHROPIC_BASE_URL` + a `claudeAiOauth` creds file the broker
/// seeds into `CLAUDE_CONFIG_DIR`) for the run leg, or
/// none for the refusal leg (which never reaches execution). the refusal leg's
/// script logs one host-path line per invocation — the never-spawned tripwire;
/// the run leg's execution count lives on the mock upstream (the host log path
/// does not exist inside the sandbox).
struct ScriptProvider {
    spec_dir: PathBuf,
    script: PathBuf,
    exec_log: PathBuf,
}

impl ScriptProvider {
    fn stage(root: &Path, broker: bool) -> Self {
        let dir = root.join("provider");
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        let exec_log = dir.join("exec.log");
        let script = dir.join("provider.sh");

        // the broker leg runs INSIDE the podman sandbox (node:22-slim), so it
        // dials the loopback broker with node's global fetch — the container
        // has no curl, and a host log path would not exist in its mount
        // namespace (execution is counted host-side by the mock upstream
        // instead). the refusal leg never runs, so its body only needs to be
        // valid; its host log line stays as the never-spawned tripwire.
        let body = if broker {
            "#!/bin/sh\n\
             set -e\n\
             cat > /dev/null\n\
             node -e '\n\
             const fs = require(\"fs\");\n\
             const creds = JSON.parse(fs.readFileSync(process.env.CLAUDE_CONFIG_DIR + \"/.credentials.json\", \"utf8\"));\n\
             const body = {model:\"claude-sonnet-5\",max_tokens:16,messages:[{role:\"user\",content:\"PING\"}]};\n\
             fetch(process.env.ANTHROPIC_BASE_URL + \"/v1/messages\", {\n\
               method: \"POST\",\n\
               headers: {\n\
                 authorization: \"Bearer \" + creds.claudeAiOauth.accessToken,\n\
                 \"content-type\": \"application/json\",\n\
                 \"anthropic-version\": \"2023-06-01\",\n\
                 \"anthropic-beta\": \"oauth-2025-04-20\",\n\
               },\n\
               body: JSON.stringify(body),\n\
             }).then((r) => r.text()).then((t) => { console.log(t); });\n\
             '\n"
                .to_string()
        } else {
            format!(
                "#!/bin/sh\n\
                 set -e\n\
                 cat > /dev/null\n\
                 echo ran >> {log}\n\
                 printf 'unreachable\\n'\n",
                log = exec_log.display(),
            )
        };
        std::fs::write(&script, body).expect("write provider script");
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&script).expect("script metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod provider script");

        let isolation = if broker {
            "[isolation]\nbroker = \"anthropic-messages\"\n"
        } else {
            ""
        };
        std::fs::write(
            spec_dir.join(format!("{TAG}.toml")),
            format!(
                "spec = 1\n\
                 [capability]\n\
                 tag = \"{TAG}\"\n\
                 description = \"sched e2e script executor\"\n\
                 [detect]\n\
                 bin = \"{TAG}-nonexistent-cli\"\n\
                 env = \"DUCKTAPE_TEST_SCHED_BIN\"\n\
                 [invoke]\n\
                 args = []\n\
                 prompt = \"stdin\"\n\
                 timeout_secs = 60\n\
                 [output]\n\
                 format = \"text\"\n\
                 {isolation}"
            ),
        )
        .expect("write provider spec");
        Self {
            spec_dir,
            script,
            exec_log,
        }
    }

    /// the env that makes a node provide the tag: the operator spec dir plus the
    /// detect override that points at the script.
    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "DUCKTAPE_CAPABILITY_DIR".into(),
                self.spec_dir.display().to_string(),
            ),
            (
                "DUCKTAPE_TEST_SCHED_BIN".into(),
                self.script.display().to_string(),
            ),
        ]
    }

    fn executions(&self) -> usize {
        std::fs::read_to_string(&self.exec_log)
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }
}

/// hide the embedded claude/codex executor specs so a dev box with a real
/// `claude`/`codex` on PATH runs identically to CI.
fn hide_builtins(root: &Path, name: &str) -> Vec<(String, String)> {
    let missing = root.join(name).join("missing-executor");
    vec![
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

// ===========================================================================
// the tests
// ===========================================================================

#[test]
fn a_granted_scheduled_run_executes_against_the_mock_upstream() {
    if !podman_available() {
        eprintln!("skipping a_granted_scheduled_run_executes_against_the_mock_upstream: no working podman");
        return;
    }
    let _serial = serial();
    let rt = Runtime::new().unwrap();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");

    // the credential node's loopback services live in THIS process; the node
    // subprocess reaches the gateway over host loopback, like a real deployment
    // (the provider container shares the host netns — no private netns here).
    let (gw_base, gw_port, upstream) = rt.block_on(boot_gateway_and_upstream());
    let seal_pk = rt.block_on(seal_credential(&gw_base, CRED_NAME));

    let provider = ScriptProvider::stage(fixtures.path(), true);
    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.wireguard = true;
    cluster.extra_toml = sandbox_toml();
    // the [sandbox] table is only HOW runs are isolated; the pool also
    // needs the user's compute grant. This run is pinned, not claimed from a
    // pool, so the grant announces nothing.
    cluster.compute_grant = Some(vec![]);
    cluster.env[0] = [provider.env(), hide_builtins(fixtures.path(), "node0")].concat();
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", CONVERGE);
    cluster.wait_marker(0, "converged root_hash=", CONVERGE);
    cluster.wait_marker(0, "gateway plane: overlay stream bound", CONVERGE);

    // the node owns the credential: bind its key to an account, map its handle,
    // register the gateway port, publish the airlock route, register the record.
    let owner = ed25519::PrivateKey::from_seed(42);
    let node_key = Cluster::identity(0);
    bind_node(&cluster, 0, &owner, &node_key);

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("owner".into()),
        }),
    );
    poll_until("owner.duck resolution", FINALIZE, || {
        resolve_handle(&cluster, 0, "owner").filter(|id| id == owner.public_key().as_ref())
    });

    let workspace = cluster.workspace(0);
    let (ok, output) = cluster.run_verb(&[
        "gateway",
        "bind",
        "--workspace",
        workspace.to_str().unwrap(),
        "--label",
        "airlock",
        "--port",
        &gw_port.to_string(),
    ]);
    assert!(ok, "airlock gateway port bind failed: {output}");

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_airlock_route(&owner, &cluster.namespace, &node_key, 1)),
    );
    poll_until("airlock route revision 1", FINALIZE, || {
        (airlock_route_revision(&cluster, 0, owner.public_key().as_ref()) == Some(1)).then_some(())
    });

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&set_credential(&owner, &cluster.namespace, &node_key, seal_pk)),
    );
    poll_until("the credential record to commit", FINALIZE, || {
        credential_record(&cluster, 0).filter(|r| r.seal_pk == seal_pk)
    });

    // the pinned run: bound to this node, drawing on its OWN credential (the
    // owner is always granted). the origin is this node's key.
    let dispatch_id = "sched-e2e-run";
    let saga_id = format!("sched\u{1f}{dispatch_id}");
    submit_sched(&cluster, 0, &saga_id, &node_key, 3);

    let view = wait_terminal(&cluster, 0, &saga_id, ROUND_TRIP);
    assert_eq!(
        view.status,
        SagaStatus::Done,
        "the granted pinned run committed a result (error: {:?})\n{}",
        view.error,
        cluster.all_log_tails(120),
    );
    let result = view.result.expect("a Done saga carries its result bytes");
    assert!(
        String::from_utf8_lossy(&result).contains("PONG"),
        "the mock upstream's reply committed into the saga result: {}",
        String::from_utf8_lossy(&result),
    );

    // the SAME output crossed the live run-output ring — the surface the app
    // tails. read it after commit, so the buffered line is deterministically
    // present.
    let secret = noded::services::read_link_token(&cluster.workspace(0))
        .expect("the node minted its service-link token");
    let saw = rt.block_on(run_output_has(
        cluster.http_ports[0],
        dispatch_id,
        "PONG",
        &secret,
        FINALIZE,
    ));
    assert!(saw, "the mock upstream's reply streamed to the run-output ring");

    // exactly-once, counted host-side where the sandbox boundary can't hide
    // it: one provider execution = one accepted upstream call.
    assert_eq!(
        *upstream.messages_hits.lock().unwrap(),
        1,
        "the provider ran exactly once"
    );
}

#[test]
fn an_ungranted_scheduled_run_is_refused_at_resolve() {
    if !podman_available() {
        eprintln!("skipping an_ungranted_scheduled_run_is_refused_at_resolve: no working podman");
        return;
    }
    let _serial = serial();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");

    // node 0 = submitter A (ungranted), node 1 = target B (owns the credential,
    // stages the provider so resolution reaches the grant gate). No wireguard /
    // airlock gateway: the refusal fires at the grant check, before any broker.
    let provider = ScriptProvider::stage(fixtures.path(), false);
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.extra_toml = sandbox_toml();
    // the [sandbox] table is only HOW runs are isolated; the pool also
    // needs the user's compute grant. This run is pinned, not claimed from a
    // pool, so the grant announces nothing.
    cluster.compute_grant = Some(vec![]);
    cluster.env[0] = hide_builtins(fixtures.path(), "node0");
    cluster.env[1] = [provider.env(), hide_builtins(fixtures.path(), "node1")].concat();
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", CONVERGE);
    cluster.spawn(1);
    for i in 0..2 {
        cluster.wait_marker(i, "converged root_hash=", CONVERGE);
    }

    // B owns the credential (bound to seed 42); A is bound to a DIFFERENT account
    // (seed 43) that is never granted.
    let owner = ed25519::PrivateKey::from_seed(42);
    let stranger = ed25519::PrivateKey::from_seed(43);
    let target = Cluster::identity(1);
    let submitter = Cluster::identity(0);
    bind_node(&cluster, 1, &owner, &target);
    bind_node(&cluster, 0, &stranger, &submitter);

    cluster.submit(
        1,
        "gateway",
        &gateway::encode_msg(&set_credential(&owner, &cluster.namespace, &target, [7u8; 32])),
    );
    poll_until("the credential record to commit", FINALIZE, || {
        credential_record(&cluster, 0)
    });

    // A submits pinned to B, naming B's credential. A was never granted.
    let saga_id = "sched\u{1f}sched-e2e-refused";
    submit_sched(&cluster, 0, saga_id, &target, 1);

    // B refuses at resolve: the saga fails carrying the named refusal, and B's
    // provider never spawned.
    let view = wait_terminal(&cluster, 0, saga_id, ROUND_TRIP);
    assert_eq!(
        view.status,
        SagaStatus::Failed,
        "an ungranted run must fail, not run\n{}",
        cluster.all_log_tails(120),
    );
    assert!(
        view.error.as_deref().unwrap_or_default().contains("credential_not_granted"),
        "the saga carries the refusal token: {:?}",
        view.error,
    );
    assert_eq!(
        provider.executions(),
        0,
        "a refused credential never launches a provider",
    );
}
