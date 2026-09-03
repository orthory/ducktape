//! the ONE per-request proof-of-possession (PoP) this node verifies, and the
//! gate that requires one on every MUTATING `/v1` route.
//!
//! ## why a signature and not a token
//!
//! a mutating request changes committed state, the node's own files, or a
//! process it hosts. "can dial this port" is not authority for any of that:
//! loopback is precisely what the audit found trusted, and widening
//! `http_listen` hands the whole surface to anyone who can open a socket. so a
//! mutation carries a signature by the key that is ACTING, and the node
//! verifies it before the handler runs. reads stay open.
//!
//! ## two namespaces, one verifier
//!
//! `/v1/admin/*` (control: shutdown, module code, log tail) already had this
//! shape ([`crate::admin`]). the data plane needs the same primitive with a
//! DIFFERENT bind, so the verifier lives here and each namespace owns its own
//! signing namespace and message:
//!
//! - control: [`crate::admin::ADMIN_REQ_NS`] over `method ‖ path ‖ node ‖ ts`.
//!   the body is deliberately unsigned there — `module-code/stage` streams a
//!   large artifact the gate must not buffer.
//! - data: [`DATA_REQ_NS`] over `method ‖ path ‖ node ‖ ts ‖ sha256(body)`.
//!   every gated body is already buffered by its handler (they all take
//!   `Bytes`/`Json`), and the WHOLE POINT of this gate is that the bytes a
//!   caller signed for are the bytes that land — an unsigned body would let a
//!   network attacker swap the chunk, the commit's changes, or the object.
//!
//! a signature is bound to the TARGET NODE's consensus key, so one minted for
//! node X never replays against node Y; and to a timestamp inside
//! [`FRESHNESS_SECS`], so a captured request dies with the window.
//!
//! ## the second credential: the node's OWN daemons
//!
//! some mutations are the NODE's, not a person's. a capability announce keys
//! the registry on the submitting node; a compute lease heartbeat and bid are
//! held BY the node; an agent run's provisioner writes duckfs on the node's
//! behalf. a user signature there would name the WRONG actor, so those callers
//! present what `/v1/admin/*` already asks of a local daemon
//! ([`crate::admin::ADMIN_TOKEN_HEADER`]): this boot's 0600 workspace secret,
//! from a loopback peer. "can read the node's own workspace", never "can dial
//! the port" — and a sandbox guest, which reaches this listener through a vsock
//! tunnel with no workspace, holds neither credential and so writes nothing.
//!
//! EITHER admits a mutating request; nothing else does. which one was presented
//! decides the ACTING identity, and that is the whole of the difference: a
//! signature acts as its key, the operator credential acts as the node.
//!
//! ## what this gate proves, and what it does not
//!
//! it proves POSSESSION of the key named in the headers, and hands the
//! verified key to the handler as [`SignedBy`] — the acting identity is the
//! caller's key, never the node's. it does NOT decide membership: there is no
//! ACL read here, so any well-formed key may act. that is the same bar
//! `/v1/submit/frame` sets (a frame's signer is whoever signed it), and
//! authorization per module stays the modules' job.
//!
//! ponytail: possession, not membership; 30s replay window; TLS is the op's job.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Verifier as _, ed25519};

use crate::NodeHandle;

/// the CLIENT half — the namespace, the header trio, the canonical message
/// and the signing — lives with the kernel's frame codec so every signer in
/// the tree links the one spelling the daemon verifies against.
pub use ::node::signed_req::{
    DATA_REQ_NS, KEY_HEADER, SIG_HEADER, TS_HEADER, now_secs, request_headers, request_message,
    sign_request,
};

/// the header trio one namespace's PoP travels in. control and data use
/// different names so a control credential can never be replayed onto a data
/// route by a proxy that copies headers wholesale.
#[derive(Clone, Copy)]
pub struct PopHeaders {
    pub key: &'static str,
    pub ts: &'static str,
    pub sig: &'static str,
}

/// the data plane's header trio.
pub const DATA_HEADERS: PopHeaders = PopHeaders {
    key: KEY_HEADER,
    ts: TS_HEADER,
    sig: SIG_HEADER,
};

/// max clock skew (seconds) between a request timestamp and this node. ONE
/// window for both namespaces: two would only ever drift apart.
pub const FRESHNESS_SECS: u64 = 30;

