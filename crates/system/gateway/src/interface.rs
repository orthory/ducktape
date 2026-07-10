use serde::{Deserialize, Serialize};

/// Commonware signing namespace for every gateway route mutation.
pub const GATEWAY_ROUTE_NS: &[u8] = b"ducktape-gateway-route-v1";
pub const ROUTE_FORMAT_VERSION: u8 = 1;
pub const MAX_CHAIN_ID_BYTES: usize = 256;
pub const MAX_ACCOUNT_ID_BYTES: usize = 128;
pub const NODE_KEY_BYTES: usize = 32;
pub const ED25519_SIGNATURE_BYTES: usize = 64;
pub const MAX_ROUTE_LABEL_BYTES: usize = 63;
pub const MAX_ROUTES_PER_ACCOUNT: usize = 64;
pub const MAX_AUDIENCE_ACCOUNTS: usize = 32;
pub const MAX_CONTENT_FILES: usize = 128;
pub const MAX_CONTENT_PATH_BYTES: usize = 512;
pub const MAX_CONTENT_SEGMENT_BYTES: usize = 128;
pub const MAX_CONTENT_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_CONTENT_TOTAL_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_REQUEST_BODY_BYTES: u64 = 1024 * 1024;
pub const MAX_RESPONSE_BODY_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_ROUTE_STATEMENT_JSON_BYTES: usize = 256 * 1024;

pub const ALLOWED_CONTENT_MIME_TYPES: &[&str] = &[
    "application/javascript",
    "application/json",
    "application/wasm",
    "font/woff2",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/svg+xml",
    "image/webp",
    "text/css",
    "text/html",
    "text/plain",
];

/// The account apex (`None`) or one DNS-shaped label below it. The account is
/// carried separately so a route name can never cross authority boundaries.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct RouteName {
    pub label: Option<String>,
}

impl RouteName {
    pub const fn apex() -> Self {
        Self { label: None }
    }

    pub fn named(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(label) = &self.label {
            validate_route_label(label)?;
        }
        Ok(())
    }

