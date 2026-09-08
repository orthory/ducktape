//! the run's node tunnel, as a CAP-CHECKED READ LANE.
//!
//! A sandboxed run's guest has no NIC: `DUCKTAPE_NODE` and the vsock tunnel
//! behind it are its entire reach at the node. That variable used to name the
//! node's whole http listener, whose `/v1/files/*` read routes take no
//! credential and whose duckfs has no read ACL — so a run capped to
//! `/shared/team` read every other account's tree with one `curl`, and the
//! `duckfs_read` caps the `ducktape mcp` tool plane enforces were a suggestion
//! rather than a boundary.
//!
//! So the tunnel terminates HERE instead: a loopback proxy bound to ONE run,
//! holding that run's agent id, in front of the node's listener. It is the
//! run's whole node surface, so it is deliberately a thin pass-through with
//! these exceptions:
//!
//! * a duckfs read is admitted only if the run's committed record permits
//!   `CapRequest::DuckfsRead` for the path it names — the SAME predicate, on
//!   the same record, that `bin/node`'s MCP read tools gate on. A prefix query
//!   (`find`, `grep`) additionally has its rows and its resume cursor filtered
//!   through [`crate::duckfs_cap`], because duckfs' own prefix
//!   rule is a raw string prefix and the cap is segment-boundary. A files read
//!   route that names no path to check at all is refused: there is no way to
//!   cap-check it, so it is not the lane's to pass.
//! * `/v1/ws` is refused. It takes no credential of any kind and carries the
//!   `logs` topic — this operator's log ring — to whoever opens it.
//! * a git transport request under `/forge/{repo}/…` is admitted under the
//!   run's committed forge cap for that repo: a fetch (`git-upload-pack`)
//!   under `CapRequest::ForgeRead`, a push (`git-receive-pack`) under
//!   `CapRequest::ForgePush`. An admitted push is forwarded carrying this
//!   node's operator credential — the proof forge's receive-pack asks for,
//!   which the guest never holds — so a stock `git push` from inside the run
//!   lands under exactly the grant the owner wrote. A guest-supplied copy of
//!   that header is dropped on every route: the lane is the only thing that
//!   may speak with the operator's authority.
//! * everything else passes through byte-for-byte. The reads the plane exists
//!   for (`/v1/query`, `/v1/status`, `/v1/peers`, `/v1/blocks`, `/v1/index/*`,
//!   `/metrics`), the self-authenticating `/v1/submit/frame`, the volatile
//!   `/v1/services/hello`, the module-bound mutations whose per-request
//!   signature IS their authority, and the gateway routes runs use for egress.
//!
//! The record is fetched per gated request, off the node's own `/v1/query`,
//! rather than snapshotted at boot: caps are committed state, and a run whose
//! grant is narrowed mid-flight must feel it on the next call. It is one
//! loopback query and only a duckfs read pays it.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures::StreamExt as _;
use runs::{CapRequest, ModelRecord};

use crate::OperatorCredential;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::oneshot;