/// axum's own default body cap, which is what every gated route that sets no
/// `DefaultBodyLimit` of its own already enforces — the json lanes
/// (`/v1/submit`, `/v1/invite`, the workspace RPC, the term routes) and the
/// filter string. Spelled here because the middleware runs OUTSIDE the route's
/// layers and so cannot read the limit they install.
const DEFAULT_JSON_BODY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PopError {
    /// a required header is missing or malformed.
    MissingAuth,
    /// timestamp outside the freshness window.
    Stale,
    /// the account key or signature bytes are not valid ed25519.
    BadKey,
    /// the signature did not verify against the account key.
    BadSig,
}

/// the key that a well-formed, fresh, correctly-signed request proves
/// possession of — or why it failed. `message` builds the canonical bytes from
/// the timestamp the headers carry, which is why it is a closure: the ts must
/// be parsed and range-checked BEFORE it can be folded into a signed message.
///
/// decides possession only. membership/ownership is a separate read (see
/// `admin::resolve_owner`); this function never consults state.
pub(crate) fn verify_pop(
    headers: &HeaderMap,
    names: PopHeaders,
    ns: &[u8],
    now: u64,
    message: impl FnOnce(u64) -> Vec<u8>,
) -> Result<Vec<u8>, PopError> {
    let key_hex = header_str(headers, names.key).ok_or(PopError::MissingAuth)?;
    let ts_str = header_str(headers, names.ts).ok_or(PopError::MissingAuth)?;
    let sig_hex = header_str(headers, names.sig).ok_or(PopError::MissingAuth)?;

    let ts: u64 = ts_str.parse().map_err(|_| PopError::MissingAuth)?;
    if now.abs_diff(ts) > FRESHNESS_SECS {
        return Err(PopError::Stale);
    }

    let key_bytes = from_hex(key_hex).ok_or(PopError::BadKey)?;
    let sig_bytes = from_hex(sig_hex).ok_or(PopError::BadKey)?;
    let pubkey = ed25519::PublicKey::decode(key_bytes.as_slice()).map_err(|_| PopError::BadKey)?;
    let sig = ed25519::Signature::decode(sig_bytes.as_slice()).map_err(|_| PopError::BadKey)?;

    match pubkey.verify(ns, &message(ts), &sig) {
        true => Ok(key_bytes),
        false => Err(PopError::BadSig),
    }
}

pub(crate) fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// decode an even-length lowercase/uppercase hex string to bytes.
pub(crate) fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// the mark [`signed_write_guard`] puts on a request that arrived from a
/// loopback peer holding this node's operator credential.
///
/// It exists for the ONE mutating route this middleware cannot decide for
/// itself: the forge's `git-receive-pack`, whose other proof (git's own push
/// certificate) is inside the packfile body. The gate establishes the fact
/// once, here, and the handler reads it rather than re-deriving the loopback
/// and constant-time-compare rules a second time.
#[derive(Clone, Copy, Debug)]
pub struct OperatorCredential;

/// the verified acting identity, put on the request by [`signed_write_guard`]
/// and read by any gated handler that submits on the caller's behalf. its
/// presence IS the proof the gate ran: a handler that finds none was reached
/// on an ungated route.
#[derive(Clone, Debug)]
pub struct SignedBy(pub Vec<u8>);

/// which mutating lane a request path belongs to — the ONE discriminant
/// [`Lane::mutates`] branches on. `Open` is everything else: reads, the
/// self-authenticating `/v1/submit/frame`, the volatile service-hello, the
/// websocket upgrades, and `/v1/admin/*` (which carries its own gate).
///
/// the ONE route family that mutates and is NOT here is the forge's
/// `git-receive-pack`: `git push` cannot attach a header of its own, so its
/// proof is git's OWN push certificate (`git push --signed`), refused inside
/// `git_http::parse_push_commands` rather than by this middleware.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Lane {
    /// `/v1/fs/workspaces…` — create, commit, delete a managed checkout.
    Workspace,
    /// `/v1/files/object/{*path}` — the S3-shaped facade. PUT is a
    /// single-change commit and DELETE a single-change rm; its GET is a read.
    /// SEPARATE from [`Lane::Files`] because it is the only lane whose write
    /// verb is PUT, and a `POST`-only arm left it open.
    Object,
    /// `/v1/files/…` — blob, stage, commit, pin, watch. every duckfs read on
    /// this prefix is a GET, so POST alone names the writes.
    Files,
    /// `/v1/term/sessions…` — create and close a node-hosted pty.
    Term,
    /// a fixed mutating path ([`SIGNED_POSTS`]).
    Fixed,
    Open,
}

