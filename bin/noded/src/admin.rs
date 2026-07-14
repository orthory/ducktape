//! the owner-gated control namespace — the node's PRIVATE RPC surface (ADR
//! A2/A5). everything under `/v1/admin/*` is CONTROL, not data: lifecycle,
//! code/upgrade staging, diagnostics. it is a NAMESPACE on the SAME listener
//! (geth `admin_` spirit), never a second port, gated as a unit by one
//! middleware ([`admin_guard`]) layered onto the admin sub-router alone.
//!
//! ## two orthogonal gates
//!
//! - EXPOSURE (where — A4): [`AdminExposure`]. `Disabled` unregisters the routes
//!   entirely so the surface is simply ABSENT (`router` never merges them);
//!   `Loopback` (the default) additionally refuses any non-loopback peer;
//!   `Public` accepts any peer the owner gate admits.
//! - OWNER (who — A5): a per-request proof-of-possession (PoP) by the owner
//!   account's key, verified statelessly against a PUBLIC pin — the coordinator
//!   auth pattern (#197). the owner is a CHAIN FACT: the committed `BindNode`
//!   that maps THIS node's key to an account (identity `OfNode`); the request's
//!   key must be one of that account's member keys.
//!
//! ## the bootstrap window
//!
//! a node with NO committed owner yet (fresh network, before the first
//! `BindNode`) has nobody to authenticate against. rather than lock the owner
//! out of their own fresh node, admin falls back to LOOPBACK-TRUST until the
//! first bind commits — never reachable off-box, collapsing to the full owner
//! gate the instant ownership exists on chain. the embedded single-writer daemon
//! (`node_key = None`, no consensus) lives permanently in this state, which is
//! exactly its threat model today: a loopback process can already read
//! `user.key` off disk, so loopback is a boundary this layer cannot tighten. the
//! PoP gate is what protects the `Public` surface — the capability W2 adds.
//!
//! ## the PoP wire (mirrors `nat-traversal::auth`)
//!
//! sign [`ADMIN_REQ_NS`] over `method ‖ 0x1f ‖ path_and_query ‖ 0x1f ‖ ts_be`.
//! headers carry the account key, the timestamp and the signature. the BODY is
//! deliberately NOT signed: `module-code/stage` streams a large artifact the
//! store never parks in memory, and buffering it in middleware to hash it would
//! regress that. ceiling: on a non-TLS `Public` exposure a network attacker can
//! tamper an owner-issued request's body within the freshness window — TLS
//! termination is the operator's job for hostile-network exposure.
//! ponytail: method+path+ts PoP, not body; 30s replay window; TLS is the op's job.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, Verifier as _, ed25519};
use serde::Deserialize;

use crate::handle::NodeCommand;
use crate::{NodeHandle, error_response};

/// PoP signing namespace: `sign(ADMIN_REQ_NS, method ‖ 0x1f ‖ path ‖ 0x1f ‖ ts)`.
pub const ADMIN_REQ_NS: &[u8] = b"ducktape-admin-req-v1";
/// max clock skew (seconds) between a request timestamp and this node.
pub const ADMIN_FRESHNESS_SECS: u64 = 30;

/// hex ed25519 owner-account public key (32 bytes).
pub const ADMIN_KEY_HEADER: &str = "x-ducktape-admin-key";
/// decimal unix seconds the request was signed at.
pub const ADMIN_TS_HEADER: &str = "x-ducktape-admin-ts";
/// hex ed25519 signature (64 bytes) over the canonical request bytes.
pub const ADMIN_SIG_HEADER: &str = "x-ducktape-admin-sig";

/// the default identity module id ownership resolves against.
pub const DEFAULT_IDENTITY_MODULE: &str = "identity";

/// how the node exposes its admin (control) namespace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AdminExposure {
    /// no admin namespace at all — the routes are never registered.
    Disabled,
    /// present, but reachable only from loopback peers (the default).
    #[default]
    Loopback,
    /// present and reachable from any peer the owner PoP gate admits.
    Public,
}