/// this run's read lane. Dropping it takes the lane down with the run — the
/// guest it fronted is gone, and a lane outliving its run would be a loopback
/// port still answering for an agent that is no longer executing.
pub(crate) struct ReadLane {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ReadLane {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

/// how a request's duckfs path is named, per route.
enum PathSource {
    /// `?path=` — one exact entry (`stat`, `read`, `ls`).
    Path,
    /// `?prefix=` — a subtree scan (`find`, `grep`).
    Prefix,
}

/// the two halves of git's smart-HTTP transport, each named by the service a
/// request speaks: a fetch reads the repo, a push writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GitService {
    UploadPack,
    ReceivePack,
}

/// what the lane does with one request, decided from its path and query alone.
enum Route {
    /// forward verbatim.
    Pass,
    /// refuse, with the stable snake_case token that says why.
    Refuse(&'static str),
    /// admit only under this run's `duckfs_read` cap; `rows`, when set, is the
    /// reply array whose own paths are re-checked afterwards.
    Capped {
        source: PathSource,
        rows: Option<&'static str>,
    },
    /// a git transport request on the named forge repo, admitted under the
    /// forge cap its service needs.
    Forge { repo: String, service: GitService },
}

/// the two query params the capped routes name a path in. A struct rather than
/// a map so a duplicate key is a decode error here exactly as it is in the
/// route's own `Query<...>` extractor upstream — a lane that parsed a
/// duplicated `path=` differently from the route it fronts would be a bypass.
#[derive(Deserialize)]
struct CapParams {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
}

struct Lane {
    /// the node's http origin, no trailing slash — where this lane forwards.
    upstream: String,
    /// the run's agent id, or `None` for a run provisioned without one, whose
    /// every duckfs read is refused (there is no record to cap it by).
    agent_id: Option<String>,
    /// this node's operator credential, lent to an admitted push; `None`
    /// refuses every push (there is no proof to lend).
    credential: Option<OperatorCredential>,
    client: reqwest::Client,
    /// reasons already logged for this run: a refusal is a per-request event
    /// and an agent in a retry loop would otherwise evict the log ring.
    logged: Mutex<BTreeSet<&'static str>>,
}

impl ReadLane {
    /// Bind this run's lane and point `envs`' `DUCKTAPE_NODE` at it, returning
    /// the lane whose lifetime is the run's.
    ///
    /// `None` when the env carries no node url: there is nothing to front, and
    /// the run is told it has no node by the same absence it always was.
    pub(crate) async fn start(
        envs: &mut [(String, String)],
        agent_id: Option<String>,
        credential: Option<OperatorCredential>,
    ) -> Result<Option<Self>, String> {
        let Some(node) = envs.iter_mut().find(|(key, _)| key == crate::NODE_URL_ENV) else {
            return Ok(None);
        };
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|e| format!("bind this run's node read lane: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("read this run's node read lane address: {e}"))?
            .port();
        let lane = Arc::new(Lane {
            upstream: node.1.trim_end_matches('/').to_string(),
            agent_id,
            credential,
            client: reqwest::Client::builder()
                // a loopback daemon is never behind a corporate proxy.
                .no_proxy()
                .build()
                .map_err(|e| format!("build this run's node read lane client: {e}"))?,
            logged: Mutex::new(BTreeSet::new()),
        });
        node.1 = format!("http://127.0.0.1:{port}");
        let app = Router::new().fallback(handle).with_state(lane);
        let (shutdown, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });
        Ok(Some(Self {
            shutdown: Some(shutdown),
            task,
        }))
    }
}

/// ONE dispatch: the request's path decides its route, and each arm is one
/// delegation. Nothing before it, nothing after it.
async fn handle(State(lane): State<Arc<Lane>>, req: Request) -> Response {
    match classify(req.uri().path(), req.uri().query().unwrap_or_default()) {
        Route::Pass => lane.forward(req).await,
        Route::Refuse(reason) => lane.refuse(reason),
        Route::Capped { source, rows } => lane.capped(req, source, rows).await,
        Route::Forge { repo, service } => lane.forge(req, &repo, service).await,
    }
}

/// the lane's whole policy, as a function of the request path and query.
///
/// Deny-by-default INSIDE the duckfs read family and pass-through outside it:
/// a `/v1/files/*` GET route that names no path (`history` is commit metadata,
/// `refs`/`has-chunks`/`diff` are the checkout engine's probes) carries no
/// argument this lane could cap-check, and `/v1/files/object/{path}` is a whole
/// file body. The writes under `/v1/files/*` (`stage`, `commit`, `pin`,
/// `watch`, the object facade's PUT/DELETE) pass: their authority is the
/// per-request signature the files module checks, which a guest can only
/// present for a key that is authorized on-chain.
fn classify(path: &str, query: &str) -> Route {
    if let Some(rest) = path.strip_prefix("/forge/") {
        return classify_forge(rest, query);
    }
    let files_read_with_no_path = matches!(
        path,
        "/v1/files/history" | "/v1/files/refs" | "/v1/files/diff" | "/v1/files/has-chunks"
    );
    let object_read = path.starts_with("/v1/files/object/");
    if files_read_with_no_path || object_read {
        return Route::Refuse("files_route_uncappable");
    }
    match path {
        // no credential of any kind, and it carries the `logs` topic: this
        // operator's log ring is not a run's to read.
        "/v1/ws" => Route::Refuse("ws_refused"),
        "/v1/files/stat" | "/v1/files/read" => Route::Capped {
            source: PathSource::Path,
            rows: None,
        },
        // `ls` names one directory; its entries are that directory's own
        // children, so the path check covers them.
        "/v1/files/ls" => Route::Capped {
            source: PathSource::Path,
            rows: None,
        },
        "/v1/files/find" => Route::Capped {
            source: PathSource::Prefix,
            rows: Some("entries"),
        },
        "/v1/files/grep" => Route::Capped {
            source: PathSource::Prefix,
            rows: Some("hits"),
        },
        _ => Route::Pass,
    }
}

/// the git smart-HTTP routes under `/forge/{repo}/…`, each named by the
/// service it speaks: the two POST endpoints by their path, the ref
/// advertisement by its `service=` query. Deny-by-default inside the forge
/// family too: any other path under the prefix has no cap to check.
fn classify_forge(rest: &str, query: &str) -> Route {
    let Some((repo, tail)) = rest.split_once('/') else {
        return Route::Refuse("forge_route_unknown");
    };
    let service = match tail {
        "git-upload-pack" => Some(GitService::UploadPack),
        "git-receive-pack" => Some(GitService::ReceivePack),
        "info/refs" => advertised_service(query),
        _ => None,
    };
    match service {
        Some(service) => Route::Forge {
            repo: repo.to_string(),
            service,
        },
        None => Route::Refuse("forge_route_unknown"),
    }
}

/// the service a smart-HTTP ref advertisement asks for, from its query.
fn advertised_service(query: &str) -> Option<GitService> {
    let service = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("service="))?;
    match service {
        "git-upload-pack" => Some(GitService::UploadPack),
        "git-receive-pack" => Some(GitService::ReceivePack),
        _ => None,
    }
}

impl Lane {
    /// a git transport request, admitted under this run's committed forge cap
    /// for the repo it names. A push carries this node's operator credential
    /// upstream: forge's receive-pack wants proof, and the run's proof IS the
    /// owner's grant, which only this lane can vouch for.
    async fn forge(&self, req: Request, repo: &str, service: GitService) -> Response {
        let Some(record) = self.record().await else {
            return self.refuse("run_record_unavailable");
        };
        match service {
            GitService::UploadPack => {
                if !record.permits(&CapRequest::ForgeRead(repo)) {
                    return self.refuse("forge_read_uncapped");
                }
                self.forward(req).await
            }
            GitService::ReceivePack => {
                if !record.permits(&CapRequest::ForgePush(repo)) {
                    return self.refuse("forge_push_uncapped");
                }
                let Some(proof) = self.operator_proof() else {
                    return self.refuse("forge_push_uncredentialed");
                };
                self.forward_as(req, Some(proof)).await
            }
        }
    }