/// the exact mutating POST paths that are not a path family. an ALLOWLIST, not
/// "every POST": `/v1/query`, `/v1/index/{m}/view` and `/v1/gateway/proxy` are
/// POSTs that read, and `/v1/submit/frame` carries its own signature inside the
/// frame (it is a longer path than `/v1/submit`, so the exact match leaves it
/// open).
const SIGNED_POSTS: &[&str] = &["/v1/log-filter", "/v1/invite", "/v1/submit"];

const WORKSPACE_PREFIX: &str = "/v1/fs/workspaces";
const OBJECT_PREFIX: &str = "/v1/files/object/";
const FILES_PREFIX: &str = "/v1/files/";
const TERM_PREFIX: &str = "/v1/term/sessions";

/// path prefix → lane, in match order. the object facade sits UNDER the files
/// prefix, so it has to be tried first; everything else here is disjoint.
const LANE_PREFIXES: &[(&str, Lane)] = &[
    (WORKSPACE_PREFIX, Lane::Workspace),
    (OBJECT_PREFIX, Lane::Object),
    (FILES_PREFIX, Lane::Files),
    (TERM_PREFIX, Lane::Term),
];

fn lane_of(path: &str) -> Lane {
    if let Some((_, lane)) = LANE_PREFIXES
        .iter()
        .find(|(prefix, _)| path.starts_with(prefix))
    {
        return *lane;
    }
    match SIGNED_POSTS.contains(&path) {
        true => Lane::Fixed,
        false => Lane::Open,
    }
}

impl Lane {
    /// does this method MUTATE on this lane? one discriminant, one match — a
    /// new route that mutates is added to [`lane_of`]'s table, never to a
    /// scattered `if` at a call site.
    fn mutates(self, method: &Method) -> bool {
        let posts = *method == Method::POST;
        let removes = *method == Method::DELETE;
        let replaces = *method == Method::PUT;
        match self {
            Lane::Workspace => posts || removes,
            Lane::Object => replaces || removes,
            Lane::Files | Lane::Term | Lane::Fixed => posts,
            Lane::Open => false,
        }
    }

    /// the largest body this gate will read in order to hash it.
    ///
    /// PER LANE, and it has to be: the cap is reached by an UNAUTHENTICATED
    /// caller — the buffering happens before the signature is checked, which is
    /// the only order a body digest can be verified in. One shared 64 MiB
    /// ceiling would therefore let anyone who can dial the port make the node
    /// hold 64 MiB for a `/v1/files/stage` whose own route rejects anything
    /// over a 1 MiB chunk. Each lane names the ceiling its route already
    /// enforces, so hashing adds no new peak on any of them.
    fn max_body(self) -> usize {
        match self {
            // the S3 facade's PUT — one whole object.
            Lane::Object => crate::MAX_OBJECT_BYTES,
            // the widest write on the `/v1/files/` prefix is the blob receipt;
            // a staged chunk (1 MiB) and the json commits sit under it.
            Lane::Files => crate::MAX_BLOB_BODY_BYTES,
            // json bodies and the log-filter string. `Open` never reaches here
            // (the guard returns before asking), and takes the small cap so a
            // table that ever disagreed fails closed rather than wide.
            Lane::Workspace | Lane::Term | Lane::Fixed | Lane::Open => DEFAULT_JSON_BODY_BYTES,
        }
    }
}

/// does this request mutate, and therefore need a credential? the guard itself
/// asks [`Lane::mutates`] on the lane it already resolved, so this pairing of
/// the two is only ever what the tests below assert against.
#[cfg(test)]
fn needs_signature(method: &Method, path: &str) -> bool {
    lane_of(path).mutates(method)
}

/// the plane a refusal is logged on. a files/duckfs mutation belongs to
/// `ducktape::files`, everything else to the node's own plane.
fn plane_of(path: &str) -> Plane {
    let is_files = path.starts_with("/v1/files") || path.starts_with(WORKSPACE_PREFIX);
    match is_files {
        true => Plane::Files,
        false => Plane::Node,
    }
}

