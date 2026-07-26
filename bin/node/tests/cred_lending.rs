//! Two-node granted-credential lending over the gateway overlay, and the
//! owner-gateway grant gate the flow forces.
//!
//! The OWNER node (0) runs the airlock LENDER DAEMON (`ducktape service run
//! airlock`) beside it, serving its disk-backed credential store (the `user cred
//! add` layout) on a loopback port the node reverse-proxies overlay ingress to.
//! It registers the credential record on-chain and grants it to the COMPUTE
//! node's (1) account. The compute
//! node resolves the name from committed state, pins the on-chain seal_pk, and —
//! claiming its GRANTED account — completes a proxied `/v1/messages` through the
//! owner's gateway over the real WireGuard overlay to a mock Anthropic upstream.
//!
//! Then the piece the flow forces: a fresh, UNGRANTED account is refused at the
//! owner's own gateway (403 `credential_not_granted`) before any credentialed
//! request — two nodes suffice, because the ACCOUNT is what's gated. The gate
//! reads the owner node's own committed gateway record; the compute-side broker
//! check (`cred_resolve`) is a separate, earlier refusal.
//!
//! Like `airlock_gateway_e2e`, this proves the node-to-node overlay hop with a
//! mock upstream (no Anthropic quota, no silicon). The self-host trust anchor is
//! the on-chain seal_pk, pinned by the client — there is no TEE quote here.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{Cluster, poll_until, serial};
use commonware_cryptography::{Signer as _, ed25519};
use gateway::{
    CredentialGrantStatement, CredentialKind, CredentialRecord, DuckDnsName, GATEWAY_CREDENTIAL_NS,
    GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization, RouteAudience, RouteDefinition,
    RouteMethod, RouteName, RoutePolicy, RouteStatement, RouteTarget, SetCredentialStatement,
    credential_use_allowed,
};
use identity::{AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth};

use airlock::client::Gateway as AirlockClient;
use airlock::seal::SealKeypair;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use tokio::runtime::Runtime;

const READY: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);

// ----- identity + duckdns helpers (mirror airlock_gateway_e2e) --------------

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

/// The airlock LoopbackHttp route: `allow_authorization = true` (the session-token
/// bearer must reach the enclave) and a real 4 MiB response cap. Same shape as
/// `airlock_gateway_e2e`'s route.
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
            gateway::GATEWAY_ROUTE_NS,
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
        GatewayReply::Route(record) => record
            .as_ref()
            .as_ref()
            .map(|record| record.statement.revision),
        _ => None,
    }
}

// ----- credential record helpers (Task 1 wire) ------------------------------