    /// the operator header an admitted push carries, as of now.
    fn operator_proof(&self) -> Option<(HeaderName, HeaderValue)> {
        let credential = self.credential.as_ref()?;
        let value = HeaderValue::from_str(&credential.value()?).ok()?;
        Some((HeaderName::from_static(credential.header_name()), value))
    }

    /// a duckfs read, admitted only under this run's committed cap.
    async fn capped(
        &self,
        req: Request,
        source: PathSource,
        rows: Option<&'static str>,
    ) -> Response {
        let Ok(Query(params)) = Query::<CapParams>::try_from_uri(req.uri()) else {
            return self.refuse("files_query_undecodable");
        };
        let named = match source {
            PathSource::Path => params.path,
            PathSource::Prefix => params.prefix,
        };
        // a read that names no path at all is a whole-filesystem scan
        // (`find`/`grep` default an absent prefix to ""), which no cap covers.
        let Some(path) = named.filter(|path| !path.is_empty()) else {
            return self.refuse("files_read_unnamed_path");
        };
        let Some(record) = self.record().await else {
            return self.refuse("run_record_unavailable");
        };
        if !record.permits(&CapRequest::DuckfsRead(&path)) {
            return self.refuse("duckfs_read_uncapped");
        }
        let Some(rows) = rows else {
            return self.forward(req).await;
        };
        self.forward_filtered(req, record, rows).await
    }