impl AdminExposure {
    /// parse the `DUCKTAPE_ADMIN` flag: `off | loopback | public`. anything
    /// unrecognized (including empty) falls to the safe default, `Loopback`.
    pub fn from_flag(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "disabled" | "none" => Self::Disabled,
            "public" | "expose" | "remote" => Self::Public,
            _ => Self::Loopback,
        }
    }

    /// read the exposure from the `DUCKTAPE_ADMIN` env flag.
    pub fn from_env() -> Self {
        std::env::var("DUCKTAPE_ADMIN")
            .map(|raw| Self::from_flag(&raw))
            .unwrap_or_default()
    }

    /// are the admin routes registered at all?
    pub fn enabled(self) -> bool {
        self != Self::Disabled
    }
}

/// the admin namespace's config, carried on [`NodeHandle`].
#[derive(Clone, Debug)]
pub struct AdminConfig {
    pub exposure: AdminExposure,
    /// this node's own consensus key — the subject of the `BindNode` that names
    /// its owner. `None` on the embedded daemon (no consensus, no owner on
    /// chain): admin stays loopback-trust there.
    pub node_key: Option<Vec<u8>>,
    /// identity module id ownership resolves against.
    pub identity_module: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            exposure: AdminExposure::default(),
            node_key: None,
            identity_module: DEFAULT_IDENTITY_MODULE.to_string(),
        }
    }
}

/// the canonical bytes an admin request's PoP signs / verifies — method, the
/// path+query, and the timestamp, each unambiguously separated.
fn pop_message(method: &str, path_and_query: &str, ts: u64) -> Vec<u8> {
    let mut m = Vec::with_capacity(method.len() + path_and_query.len() + 10);
    m.extend_from_slice(method.as_bytes());
    m.push(0x1f);
    m.extend_from_slice(path_and_query.as_bytes());
    m.push(0x1f);
    m.extend_from_slice(&ts.to_be_bytes());
    m
}

/// sign one admin request with the owner account's key. exposed so the app's
/// signing verb (and tests) produce the exact bytes [`verify_pop`] checks.
pub fn sign_admin(
    signer: &ed25519::PrivateKey,
    method: &str,
    path_and_query: &str,
    ts: u64,
) -> ed25519::Signature {
    signer.sign(ADMIN_REQ_NS, &pop_message(method, path_and_query, ts))
}

/// wall-clock seconds since the Unix epoch (saturating before 1970).
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, PartialEq, Eq)]
enum PopError {
    /// a required header is missing or malformed.
    MissingAuth,
    /// timestamp outside the freshness window.
    Stale,
    /// the account key or signature bytes are not valid ed25519.
    BadKey,
    /// the signature did not verify against the account key.
    BadSig,
}

