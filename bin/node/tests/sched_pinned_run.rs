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
//! - `a_delegated_run_draws_on_the_submitters_grant`: the delegated shape,
//!   against the REAL lender (`ducktape service run airlock`, the only gateway
//!   that carries a grant gate). Node 0 owns the credential and SUBMITS; node 1
//!   EXECUTES and dials node 0's airlock over the overlay. FOUR directions on
//!   ONE cluster, so exactly one thing differs between any two of them:
//!
//!   | # | shape | state | outcome |
//!   |---|---|---|---|
//!   | 0 | 0 submits, pinned to 1 | 1 admits nobody | `work_not_admitted` |
//!   | 1 | 1 submits, pinned to itself | nobody granted | `credential_not_granted` |
//!   | 2 | 0 submits, pinned to 1 | 1 admitted, **still ungranted** | `Done` + `PONG` |
//!   | 2b | direction 2's pointer, replayed once its saga is terminal | unchanged | **refused** |
//!   | 3 | 1 submits, pinned to itself | 1 granted | `Done` + `PONG` |
//!
//!   Direction 2 is the whole campaign: A submits, B executes, and the draw is
//!   on A's grant. B supplies only a POINTER — the run's committed saga id — and
//!   the lender resolves out of its own state that A submitted it and that B
//!   holds its lease. Direction 3 is the non-regression: an executor granted in
//!   its own right still draws in its own right.
//!
//!   Directions 0 and 1 are two consents in OPPOSITE directions and both must
//!   hold: node 1 decides whose work it runs, node 0 decides whose account may
//!   draw on its credential. Neither substitutes for the other.
//!
//! ## what a sandboxed run costs this suite in evidence
//!
//! Both legs boot each node's `ducktape service run compute` daemon and execute
//! providers INSIDE a container, so `[sandbox]` is mandatory and a host path is
//! no longer a shared surface. The granted leg counts executions on the MOCK
//! UPSTREAM, which is host-side and outside the sandbox; the refusal leg has
//! only committed state (see its closing assertions). A host that cannot
//! sandbox FAILS this suite unless `DUCKTAPE_ALLOW_MISSING_TOOLS=1` opts into
//! skipping — a captured "skipping" line is not a signal anyone sees.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test sched_pinned_run -- --nocapture

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{Cluster, poll_until, sandbox_toml, serial, skip_unless_sandboxed};
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

// The granted leg used to name its own container image: its provider script
// dials the loopback broker with node's global `fetch`, which the harness's
// busybox default did not have. Every node now boots the same shared guest
// rootfs, so that choice moved to where the image is built
// (ops/build-guest-rootfs.sh) — and the per-node cost that made it a trade is
// gone, since one read-only image serves every node instead of each compute
// daemon filling its own graph root at boot.

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
        IdentityReply::Accounts(_) => None,
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