    /// forward a prefix query and re-check every row and the resume cursor it
    /// answers with — duckfs' prefix rule is a raw string prefix, the cap is
    /// segment-boundary, so passing the gate on `prefix` does not make the rows
    /// covered.
    async fn forward_filtered(&self, req: Request, record: ModelRecord, rows: &str) -> Response {
        let response = self.forward(req).await;
        let (parts, body) = response.into_parts();
        let Ok(bytes) = axum::body::to_bytes(body, MAX_FILTERED_REPLY_BYTES).await else {
            return self.refuse("files_reply_too_large");
        };
        let Ok(mut reply) = serde_json::from_slice::<Value>(&bytes) else {
            // a non-json reply from a json route is an upstream error body;
            // hand it back untouched rather than inventing one.
            return Response::from_parts(parts, Body::from(bytes));
        };
        crate::duckfs_cap::retain_capped_rows(&record, &mut reply, rows);
        crate::duckfs_cap::scrub_uncapped_cursor(&record, &mut reply, rows);
        (parts.status, axum::Json(reply)).into_response()
    }

    /// this run's committed record, read off the node's own `/v1/query` — the
    /// same query `bin/node`'s MCP `Run::record` makes.
    async fn record(&self) -> Option<ModelRecord> {
        let agent_id = self.agent_id.as_deref()?;
        let body = json!({
            "target": "runs",
            "query": {"model": {"query": {"agent": {"agent_id": agent_id}}}},
        });
        let reply: Value = self
            .client
            .post(format!("{}/v1/query", self.upstream))
            .json(&body)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        // ModelReply::Agent(Option<ModelRecord>) — externally tagged, so the
        // record sits under "agent" and is null for an id the registry does not
        // hold.
        serde_json::from_value(reply.get("model")?.get("agent")?.clone()).ok()
    }

    /// pass a request to the node's listener and stream its answer back.
    async fn forward(&self, req: Request) -> Response {
        self.forward_as(req, None).await
    }

    /// [`Self::forward`], carrying `proof` — the operator header an admitted
    /// push presents — in place of anything the guest sent under that name.
    async fn forward_as(
        &self,
        req: Request,
        proof: Option<(HeaderName, HeaderValue)>,
    ) -> Response {
        let (parts, body) = req.into_parts();
        let path_and_query = parts
            .uri
            .path_and_query()
            .map(ToString::to_string)
            .unwrap_or_default();
        let mut out = self
            .client
            .request(parts.method, format!("{}{path_and_query}", self.upstream))
            .body(reqwest::Body::wrap_stream(body.into_data_stream()));
        let operator_header = self
            .credential
            .as_ref()
            .map(|credential| HeaderName::from_static(credential.header_name()));
        for (name, value) in parts.headers.iter() {
            // the lane's own authority is not the node's: HOST names this
            // loopback port and would be a lie upstream, and the operator
            // header is the lane's to present, never the guest's.
            let guest_claims_operator = operator_header.as_ref() == Some(name);
            if name != header::HOST && !guest_claims_operator {
                out = out.header(name, value);
            }
        }
        if let Some((name, value)) = proof {
            out = out.header(name, value);
        }
        let upstream = match out.send().await {
            Ok(upstream) => upstream,
            Err(error) => {
                // per REQUEST, and only when the node itself is unreachable —
                // the run is about to fail anyway.
                tracing::debug!(
                    target: "ducktape::sandbox",
                    reason = "read_lane_upstream_unreachable",
                    %error,
                    "this run's node read lane could not reach the node"
                );
                return (StatusCode::BAD_GATEWAY, "the node did not answer").into_response();
            }
        };
        let status = upstream.status();
        let headers = upstream.headers().clone();
        let stream = upstream
            .bytes_stream()
            .map(|chunk| chunk.map_err(std::io::Error::other));
        let mut response = Response::new(Body::from_stream(stream));
        *response.status_mut() = status;
        *response.headers_mut() = headers;
        response
    }