/// the account key that a well-formed, fresh, correctly-signed request proves
/// possession of — or why it failed. does NOT decide ownership; that is a
/// separate chain read in [`resolve_owner`].
fn verify_pop(
    headers: &HeaderMap,
    method: &str,
    path_and_query: &str,
    now: u64,
) -> Result<Vec<u8>, PopError> {
    let key_hex = header_str(headers, ADMIN_KEY_HEADER).ok_or(PopError::MissingAuth)?;
    let ts_str = header_str(headers, ADMIN_TS_HEADER).ok_or(PopError::MissingAuth)?;
    let sig_hex = header_str(headers, ADMIN_SIG_HEADER).ok_or(PopError::MissingAuth)?;

    let ts: u64 = ts_str.parse().map_err(|_| PopError::MissingAuth)?;
    if now.abs_diff(ts) > ADMIN_FRESHNESS_SECS {
        return Err(PopError::Stale);
    }

    let key_bytes = from_hex(key_hex).ok_or(PopError::BadKey)?;
    let sig_bytes = from_hex(sig_hex).ok_or(PopError::BadKey)?;
    let pubkey = ed25519::PublicKey::decode(key_bytes.as_slice()).map_err(|_| PopError::BadKey)?;
    let sig = ed25519::Signature::decode(sig_bytes.as_slice()).map_err(|_| PopError::BadKey)?;

    if pubkey.verify(ADMIN_REQ_NS, &pop_message(method, path_and_query, ts), &sig) {
        Ok(key_bytes)
    } else {
        Err(PopError::BadSig)
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// decode an even-length lowercase/uppercase hex string to bytes.
fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// the outcome of resolving this node's committed owner.
enum OwnerResolve {
    /// this node's key is bound to an account; these are its member keys.
    Owned(Vec<Vec<u8>>),
    /// no `BindNode` names this node yet — the bootstrap window.
    NoOwner,
    /// the identity module could not be reached (actor gone / not registered).
    Unavailable,
}

/// resolve the account that owns `node_key` from committed state, over the same
/// actor query lane every read uses. NEVER trusts the connection — ownership is
/// read from `identity` `OfNode`.
async fn resolve_owner(handle: &NodeHandle, identity_module: &str, node_key: &[u8]) -> OwnerResolve {
    let req = identity::encode_query(&identity::IdentityQuery::OfNode {
        node_key: node_key.to_vec(),
    });
    let (reply, rx) = futures::channel::oneshot::channel();
    if handle
        .send(NodeCommand::Query {
            target: identity_module.to_string(),
            req,
            reply,
        })
        .await
        .is_err()
    {
        return OwnerResolve::Unavailable;
    }
    let bytes = match rx.await {
        Ok(Ok(bytes)) => bytes,
        // a module error (e.g. no identity module registered) means we cannot
        // prove ownership; fail closed to Unavailable, not open.
        _ => return OwnerResolve::Unavailable,
    };
    match identity::decode_reply(&bytes) {
        Ok(identity::IdentityReply::Account(Some(view))) => {
            OwnerResolve::Owned(view.member_keys.into_iter().map(|m| m.pubkey).collect())
        }
        Ok(identity::IdentityReply::Account(None)) => OwnerResolve::NoOwner,
        _ => OwnerResolve::Unavailable,
    }
}

/// is the request's peer a loopback address? a request with NO `ConnectInfo`
/// is an in-process (tower) caller — treated as loopback, since it never left
/// the box.
fn peer_is_loopback(req: &axum::extract::Request) -> bool {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().is_loopback())
        .unwrap_or(true)
}

/// the ONE gate over `/v1/admin/*`: exposure, then owner PoP (with the
/// bootstrap fallback). runs BEFORE the body is read, so a large staged
/// artifact never buffers here.
pub async fn admin_guard(
    State(handle): State<NodeHandle>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let cfg = &handle.admin;
    let loopback = peer_is_loopback(&req);

    // EXPOSURE gate.
    match cfg.exposure {
        // `router` never mounts these when disabled; this arm is belt-and-braces.
        AdminExposure::Disabled => return error_response(StatusCode::NOT_FOUND, "not found"),
        AdminExposure::Loopback if !loopback => {
            return error_response(StatusCode::FORBIDDEN, "admin namespace is loopback-only");
        }
        _ => {}
    }

    // OWNER gate. no owner exists to check against (embedded daemon, or a full
    // node still in the bootstrap window) ⇒ loopback-trust.
    let Some(node_key) = cfg.node_key.as_deref() else {
        return require_loopback(loopback, req, next).await;
    };
    match resolve_owner(&handle, &cfg.identity_module, node_key).await {
        OwnerResolve::Unavailable => {
            error_response(StatusCode::SERVICE_UNAVAILABLE, "cannot resolve node owner")
        }
        OwnerResolve::NoOwner => require_loopback(loopback, req, next).await,
        OwnerResolve::Owned(members) => {
            let method = req.method().as_str().to_string();
            let path = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| req.uri().path().to_string());
            match verify_pop(req.headers(), &method, &path, now_secs()) {
                Ok(account_key) if members.contains(&account_key) => next.run(req).await,
                Ok(_) => error_response(StatusCode::FORBIDDEN, "signer is not the node owner"),
                Err(PopError::Stale) => {
                    error_response(StatusCode::UNAUTHORIZED, "admin request timestamp is stale")
                }
                Err(_) => error_response(
                    StatusCode::UNAUTHORIZED,
                    "admin request needs a valid owner signature",
                ),
            }
        }
    }
}