#[derive(Clone, Copy)]
enum Plane {
    Files,
    Node,
}

/// Why a mutating request was turned away. status and the stable snake_case
/// `reason` are DERIVED from the variant, exactly like [`crate::admin::AdminRefusal`],
/// so they cannot drift apart. no message names the expected credential or the
/// request URI — the caller learns only which check it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteRefusal {
    /// no signature headers at all, or one of them is unparseable.
    SignatureMissing,
    /// the timestamp is outside the freshness window.
    SignatureStale,
    /// the key or signature bytes are not valid ed25519.
    SignatureMalformed,
    /// a well-formed signature that does not bind this request.
    SignatureInvalid,
    /// the body is larger than any gated route accepts (or its stream broke).
    BodyOverCap,
}

impl WriteRefusal {
    /// the stable snake_case token — greppable, countable, never prose.
    pub fn reason(self) -> &'static str {
        match self {
            Self::SignatureMissing => "signature_missing",
            Self::SignatureStale => "signature_stale",
            Self::SignatureMalformed => "signature_malformed",
            Self::SignatureInvalid => "signature_invalid",
            Self::BodyOverCap => "body_over_cap",
        }
    }

    /// 401 for "you presented nothing usable"; 413 for a body this gate could
    /// not read to hash.
    pub fn status(self) -> StatusCode {
        match self {
            Self::SignatureMissing
            | Self::SignatureStale
            | Self::SignatureMalformed
            | Self::SignatureInvalid => StatusCode::UNAUTHORIZED,
            Self::BodyOverCap => StatusCode::PAYLOAD_TOO_LARGE,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::SignatureMissing => {
                "this route mutates state, so it requires a signed request \
                 (x-ducktape-key / -ts / -sig) or this node's operator credential"
            }
            Self::SignatureStale => "the request timestamp is outside this node's freshness window",
            Self::SignatureMalformed => "the request key or signature is not a valid ed25519 value",
            Self::SignatureInvalid => {
                "the signature does not bind this method, path, node, timestamp and body"
            }
            Self::BodyOverCap => "the request body is larger than this node accepts",
        }
    }
}

/// does this request carry THIS node's operator credential, presented by a
/// loopback peer? the SAME conjunction `/v1/admin/*` requires
/// ([`crate::admin`]), compared the same constant-time way — "can read the
/// node's own workspace", never "can dial the port".
///
/// a node that minted no credential verifies none: it fails closed here and the
/// request falls through to the signature check, exactly as if nothing had been
/// presented.
pub(crate) fn operator_credential_matches(
    cfg: &crate::AdminConfig,
    headers: &HeaderMap,
    on_box: bool,
) -> bool {
    let Some(expected) = cfg.operator_token.as_deref() else {
        return false;
    };
    let Some(offered) = header_str(headers, crate::admin::ADMIN_TOKEN_HEADER) else {
        return false;
    };
    on_box && crate::services::token_matches(offered, expected)
}