/// The owner-signed registration for a claude credential, publisher = the owner
/// node, `grants` empty (grants are added by `signed_grant`). Signed under
/// [`GATEWAY_CREDENTIAL_NS`] exactly as the module verifies it.
fn signed_set_credential(
    owner: &ed25519::PrivateKey,
    chain: &str,
    publisher_node: &[u8],
    name: &str,
    seal_pk: [u8; 32],
) -> GatewayMsg {
    let statement = SetCredentialStatement {
        chain_id: chain.into(),
        record: CredentialRecord {
            name: name.into(),
            owner_account: owner.public_key().as_ref().to_vec(),
            publisher_node: publisher_node.to_vec(),
            kind: CredentialKind::Claude,
            seal_pk,
            grants: std::collections::BTreeSet::new(),
        },
    };
    let signature = owner
        .sign(
            GATEWAY_CREDENTIAL_NS,
            &gateway::set_credential_preimage(&statement).unwrap(),
        )
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

/// The owner-signed grant of `name` to `grantee`.
fn signed_grant(
    owner: &ed25519::PrivateKey,
    chain: &str,
    name: &str,
    grantee: &[u8],
) -> GatewayMsg {
    let statement = CredentialGrantStatement {
        chain_id: chain.into(),
        owner_account: owner.public_key().as_ref().to_vec(),
        name: name.into(),
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

fn query_credential(cluster: &Cluster, reader: usize, name: &str) -> Option<CredentialRecord> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gateway::encode_query(&GatewayQuery::Credential {
            name: name.to_string(),
        }),
    )?;
    match gateway::decode_reply(&bytes).ok()? {
        GatewayReply::Credential(record) => record,
        _ => None,
    }
}

/// Seed the owner's disk-backed store with a claude credential dir (the layout the
/// lender's `load_seeds` reads: a `kind` marker + a `.credentials.json` refresh
/// token). Done BEFORE the daemon spawns so it is served from its first session.
fn seed_claude_store(storage: &std::path::Path, name: &str, refresh: &str) {
    let dir = storage.join("airlock-creds").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kind"), "claude\n").unwrap();
    std::fs::write(
        dir.join(".credentials.json"),
        format!(r#"{{"claudeAiOauth":{{"refreshToken":"{refresh}"}}}}"#),
    )
    .unwrap();
}

/// The seal PUBLIC key the owner's lender daemon minted when it opened the store
/// — read from the store's `seal.key` (32-byte secret, 0600). This is the anchor
/// the owner puts on-chain and the compute node pins.
fn seal_pk_from_store(storage: &std::path::Path) -> [u8; 32] {
    let bytes = std::fs::read(storage.join("airlock-creds").join("seal.key")).expect("seal.key");
    let secret: [u8; 32] = bytes.as_slice().try_into().expect("32-byte seal secret");
    SealKeypair::from_secret_bytes(secret).public_bytes()
}

// ----- mock Anthropic upstream (mirror airlock_gateway_e2e) -----------------

/// `/oauth/token` mints `acc-N`; `/v1/messages` accepts ONLY `Bearer acc-N` (so a
/// 200 proves the gateway refreshed the lent credential and swapped the session
/// token for the real one) and returns the `AIRLOCK-OK` marker.
#[derive(Default)]
struct MockUpstream {
    n: Mutex<u64>,
}

async fn mock_oauth(State(st): State<Arc<MockUpstream>>) -> Json<serde_json::Value> {
    let mut n = st.n.lock().unwrap();
    *n += 1;
    Json(json!({ "access_token": format!("acc-{n}"), "refresh_token": format!("ref-{n}"), "expires_in": 3600 }))
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
    ([("content-type", "text/event-stream")], "data: AIRLOCK-OK\n\n").into_response()
}

async fn bind_and_serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[test]
fn granted_credential_resolves_and_round_trips_across_nodes() {
    let _serial = serial();
    let rt = Runtime::new().unwrap();

    // The owner's mock Anthropic upstream lives in THIS process; the owner node
    // subprocess reaches it over host loopback, exactly like a real deployment
    // reaches api.anthropic.com.
    let (upstream, oauth_url) = rt.block_on(async {
        let base = bind_and_serve(
            Router::new()
                .route("/oauth/token", post(mock_oauth))
                .route("/v1/messages", post(mock_messages))
                .with_state(Arc::new(MockUpstream::default())),
        )
        .await;
        let oauth = format!("{base}/oauth/token");
        (base, oauth)
    });

    // Two real WireGuard nodes: owner (0, publisher) and compute (1, grantee).
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.wireguard = true;

    // The owner co-hosts the store: seed the credential dir BEFORE the lender
    // daemon starts so it is served from the first session, and point the
    // daemon's upstream + oauth at the mock.
    let owner_storage = cluster.workspace(0);
    seed_claude_store(&owner_storage, "owner-claude-1", "rt-e2e");
    cluster.env[0] = vec![
        ("DUCKTAPE_AIRLOCK_ANTHROPIC_BASE".into(), upstream.clone()),
        ("DUCKTAPE_AIRLOCK_OAUTH_TOKEN_URL".into(), oauth_url.clone()),
    ];

    for index in 0..2 {
        cluster.spawn(index);
    }
    for index in 0..2 {
        cluster.wait_marker(index, "rpc listening on", READY);
        cluster.wait_marker(index, "converged root_hash=", READY);
        cluster.wait_marker(index, "peer handshake COMPLETE", READY);
        cluster.wait_marker(index, "gateway plane: overlay stream bound", READY);
    }
    // Only NOW start the lender: the daemon's first hello must land, so its
    // node's http surface has to be listening before it starts. It opens the
    // store (minting seal.key), binds loopback and registers its port as the
    // `airlock` gateway route the node reverse-proxies to.
    cluster.spawn_service(0, "airlock");
    cluster.wait_service_marker(0, "airlock", "airlock daemon serving", READY);

    let owner = ed25519::PrivateKey::from_seed(42);
    let compute = ed25519::PrivateKey::from_seed(43);
    let owner_node = Cluster::identity(0);
    let compute_node = Cluster::identity(1);
    for (index, member, node) in [
        (0usize, &owner, owner_node.as_slice()),
        (1usize, &compute, compute_node.as_slice()),
    ] {
        cluster.submit(
            index,
            "identity",
            &identity::encode_msg(&IdentityMsg::BindNode {
                authorizer: bind_auth(member, &cluster.namespace, node),
            }),
        );
        poll_until("identity binding", FINALIZE, || {
            account_of_node(&cluster, index, node)
                .filter(|account| account.account_id == member.public_key().as_ref())
        });
    }

    // Owner maps `owner.duck`, so `airlock.owner.duck` resolves to her account.
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&GatewayMsg::SetHandle {
            handle: Some("owner".into()),
        }),
    );
    for reader in 0..2 {
        poll_until("owner.duck resolution", FINALIZE, || {
            resolve_handle(&cluster, reader, "owner")
        });
    }

    // Publish the signed airlock route (the one manual operator act; the daemon
    // already registered its loopback port).
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_airlock_route(&owner, &cluster.namespace, &owner_node, 1)),
    );
    poll_until("airlock route revision 1", FINALIZE, || {
        (airlock_route_revision(&cluster, 1, owner.public_key().as_ref()) == Some(1)).then_some(())
    });

    // Register the credential record on-chain with the store's seal_pk, then grant
    // it to the compute account. The seal.key exists once the daemon opened the store.
    let seal_pk = seal_pk_from_store(&owner_storage);
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_set_credential(
            &owner,
            &cluster.namespace,
            &owner_node,
            "owner-claude-1",
            seal_pk,
        )),
    );
    poll_until("credential record committed", FINALIZE, || {
        query_credential(&cluster, 1, "owner-claude-1")
    });

    let compute_account = compute.public_key().as_ref().to_vec();
    cluster.submit(
        0,
        "gateway",
        &gateway::encode_msg(&signed_grant(
            &owner,
            &cluster.namespace,
            "owner-claude-1",
            &compute_account,
        )),
    );
    poll_until("grant committed", FINALIZE, || {
        query_credential(&cluster, 1, "owner-claude-1")
            .filter(|record| credential_use_allowed(record, &compute_account))
            .map(|_| ())
    });

    // Compute resolves the name from committed state and pins its on-chain seal_pk.
    let record = query_credential(&cluster, 1, "owner-claude-1").expect("record");
    assert_eq!(record.seal_pk, seal_pk, "the pinned seal_pk is the store's own key");

    // Compute's browser-gateway origin base — the `via` a compute-side host posts
    // to (no Origin header ⇒ it passes the only guard).
    let (status, browser) = cluster.http(1, "GET", "/v1/gateway/browser", None);
    assert_eq!(status, 200, "browser base failed: {browser}");
    let via = browser["base"].as_str().unwrap().to_string();

    // THE PROOF: from compute, over the overlay, claiming the GRANTED account.
    // Every hop is remote (compute -> overlay -> owner -> loopback upstream).
    let reply = rt.block_on(async {
        let gw = AirlockClient::remote("airlock.owner.duck".into(), via.clone());
        let token = gw
            .open_session_as(&record.seal_pk, "owner-claude-1", &compute_account)
            .await
            .expect("granted session opens over the overlay");
        let resp = gw
            .route(gw.http().post(gw.url("/v1/messages")))
            .bearer_auth(&token)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .body(r#"{"model":"claude-sonnet-5","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#)
            .send()
            .await
            .expect("proxied messages over overlay");
        (resp.status(), resp.text().await.unwrap())
    });
    assert_eq!(reply.0, reqwest::StatusCode::OK, "granted overlay call: {reply:?}");
    assert!(
        reply.1.contains("AIRLOCK-OK"),
        "the lent credential's reply must return over the overlay: {reply:?}"
    );

    // NEGATIVE: a fresh, UNGRANTED account is refused at the owner's own gateway,
    // before any credentialed request — the account is what's gated. Two nodes
    // suffice; the stranger keypair is bound to no node and granted nothing.
    let stranger = ed25519::PrivateKey::from_seed(9_999);
    let stranger_account = stranger.public_key().as_ref().to_vec();
    let refused = rt.block_on(async {
        let gw = AirlockClient::remote("airlock.owner.duck".into(), via);
        gw.open_session_as(&record.seal_pk, "owner-claude-1", &stranger_account)
            .await
    });
    assert!(
        refused.is_err(),
        "an ungranted account must be refused at session open, not proxied: {refused:?}"
    );
}