    /// the refusal an agent sees, and the ONE line the operator sees per run
    /// per reason. Never the path: the log ring is visible in the app, and a
    /// refused read's path is the agent's, not the operator's, to spread.
    fn refuse(&self, reason: &'static str) -> Response {
        let first_time = self
            .logged
            .lock()
            .map(|mut logged| logged.insert(reason))
            .unwrap_or(false);
        if first_time {
            tracing::warn!(
                target: "ducktape::sandbox",
                reason,
                agent = self.agent_id.as_deref().unwrap_or("<unbound>"),
                "this run's node read lane refused a request"
            );
        }
        (
            StatusCode::FORBIDDEN,
            axum::Json(json!({
                "error": format!(
                    "this run's node lane refused the request ({reason}); duckfs reads are \
                     limited to your agent's duckfs_read caps, forge fetches to its \
                     forge_read caps and forge pushes to its forge_push caps"
                )
            })),
        )
            .into_response()
    }
}

/// the ceiling on a reply this lane has to decode to filter. duckfs pages its
/// prefix queries (`MAX_PAGE` rows), so a page is kilobytes; this is slack, not
/// a budget.
const MAX_FILTERED_REPLY_BYTES: usize = 8 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    fn team_capped_record() -> ModelRecord {
        ModelRecord {
            account: 2,
            agent_id: "bot".into(),
            owner: runs::RunOrigin::External(vec![9; 32]),
            display_name: "BOT".into(),
            capability: "model-1".into(),
            allowed_actions: vec![],
            status: runs::ModelStatus::Active,
            role: runs::ModelRole::General,
            created_at: 0,
            updated_at: 0,
            recipe_hash: vec![],
            caps: runs::ResourceCaps {
                duckfs_read: vec!["/shared/team".into()],
                ..Default::default()
            },
            skills: vec![],
        }
    }

    /// a record whose forge grant is exactly `read` and `push`.
    fn forge_capped_record(read: &[&str], push: &[&str]) -> ModelRecord {
        let mut record = team_capped_record();
        record.caps.forge_read = read.iter().map(|repo| (*repo).to_string()).collect();
        record.caps.forge_push = push.iter().map(|repo| (*repo).to_string()).collect();
        record
    }

    /// the credential a test node lends, under the header name the real node
    /// reads; `None` is a node with nothing to lend.
    fn operator(token: Option<&'static str>) -> Option<OperatorCredential> {
        token.map(|token| OperatorCredential::new("x-test-operator", move || Some(token.to_string())))
    }

    /// what the stand-in forge routes answer: the service they were asked
    /// for and the operator header they saw, if any.
    async fn forge_echo(req: Request) -> axum::Json<Value> {
        let operator = req
            .headers()
            .get("x-test-operator")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        axum::Json(json!({
            "path": req.uri().path(),
            "query": req.uri().query().unwrap_or_default(),
            "operator": operator,
        }))
    }