/// in the loopback-trust states (embedded daemon / bootstrap window) a loopback
/// peer passes; a non-loopback peer is refused even under `Public` exposure —
/// there is no owner key to authenticate an off-box caller against.
async fn require_loopback(loopback: bool, req: axum::extract::Request, next: Next) -> Response {
    if loopback {
        next.run(req).await
    } else {
        error_response(
            StatusCode::FORBIDDEN,
            "node has no committed owner; admin is loopback-only until it is bound",
        )
    }
}

/// the admin sub-router: control routes plus the owner gate. merged into the
/// main router only when exposure is enabled (see [`crate::router`]).
pub fn admin_router(handle: NodeHandle) -> Router<NodeHandle> {
    Router::new()
        .route("/v1/admin/ping", get(ping))
        .route("/v1/admin/shutdown", post(crate::shutdown))
        .route("/v1/admin/logs/tail", get(logs_tail))
        // upgrade staging: ingest + fan a wasm artifact out to members. kept
        // exactly as it was on the public surface (no per-route body limit) —
        // this move only changes the GATE, never the handler.
        .route(
            "/v1/admin/module-code/stage",
            post(crate::module_code::stage_module_code),
        )
        .route(
            "/v1/admin/module-code/{digest}",
            get(crate::module_code::module_code_status),
        )
        .layer(axum::middleware::from_fn_with_state(handle, admin_guard))
}

/// GET /v1/admin/ping — a cheap authenticated liveness probe. reaching it (a
/// 200) is exactly "admin namespace reachable" for the app's control predicate.
async fn ping() -> Response {
    Json(serde_json::json!({ "ok": true })).into_response()
}

/// how many ring lines a tail returns by default / at most.
const TAIL_DEFAULT: usize = 200;
const TAIL_MAX: usize = 2000;

#[derive(Debug, Deserialize)]
struct TailParams {
    /// return lines with seq strictly greater than this. `0` (default) tails
    /// the most recent `limit` lines.
    #[serde(default)]
    after: u64,
    #[serde(default)]
    limit: Option<usize>,
}

/// GET /v1/admin/logs/tail — a simple read of the same in-memory log ring the
/// ws `logs` topic streams. diagnostics for an owner whose node is misbehaving.
async fn logs_tail(State(handle): State<NodeHandle>, Query(params): Query<TailParams>) -> Response {
    let limit = params.limit.unwrap_or(TAIL_DEFAULT).clamp(1, TAIL_MAX);
    let ring = handle.hub.log_ring();
    let after = if params.after == 0 {
        ring.latest_seq().saturating_sub(limit as u64)
    } else {
        params.after
    };
    let (rows, floor) = ring.read_after(after, limit);
    let lines: Vec<_> = rows
        .into_iter()
        .map(|(seq, line)| serde_json::json!({ "seq": seq, "line": line }))
        .collect();
    Json(serde_json::json!({ "floor": floor, "lines": lines })).into_response()
}

#[cfg(test)]
mod tests {
    // `Signer` (for `from_seed`/`public_key`) and `ed25519` arrive via this glob.
    use super::*;

    fn key(seed: u64) -> ed25519::PrivateKey {
        ed25519::PrivateKey::from_seed(seed)
    }