    /// Stable node-local key. `_apex` cannot collide with a legal DNS label.
    pub fn local_key(&self) -> &str {
        self.label.as_deref().unwrap_or("_apex")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RouteMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl RouteMethod {
    pub const fn as_http_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    pub const fn permits_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}

/// A globally signed audience. Transport peers still have to be admitted by
/// the current network set; this policy then resolves their authenticated node
/// key through Identity before a request reaches the local upstream.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RouteAudience {
    /// Only a node currently bound to the owning account.
    Owner,
    /// Any admitted network node currently bound to an Identity account.
    Network,
    /// Only the sorted, explicit account list. The owner is not implicit.
    Accounts { account_ids: Vec<Vec<u8>> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoutePolicy {
    pub audience: RouteAudience,
    /// Strictly sorted and unique. Unknown/unsafe HTTP methods do not exist in
    /// the wire enum, and each method must be explicitly signed into policy.
    pub methods: Vec<RouteMethod>,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    /// Authorization is never ambient. A caller may forward the explicit
    /// header only when this signed bit is true; Cookie is never supported.
    pub allow_authorization: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentFile {
    pub path: String,
    pub mime: String,
    pub size: u64,
    /// Lowercase SHA-256 of the exact bytes.
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DuckFsContent {
    /// Optional path served for `/`. It must name one declared file.
    pub default_path: Option<String>,
    /// Strictly path-sorted, unique, bounded content declarations.
    pub files: Vec<ContentFile>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RouteTarget {
    /// Exact, hash-bound files under the publisher's DuckFS gateway root.
    DuckFs { content: DuckFsContent },
    /// HTTP semantics to one node-local loopback route. No port or URL is
    /// global state.
    LoopbackHttp,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteTargetKind {
    DuckFs,
    LoopbackHttp,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteDefinition {
    pub target: RouteTarget,
    pub policy: RoutePolicy,
}

/// `route = None` is an authenticated tombstone and still advances the
/// independent per-name revision, preventing replay.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteStatement {
    pub version: u8,
    pub chain_id: String,
    pub account_id: Vec<u8>,
    pub name: RouteName,
    pub publisher_node: Vec<u8>,
    pub revision: u64,
    pub route: Option<RouteDefinition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemberAuthorization {
    pub signer: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteRecord {
    pub statement: RouteStatement,
    pub authorization: MemberAuthorization,
}

/// Bounded management projection of one live route. In particular, DuckFS
/// manifests and signatures remain behind the exact-name `Get` query.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteSummary {
    pub name: RouteName,
    pub publisher_node: Vec<u8>,
    pub revision: u64,
    pub target: RouteTargetKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayMsg {
    SetRoute {
        statement: RouteStatement,
        authorization: MemberAuthorization,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayQuery {
    Get {
        account_id: Vec<u8>,
        name: RouteName,
    },
    /// Every currently published route owned by one account, in canonical
    /// [`RouteName`] order. Tombstones remain queryable through `Get` so a
    /// publisher can continue the revision stream, but are not management
    /// surface entries.
    List { account_id: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayReply {
    /// Boxed only to keep the Rust reply enum bounded after adding the compact
    /// list variant; `Box` is transparent in the JSON wire representation.
    Route(Box<Option<RouteRecord>>),
    Routes(Vec<RouteSummary>),
}

pub fn encode_msg(message: &GatewayMsg) -> Vec<u8> {
    serde_json::to_vec(message).expect("serializable")
}

pub fn decode_msg(bytes: &[u8]) -> Result<GatewayMsg, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn encode_query(query: &GatewayQuery) -> Vec<u8> {
    serde_json::to_vec(query).expect("serializable")
}

pub fn decode_query(bytes: &[u8]) -> Result<GatewayQuery, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn encode_reply(reply: &GatewayReply) -> Vec<u8> {
    serde_json::to_vec(reply).expect("serializable")
}

pub fn decode_reply(bytes: &[u8]) -> Result<GatewayReply, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub fn validate_route_statement(statement: &RouteStatement) -> Result<(), String> {
    if statement.version != ROUTE_FORMAT_VERSION {
        return Err(format!(
            "gateway: unsupported route version {}",
            statement.version
        ));
    }
    if statement.chain_id.is_empty() || statement.chain_id.len() > MAX_CHAIN_ID_BYTES {
        return Err(format!(
            "gateway: chain id must be 1..={MAX_CHAIN_ID_BYTES} bytes"
        ));
    }
    validate_account_id(&statement.account_id)?;
    statement.name.validate()?;
    if statement.publisher_node.len() != NODE_KEY_BYTES {
        return Err(format!(
            "gateway: publisher node must be {NODE_KEY_BYTES} bytes"
        ));
    }
    if statement.revision == 0 {
        return Err("gateway: route revision starts at 1".into());
    }
    if let Some(route) = &statement.route {
        validate_route(route)?;
    }
    if serde_json::to_vec(statement)
        .expect("route statement serializes")
        .len()
        > MAX_ROUTE_STATEMENT_JSON_BYTES
    {
        return Err(format!(
            "gateway: route statement exceeds {MAX_ROUTE_STATEMENT_JSON_BYTES} bytes"
        ));
    }
    Ok(())
}

pub fn validate_authorization(authorization: &MemberAuthorization) -> Result<(), String> {
    if authorization.signer.len() != NODE_KEY_BYTES {
        return Err("gateway: signer must be a 32-byte Ed25519 key".into());
    }
    if authorization.signature.len() != ED25519_SIGNATURE_BYTES {
        return Err(format!(
            "gateway: signature must be {ED25519_SIGNATURE_BYTES} bytes"
        ));
    }
    Ok(())
}

pub fn validate_route_label(label: &str) -> Result<(), String> {
    if label.is_empty() || label.len() > MAX_ROUTE_LABEL_BYTES {
        return Err(format!(
            "gateway: route label must be 1..={MAX_ROUTE_LABEL_BYTES} bytes"
        ));
    }
    if label.starts_with('-')
        || label.ends_with('-')
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!("gateway: invalid route label: {label:?}"));
    }
    Ok(())
}

pub fn validate_account_id(account_id: &[u8]) -> Result<(), String> {
    if account_id.is_empty() || account_id.len() > MAX_ACCOUNT_ID_BYTES {
        return Err(format!(
            "gateway: account id must be 1..={MAX_ACCOUNT_ID_BYTES} bytes"
        ));
    }
    Ok(())
}

pub fn validate_route(route: &RouteDefinition) -> Result<(), String> {
    validate_policy(&route.policy)?;
    match &route.target {
        RouteTarget::DuckFs { content } => {
            validate_content(content)?;
            if route.policy.methods != [RouteMethod::Get, RouteMethod::Head]
                || route.policy.max_request_bytes != 0
                || route.policy.allow_authorization
            {
                return Err(
                    "gateway: content providers require GET+HEAD, no request body, and no Authorization"
                        .into(),
                );
            }
            if content
                .files
                .iter()
                .any(|file| file.size > route.policy.max_response_bytes)
            {
                return Err("gateway: every DuckFS file must fit the signed response cap".into());
            }
        }
        RouteTarget::LoopbackHttp => {}
    }
    Ok(())
}

pub fn validate_policy(policy: &RoutePolicy) -> Result<(), String> {
    if policy.methods.is_empty() {
        return Err("gateway: policy must allow at least one method".into());
    }
    let mut previous = None;
    for method in &policy.methods {
        if previous.is_some_and(|old| old >= method) {
            return Err("gateway: methods must be strictly sorted and unique".into());
        }
        previous = Some(method);
    }
    if policy.max_request_bytes > MAX_REQUEST_BODY_BYTES {
        return Err(format!(
            "gateway: request body cap exceeds {MAX_REQUEST_BODY_BYTES} bytes"
        ));
    }
    if policy.max_response_bytes == 0 || policy.max_response_bytes > MAX_RESPONSE_BODY_BYTES {
        return Err(format!(
            "gateway: response body cap must be 1..={MAX_RESPONSE_BODY_BYTES} bytes"
        ));
    }
    if policy.methods.iter().any(|method| method.permits_body()) && policy.max_request_bytes == 0 {
        return Err("gateway: a body-bearing method requires a non-zero request cap".into());
    }
    match &policy.audience {
        RouteAudience::Owner | RouteAudience::Network => {}
        RouteAudience::Accounts { account_ids } => {
            if account_ids.is_empty() || account_ids.len() > MAX_AUDIENCE_ACCOUNTS {
                return Err(format!(
                    "gateway: explicit audience needs 1..={MAX_AUDIENCE_ACCOUNTS} accounts"
                ));
            }
            let mut previous: Option<&[u8]> = None;
            for account in account_ids {
                validate_account_id(account)?;
                if previous.is_some_and(|old| old >= account.as_slice()) {
                    return Err(
                        "gateway: audience accounts must be strictly sorted and unique".into(),
                    );
                }
                previous = Some(account);
            }
        }
    }
    Ok(())
}

pub fn validate_content(content: &DuckFsContent) -> Result<(), String> {
    if content.files.is_empty() || content.files.len() > MAX_CONTENT_FILES {
        return Err(format!(
            "gateway: DuckFS target needs 1..={MAX_CONTENT_FILES} files"
        ));
    }
    if let Some(path) = &content.default_path {
        validate_content_path(path)?;
    }
    let mut total = 0u64;
    let mut previous: Option<&str> = None;
    let mut default_exists = content.default_path.is_none();
    for file in &content.files {
        validate_content_path(&file.path)?;
        if previous.is_some_and(|old| old >= file.path.as_str()) {
            return Err("gateway: content files must be strictly path-sorted".into());
        }
        previous = Some(&file.path);
        if !ALLOWED_CONTENT_MIME_TYPES.contains(&file.mime.as_str()) {
            return Err(format!("gateway: MIME type is not allowed: {}", file.mime));
        }
        if file.size > MAX_CONTENT_FILE_BYTES {
            return Err(format!(
                "gateway: file {} exceeds {MAX_CONTENT_FILE_BYTES} bytes",
                file.path
            ));
        }
        total = total
            .checked_add(file.size)
            .ok_or_else(|| "gateway: total content size overflows u64".to_string())?;
        if !is_canonical_sha256(&file.sha256) {
            return Err(format!(
                "gateway: file {} has a non-canonical SHA-256",
                file.path
            ));
        }
        if content.default_path.as_deref() == Some(file.path.as_str()) {
            default_exists = true;
        }
    }
    if total > MAX_CONTENT_TOTAL_BYTES {
        return Err(format!(
            "gateway: DuckFS target exceeds {MAX_CONTENT_TOTAL_BYTES} total bytes"
        ));
    }
    if !default_exists {
        return Err("gateway: default_path must name one declared file".into());
    }
    Ok(())
}

pub fn validate_content_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > MAX_CONTENT_PATH_BYTES || path.starts_with('/') {
        return Err(format!(
            "gateway: content path must be a 1..={MAX_CONTENT_PATH_BYTES}-byte relative path"
        ));
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.len() > MAX_CONTENT_SEGMENT_BYTES
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(format!("gateway: non-canonical content path: {path:?}"));
        }
    }
    Ok(())
}

pub fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Deterministic, append-only bytes signed under [`GATEWAY_ROUTE_NS`].
pub fn route_signing_preimage(statement: &RouteStatement) -> Result<Vec<u8>, String> {
    validate_route_statement(statement)?;
    let mut out = Vec::new();
    out.push(statement.version);
    push_bytes(&mut out, statement.chain_id.as_bytes());
    push_bytes(&mut out, &statement.account_id);
    match &statement.name.label {
        None => out.push(0),
        Some(label) => {
            out.push(1);
            push_bytes(&mut out, label.as_bytes());
        }
    }
    push_bytes(&mut out, &statement.publisher_node);
    out.extend_from_slice(&statement.revision.to_le_bytes());
    let Some(route) = &statement.route else {
        out.push(0);
        return Ok(out);
    };
    out.push(1);
    encode_policy(&mut out, &route.policy);
    match &route.target {
        RouteTarget::DuckFs { content } => {
            out.push(1);
            match &content.default_path {
                None => out.push(0),
                Some(path) => {
                    out.push(1);
                    push_bytes(&mut out, path.as_bytes());
                }
            }
            out.extend_from_slice(&(content.files.len() as u64).to_le_bytes());
            for file in &content.files {
                push_bytes(&mut out, file.path.as_bytes());
                push_bytes(&mut out, file.mime.as_bytes());
                out.extend_from_slice(&file.size.to_le_bytes());
                out.extend_from_slice(&decode_lower_hex_32(&file.sha256)?);
            }
        }
        RouteTarget::LoopbackHttp => out.push(2),
    }
    Ok(out)
}

fn encode_policy(out: &mut Vec<u8>, policy: &RoutePolicy) {
    match &policy.audience {
        RouteAudience::Owner => out.push(1),
        RouteAudience::Network => out.push(2),
        RouteAudience::Accounts { account_ids } => {
            out.push(3);
            out.extend_from_slice(&(account_ids.len() as u64).to_le_bytes());
            for account in account_ids {
                push_bytes(out, account);
            }
        }
    }
    out.extend_from_slice(&(policy.methods.len() as u64).to_le_bytes());
    for method in &policy.methods {
        out.push(match method {
            RouteMethod::Get => 1,
            RouteMethod::Head => 2,
            RouteMethod::Post => 3,
            RouteMethod::Put => 4,
            RouteMethod::Patch => 5,
            RouteMethod::Delete => 6,
        });
    }
    out.extend_from_slice(&policy.max_request_bytes.to_le_bytes());
    out.extend_from_slice(&policy.max_response_bytes.to_le_bytes());
    out.push(u8::from(policy.allow_authorization));
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn decode_lower_hex_32(value: &str) -> Result<[u8; 32], String> {
    if !is_canonical_sha256(value) {
        return Err("gateway: SHA-256 must be 64 lowercase hex characters".into());
    }
    let mut out = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(out)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("validated lowercase hex"),
    }
}