/// the ONE gate over every mutating `/v1` route: decide, then run or refuse.
/// an open (read) route is passed straight through, body untouched.
pub(crate) async fn signed_write_guard(
    State(handle): State<NodeHandle>,
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    // the node's own daemons, acting AS the node. established for EVERY
    // request, gated or not, because the forge's git lane needs the same fact
    // and cannot re-derive it (see [`OperatorCredential`]). no `SignedBy` is
    // inserted, so the acting origin stays the node's own name — which is what
    // a daemon's write is.
    let on_box = crate::admin::peer_is_loopback(&req);
    let is_operator = operator_credential_matches(&handle.admin, req.headers(), on_box);
    if is_operator {
        req.extensions_mut().insert(OperatorCredential);
    }
    let lane = lane_of(&path);
    if !lane.mutates(&method) {
        return next.run(req).await;
    }
    // admitted without touching the body: nothing is signed over it on this
    // path, so a whole packfile or a 4 MiB blob still streams to its handler
    // instead of buffering in middleware.
    if is_operator {
        return next.run(req).await;
    }
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| path.clone());
    let headers = req.headers().clone();
    let (parts, body) = req.into_parts();
    let body = match axum::body::to_bytes(body, lane.max_body()).await {
        Ok(bytes) => bytes,
        // `to_bytes` collapses "over the cap" and "the stream broke" into one
        // error, and the cap is the likelier of the two by far.
        Err(_) => return refuse(&path, WriteRefusal::BodyOverCap),
    };
    // the node key SALTS the signature: a mutation signed for this node cannot
    // be replayed against another node the same key acts on. absent on the
    // embedded daemon (no consensus identity), which binds the empty salt.
    let node_key = handle.admin.node_key.clone().unwrap_or_default();
    let verified = verify_pop(&headers, DATA_HEADERS, DATA_REQ_NS, now_secs(), |ts| {
        request_message(method.as_str(), &path_and_query, &node_key, ts, &body)
    });
    let acting = match verified {
        Ok(key) => key,
        Err(PopError::MissingAuth) => return refuse(&path, WriteRefusal::SignatureMissing),
        Err(PopError::Stale) => return refuse(&path, WriteRefusal::SignatureStale),
        Err(PopError::BadKey) => return refuse(&path, WriteRefusal::SignatureMalformed),
        Err(PopError::BadSig) => return refuse(&path, WriteRefusal::SignatureInvalid),
    };
    let mut req = axum::extract::Request::from_parts(parts, axum::body::Body::from(body));
    req.extensions_mut().insert(SignedBy(acting));
    next.run(req).await
}

/// the write half: one refusal body, the plane's `reason` token, and nothing
/// about the URI.
///
/// LATCHED at `warn`: a refusal is client-driven and any process that can dial
/// the port can mint one per request, so an unconditional line here would evict
/// the 4096-line ring between two useful ones. first occurrence, then every
/// 50th, carrying `occurrences` — the counter is the diagnosis anyway. keyed by
/// the reason token, which comes from a FIXED variant set, so no caller string
/// can vary the key to mint unbounded "first occurrences".
fn refuse(path: &str, refusal: WriteRefusal) -> Response {
    static REFUSED: crate::log::Latch = crate::log::Latch::new(50);
    if let Some(occurrences) = REFUSED.hit(refusal.reason()) {
        match plane_of(path) {
            Plane::Files => tracing::warn!(
                target: "ducktape::files",
                reason = refusal.reason(),
                status = refusal.status().as_u16(),
                occurrences,
                "mutating request refused: unsigned"
            ),
            Plane::Node => tracing::warn!(
                target: "ducktape::node",
                reason = refusal.reason(),
                status = refusal.status().as_u16(),
                occurrences,
                "mutating request refused: unsigned"
            ),
        }
    }
    (
        refusal.status(),
        Json(serde_json::json!({
            "error": refusal.message(),
            "reason": refusal.reason(),
        })),
    )
        .into_response()
}