    /// a node stand-in: answers the runs model query with `record`, the forge
    /// transport routes with an echo of what reached them, and every files
    /// route with a fixed grep page whose second hit is out of cap.
    async fn fake_node(record: ModelRecord) -> String {
        let app = Router::new()
            .route(
                "/v1/query",
                axum::routing::post(move || {
                    let record = record.clone();
                    async move { axum::Json(json!({"model": {"agent": record}})) }
                }),
            )
            .route("/forge/{repo}/info/refs", axum::routing::get(forge_echo))
            .route("/forge/{repo}/git-upload-pack", axum::routing::post(forge_echo))
            .route("/forge/{repo}/git-receive-pack", axum::routing::post(forge_echo))
            .route(
                "/v1/status",
                axum::routing::get(|| async { axum::Json(json!({"ok": true})) }),
            )
            .route(
                "/v1/ws",
                axum::routing::get(|| async { axum::Json(json!({"logs": "leaked"})) }),
            )
            .fallback(|req: Request| async move {
                let query = req.uri().query().unwrap_or_default().to_string();
                axum::Json(json!({
                    "asked": query,
                    "hits": [
                        {"path": "/shared/team/a.txt"},
                        {"path": "/shared/team-secrets/creds.txt"},
                    ],
                    "next": "/shared/team-secrets/creds.txt",
                }))
            });
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        base
    }

    /// the lane in front of that node, plus the url the guest would be handed.
    async fn lane_for(record: ModelRecord) -> (ReadLane, String) {
        lane_with(record, None).await
    }

    /// [`lane_for`] on a node lending `credential`.
    async fn lane_with(
        record: ModelRecord,
        credential: Option<OperatorCredential>,
    ) -> (ReadLane, String) {
        let node = fake_node(record).await;
        let mut envs = vec![(crate::NODE_URL_ENV.to_string(), node)];
        let lane = ReadLane::start(&mut envs, Some("bot".into()), credential)
            .await
            .unwrap()
            .expect("a lane for a node url");
        (lane, envs[0].1.clone())
    }

    /// one request through the lane with the guest's own headers.
    async fn send(
        base: &str,
        method: reqwest::Method,
        path_and_query: &str,
        headers: &[(&str, &str)],
    ) -> (StatusCode, Value) {
        let mut request = reqwest::Client::new().request(method, format!("{base}{path_and_query}"));
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let body = response.json().await.unwrap_or(Value::Null);
        (StatusCode::from_u16(status.as_u16()).unwrap(), body)
    }

    async fn get(base: &str, path_and_query: &str) -> (StatusCode, Value) {
        let response = reqwest::Client::new()
            .get(format!("{base}{path_and_query}"))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body = response.json().await.unwrap_or(Value::Null);
        (StatusCode::from_u16(status.as_u16()).unwrap(), body)
    }