    fn headers_for(sig_hex: &str, key_hex: &str, ts: u64) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(ADMIN_KEY_HEADER, key_hex.parse().unwrap());
        h.insert(ADMIN_SIG_HEADER, sig_hex.parse().unwrap());
        h.insert(ADMIN_TS_HEADER, ts.to_string().parse().unwrap());
        h
    }

    #[test]
    fn pop_message_binds_method_path_and_time() {
        // method, path, and timestamp each move the signed bytes — no field can
        // bleed into another.
        assert_ne!(
            pop_message("POST", "/v1/admin/shutdown", 100),
            pop_message("GET", "/v1/admin/shutdown", 100),
        );
        assert_ne!(
            pop_message("POST", "/v1/admin/shutdown", 100),
            pop_message("POST", "/v1/admin/logs/tail", 100),
        );
        assert_ne!(
            pop_message("POST", "/v1/admin/shutdown", 100),
            pop_message("POST", "/v1/admin/shutdown", 101),
        );
    }

    #[test]
    fn owner_signed_request_verifies_and_forged_fails() {
        let owner = key(1);
        let now = 1_000_000;
        let sig = sign_admin(&owner, "POST", "/v1/admin/shutdown", now);
        let key_hex = duckfs_core::to_hex(owner.public_key().as_ref());
        let sig_hex = duckfs_core::to_hex(sig.as_ref());
        let headers = headers_for(&sig_hex, &key_hex, now);
        let subject =
            verify_pop(&headers, "POST", "/v1/admin/shutdown", now).expect("owner sig verifies");
        assert_eq!(subject, owner.public_key().as_ref().to_vec());

        // a different key signed it: PoP must fail.
        let attacker = key(2);
        let forged = sign_admin(&attacker, "POST", "/v1/admin/shutdown", now);
        let bad = headers_for(
            &duckfs_core::to_hex(forged.as_ref()),
            &key_hex, // claims the owner's key ...
            now,
        );
        assert_eq!(
            verify_pop(&bad, "POST", "/v1/admin/shutdown", now),
            Err(PopError::BadSig)
        );
    }

    #[test]
    fn a_signature_bound_to_a_different_path_is_rejected() {
        let owner = key(3);
        let now = 2_000_000;
        // signed for shutdown, replayed against logs/tail.
        let sig = sign_admin(&owner, "POST", "/v1/admin/shutdown", now);
        let headers = headers_for(
            &duckfs_core::to_hex(sig.as_ref()),
            &duckfs_core::to_hex(owner.public_key().as_ref()),
            now,
        );
        assert_eq!(
            verify_pop(&headers, "GET", "/v1/admin/logs/tail", now),
            Err(PopError::BadSig)
        );
    }

    #[test]
    fn stale_timestamp_is_rejected_both_directions() {
        let owner = key(4);
        let signed_at = 5_000_000;
        let sig = sign_admin(&owner, "POST", "/v1/admin/shutdown", signed_at);
        let headers = headers_for(
            &duckfs_core::to_hex(sig.as_ref()),
            &duckfs_core::to_hex(owner.public_key().as_ref()),
            signed_at,
        );
        // one second past the window, both directions.
        assert_eq!(
            verify_pop(&headers, "POST", "/v1/admin/shutdown", signed_at + ADMIN_FRESHNESS_SECS + 1),
            Err(PopError::Stale)
        );
        assert_eq!(
            verify_pop(&headers, "POST", "/v1/admin/shutdown", signed_at - ADMIN_FRESHNESS_SECS - 1),
            Err(PopError::Stale)
        );
        // exactly the window is still fresh.
        assert!(
            verify_pop(&headers, "POST", "/v1/admin/shutdown", signed_at + ADMIN_FRESHNESS_SECS)
                .is_ok()
        );
    }

    #[test]
    fn missing_headers_are_missing_auth_not_a_crash() {
        let empty = HeaderMap::new();
        assert_eq!(
            verify_pop(&empty, "POST", "/v1/admin/shutdown", 1),
            Err(PopError::MissingAuth)
        );
    }

    #[test]
    fn exposure_flag_parses_and_defaults_safe() {
        assert_eq!(AdminExposure::from_flag("off"), AdminExposure::Disabled);
        assert_eq!(AdminExposure::from_flag("public"), AdminExposure::Public);
        assert_eq!(AdminExposure::from_flag("loopback"), AdminExposure::Loopback);
        // unknown / empty ⇒ the safe default.
        assert_eq!(AdminExposure::from_flag("garbage"), AdminExposure::Loopback);
        assert_eq!(AdminExposure::from_flag(""), AdminExposure::Loopback);
        assert!(!AdminExposure::Disabled.enabled());
        assert!(AdminExposure::Loopback.enabled());
    }

    #[test]
    fn hex_roundtrips_and_rejects_malformed() {
        assert_eq!(from_hex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(from_hex(""), Some(vec![]));
        assert_eq!(from_hex("abc"), None); // odd length
        assert_eq!(from_hex("zz"), None); // non-hex
    }
}