/// Seed the owner's DISK store the way `ducktape user cred add` writes it, so the
/// real lender daemon serves it from its first session.
///
/// The in-process testkit gateway above wires no grant gate, so no grant can be
/// proven against it. `cred_lending` does use the real gated lender and does
/// exercise a grant — but with the borrower and the executor being the same node,
/// so what nothing exercised was the grant against an executor DISTINCT from the
/// submitter. That is this test, and it needs the real lender.
fn seed_claude_store(storage: &Path, name: &str, refresh: &str) {
    let dir = storage.join("airlock-creds").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kind"), "claude\n").unwrap();
    std::fs::write(
        dir.join(".credentials.json"),
        format!(r#"{{"claudeAiOauth":{{"refreshToken":"{refresh}"}}}}"#),
    )
    .unwrap();
}

/// The seal PUBLIC key the lender minted when it opened its store — the on-chain
/// anchor the executing node pins. Self-host has no quote.
/// Give a node's workspace a work-admission policy admitting `account`.
///
/// Written as the FILE, not through the writer: an integration test should pin
/// the on-disk contract the operator (and `ducktape node work admit`) produces,
/// not a second copy of the producer. The daemon re-reads it on every decision,
/// so this lands with no restart — which is itself part of what the test proves.
fn admit_work_from(workspace: &Path, account: &[u8]) {
    std::fs::write(
        workspace.join("work-admit.toml"),
        format!("admit = [\"{}\"]\n", common::hex(account)),
    )
    .expect("write work-admit.toml");
}

fn seal_pk_from_store(storage: &Path) -> [u8; 32] {
    let bytes = std::fs::read(storage.join("airlock-creds").join("seal.key")).expect("seal.key");
    let secret: [u8; 32] = bytes.as_slice().try_into().expect("32-byte seal secret");
    airlock::seal::SealKeypair::from_secret_bytes(secret).public_bytes()
}

/// Owner-signed grant of `CRED_NAME` to `grantee` — which, under this flow, is
/// the account of the node that will RUN the workload.
fn signed_grant(owner: &ed25519::PrivateKey, chain: &str, grantee: &[u8]) -> GatewayMsg {
    let statement = gateway::CredentialGrantStatement {
        chain_id: chain.into(),
        owner_account: owner.public_key().as_ref().to_vec(),
        name: CRED_NAME.into(),
        account: grantee.to_vec(),
    };
    let signature = owner
        .sign(
            GATEWAY_CREDENTIAL_NS,
            &gateway::grant_credential_preimage(&statement).unwrap(),
        )
        .as_ref()
        .to_vec();
    GatewayMsg::GrantCredential {
        statement,
        authorization: MemberAuthorization {
            signer: owner.public_key().as_ref().to_vec(),
            signature,
        },
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

/// saga's id space is namespaced per trigger origin, and `/v1/submit` re-signs
/// with the RECEIVING node's own key — so an id node `idx` can trigger lives
/// under that node's actor namespace, and no other member can create it.
fn node_sid(cluster: &Cluster, idx: usize, id: &str) -> String {
    let key = Cluster::identity(cluster.peer_ids[idx]);
    saga::namespaced_id(&sdk::Origin::External(key), id)
}

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
/// seeds into `CLAUDE_CONFIG_DIR`) for the run leg, or none for the refusal leg
/// (which never reaches execution).
///
/// The script runs INSIDE the run's container, so its `stdout` is the whole of
/// its observable behaviour: it can touch no host path (none exists in that
/// mount namespace) and nothing beyond what the image provides. Execution
/// counting therefore lives on the mock upstream, which is host-side.
struct ScriptProvider {
    spec_dir: PathBuf,
    script: PathBuf,
}

impl ScriptProvider {
    fn stage(root: &Path, broker: bool) -> Self {
        let dir = root.join("provider");
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        let script = dir.join("provider.sh");

        // the broker leg runs INSIDE the podman sandbox ([`BROKER_IMAGE`]), so
        // it dials the loopback broker with node's global fetch — the container
        // has no curl. execution is counted host-side by the mock upstream. the
        // refusal leg never runs, so its body only needs to be valid.
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
            "#!/bin/sh\n\
             set -e\n\
             cat > /dev/null\n\
             printf 'unreachable\\n'\n"
                .to_string()
        };
        std::fs::write(&script, body).expect("write provider script");
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&script).expect("script metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod provider script");

        // `config_home_env` is NOT optional decoration for a claude-broker spec:
        // the broker seeds the per-run bearer as a `claudeAiOauth` credentials
        // FILE (that shape is what puts Claude Code in subscription mode), and it
        // has nowhere to write one without a config home. Omitting it is how this
        // leg died — `claude broker run has no config home to seed credentials`,
        // raised mid-run, long after the spec loaded. The script reads exactly
        // this variable back.
        let isolation = if broker {
            "[isolation]\nbroker = \"anthropic-messages\"\nconfig_home_env = \"CLAUDE_CONFIG_DIR\"\n"
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
        Self { spec_dir, script }
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
    if skip_unless_sandboxed("a_granted_scheduled_run_executes_against_the_mock_upstream").is_some() {
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
    // the compute plane is a SEPARATE PROCESS: without this the suite has no eye
    // on it, and a daemon that died at boot leaves a cluster that looks
    // perfectly healthy (the node is) until the pinned lease burns every attempt
    // and reports `lease attempts exhausted` — a diagnosis pointing at
    // consensus, three minutes from the actual cause. Its own marker names it
    // immediately.
    //
    // What it covers, exactly: `PodmanService` + provider discovery. NOT the
    // image — nothing is pulled at boot, so an unpullable tag passes this marker
    // and fails the run ~20s later. A dead libpod socket never prints it and
    // burns the full budget. Neither is a skip: past `probe()` this suite has
    // declared the host capable, so an unusable sandbox is a failure.
    cluster.wait_compute_marker(0, "compute daemon serving", CONVERGE);

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
    // 64 ascii-hex, because that is what the product mints
    // (`agent_cli::fresh_dispatch_id`) and what the node's ws `run_output` gate
    // admits. A short hand-made id — which this fixture used to carry — is
    // dropped at the node as `malformed_run_id`, so the ring assertion below
    // could only ever have failed on a real run.
    let dispatch_id = "5c4ed0e2be5f0ab8f8dc5d0f4c2b1a9e7d3f60518c2a4b6d8e0f1a3c5e7b9d02";
    let saga_id = node_sid(&cluster, 0, &format!("sched\u{1f}{dispatch_id}"));
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
fn a_delegated_run_draws_on_the_submitters_grant() {
    if skip_unless_sandboxed("a_delegated_run_draws_on_the_submitters_grant").is_some() {
        return;
    }
    let _serial = serial();
    let rt = Runtime::new().unwrap();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");

    // Only the mock UPSTREAM lives in this process. The gateway is the real
    // lender daemon in the node's own process — `build_with_quoter` above wires
    // NO grant gate, so a grant cannot be proven against it, and that gap is
    // exactly why the compute side used to carry a grant check of its own.
    let upstream = rt.block_on(bind_and_serve(
        Router::new()
            .route("/oauth/token", post(mock_oauth))
            .route("/v1/messages", post(mock_messages))
            .with_state(Arc::new(MockUpstream::default())),
    ));

    // node 0 = OWNER: owns the credential, runs the lender, and SUBMITS.
    // node 1 = EXECUTOR: runs the compute daemon and makes the gateway hop.
    let provider = ScriptProvider::stage(fixtures.path(), true);
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.wireguard = true;
    cluster.extra_toml = sandbox_toml();
    cluster.compute_grant = Some(vec![]);
    let owner_storage = cluster.workspace(0);
    seed_claude_store(&owner_storage, CRED_NAME, "rt-delegated");
    cluster.env[0] = [
        hide_builtins(fixtures.path(), "node0"),
        vec![
            ("DUCKTAPE_AIRLOCK_ANTHROPIC_BASE".into(), upstream.clone()),
            (
                "DUCKTAPE_AIRLOCK_OAUTH_TOKEN_URL".into(),
                format!("{upstream}/oauth/token"),
            ),
        ],
    ]
    .concat();
    cluster.env[1] = [provider.env(), hide_builtins(fixtures.path(), "node1")].concat();

    for index in 0..2 {
        cluster.spawn(index);
    }
    for index in 0..2 {
        cluster.wait_marker(index, "rpc listening on", CONVERGE);
        cluster.wait_marker(index, "converged root_hash=", CONVERGE);
        cluster.wait_marker(index, "peer handshake COMPLETE", CONVERGE);
        cluster.wait_marker(index, "gateway plane: overlay stream bound", CONVERGE);
    }
    // the EXECUTOR's compute daemon is what runs the workload and dials the
    // lender; the owner's is incidental to this proof.
    cluster.wait_compute_marker(1, "compute daemon serving", CONVERGE);
    // the lender starts only once its node's http surface is up: it opens the
    // store (minting seal.key) and registers its loopback port as the route.
    cluster.spawn_service(0, "airlock");
    cluster.wait_service_marker(0, "airlock", "airlock daemon serving", CONVERGE);

    let owner = ed25519::PrivateKey::from_seed(42);
    let executor = ed25519::PrivateKey::from_seed(43);
    let owner_node = Cluster::identity(0);
    let executor_node = Cluster::identity(1);
    bind_node(&cluster, 0, &owner, &owner_node);
    bind_node(&cluster, 1, &executor, &executor_node);

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("owner".into()),
        }),
    );
    for reader in 0..2 {
        poll_until("owner.duck resolution", FINALIZE, || {
            resolve_handle(&cluster, reader, "owner").filter(|id| id == owner.public_key().as_ref())
        });
    }

    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_airlock_route(
            &owner,
            &cluster.namespace,
            &owner_node,
            1,
        )),
    );
    poll_until("airlock route revision 1", FINALIZE, || {
        (airlock_route_revision(&cluster, 1, owner.public_key().as_ref()) == Some(1)).then_some(())
    });

    // the record carries the STORE's seal_pk (self-host has no quote) and an
    // empty grant set — nobody may draw on it yet.
    let seal_pk = seal_pk_from_store(&owner_storage);
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&set_credential(&owner, &cluster.namespace, &owner_node, seal_pk)),
    );
    poll_until("the credential record to commit", FINALIZE, || {
        credential_record(&cluster, 1)
    });

    // ---- DIRECTION 0: the executor does not run this submitter's work ------
    //
    // TWO consents in opposite directions, and this is the first: before the
    // lender is ever dialled, node 1 decides whether it runs node 0's work at
    // all. Its default is owner-only and these are two accounts, so the run is
    // refused HERE — no container, no gateway hop, no session. Without this
    // step the credential lane below is not even reachable.
    //
    // They COMPOSE, and this direction is where that is visible: the CREDENTIAL
    // consent is already satisfied for this exact run shape — the owner submits
    // it and the saga module leases it to node 1, which is precisely what
    // direction 2 delegates on — and it makes no difference. One consent
    // satisfied is not the other consent granted.
    let unadmitted_id = &node_sid(&cluster, 0, "sched\u{1f}sched-delegated-unadmitted");
    submit_sched(&cluster, 0, unadmitted_id, &executor_node, 1);
    let view = wait_terminal(&cluster, 0, unadmitted_id, ROUND_TRIP);
    assert_eq!(
        view.status,
        SagaStatus::Failed,
        "a node that does not admit the submitter's account runs nothing for it\n{}",
        cluster.all_log_tails(120),
    );
    assert!(
        view.error
            .as_deref()
            .unwrap_or_default()
            .contains("work_not_admitted"),
        "the EXECUTOR's own admission refuses first, and says so: {:?}",
        view.error,
    );

    // ---- the admission, on the EXECUTOR, for the SUBMITTER's account -------
    //
    // The opposite direction from the grant below: this is node 1 saying whose
    // work it will run, not node 0 saying who may draw on its credential. No
    // restart — the policy is re-read on every decision.
    admit_work_from(&cluster.workspace(1), owner.public_key().as_ref());

    // ---- DIRECTION 1: nobody is granted ------------------------------------
    //
    // Node 1 submits this one and pins it to ITSELF, so there is no delegation
    // to be had: the committed origin, the assignee and the account the lender's
    // node stamps on the hop are all node 1, and node 1 is on no grant list. It
    // also takes the admission's `ThisNode` path, which isolates the CREDENTIAL
    // gate from the work gate direction 0 just proved.
    let refused_id = &node_sid(&cluster, 1, "sched\u{1f}sched-delegated-ungranted");
    submit_sched(&cluster, 1, refused_id, &executor_node, 1);
    let view = wait_terminal(&cluster, 1, refused_id, ROUND_TRIP);
    assert_eq!(
        view.status,
        SagaStatus::Failed,
        "an executor granted nothing, drawing for nobody but itself, must fail\n{}",
        cluster.all_log_tails(120),
    );
    assert!(
        view.error
            .as_deref()
            .unwrap_or_default()
            .contains("credential_not_granted"),
        "the lender's own refusal token reaches the saga: {:?}",
        view.error,
    );

    // ---- DIRECTION 2: DELEGATION, and the grant list is untouched -----------
    //
    // THE POINT OF THE CAMPAIGN. Node 1 is still granted nothing — direction 1
    // just proved it, and no grant op is submitted until below. What changes is
    // WHO SUBMITS: the owner does, so the run's committed origin is the owner's
    // node key (proven by the signature `/v1/submit` re-stamped on the op), and
    // the saga module leased the attempt to node 1.
    //
    // Node 1's broker sends the saga id and nothing else. The lender reads both
    // facts out of its own committed state — the owner submitted it, node 1
    // holds its lease — and authorizes the draw on the OWNER's grant. A run
    // submitted by A and executed on B, drawing as A: the thing that has never
    // worked before this PR.
    let delegated_id = &node_sid(&cluster, 0, "sched\u{1f}sched-delegated-pointer");
    submit_sched(&cluster, 0, delegated_id, &executor_node, 1);
    let view = wait_terminal(&cluster, 0, delegated_id, ROUND_TRIP);
    assert_eq!(
        view.status,
        SagaStatus::Done,
        "an ungranted executor draws on the SUBMITTER's grant for work it holds: {:?}\n{}",
        view.error,
        cluster.all_log_tails(120),
    );
    assert!(
        String::from_utf8_lossy(&view.result.unwrap_or_default()).contains("PONG"),
        "and the lent credential's reply crosses back into the saga result",
    );
    assert!(
        !gateway::credential_use_allowed(
            &credential_record(&cluster, 1).expect("the record is still committed"),
            executor.public_key().as_ref(),
        ),
        "and it did so with the executor on NO grant list — otherwise this \
         direction proves nothing the next one does not",
    );

    // ---- DIRECTION 2b: and the pointer DIES with the run -------------------
    //
    // The same saga id, the same executor, the same lender, one moment later —
    // replayed by hand through node 1's browser gateway, exactly the path its own
    // broker took to reach the lender above. The saga module clears neither
    // `assignee` nor `pinned_assignee` on any terminal path, so without a
    // liveness condition this one completed run is a permanent, unmetered
    // licence: node 1 re-POSTs this body forever and mints a fresh token each
    // time (the session budget is keyed on the CREDENTIAL and refilled on every
    // open, so it caps nothing). The owner would have nothing to revoke — node 1
    // holds no grant, so `user cred revoke` has no subject.
    let (status, browser) = cluster.http(1, "GET", "/v1/gateway/browser", None);
    assert_eq!(status, 200, "browser base failed: {browser}");
    let via = browser["base"].as_str().unwrap().to_string();
    let seal_pk = credential_record(&cluster, 1).expect("the record").seal_pk;
    let replayed = rt.block_on(async {
        AirlockClient::remote("airlock.owner.duck".into(), via)
            .open_session(
                &seal_pk,
                CRED_NAME,
                &airlock::wire::WorkRef::Saga {
                    saga_id: delegated_id.into(),
                },
            )
            .await
    });
    let replayed = replayed.expect_err("a finished run is not a standing licence");
    assert!(
        format!("{replayed}").contains("credential_not_granted"),
        "the pointer that drew for a live run must stop drawing once it ends: {replayed}"
    );

    // ---- the grant, and the ONLY thing that changes -------------------------
    let executor_account = executor.public_key().as_ref().to_vec();
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_grant(&owner, &cluster.namespace, &executor_account)),
    );
    poll_until("the grant to commit", FINALIZE, || {
        credential_record(&cluster, 1)
            .filter(|record| gateway::credential_use_allowed(record, &executor_account))
            .map(|_| ())
    });

    // ---- DIRECTION 3: direction 1's exact shape, now granted ----------------
    //
    // The non-regression half: an executor granted in its OWN right still draws
    // in its own right, with no pointer doing any work — origin, assignee and
    // caller are all node 1 again. The grant is the only thing that changed
    // since direction 1.
    let granted_id = &node_sid(&cluster, 1, "sched\u{1f}sched-delegated-granted");
    submit_sched(&cluster, 1, granted_id, &executor_node, 1);
    let view = wait_terminal(&cluster, 1, granted_id, ROUND_TRIP);
    assert_eq!(
        view.status,
        SagaStatus::Done,
        "granting the EXECUTING node still makes its own run work: {:?}\n{}",
        view.error,
        cluster.all_log_tails(120),
    );
    let result = view.result.unwrap_or_default();
    assert!(
        String::from_utf8_lossy(&result).contains("PONG"),
        "the lent credential's reply crosses back into the saga result: {}",
        String::from_utf8_lossy(&result),
    );
}