    #[tokio::test]
    async fn a_read_inside_the_cap_passes_and_one_outside_it_is_refused() {
        let (_lane, base) = lane_for(team_capped_record()).await;
        let (ok, _) = get(&base, "/v1/files/read?path=/shared/team/x").await;
        assert_eq!(ok, StatusCode::OK);
        let (refused, _) = get(&base, "/v1/files/read?path=/home/other/secret.txt").await;
        assert_eq!(refused, StatusCode::FORBIDDEN);
        // percent-encoding is not a way around the gate.
        let (encoded, _) = get(&base, "/v1/files/read?path=%2Fhome%2Fother%2Fsecret.txt").await;
        assert_eq!(encoded, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn grep_hits_outside_the_cap_are_filtered_out_of_an_admitted_page() {
        let (_lane, base) = lane_for(team_capped_record()).await;
        let (status, body) = get(&base, "/v1/files/grep?pattern=x&prefix=/shared/team").await;
        assert_eq!(status, StatusCode::OK);
        let paths: Vec<&str> = body["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, vec!["/shared/team/a.txt"]);
        assert_eq!(body["next"], json!("/shared/team/a.txt"));
    }

    #[tokio::test]
    async fn the_log_ring_websocket_is_refused() {
        let (_lane, base) = lane_for(team_capped_record()).await;
        let (status, _) = get(&base, "/v1/ws").await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn the_uncappable_files_routes_are_refused() {
        let (_lane, base) = lane_for(team_capped_record()).await;
        for path in [
            "/v1/files/history?limit=10",
            "/v1/files/object/home/other/secret.txt",
            "/v1/files/find",
            "/v1/files/grep?pattern=x",
        ] {
            let (status, _) = get(&base, path).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
    }

    /// the lane and the MCP tool plane are two doors onto the SAME duckfs
    /// reads, and a second hand-rolled copy of the cap filter in either one is
    /// how they drift into disagreeing. Both must reach for `duckfs_cap`, and
    /// neither may define its own.
    #[test]
    fn the_lane_and_the_mcp_tools_share_one_cap_filter() {
        // every needle is ASSEMBLED, never written whole: this test reads its
        // OWN file, and a literal here would match itself and pass on nothing.
        let filters = [
            format!("duckfs_cap::retain_capped_{}(", "rows"),
            format!("duckfs_cap::scrub_uncapped_{}(", "cursor"),
        ];
        let copies = [
            format!("fn retain_{}", "capped"),
            format!("fn scrub_{}", "uncapped"),
        ];
        let read = |path: &str| std::fs::read_to_string(path).expect(path);
        let lane = read(concat!(env!("CARGO_MANIFEST_DIR"), "/src/read_lane.rs"));
        let mcp = read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../bin/node/src/mcp/tools/read.rs"
        ));
        for (name, source) in [("read_lane.rs", &lane), ("mcp/tools/read.rs", &mcp)] {
            for filter in &filters {
                assert!(
                    source.contains(filter),
                    "{name} must filter duckfs replies through ModelRecord's {filter}"
                );
            }
            for copy in &copies {
                assert!(
                    !source.contains(copy),
                    "{name} defines its own copy of the cap filter ({copy})"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_fetch_needs_forge_read_and_a_push_needs_forge_push() {
        let (_lane, base) = lane_with(forge_capped_record(&["docs"], &[]), operator(Some("t"))).await;
        let fetches = [
            (reqwest::Method::GET, "/forge/docs/info/refs?service=git-upload-pack"),
            (reqwest::Method::POST, "/forge/docs/git-upload-pack"),
        ];
        for (method, path) in fetches {
            let (status, body) = send(&base, method, path, &[]).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert_eq!(body["operator"], Value::Null, "a fetch lends no credential");
        }
        let refused = [
            (reqwest::Method::GET, "/forge/docs/info/refs?service=git-receive-pack"),
            (reqwest::Method::POST, "/forge/docs/git-receive-pack"),
            (reqwest::Method::GET, "/forge/other/info/refs?service=git-upload-pack"),
            (reqwest::Method::POST, "/forge/other/git-upload-pack"),
            // no service named, or a path that is not a transport endpoint
            (reqwest::Method::GET, "/forge/docs/info/refs"),
            (reqwest::Method::GET, "/forge/docs/HEAD"),
        ];
        for (method, path) in refused {
            let (status, _) = send(&base, method, path, &[]).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
    }

    #[tokio::test]
    async fn an_admitted_push_carries_the_operator_credential_the_guest_never_holds() {
        let (_lane, base) =
            lane_with(forge_capped_record(&[], &["app"]), operator(Some("node-secret"))).await;
        let (status, body) = send(
            &base,
            reqwest::Method::POST,
            "/forge/app/git-receive-pack",
            // a guest claiming the operator header is overwritten, never relayed
            &[("x-test-operator", "forged")],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operator"], json!("node-secret"));
        let (status, body) = send(
            &base,
            reqwest::Method::GET,
            "/forge/app/info/refs?service=git-receive-pack",
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operator"], json!("node-secret"));
        // push implies read: the same record fetches
        let (status, body) = send(
            &base,
            reqwest::Method::POST,
            "/forge/app/git-upload-pack",
            &[("x-test-operator", "forged")],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operator"], Value::Null, "a guest's claim is dropped on every route");
    }

    #[tokio::test]
    async fn a_push_on_a_node_with_no_credential_to_lend_is_refused() {
        let (_lane, base) = lane_with(forge_capped_record(&[], &["app"]), None).await;
        let (status, _) = send(&base, reqwest::Method::POST, "/forge/app/git-receive-pack", &[]).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn everything_the_run_plane_needs_still_passes_through() {
        let (_lane, base) = lane_for(team_capped_record()).await;
        let (status, body) = get(&base, "/v1/status").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], json!(true));
    }
}
