use super::*;

pub(super) async fn load(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<operator::GatewayData>, String> {
    let owned_client = local_client(node, workspace)?;
    let Some(client) = node.or(owned_client.as_ref()) else {
        return Ok(None);
    };
    let (status, account) = gateway_account(client, workspace).await?;
    let Some(account) = account else {
        return Ok(Some(operator::GatewayData {
            routes: Vec::new(),
            handle: None,
            account_bound: false,
            desktop_signer: false,
            managed_workspace: workspace.is_some(),
        }));
    };
    let account_id = account_id(&account)?;
    let account_hex = bytes_hex(&account_id);
    let handle = account_handle(client, &account_id).await.unwrap_or(None);
    let reply = client
        .query("gateway", json!({ "list": { "account_id": account_id } }))
        .await
        .map_err(|error| error.to_string())?;
    let summaries = variant_array(&reply, "routes")?;
    let peer = status.public_key.unwrap_or_default();
    let routes = summaries
        .iter()
        .map(|summary| gateway_summary(summary, handle.as_deref(), &account_hex, &peer))
        .collect::<Result<_, _>>()?;
    let desktop_signer = match backend {
        Some(backend) => backend
            .identity_state()
            .await
            .is_ok_and(|identity| identity.state == IdentityStatus::Unlocked),
        None => false,
    };
    Ok(Some(operator::GatewayData {
        routes,
        handle,
        account_bound: true,
        desktop_signer,
        managed_workspace: workspace.is_some(),
    }))
}

pub(super) async fn load_route(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    key: &str,
) -> Result<operator::GatewayDraft, String> {
    let label = route_label(key)?;
    let owned_client = local_client(node, workspace)?;
    let client = node
        .or(owned_client.as_ref())
        .ok_or_else(|| "connect a node before loading gateway routes".to_string())?;
    let (_, account) = gateway_account(client, workspace).await?;
    let account = account.ok_or_else(|| "bind this node to an account first".to_string())?;
    let account_id = account_id(&account)?;
    let handle = account_handle(client, &account_id).await.unwrap_or(None);
    let reply = client
        .query(
            "gateway",
            json!({ "get": { "account_id": account_id, "name": { "label": label } } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let record = reply
        .get("route")
        .ok_or_else(|| "gateway reply is missing route".to_string())?;
    if record.is_null() {
        return Err("the selected gateway route no longer exists".into());
    }
    gateway_draft(record, handle.as_deref())
}

async fn gateway_account(
    client: &NodeClient,
    workspace: Option<&Workspace>,
) -> Result<(NodeStatus, Option<Value>), String> {
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    let public_key = status
        .public_key
        .as_deref()
        .ok_or_else(|| "this node does not report its public key".to_string())?;
    let reply = client
        .query(
            "identity",
            json!({ "of_node": { "node_key": decode_key(public_key)? } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let account = reply
        .get("account")
        .ok_or_else(|| "identity reply is missing account".to_string())?;
    Ok((status, (!account.is_null()).then(|| account.clone())))
}

async fn account_handle(
    client: &NodeClient,
    wanted_account: &[u8],
) -> Result<Option<String>, String> {
    let reply = client
        .query(
            "duckdns",
            json!({ "registrations": { "from": 0, "limit": 256 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    for row in variant_array(&reply, "registrations")? {
        if account_id(row)? == wanted_account {
            return Ok(row.get("handle").and_then(Value::as_str).map(str::to_owned));
        }
    }
    Ok(None)
}

struct GatewayMutationContext<'a> {
    backend: &'a Backend,
    client: NodeClient,
    workspace: &'a Workspace,
    account_id: Vec<u8>,
    publisher_node: Vec<u8>,
}

async fn gateway_mutation_context<'a>(
    backend: Option<&'a Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&'a Workspace>,
) -> Result<GatewayMutationContext<'a>, String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace =
        workspace.ok_or_else(|| "gateway writes require a managed workspace".to_string())?;
    let client = match node {
        Some(client) => client.clone(),
        None => NodeClient::local(workspace.ports.http).map_err(|error| error.to_string())?,
    };
    let (status, account) = gateway_account(&client, Some(workspace)).await?;
    let account =
        account.ok_or_else(|| "bind this node to an account before publishing".to_string())?;
    let public_key = status
        .public_key
        .as_deref()
        .ok_or_else(|| "this node does not report its public key".to_string())?;
    Ok(GatewayMutationContext {
        backend,
        client,
        workspace,
        account_id: account_id(&account)?,
        publisher_node: decode_key(public_key)?,
    })
}

pub(super) async fn save_route(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    draft: operator::GatewayDraft,
) -> Result<(), String> {
    let context = gateway_mutation_context(backend, node, workspace).await?;
    let label = normalized_route_label(&draft.label)?;
    let previous = gateway_record(&context.client, &context.account_id, label.as_deref()).await?;
    let previous_port =
        local_gateway_port(context.backend, &context.workspace.id, label.as_deref()).await?;
    let revision = next_gateway_revision(previous.as_ref())?;
    let route = match draft.target {
        operator::RouteTarget::DuckFs => {
            let manifest_sha256 = build_content_manifest(
                context.backend,
                &context.client,
                &context.publisher_node,
                label.as_deref(),
                &draft.default_path,
            )
            .await?;
            let response = kibibytes(&draft.response_kib, 64 * 1024, "response cap")?;
            if response == 0 {
                return Err("DuckFS response cap must be greater than zero".into());
            }
            json!({
                "target": { "kind": "duck_fs", "manifest_sha256": manifest_sha256 },
                "policy": {
                    "audience": gateway_audience(&draft)?,
                    "methods": ["get", "head"],
                    "max_request_bytes": 0,
                    "max_response_bytes": response,
                    "allow_authorization": false,
                    "allow_upgrade": false,
                }
            })
        }
        operator::RouteTarget::LoopbackHttp => {
            let methods = gateway_methods(&draft.methods)?;
            let request = kibibytes(&draft.request_kib, 1024, "request cap")?;
            if methods
                .iter()
                .any(|method| matches!(*method, "post" | "put" | "patch" | "delete"))
                && request == 0
            {
                return Err("body-bearing gateway methods require a request cap".into());
            }
            json!({
                "target": { "kind": "loopback_http" },
                "policy": {
                    "audience": gateway_audience(&draft)?,
                    "methods": methods,
                    "max_request_bytes": request,
                    "max_response_bytes": kibibytes(&draft.response_kib, 64 * 1024, "response cap")?,
                    "allow_authorization": draft.allow_authorization,
                    "allow_upgrade": draft.allow_upgrade,
                }
            })
        }
    };
    let statement = gateway_statement(&context, label.as_deref(), revision, Some(route));
    let signed = sign_gateway_statement(context.backend, &statement).await?;

    let wanted_port = if draft.target == operator::RouteTarget::LoopbackHttp {
        Some(
            draft
                .port
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0)
                .ok_or_else(|| "loopback port must be 1..65535".to_string())?,
        )
    } else {
        None
    };
    apply_local_gateway_binding(
        context.backend,
        &context.workspace.id,
        label.clone(),
        wanted_port,
    )
    .await?;
    if let Err(error) = submit_signed_gateway(&context.client, signed).await {
        let recovery = apply_local_gateway_binding(
            context.backend,
            &context.workspace.id,
            label,
            previous_port,
        )
        .await;
        return Err(with_gateway_recovery(error, recovery));
    }
    Ok(())
}

pub(super) async fn remove_route(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    key: &str,
) -> Result<(), String> {
    let context = gateway_mutation_context(backend, node, workspace).await?;
    let label = route_label(key)?.map(str::to_owned);
    let previous = gateway_record(&context.client, &context.account_id, label.as_deref())
        .await?
        .ok_or_else(|| "the selected gateway route no longer exists".to_string())?;
    let revision = next_gateway_revision(Some(&previous))?;
    let previous_port =
        local_gateway_port(context.backend, &context.workspace.id, label.as_deref()).await?;
    let statement = gateway_statement(&context, label.as_deref(), revision, None);
    let signed = sign_gateway_statement(context.backend, &statement).await?;
    apply_local_gateway_binding(context.backend, &context.workspace.id, label.clone(), None)
        .await?;
    if let Err(error) = submit_signed_gateway(&context.client, signed).await {
        let recovery = apply_local_gateway_binding(
            context.backend,
            &context.workspace.id,
            label,
            previous_port,
        )
        .await;
        return Err(with_gateway_recovery(error, recovery));
    }
    Ok(())
}

pub(super) async fn create_starter(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    draft: &operator::GatewayDraft,
) -> Result<(), String> {
    let context = gateway_mutation_context(backend, node, workspace).await?;
    let label = normalized_route_label(&draft.label)?;
    let root = gateway_content_root(&context.publisher_node, label.as_deref());
    let title = draft
        .address
        .split('.')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("Gateway");
    let safe = |value: &str| {
        value
            .chars()
            .filter(|character| !matches!(character, '<' | '>' | '&' | '"'))
            .collect::<String>()
    };
    let title = safe(title);
    let address = safe(if draft.address.is_empty() {
        "this account's optional .duck name"
    } else {
        &draft.address
    });
    let body = format!(r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>{title}</title>
  <style>body{{font:16px system-ui;max-width:720px;margin:12vh auto;padding:24px;color:#242422}}code{{background:#eee;padding:2px 5px;border-radius:4px}}</style>
</head>
<body>
  <h1>{title}</h1>
  <p>This route is published at <code>{address}</code>.</p>
  <p>Static files and same-route API calls share one signed gateway policy.</p>
</body>
</html>"#).into_bytes();
    upload_duckfs_file(
        context.backend,
        &context.client,
        format!("{root}/index.html"),
        body,
        BTreeMap::from([("mime".into(), "text/html".into())]),
        "create gateway starter",
    )
    .await
    .map(|_| ())
}

fn normalized_route_label(label: &str) -> Result<Option<String>, String> {
    let label = label.trim();
    if label.is_empty() {
        Ok(None)
    } else {
        route_label(label).map(|label| label.map(str::to_owned))
    }
}

async fn gateway_record(
    client: &NodeClient,
    account_id: &[u8],
    label: Option<&str>,
) -> Result<Option<Value>, String> {
    let reply = client
        .query(
            "gateway",
            json!({ "get": { "account_id": account_id, "name": { "label": label } } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let record = reply
        .get("route")
        .ok_or_else(|| "gateway reply is missing route".to_string())?;
    Ok((!record.is_null()).then(|| record.clone()))
}

fn next_gateway_revision(record: Option<&Value>) -> Result<u64, String> {
    record
        .map(|record| {
            record
                .get("statement")
                .and_then(|statement| statement.get("revision"))
                .and_then(Value::as_u64)
                .ok_or_else(|| "gateway record has no revision".to_string())?
                .checked_add(1)
                .ok_or_else(|| "gateway route revision overflowed".to_string())
        })
        .unwrap_or(Ok(1))
}

fn gateway_statement(
    context: &GatewayMutationContext<'_>,
    label: Option<&str>,
    revision: u64,
    route: Option<Value>,
) -> Value {
    json!({
        "version": 1,
        "chain_id": context.workspace.chain_id,
        "account_id": context.account_id,
        "name": { "label": label },
        "publisher_node": context.publisher_node,
        "revision": revision,
        "route": route,
    })
}

fn gateway_audience(draft: &operator::GatewayDraft) -> Result<Value, String> {
    Ok(match draft.audience {
        operator::RouteAudience::Network => json!({ "kind": "network" }),
        operator::RouteAudience::Owner => json!({ "kind": "owner" }),
        operator::RouteAudience::Accounts => {
            let mut accounts = draft
                .audience_accounts
                .iter()
                .map(|account| {
                    let bytes = decode_hex(account)?;
                    if !(1..=128).contains(&bytes.len()) {
                        return Err("gateway audience account id has an invalid length".into());
                    }
                    Ok(bytes)
                })
                .collect::<Result<Vec<Vec<u8>>, String>>()?;
            accounts.sort();
            accounts.dedup();
            if accounts.is_empty() || accounts.len() > 32 {
                return Err("gateway explicit audience requires 1..32 accounts".into());
            }
            json!({ "kind": "accounts", "account_ids": accounts })
        }
    })
}

fn gateway_methods(methods: &[operator::RouteMethod]) -> Result<Vec<&'static str>, String> {
    let mut methods = methods.to_vec();
    methods.sort();
    methods.dedup();
    if methods.is_empty() {
        return Err("gateway policy must allow at least one method".into());
    }
    Ok(methods
        .into_iter()
        .map(|method| match method {
            operator::RouteMethod::Get => "get",
            operator::RouteMethod::Head => "head",
            operator::RouteMethod::Post => "post",
            operator::RouteMethod::Put => "put",
            operator::RouteMethod::Patch => "patch",
            operator::RouteMethod::Delete => "delete",
        })
        .collect())
}

fn kibibytes(value: &str, max_kib: u64, label: &str) -> Result<u64, String> {
    let kib = value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{label} must be 0..{max_kib} KiB"))?;
    if kib > max_kib {
        return Err(format!("{label} must be 0..{max_kib} KiB"));
    }
    kib.checked_mul(1024)
        .ok_or_else(|| format!("{label} overflowed"))
}

async fn sign_gateway_statement(backend: &Backend, statement: &Value) -> Result<Value, String> {
    let encoded = serde_json::to_string(statement).map_err(|error| error.to_string())?;
    if encoded.len() > 4 * 1024 {
        return Err("gateway route statement exceeds the signer limit".into());
    }
    let signed = backend.sign_gateway_route(encoded).await.map_err(|error| {
        if error == "identity-locked" {
            "unlock your identity to publish gateway routes".into()
        } else {
            error
        }
    })?;
    let signed: Value = serde_json::from_str(&signed)
        .map_err(|_| "gateway signer returned invalid JSON".to_string())?;
    let set = signed
        .get("set_route")
        .ok_or_else(|| "gateway signer returned an invalid message".to_string())?;
    if set.get("statement") != Some(statement) {
        return Err("gateway signer changed the route statement".into());
    }
    let authorization = set
        .get("authorization")
        .ok_or_else(|| "gateway signer omitted authorization".to_string())?;
    if value_bytes(
        authorization
            .get("signer")
            .ok_or("gateway signer omitted signer")?,
    )?
    .len()
        != 32
        || value_bytes(
            authorization
                .get("signature")
                .ok_or("gateway signer omitted signature")?,
        )?
        .len()
            != 64
    {
        return Err("gateway signer returned an invalid authorization".into());
    }
    Ok(signed)
}

async fn submit_signed_gateway(client: &NodeClient, signed: Value) -> Result<(), String> {
    client
        .submit("gateway", signed, None)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn local_gateway_port(
    backend: &Backend,
    workspace: &str,
    label: Option<&str>,
) -> Result<Option<u16>, String> {
    Ok(backend
        .gateway_route_list(workspace.to_string())
        .await?
        .into_iter()
        .find(|route| route.name.label.as_deref() == label)
        .map(|route| route.port))
}

async fn apply_local_gateway_binding(
    backend: &Backend,
    workspace: &str,
    label: Option<String>,
    port: Option<u16>,
) -> Result<(), String> {
    let current = backend
        .gateway_route_list(workspace.to_string())
        .await?
        .into_iter()
        .find(|route| route.name.label.as_deref() == label.as_deref())
        .map(|route| route.port);
    if current == port {
        return Ok(());
    }
    match port {
        Some(port) => {
            backend
                .gateway_route_bind(workspace.to_string(), label, port)
                .await
        }
        None => {
            backend
                .gateway_route_unbind(workspace.to_string(), label)
                .await
        }
    }
}

fn with_gateway_recovery(error: String, recovery: Result<(), String>) -> String {
    match recovery {
        Ok(()) => {
            format!("gateway publication failed: {error}; the previous local binding was restored")
        }
        Err(recovery) => format!(
            "gateway publication failed: {error}; restoring the previous local binding also failed: {recovery}"
        ),
    }
}

async fn build_content_manifest(
    backend: &Backend,
    client: &NodeClient,
    publisher_node: &[u8],
    label: Option<&str>,
    default_path: &str,
) -> Result<String, String> {
    validate_content_path(default_path)?;
    let root = gateway_content_root(publisher_node, label);
    let snapshot = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head
        .ok_or_else(|| "DuckFS is empty".to_string())?;
    let mut after: Option<String> = None;
    let mut entries = Vec::new();
    loop {
        let reply = client
            .query(
                "files",
                json!({ "find": {
                    "prefix": format!("{root}/"),
                    "snapshot": snapshot,
                    "after": after,
                    "limit": 256,
                }}),
            )
            .await
            .map_err(|error| error.to_string())?;
        let page = reply
            .get("find")
            .ok_or_else(|| "DuckFS find reply is missing".to_string())?;
        let page_entries: Vec<FileEntry> = serde_json::from_value(
            page.get("entries")
                .cloned()
                .ok_or_else(|| "DuckFS find reply has no entries".to_string())?,
        )
        .map_err(|_| "DuckFS find entries are invalid".to_string())?;
        if entries.len().saturating_add(page_entries.len()) > 32_768 {
            return Err("gateway content tree is too large".into());
        }
        entries.extend(page_entries);
        let next = page.get("next").and_then(Value::as_str).map(str::to_owned);
        match next {
            Some(next) if after.as_ref() != Some(&next) && !next.is_empty() => after = Some(next),
            Some(_) => return Err("DuckFS find returned a stalled cursor".into()),
            None => break,
        }
    }
    if entries.iter().any(|entry| entry.kind == "symlink") {
        return Err("gateway content cannot contain symlinks".into());
    }
    let files = entries
        .into_iter()
        .filter(|entry| entry.kind == "file" && !entry.path.ends_with("/.manifest.json"))
        .collect::<Vec<_>>();
    if files.is_empty() || files.len() > 16_384 {
        return Err("gateway content requires 1..16384 files".into());
    }
    let mut total = 0_u64;
    let mut declarations = Vec::with_capacity(files.len());
    for entry in files {
        let relative = entry
            .path
            .strip_prefix(&format!("{root}/"))
            .ok_or_else(|| "gateway content escaped its root".to_string())?;
        validate_content_path(relative)?;
        if entry.size > 64 * 1024 * 1024 {
            return Err(format!("{relative} exceeds the gateway file cap"));
        }
        total = total
            .checked_add(entry.size)
            .filter(|total| *total <= 1024 * 1024 * 1024)
            .ok_or_else(|| "gateway content is too large".to_string())?;
        let bytes = client
            .files_read_exact(&entry.path, &snapshot, entry.size)
            .await
            .map_err(|error| error.to_string())?;
        let mime = entry
            .meta
            .get("mime")
            .filter(|mime| !mime.is_empty() && mime.len() <= 128)
            .cloned()
            .unwrap_or_else(|| mime_for_path(relative).into());
        declarations.push(json!({
            "path": relative,
            "mime": mime,
            "size": entry.size,
            "sha256": sha256_hex(&bytes),
        }));
    }
    declarations.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    if !declarations
        .iter()
        .any(|file| file.get("path").and_then(Value::as_str) == Some(default_path))
    {
        return Err("gateway default path is not present in DuckFS content".into());
    }
    let manifest = serde_json::to_vec(&json!({
        "default_path": default_path,
        "files": declarations,
    }))
    .map_err(|error| error.to_string())?;
    let digest = sha256_hex(&manifest);
    upload_duckfs_file_at(
        backend,
        client,
        format!("{root}/.manifest.json"),
        manifest,
        BTreeMap::from([("mime".into(), "application/json".into())]),
        "gateway: publish route manifest",
        Some(snapshot),
    )
    .await?;
    Ok(digest)
}

fn gateway_content_root(publisher_node: &[u8], label: Option<&str>) -> String {
    format!(
        "/home/ext:{}/.duck/gateway/{}",
        bytes_hex(publisher_node),
        label.unwrap_or("_apex")
    )
}

fn validate_content_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 512 || path.starts_with('/') {
        return Err("gateway content path must be a bounded relative path".into());
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || matches!(segment, "." | "..")
            || segment.len() > 128
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(format!("gateway content path is not canonical: {path}"));
        }
    }
    Ok(())
}

fn mime_for_path(path: &str) -> &'static str {
    let path = path.to_ascii_lowercase();
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

async fn upload_duckfs_file(
    backend: &Backend,
    client: &NodeClient,
    path: String,
    bytes: Vec<u8>,
    meta: BTreeMap<String, String>,
    message: &str,
) -> Result<String, String> {
    let base = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    upload_duckfs_file_at(backend, client, path, bytes, meta, message, base).await
}

async fn upload_duckfs_file_at(
    backend: &Backend,
    client: &NodeClient,
    path: String,
    bytes: Vec<u8>,
    meta: BTreeMap<String, String>,
    message: &str,
    base_snapshot: Option<String>,
) -> Result<String, String> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("DuckFS upload exceeds the file cap".into());
    }
    if message.len() > 4_096 {
        return Err("DuckFS commit message exceeds the limit".into());
    }
    let mut chunks = Vec::new();
    for chunk in bytes.chunks(1024 * 1024) {
        chunks.push(
            client
                .files_stage(chunk.to_vec())
                .await
                .map_err(|error| error.to_string())?,
        );
    }
    let digest = sha256_hex(&bytes);
    let commit = json!({
        "base_snapshot": base_snapshot,
        "message": message,
        "changes": [{ "put": {
            "path": path,
            "exec": false,
            "meta": meta,
            "content": { "chunks": { "size": bytes.len(), "chunks": chunks } },
        }}],
    });
    let payload =
        serde_json::to_vec(&json!({ "commit": commit })).map_err(|error| error.to_string())?;
    let frame = backend
        .sign_content_frame(ContentTarget::Files, hex(&payload))
        .await
        .map_err(|error| {
            if error == "identity-locked" {
                "unlock your identity to write gateway content".into()
            } else {
                error
            }
        })?;
    client
        .submit_frame(decode_hex(&frame)?)
        .await
        .map_err(|error| error.to_string())?;
    Ok(digest)
}

fn sha256_hex(input: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(
                block[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("four bytes"),
            );
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let upper = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(upper)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let lower = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = lower.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

pub(super) async fn check_health(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    key: &str,
) -> Result<operator::RouteHealth, String> {
    let label = route_label(key)?;
    let owned_client = local_client(node, workspace)?;
    let client = node
        .or(owned_client.as_ref())
        .ok_or_else(|| "connect a node before checking gateway health".to_string())?;
    let (_, account) = gateway_account(client, workspace).await?;
    let account = account.ok_or_else(|| "bind this node to an account first".to_string())?;
    let account_id = account_id(&account)?;
    let reply = client
        .query(
            "gateway",
            json!({ "get": { "account_id": account_id, "name": { "label": label } } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let record = reply
        .get("route")
        .filter(|record| !record.is_null())
        .ok_or_else(|| "the selected gateway route no longer exists".to_string())?;
    let statement = record
        .get("statement")
        .ok_or_else(|| "gateway record has no statement".to_string())?;
    let route = statement
        .get("route")
        .filter(|route| !route.is_null())
        .ok_or_else(|| "gateway route is unpublished".to_string())?;
    let methods = route
        .get("policy")
        .and_then(|policy| policy.get("methods"))
        .and_then(Value::as_array)
        .ok_or_else(|| "gateway route methods are invalid".to_string())?;
    if !methods.iter().any(|method| method.as_str() == Some("head")) {
        return Ok(operator::RouteHealth::Disabled);
    }
    let revision = statement
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| "gateway record has no revision".to_string())?;
    let statement_account = statement
        .get("account_id")
        .ok_or_else(|| "gateway record has no account id".to_string())?;
    if value_bytes(statement_account)? != account_id {
        return Err("gateway record account does not match the requested account".into());
    }
    let statement_name = statement
        .get("name")
        .ok_or_else(|| "gateway record has no route name".to_string())?;
    if statement_name.get("label").and_then(Value::as_str) != label
        || (label.is_none() && !statement_name.get("label").is_some_and(Value::is_null))
    {
        return Err("gateway record name does not match the requested route".into());
    }
    let status = client
        .gateway_proxy_status(json!({
            "account_id": statement_account,
            "name": statement_name,
            "revision": revision,
            "method": "head",
            "path_and_query": "/",
            "headers": [],
            "body_len": 0,
        }))
        .await;
    match status {
        Ok(status) if status < 400 => Ok(operator::RouteHealth::Serving(status)),
        Ok(status) if status < 500 => Ok(operator::RouteHealth::Reachable(status)),
        Ok(status) => Ok(operator::RouteHealth::Failing(status)),
        Err(_) => Ok(operator::RouteHealth::Unavailable),
    }
}

fn gateway_summary(
    value: &Value,
    handle: Option<&str>,
    account_hex: &str,
    peer: &str,
) -> Result<operator::GatewayRoute, String> {
    let name = value
        .get("name")
        .and_then(|name| name.get("label"))
        .and_then(Value::as_str);
    let key = name.unwrap_or("_apex").to_owned();
    let address = match handle {
        Some(handle) if name.is_some() => format!("{}.{}.duck", name.unwrap_or_default(), handle),
        Some(handle) => format!("{handle}.duck"),
        None => format!("Account {}", short(account_hex)),
    };
    let publisher = value
        .get("publisher_node")
        .map(value_key)
        .transpose()?
        .map(|bytes| bytes_hex(&bytes))
        .unwrap_or_default();
    Ok(operator::GatewayRoute {
        key,
        label: name.unwrap_or("Account apex").to_owned(),
        address,
        target: match value.get("target").and_then(Value::as_str) {
            Some("duck_fs") => operator::RouteTarget::DuckFs,
            Some("loopback_http") => operator::RouteTarget::LoopbackHttp,
            _ => return Err("gateway route has an unsupported target".into()),
        },
        revision: value
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(|| "gateway route has no revision".to_string())?,
        this_node: publisher.eq_ignore_ascii_case(peer),
    })
}

fn gateway_draft(record: &Value, handle: Option<&str>) -> Result<operator::GatewayDraft, String> {
    let statement = record
        .get("statement")
        .ok_or_else(|| "gateway record has no statement".to_string())?;
    let label = statement
        .get("name")
        .and_then(|name| name.get("label"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let route = statement
        .get("route")
        .filter(|route| !route.is_null())
        .ok_or_else(|| "gateway route is unpublished".to_string())?;
    let policy = route
        .get("policy")
        .ok_or_else(|| "gateway route has no policy".to_string())?;
    let target = route
        .get("target")
        .and_then(|target| target.get("kind"))
        .and_then(Value::as_str);
    let audience = policy
        .get("audience")
        .ok_or_else(|| "gateway route has no audience".to_string())?;
    let audience_kind = audience
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "gateway route audience is invalid".to_string())?;
    let methods = policy
        .get("methods")
        .and_then(Value::as_array)
        .ok_or_else(|| "gateway route methods are invalid".to_string())?
        .iter()
        .map(|method| match method.as_str() {
            Some("get") => Ok(operator::RouteMethod::Get),
            Some("head") => Ok(operator::RouteMethod::Head),
            Some("post") => Ok(operator::RouteMethod::Post),
            Some("put") => Ok(operator::RouteMethod::Put),
            Some("patch") => Ok(operator::RouteMethod::Patch),
            Some("delete") => Ok(operator::RouteMethod::Delete),
            _ => Err("gateway route contains an unsupported method".to_string()),
        })
        .collect::<Result<_, _>>()?;
    let address = match handle {
        Some(handle) if !label.is_empty() => format!("{label}.{handle}.duck"),
        Some(handle) => format!("{handle}.duck"),
        None => "Account ID route".into(),
    };
    Ok(operator::GatewayDraft {
        label,
        address,
        target: match target {
            Some("duck_fs") => operator::RouteTarget::DuckFs,
            Some("loopback_http") => operator::RouteTarget::LoopbackHttp,
            _ => return Err("gateway route has an unsupported target".into()),
        },
        audience: match audience_kind {
            "network" => operator::RouteAudience::Network,
            "owner" => operator::RouteAudience::Owner,
            "accounts" => operator::RouteAudience::Accounts,
            _ => return Err("gateway route has an unsupported audience".into()),
        },
        audience_accounts: audience
            .get("account_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|account| {
                value_bytes(account).and_then(|bytes| {
                    if (1..=128).contains(&bytes.len()) {
                        Ok(bytes_hex(&bytes))
                    } else {
                        Err("gateway audience account id has an invalid length".into())
                    }
                })
            })
            .collect::<Result<_, _>>()?,
        default_path: "index.html".into(),
        // The port is deliberately node-local and absent from the consensus record.
        port: "3000".into(),
        methods,
        request_kib: bytes_as_kib(policy, "max_request_bytes")?,
        response_kib: bytes_as_kib(policy, "max_response_bytes")?,
        allow_authorization: policy
            .get("allow_authorization")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        allow_upgrade: policy
            .get("allow_upgrade")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        revision: statement.get("revision").and_then(Value::as_u64),
    })
}

fn bytes_as_kib(policy: &Value, key: &str) -> Result<String, String> {
    let bytes = policy
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("gateway policy has no {key}"))?;
    if bytes % 1024 != 0 {
        return Err(format!("gateway policy {key} is not whole KiB"));
    }
    Ok((bytes / 1024).to_string())
}

fn route_label(key: &str) -> Result<Option<&str>, String> {
    if key == "_apex" {
        return Ok(None);
    }
    if key.is_empty()
        || key.len() > 63
        || key.starts_with('-')
        || key.ends_with('-')
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("gateway route key is invalid".into());
    }
    Ok(Some(key))
}

fn required_bytes(value: &Value, key: &str) -> Result<Vec<u8>, String> {
    value
        .get(key)
        .ok_or_else(|| format!("wire value is missing {key}"))
        .and_then(value_bytes)
}

fn account_id(value: &Value) -> Result<Vec<u8>, String> {
    let bytes = required_bytes(value, "account_id")?;
    if !(1..=128).contains(&bytes.len()) {
        return Err("identity account id has an invalid length".into());
    }
    Ok(bytes)
}

fn short(value: &str) -> String {
    if value.len() <= 16 {
        value.into()
    } else {
        format!("{}…{}", &value[..8], &value[value.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_wire_mapping_rejects_unknown_methods() {
        let record = json!({
            "statement": {
                "name": { "label": "api" },
                "revision": 3,
                "route": {
                    "target": { "kind": "loopback_http" },
                    "policy": {
                        "audience": { "kind": "network" },
                        "methods": ["get", "brew"],
                        "max_request_bytes": 1024,
                        "max_response_bytes": 4096,
                        "allow_authorization": false,
                        "allow_upgrade": false
                    }
                }
            }
        });
        assert!(gateway_draft(&record, Some("alice")).is_err());
    }

    #[test]
    fn gateway_content_paths_and_hashes_match_wire_contract() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(validate_content_path("assets/app.js").is_ok());
        assert!(validate_content_path("../secret").is_err());
        assert!(validate_content_path("assets//app.js").is_err());
        assert!(normalized_route_label("docs-v2").is_ok());
        assert!(normalized_route_label("Docs").is_err());
    }
}