/// the acting identity a gated handler submits under. `None` is unreachable on
/// a gated route (the guard inserts it or refuses), and the fallback is the
/// node's own name rather than a panic — a handler must never 500 because a
/// route table and this table disagreed.
pub(crate) fn acting_origin(signed: Option<&SignedBy>) -> Vec<u8> {
    match signed {
        Some(SignedBy(key)) => key.clone(),
        None => crate::DEFAULT_ORIGIN.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    fn key(seed: u64) -> ed25519::PrivateKey {
        ed25519::PrivateKey::from_seed(seed)
    }

    const NODE: [u8; 32] = [0xab; 32];

    fn headers_for(signer: &ed25519::PrivateKey, sig: &ed25519::Signature, ts: u64) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            KEY_HEADER,
            duckfs_core::to_hex(signer.public_key().as_ref())
                .parse()
                .unwrap(),
        );
        h.insert(TS_HEADER, ts.to_string().parse().unwrap());
        h.insert(
            SIG_HEADER,
            duckfs_core::to_hex(sig.as_ref()).parse().unwrap(),
        );
        h
    }

    fn verify(
        headers: &HeaderMap,
        method: &str,
        path: &str,
        node: &[u8],
        body: &[u8],
        now: u64,
    ) -> Result<Vec<u8>, PopError> {
        verify_pop(headers, DATA_HEADERS, DATA_REQ_NS, now, |ts| {
            request_message(method, path, node, ts, body)
        })
    }

    #[test]
    fn the_message_binds_method_path_node_time_and_body() {
        // every field moves the signed bytes; none can bleed into another.
        let base = request_message("POST", "/v1/files/commit", &NODE, 100, b"body");
        assert_ne!(
            base,
            request_message("PUT", "/v1/files/commit", &NODE, 100, b"body")
        );
        assert_ne!(
            base,
            request_message("POST", "/v1/files/pin", &NODE, 100, b"body")
        );
        assert_ne!(
            base,
            request_message("POST", "/v1/files/commit", &[0xcd; 32], 100, b"body")
        );
        assert_ne!(
            base,
            request_message("POST", "/v1/files/commit", &NODE, 101, b"body")
        );
        assert_ne!(
            base,
            request_message("POST", "/v1/files/commit", &NODE, 100, b"other")
        );
    }

    #[test]
    fn a_signed_request_verifies_and_a_forged_one_does_not() {
        let caller = key(1);
        let now = 1_000_000;
        let sig = sign_request(&caller, "POST", "/v1/files/stage", &NODE, now, b"chunk");
        let headers = headers_for(&caller, &sig, now);
        assert_eq!(
            verify(&headers, "POST", "/v1/files/stage", &NODE, b"chunk", now),
            Ok(caller.public_key().as_ref().to_vec())
        );
        // the attacker signs, but claims the caller's key.
        let attacker = key(2);
        let forged = sign_request(&attacker, "POST", "/v1/files/stage", &NODE, now, b"chunk");
        let mut bad = headers_for(&caller, &sig, now);
        bad.insert(
            SIG_HEADER,
            duckfs_core::to_hex(forged.as_ref()).parse().unwrap(),
        );
        assert_eq!(
            verify(&bad, "POST", "/v1/files/stage", &NODE, b"chunk", now),
            Err(PopError::BadSig)
        );
    }

    /// THE reason the data plane hashes the body: a signature that authenticates
    /// the caller must not leave the bytes swappable.
    #[test]
    fn a_swapped_body_is_rejected() {
        let caller = key(3);
        let now = 2_000_000;
        let sig = sign_request(
            &caller,
            "POST",
            "/v1/files/stage",
            &NODE,
            now,
            b"the real chunk",
        );
        let headers = headers_for(&caller, &sig, now);
        assert_eq!(
            verify(
                &headers,
                "POST",
                "/v1/files/stage",
                &NODE,
                b"a swapped chunk",
                now
            ),
            Err(PopError::BadSig)
        );
        assert!(
            verify(
                &headers,
                "POST",
                "/v1/files/stage",
                &NODE,
                b"the real chunk",
                now
            )
            .is_ok()
        );
    }

    /// the cross-node replay: captured traffic for node X, aimed at node Y.
    #[test]
    fn a_signature_bound_to_another_node_is_rejected() {
        let caller = key(4);
        let now = 3_000_000;
        let sig = sign_request(&caller, "POST", "/v1/invite", &NODE, now, b"{}");
        let headers = headers_for(&caller, &sig, now);
        assert_eq!(
            verify(&headers, "POST", "/v1/invite", &[0xcd; 32], b"{}", now),
            Err(PopError::BadSig)
        );
        assert!(verify(&headers, "POST", "/v1/invite", &NODE, b"{}", now).is_ok());
    }

    #[test]
    fn a_stale_timestamp_is_rejected_both_directions() {
        let caller = key(5);
        let signed_at = 5_000_000;
        let sig = sign_request(
            &caller,
            "POST",
            "/v1/log-filter",
            &NODE,
            signed_at,
            b"debug",
        );
        let headers = headers_for(&caller, &sig, signed_at);
        let at = |now| verify(&headers, "POST", "/v1/log-filter", &NODE, b"debug", now);
        assert_eq!(at(signed_at + FRESHNESS_SECS + 1), Err(PopError::Stale));
        assert_eq!(at(signed_at - FRESHNESS_SECS - 1), Err(PopError::Stale));
        assert!(at(signed_at + FRESHNESS_SECS).is_ok());
    }

    #[test]
    fn missing_headers_are_missing_auth_not_a_crash() {
        assert_eq!(
            verify(&HeaderMap::new(), "POST", "/v1/invite", &NODE, b"", 1),
            Err(PopError::MissingAuth)
        );
    }

    /// the whole table, in one place: every mutating `/v1` route, and the reads
    /// that must not be dragged in with them. the pairs that matter most are the
    /// ones that share a path with their own read — `/v1/files/blob` (POST
    /// writes, GET fetches), the object facade (PUT/DELETE write, GET reads) and
    /// `/v1/submit` vs `/v1/submit/frame`.
    #[test]
    fn the_gate_covers_every_mutating_route() {
        let gated: &[(Method, &str)] = &[
            (Method::POST, "/v1/log-filter"),
            (Method::POST, "/v1/invite"),
            (Method::POST, "/v1/submit"),
            (Method::POST, "/v1/fs/workspaces"),
            (Method::POST, "/v1/fs/workspaces/abc/commit"),
            (Method::DELETE, "/v1/fs/workspaces/abc"),
            (Method::POST, "/v1/files/blob"),
            (Method::POST, "/v1/files/stage"),
            (Method::POST, "/v1/files/commit"),
            (Method::POST, "/v1/files/pin"),
            (Method::POST, "/v1/files/watch"),
            (Method::PUT, "/v1/files/object/shared/a.txt"),
            (Method::DELETE, "/v1/files/object/shared/a.txt"),
            (Method::POST, "/v1/term/sessions"),
            (Method::POST, "/v1/term/sessions/abc/close"),
        ];
        for (method, path) in gated {
            assert!(
                needs_signature(method, path),
                "{method} {path} must be gated"
            );
        }
        let open: &[(Method, &str)] = &[
            (Method::GET, "/v1/status"),
            (Method::GET, "/v1/files/blob/aa"),
            (Method::GET, "/v1/files/ls"),
            (Method::GET, "/v1/files/object/shared/a.txt"),
            (Method::POST, "/v1/query"),
            (Method::POST, "/v1/index/chat/view"),
            (Method::POST, "/v1/gateway/proxy"),
            // self-authenticating: the frame carries its own signature.
            (Method::POST, "/v1/submit/frame"),
            (Method::POST, "/v1/services/hello"),
            (Method::GET, "/v1/ws"),
            // git cannot attach a header, so the forge's proof is git's own
            // push certificate — refused in `git_http`, not here.
            (Method::POST, "/forge/lab/git-receive-pack"),
        ];
        for (method, path) in open {
            assert!(
                !needs_signature(method, path),
                "{method} {path} must stay open"
            );
        }
    }

    /// the hashing cap is reached by an UNAUTHENTICATED caller, so no lane may
    /// inherit a wider one than its own route accepts — a shared 64 MiB ceiling
    /// would let anyone make the node hold 64 MiB for a 1 MiB chunk route.
    #[test]
    fn no_lane_buffers_more_than_its_own_route_accepts() {
        let cap = |path: &str| lane_of(path).max_body();
        assert_eq!(cap("/v1/files/object/shared/a.bin"), crate::MAX_OBJECT_BYTES);
        assert_eq!(cap("/v1/files/stage"), crate::MAX_BLOB_BODY_BYTES);
        assert_eq!(cap("/v1/submit"), DEFAULT_JSON_BODY_BYTES);
        assert_eq!(cap("/v1/fs/workspaces"), DEFAULT_JSON_BODY_BYTES);
        assert_eq!(cap("/v1/term/sessions"), DEFAULT_JSON_BODY_BYTES);
        assert!(cap("/v1/files/stage") < cap("/v1/files/object/shared/a.bin"));
    }

    #[test]
    fn a_refusal_names_one_stable_reason_per_variant() {
        let all = [
            WriteRefusal::SignatureMissing,
            WriteRefusal::SignatureStale,
            WriteRefusal::SignatureMalformed,
            WriteRefusal::SignatureInvalid,
            WriteRefusal::BodyOverCap,
        ];
        let mut reasons: Vec<&str> = all.iter().map(|r| r.reason()).collect();
        reasons.sort_unstable();
        let unique = reasons.len();
        reasons.dedup();
        assert_eq!(reasons.len(), unique, "two refusals share a reason token");
        assert!(all.iter().all(|r| !r.status().is_success()));
    }

    #[test]
    fn hex_roundtrips_and_rejects_malformed() {
        assert_eq!(from_hex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(from_hex(""), Some(vec![]));
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }
}
