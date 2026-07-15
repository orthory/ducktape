//! Backend/transport edge for the presentation-only operator and settings screens.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::backend::{
    Backend, ContentTarget, IdentityStatus, SandboxChoice, Workspace, private_fs,
};
use crate::screens::{operator, settings};
use crate::theme;
use crate::transport::{FileEntry, ModuleStatus, NodeClient, NodeStatus, SubmitReceipt};

const PREFERENCES_FILE: &str = "iced-preferences.json";
const MAX_MUTED_CHANNELS: usize = 256;
const MAX_CHANNEL_BYTES: usize = 128;
const DEFAULT_VOTING_PERIOD: u64 = 1_000_000;

static PREFERENCES_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static METRICS_PREVIOUS: OnceLock<Mutex<BTreeMap<String, TimedMetrics>>> = OnceLock::new();

#[derive(Debug, Clone, Default)]
struct TimedMetrics {
    time_ms: u64,
    blocks_total: u64,
    planes: BTreeMap<(String, String), (u64, u64)>,
    sync_bytes: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default)]
struct ParsedMetrics {
    present: bool,
    block_height: u64,
    blocks_total: u64,
    connected_peers: usize,
    accepted: u64,
    rejected: u64,
    buckets: Vec<(f64, u64)>,
    latency_count: u64,
    planes: BTreeMap<(String, String), ParsedPlane>,
    sync_peers: BTreeMap<String, ParsedSyncPeer>,
}

#[derive(Debug, Clone, Default)]
struct ParsedPlane {
    service: String,
    owner: String,
    age_seconds: f64,
    halted: bool,
    tx_bytes: u64,
    rx_bytes: u64,
    drops: u64,
}

#[derive(Debug, Clone, Default)]
struct ParsedSyncPeer {
    peer: String,
    age_seconds: f64,
    bytes_tx: u64,
    frames: u64,
    boundary_height: Option<u64>,
    served_height: Option<u64>,
    requests: BTreeMap<String, u64>,
    last_kind: Option<String>,
}

/// Settings-only facts that do not belong to the daemon or workspace registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsContext {
    pub active_channel: Option<String>,
    pub forget_needs_force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPreferences {
    pub mode: theme::Mode,
    pub accent: usize,
    pub notifications: settings::NotificationPrefs,
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            mode: theme::Mode::Light,
            accent: 0,
            notifications: settings::NotificationPrefs::default(),
        }
    }
}

/// Execute one operator-screen effect. Shell-owned effects are rejected here so
/// callers cannot accidentally report a platform action as completed.
pub async fn execute_operator(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    command: operator::Command,
) -> operator::ServiceEvent {
    use operator::{Command, Screen, ServiceEvent};

    match command {
        Command::LoadNode => ServiceEvent::NodeLoaded(
            load_node(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::LoadGateway => ServiceEvent::GatewayLoaded(
            load_gateway(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::LoadGatewayRoute(key) => ServiceEvent::GatewayRouteLoaded(
            load_gateway_route(node.as_ref(), workspace.as_ref(), &key).await,
        ),
        Command::LoadModules => {
            ServiceEvent::ModulesLoaded(load_modules(node.as_ref(), workspace.as_ref()).await)
        }
        Command::StartNode => ServiceEvent::ActionFinished {
            screen: Screen::Node,
            result: managed_node_action(backend.as_ref(), workspace.as_ref(), true).await,
        },
        Command::StopNode => ServiceEvent::ActionFinished {
            screen: Screen::Node,
            result: managed_node_action(backend.as_ref(), workspace.as_ref(), false).await,
        },
        Command::PauseMetrics(_) => ServiceEvent::ActionFinished {
            screen: Screen::Metrics,
            result: Ok(()),
        },
        Command::LoadMetrics => {
            ServiceEvent::MetricsLoaded(load_metrics(node.as_ref(), workspace.as_ref()).await)
        }
        Command::LoadSandbox | Command::CheckSandbox => ServiceEvent::SandboxLoaded(
            load_sandbox(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::ApplySandbox(mode) => ServiceEvent::ActionFinished {
            screen: Screen::Sandbox,
            result: apply_sandbox(backend.as_ref(), workspace.as_ref(), mode).await,
        },
        Command::StartSandboxSetup { check, agent } => ServiceEvent::ActionFinished {
            screen: Screen::Sandbox,
            result: start_sandbox_setup(node.as_ref(), workspace.as_ref(), &check, &agent).await,
        },
        Command::SaveGatewayRoute(draft) => ServiceEvent::ActionFinished {
            screen: Screen::Gateway,
            result: save_gateway_route(backend.as_ref(), node.as_ref(), workspace.as_ref(), draft)
                .await,
        },
        Command::RemoveGatewayRoute(key) => ServiceEvent::ActionFinished {
            screen: Screen::Gateway,
            result: remove_gateway_route(backend.as_ref(), node.as_ref(), workspace.as_ref(), &key)
                .await,
        },
        Command::CreateGatewayStarter(draft) => ServiceEvent::ActionFinished {
            screen: Screen::Gateway,
            result: create_gateway_starter(
                backend.as_ref(),
                node.as_ref(),
                workspace.as_ref(),
                &draft,
            )
            .await,
        },
        Command::CheckGatewayHealth(key) => ServiceEvent::GatewayHealthChecked(
            check_gateway_health(node.as_ref(), workspace.as_ref(), &key).await,
        ),
        Command::CopyText(_) => ServiceEvent::ActionFinished {
            screen: Screen::Node,
            result: Err("clipboard effects must be handled by the desktop shell".into()),
        },
    }
}

/// Execute one settings-screen effect.
pub async fn execute_settings(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    context: SettingsContext,
    command: settings::Command,
) -> settings::ServiceEvent {
    use settings::{Command, ServiceEvent};

    match command {
        Command::Load => {
            ServiceEvent::Loaded(load_settings(node.as_ref(), workspace.as_ref(), context).await)
        }
        Command::SetTheme(mode) => {
            ServiceEvent::PreferencesSaved(update_preferences(|preferences| {
                preferences.mode = mode
            }))
        }
        Command::SetAccent(accent) => ServiceEvent::PreferencesSaved(if accent < 5 {
            update_preferences(|preferences| preferences.accent = accent)
        } else {
            Err("accent index is outside the supported palette".into())
        }),
        Command::SetNotifications(notifications) => ServiceEvent::PreferencesSaved(
            validate_notifications(&notifications)
                .and_then(|()| update_preferences(|prefs| prefs.notifications = notifications)),
        ),
        Command::RequestLeave => ServiceEvent::DangerFinished(
            request_leave(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::ForgetWorkspace { force } => ServiceEvent::DangerFinished(
            forget_workspace(backend.as_ref(), workspace.as_ref(), force).await,
        ),
        Command::OpenAccount | Command::OpenNetworks | Command::OpenMembers | Command::OpenNode => {
            ServiceEvent::PreferencesSaved(Err(
                "settings navigation must be handled by the desktop shell".into(),
            ))
        }
    }
}

pub fn load_preferences() -> Result<DesktopPreferences, String> {
    load_preferences_at(&preferences_path()?)
}

async fn load_node(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<operator::NodeSnapshot>, String> {
    let owned_client = local_client(node, workspace)?;
    let client = node.or(owned_client.as_ref());
    if client.is_none() && workspace.is_none() {
        return Ok(None);
    }

    let status = match client {
        Some(client) => client.status().await.ok(),
        None => None,
    };
    if let (Some(status), Some(workspace)) = (&status, workspace) {
        validate_node_identity(status, workspace)?;
    }
    let modules = status
        .as_ref()
        .map(|status| status.modules.iter().map(module_root).collect())
        .unwrap_or_default();
    let peer = status
        .as_ref()
        .and_then(|status| status.public_key.clone())
        .or_else(|| workspace.map(|workspace| workspace.pubkey.clone()))
        .unwrap_or_default();
    let validators = match (client, status.is_some()) {
        (Some(client), true) => query_keys(client, "validators").await.ok(),
        _ => None,
    };
    let validator_count = validators.as_ref().map(Vec::len).unwrap_or(usize::from(
        workspace.is_some_and(|workspace| workspace.member),
    ));
    let connections = validators
        .unwrap_or_default()
        .into_iter()
        .filter(|candidate| !candidate.eq_ignore_ascii_case(&peer))
        .map(|candidate| operator::ConnectionRow {
            peer: candidate,
            direction: "validator".into(),
            state: "committed".into(),
            age: "current set".into(),
        })
        .collect();
    let logs = match (backend, workspace) {
        (Some(backend), Some(workspace)) => backend
            .workspace_log_tail(workspace.id.clone())
            .await
            .map(|tail| parse_logs(&tail.tail))
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    Ok(Some(operator::NodeSnapshot {
        connected: status.is_some(),
        managed: workspace.is_some(),
        workspace_name: workspace
            .map(|workspace| workspace.name.clone())
            .unwrap_or_else(|| "Remote node".into()),
        role: role(workspace),
        peer,
        version: status
            .as_ref()
            .map(|status| status.version.clone())
            .unwrap_or_else(|| "—".into()),
        height: status.as_ref().map_or(0, |status| status.height),
        app_hash: status
            .as_ref()
            .map(|status| status.app_hash.clone())
            .unwrap_or_else(|| "—".into()),
        modules,
        validator_count,
        connections,
        logs,
        blocks_per_second: None,
        apply_p95_ms: None,
    }))
}

async fn load_modules(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<Vec<operator::ModuleRoot>>, String> {
    let owned_client = local_client(node, workspace)?;
    let Some(client) = node.or(owned_client.as_ref()) else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    Ok(Some(status.modules.iter().map(module_root).collect()))
}

async fn load_gateway(
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

async fn load_gateway_route(
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

async fn managed_node_action(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    start: bool,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = workspace.ok_or_else(|| "no managed workspace is active".to_string())?;
    if start {
        backend.start_workspace_node(workspace.id.clone()).await?;
    } else {
        backend.stop_workspace_node(workspace.id.clone()).await?;
    }
    Ok(())
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

async fn save_gateway_route(
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

async fn remove_gateway_route(
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

async fn create_gateway_starter(
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

async fn load_metrics(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<operator::MetricsSnapshot>, String> {
    let owned_client = local_client(node, workspace)?;
    let Some(client) = node.or(owned_client.as_ref()) else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    let text = client
        .metrics_text()
        .await
        .map_err(|error| error.to_string())?;
    let time_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?
        .as_millis()
        .min(u64::MAX as u128) as u64;
    let parsed = parse_metrics(&text)?;
    if !parsed.present {
        return Err("the connected node does not expose Ducktape metrics".into());
    }
    let key = client.cache_key();
    let mut cache = METRICS_PREVIOUS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| "metrics sample cache is unavailable".to_string())?;
    let previous = cache.get(&key);
    let snapshot = metrics_snapshot(&parsed, time_ms, previous);
    cache.insert(key, timed_metrics(&parsed, time_ms));
    Ok(Some(snapshot))
}

async fn check_gateway_health(
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

async fn load_sandbox(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<operator::SandboxData>, String> {
    let Some(workspace) = workspace else {
        return Ok(None);
    };
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let preflight = backend.sandbox_preflight(workspace.id.clone()).await?;
    let owned_client = local_client(node, Some(workspace))?;
    let client = node.or(owned_client.as_ref());
    if let Some(client) = client {
        let status = client.status().await.map_err(|error| error.to_string())?;
        validate_node_identity(&status, workspace)?;
    }
    let (active_agents, active_channel) = match client {
        Some(client) => (
            active_agents(client).await.unwrap_or_default(),
            active_channel(client).await.unwrap_or(None),
        ),
        None => (Vec::new(), None),
    };
    let state = |probe: &Option<crate::backend::ProbeResult>| match probe {
        Some(probe) if probe.ok => operator::CheckState::Ok,
        Some(_) => operator::CheckState::Failed,
        None => operator::CheckState::Unknown,
    };
    let detail = |probe: &Option<crate::backend::ProbeResult>, fallback: &str| {
        probe
            .as_ref()
            .map_or_else(|| fallback.to_string(), |probe| probe.detail.clone())
    };
    let backend_state = state(&preflight.backend_binary);
    let image_state = state(&preflight.base_image);
    let cgroup_state = state(&preflight.cgroup_delegation);
    let checks = vec![
        operator::SandboxCheck {
            id: "backend".into(),
            label: format!(
                "{} binary installed",
                if preflight.backend.is_empty() {
                    "podman"
                } else {
                    &preflight.backend
                }
            ),
            detail: detail(&preflight.backend_binary, "run preflight on the node host"),
            state: backend_state,
            fixable: backend_state == operator::CheckState::Failed,
        },
        operator::SandboxCheck {
            id: "image".into(),
            label: format!("base image {} pulled", preflight.image),
            detail: detail(
                &preflight.base_image,
                if preflight.os == "macos" {
                    "tart uses VM base images"
                } else {
                    "run preflight on the node host"
                },
            ),
            state: image_state,
            fixable: image_state == operator::CheckState::Failed,
        },
        operator::SandboxCheck {
            id: "cgroup".into(),
            label: "cgroup v2 cpu + memory delegation".into(),
            detail: detail(
                &preflight.cgroup_delegation,
                if preflight.os == "linux" {
                    "run preflight on the node host"
                } else {
                    "not applicable on this OS"
                },
            ),
            state: cgroup_state,
            fixable: false,
        },
    ];
    let current_mode = if !preflight.announce_capabilities {
        operator::SandboxMode::Off
    } else if preflight.mode == "tart" {
        operator::SandboxMode::Tart
    } else {
        operator::SandboxMode::Podman
    };
    let mut available_modes = vec![operator::SandboxMode::Off, operator::SandboxMode::Podman];
    if preflight.os == "macos" {
        available_modes.push(operator::SandboxMode::Tart);
    }
    Ok(Some(operator::SandboxData {
        can_control: true,
        backend: preflight.backend,
        os: preflight.os,
        current_mode,
        available_modes,
        serving: preflight.announce_capabilities,
        checks,
        active_agents,
        active_channel,
    }))
}

async fn apply_sandbox(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    mode: operator::SandboxMode,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = workspace.ok_or_else(|| "no managed workspace is active".to_string())?;
    backend
        .apply_workspace_sandbox(
            workspace.id.clone(),
            match mode {
                operator::SandboxMode::Off => SandboxChoice::Off,
                operator::SandboxMode::Podman => SandboxChoice::Podman,
                operator::SandboxMode::Tart => SandboxChoice::Tart,
            },
        )
        .await
}

async fn active_agents(client: &NodeClient) -> Result<Vec<(String, String)>, String> {
    let reply = client
        .query("agent", Value::String("agents".into()))
        .await
        .map_err(|error| error.to_string())?;
    let mut agents = variant_array(&reply, "agents")?
        .iter()
        .filter(|agent| agent.get("status").and_then(Value::as_str) == Some("active"))
        .map(|agent| {
            let id = agent
                .get("agent_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= 128)
                .ok_or_else(|| "agent roster contains an invalid id".to_string())?;
            let name = agent
                .get("display_name")
                .and_then(Value::as_str)
                .filter(|name| name.len() <= 256)
                .unwrap_or(id);
            Ok((id.to_string(), name.to_string()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    agents.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
    Ok(agents)
}

async fn active_channel(client: &NodeClient) -> Result<Option<String>, String> {
    let reply = client
        .query("chat", Value::String("channels".into()))
        .await
        .map_err(|error| error.to_string())?;
    let channels = variant_array(&reply, "channels")?;
    let candidate = channels
        .iter()
        .filter(|channel| channel.get("archived").and_then(Value::as_bool) != Some(true))
        .filter_map(|channel| channel.get("id").and_then(Value::as_str))
        .filter(|id| !id.contains(':') && !id.is_empty() && id.len() <= 128)
        .min_by_key(|id| if *id == "general" { 0 } else { 1 });
    Ok(candidate.map(str::to_owned))
}

async fn start_sandbox_setup(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    check: &str,
    agent: &str,
) -> Result<(), String> {
    if !matches!(check, "backend" | "image") {
        return Err("this sandbox check is not agent-fixable".into());
    }
    if agent.is_empty() || agent.len() > 128 {
        return Err("sandbox setup agent id is invalid".into());
    }
    let owned_client = local_client(node, workspace)?;
    let client = node
        .or(owned_client.as_ref())
        .ok_or_else(|| "connect a node before starting sandbox setup".to_string())?;
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    if !active_agents(client)
        .await?
        .iter()
        .any(|(id, _)| id == agent)
    {
        return Err("the selected setup agent is no longer active".into());
    }
    let channel = active_channel(client)
        .await?
        .ok_or_else(|| "create or open a chat channel before starting setup".to_string())?;
    let mode = if cfg!(target_os = "macos") {
        "tart"
    } else {
        "podman"
    };
    let prompt = if mode == "tart" {
        "Install and configure the tart sandbox backend for this Ducktape node: install tart and sshpass (Apple Silicon), create/pull the base VM image, verify `tart run` boots it and SSH reaches the guest, report results."
    } else {
        "Install and configure the podman sandbox backend for this Ducktape node: install rootless podman, pull docker.io/library/node:22-slim, verify cgroup v2 cpu delegation, report results."
    };
    let origin = status
        .public_key
        .as_deref()
        .ok_or_else(|| "this node does not report its public key".to_string())?;
    let message_id = random_message_id()?;
    client
        .submit(
            "chat",
            json!({ "post_message": {
                "channel_id": channel,
                "message_id": message_id,
                "blocks": [{ "paragraph": [{ "text": prompt, "marks": [] }] }],
                "thread": null,
                "as_agent": null,
            }}),
            Some(origin),
        )
        .await
        .map_err(|error| error.to_string())?;
    let reply = client
        .query(
            "chat",
            json!({ "messages_latest": { "channel_id": channel, "limit": 1 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let latest = variant_array(&reply, "messages")?
        .first()
        .ok_or_else(|| "the setup prompt committed but could not be anchored".to_string())?;
    if latest
        .get("head")
        .and_then(|head| head.get("message_id"))
        .and_then(Value::as_str)
        != Some(message_id.as_str())
    {
        return Err(
            "the setup prompt committed but another message won the channel head; retry".into(),
        );
    }
    let seq = latest
        .get("seq")
        .and_then(Value::as_u64)
        .filter(|seq| *seq > 0)
        .ok_or_else(|| "the setup prompt has no committed sequence".to_string())?;
    client
        .submit(
            "runs",
            json!({ "request_run": { "agent_id": agent, "channel_id": channel, "anchor_seq": seq } }),
            Some(origin),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn random_message_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|_| "could not mint a message id".to_string())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes_hex(&bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

fn parse_metrics(text: &str) -> Result<ParsedMetrics, String> {
    let mut metrics = ParsedMetrics::default();
    for line in text.lines() {
        let Some((name, labels, value)) = parse_metric_line(line) else {
            continue;
        };
        match name {
            "ducktape_block_height" => {
                metrics.present = true;
                metrics.block_height = metric_u64(value);
            }
            "ducktape_blocks_total" => {
                metrics.present = true;
                metrics.blocks_total = metric_u64(value);
            }
            "ducktape_consensus_reachable_validators" => {
                metrics.present = true;
                metrics.connected_peers = metric_u64(value).min(usize::MAX as u64) as usize;
            }
            "ducktape_ops_outcome_total" => match labels.get("outcome").map(String::as_str) {
                Some("applied") => metrics.accepted = metric_u64(value),
                Some("rejected") => metrics.rejected = metric_u64(value),
                _ => {}
            },
            "ducktape_block_apply_latency_seconds_count" => {
                metrics.present = true;
                metrics.latency_count = metric_u64(value);
            }
            "ducktape_block_apply_latency_seconds_bucket" => {
                metrics.present = true;
                let le = match labels.get("le").map(String::as_str) {
                    Some("+Inf") => f64::INFINITY,
                    Some(le) => le.parse().unwrap_or(f64::NAN),
                    None => f64::NAN,
                };
                if le.is_finite() || le == f64::INFINITY {
                    metrics.buckets.push((le, metric_u64(value)));
                }
            }
            name if name.starts_with("ducktape_dataplane_") => {
                let service = labels.get("service").cloned().unwrap_or_else(|| "?".into());
                let owner = labels.get("owner").cloned().unwrap_or_else(|| "?".into());
                let plane = metrics
                    .planes
                    .entry((service.clone(), owner.clone()))
                    .or_insert_with(|| ParsedPlane {
                        service,
                        owner,
                        ..ParsedPlane::default()
                    });
                match name {
                    "ducktape_dataplane_halted" => plane.halted = value > 0.0,
                    "ducktape_dataplane_age_seconds" => plane.age_seconds = value.max(0.0),
                    "ducktape_dataplane_bytes" => match labels.get("dir").map(String::as_str) {
                        Some("tx") => {
                            plane.tx_bytes = plane.tx_bytes.saturating_add(metric_u64(value))
                        }
                        Some("rx") => {
                            plane.rx_bytes = plane.rx_bytes.saturating_add(metric_u64(value))
                        }
                        _ => {}
                    },
                    "ducktape_dataplane_drops" => {
                        plane.drops = plane.drops.saturating_add(metric_u64(value));
                    }
                    _ => {}
                }
            }
            name if name.starts_with("ducktape_statesync_serve_") => {
                let peer = labels.get("peer").cloned().unwrap_or_else(|| "?".into());
                let sync =
                    metrics
                        .sync_peers
                        .entry(peer.clone())
                        .or_insert_with(|| ParsedSyncPeer {
                            peer,
                            ..ParsedSyncPeer::default()
                        });
                match name {
                    "ducktape_statesync_serve_age_seconds" => sync.age_seconds = value.max(0.0),
                    "ducktape_statesync_serve_bytes" => sync.bytes_tx = metric_u64(value),
                    "ducktape_statesync_serve_frames" => sync.frames = metric_u64(value),
                    "ducktape_statesync_serve_boundary_height" => {
                        sync.boundary_height = Some(metric_u64(value))
                    }
                    "ducktape_statesync_serve_frame_height" => {
                        sync.served_height = Some(metric_u64(value))
                    }
                    "ducktape_statesync_serve_requests" => {
                        sync.requests.insert(
                            labels.get("kind").cloned().unwrap_or_else(|| "?".into()),
                            metric_u64(value),
                        );
                    }
                    "ducktape_statesync_serve_last_request" => {
                        sync.last_kind = labels.get("kind").cloned()
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    metrics
        .buckets
        .sort_by(|left, right| left.0.total_cmp(&right.0));
    Ok(metrics)
}

fn parse_metric_line(line: &str) -> Option<(&str, BTreeMap<String, String>, f64)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.len() > 16 * 1024 {
        return None;
    }
    let (head, rest) = if let Some(open) = line.find('{') {
        let close = line[open + 1..].find('}')? + open + 1;
        (&line[..open], (&line[open + 1..close], &line[close + 1..]))
    } else {
        let split = line.find(char::is_whitespace)?;
        (&line[..split], ("", &line[split..]))
    };
    if !head.starts_with("ducktape_") {
        return None;
    }
    let labels = rest
        .0
        .split(',')
        .filter_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            let value = value.strip_prefix('"')?.strip_suffix('"')?;
            (key.len() <= 64 && value.len() <= 256).then(|| (key.into(), value.into()))
        })
        .collect();
    let token = rest.1.split_whitespace().next()?;
    let value = token.parse::<f64>().ok()?;
    value.is_finite().then_some((head, labels, value))
}

fn metric_u64(value: f64) -> u64 {
    if value <= 0.0 {
        0
    } else if value >= u64::MAX as f64 {
        u64::MAX
    } else {
        value as u64
    }
}

fn histogram_quantile(metrics: &ParsedMetrics, q: f64) -> f64 {
    if metrics.latency_count == 0 || metrics.buckets.is_empty() {
        return 0.0;
    }
    let rank = q.clamp(0.0, 1.0) * metrics.latency_count as f64;
    let (mut previous_le, mut previous_count) = (0.0, 0_u64);
    for &(le, cumulative) in &metrics.buckets {
        if cumulative as f64 >= rank {
            if le == f64::INFINITY {
                return previous_le;
            }
            let within = cumulative.saturating_sub(previous_count);
            return if within == 0 {
                previous_le
            } else {
                previous_le + (le - previous_le) * (rank - previous_count as f64) / within as f64
            };
        }
        if le.is_finite() {
            previous_le = le;
        }
        previous_count = cumulative;
    }
    previous_le
}

fn rate(previous: u64, current: u64, elapsed_ms: u64) -> f64 {
    if elapsed_ms == 0 || current < previous {
        0.0
    } else {
        (current - previous) as f64 * 1000.0 / elapsed_ms as f64
    }
}

fn format_age(seconds: f64) -> String {
    let seconds = metric_u64(seconds);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86_400 {
        format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60)
    } else {
        format!("{}d {}h", seconds / 86_400, seconds % 86_400 / 3_600)
    }
}

fn timed_metrics(metrics: &ParsedMetrics, time_ms: u64) -> TimedMetrics {
    TimedMetrics {
        time_ms,
        blocks_total: metrics.blocks_total,
        planes: metrics
            .planes
            .iter()
            .map(|(key, plane)| (key.clone(), (plane.tx_bytes, plane.rx_bytes)))
            .collect(),
        sync_bytes: metrics
            .sync_peers
            .iter()
            .map(|(peer, sync)| (peer.clone(), sync.bytes_tx))
            .collect(),
    }
}

fn metrics_snapshot(
    metrics: &ParsedMetrics,
    time_ms: u64,
    previous: Option<&TimedMetrics>,
) -> operator::MetricsSnapshot {
    let elapsed = previous.map_or(0, |previous| time_ms.saturating_sub(previous.time_ms));
    let blocks_per_second = previous.map_or(0.0, |previous| {
        rate(previous.blocks_total, metrics.blocks_total, elapsed)
    });
    let data_planes = metrics
        .planes
        .iter()
        .map(|(key, plane)| {
            let prior = previous
                .and_then(|sample| sample.planes.get(key))
                .copied()
                .unwrap_or((plane.tx_bytes, plane.rx_bytes));
            operator::DataPlaneMetric {
                service: plane.service.clone(),
                owner: plane.owner.clone(),
                age: format_age(plane.age_seconds),
                tx_bytes_per_second: rate(prior.0, plane.tx_bytes, elapsed),
                rx_bytes_per_second: rate(prior.1, plane.rx_bytes, elapsed),
                total_bytes: plane.tx_bytes.saturating_add(plane.rx_bytes),
                dropped: plane.drops,
                halted: plane.halted,
            }
        })
        .collect();
    let sync_peers = metrics
        .sync_peers
        .iter()
        .map(|(peer, sync)| {
            let prior = previous
                .and_then(|sample| sample.sync_bytes.get(peer))
                .copied()
                .unwrap_or(sync.bytes_tx);
            let reach = sync.served_height.or(sync.boundary_height);
            let parked = sync.last_kind.as_deref() == Some("tip_coords") && reach.is_some();
            let blocks_left = (!parked)
                .then(|| reach.map(|height| metrics.block_height.saturating_sub(height)))
                .flatten();
            let progress = (!parked && metrics.block_height > 0)
                .then(|| {
                    reach
                        .map(|height| (height as f64 / metrics.block_height as f64).min(1.0) as f32)
                })
                .flatten();
            operator::SyncPeerMetric {
                peer: sync.peer.clone(),
                phase: sync_phase(sync),
                age: format_age(sync.age_seconds),
                progress,
                blocks_left,
                tx_bytes_per_second: rate(prior, sync.bytes_tx, elapsed),
                total_bytes: sync.bytes_tx,
                frames: sync.frames,
            }
        })
        .collect();
    operator::MetricsSnapshot {
        block_height: metrics.block_height,
        connected_peers: metrics.connected_peers,
        blocks_per_second,
        apply_p50_ms: histogram_quantile(metrics, 0.5) * 1000.0,
        apply_p95_ms: histogram_quantile(metrics, 0.95) * 1000.0,
        accepted: metrics.accepted,
        rejected: metrics.rejected,
        data_planes,
        sync_peers,
        sampled_at: format!("{time_ms} ms"),
    }
}

fn sync_phase(peer: &ParsedSyncPeer) -> String {
    match peer.last_kind.as_deref() {
        Some("manifest") => "manifest served",
        Some("chunk" | "module" | "index_chunk" | "index_modules") => "restoring snapshot",
        Some("frames") => "replaying frames",
        Some("tip_coords") if peer.served_height.or(peer.boundary_height).is_some() => "parked",
        Some("tip_coords") => "polling tip",
        Some("blob") => "fetching blobs",
        Some(kind) => kind,
        None if peer.served_height.is_some() => "replaying frames",
        None if ["chunk", "module", "index_chunk", "index_modules"]
            .iter()
            .any(|kind| peer.requests.get(*kind).copied().unwrap_or(0) > 0) =>
        {
            "restoring snapshot"
        }
        None if peer.boundary_height.is_some() => "manifest served",
        None if peer.requests.get("tip_coords").copied().unwrap_or(0) > 0 => "polling tip",
        None => "fetching blobs",
    }
    .into()
}

async fn load_settings(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    context: SettingsContext,
) -> Result<Option<settings::SettingsData>, String> {
    if node.is_none() && workspace.is_none() {
        return Ok(None);
    }
    let owned_client = local_client(node, workspace)?;
    let client = node.or(owned_client.as_ref());
    let validators = match client {
        Some(client) => match client.status().await {
            Ok(status) => {
                if let Some(workspace) = workspace {
                    validate_node_identity(&status, workspace)?;
                }
                query_keys(client, "validators").await.ok()
            }
            Err(_) => None,
        },
        None => None,
    };
    let roster_loaded = validators.is_some();
    let validator_count = validators.as_ref().map(Vec::len).unwrap_or(usize::from(
        workspace.is_some_and(|workspace| workspace.member),
    ));
    let in_validator_set = validators.as_ref().map_or_else(
        || workspace.is_some_and(|workspace| workspace.member),
        |validators| {
            workspace.is_some_and(|workspace| {
                validators
                    .iter()
                    .any(|key| key.eq_ignore_ascii_case(&workspace.pubkey))
            })
        },
    );
    Ok(Some(settings::SettingsData {
        client_mode: workspace.is_none(),
        can_control_node: workspace.is_some(),
        workspace_name: workspace.map(|workspace| workspace.name.clone()),
        network_id: workspace.map(|workspace| workspace.chain_id.clone()),
        active_channel: context.active_channel,
        in_validator_set,
        validator_count,
        roster_loaded,
        forget_needs_force: context.forget_needs_force,
    }))
}

async fn forget_workspace(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    force: bool,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = workspace.ok_or_else(|| "no managed workspace is active".to_string())?;
    backend
        .forget_workspace(workspace.id.clone(), force)
        .await
        .map(|_| ())
}

async fn request_leave(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = workspace.ok_or_else(|| "no managed workspace is active".to_string())?;
    let owned_client = local_client(node, Some(workspace))?;
    let client = node
        .or(owned_client.as_ref())
        .ok_or_else(|| "the managed node is unavailable".to_string())?;
    let status = client.status().await.map_err(|error| error.to_string())?;
    validate_node_identity(&status, workspace)?;
    let key = decode_key(&workspace.pubkey)?;
    let validators = query_keys(client, "validators").await?;
    if !validators
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&workspace.pubkey))
    {
        return Err("this node is not in the current validator set".into());
    }
    if validators.len() < 2 {
        return Err("a solo node cannot remove the last validator; forget it instead".into());
    }

    let action = json!({ "remove_validator": { "key": key } });
    let mut proposals = governance_proposals(client).await?;
    let existing = proposals.iter().find(|proposal| {
        proposal.get("status").and_then(Value::as_str) == Some("open")
            && proposal.get("action") == Some(&action)
    });
    let proposal_id = existing
        .and_then(|proposal| proposal.get("proposal_id").and_then(Value::as_str))
        .map(str::to_owned)
        .unwrap_or_else(|| mint_proposal_id(&proposals, &workspace.pubkey));
    if existing.is_none() {
        submit_governance(
            backend,
            client,
            json!({
                "propose": {
                    "proposal_id": proposal_id.clone(),
                    "action": action,
                    "voting_period": DEFAULT_VOTING_PERIOD
                }
            }),
        )
        .await?;
    }
    submit_governance(
        backend,
        client,
        json!({ "vote": { "proposal_id": proposal_id.clone(), "approve": true } }),
    )
    .await?;
    proposals = governance_proposals(client).await?;
    let voted = proposal(&proposals, &proposal_id)?;
    if voted.get("status").and_then(Value::as_str) == Some("open")
        && can_settle_early(voted, validators.len())?
    {
        submit_governance(
            backend,
            client,
            json!({ "execute": { "proposal_id": proposal_id.clone() } }),
        )
        .await?;
        proposals = governance_proposals(client).await?;
    }
    match proposal(&proposals, &proposal_id)?
        .get("status")
        .and_then(Value::as_str)
    {
        Some("passed") => Ok(()),
        Some("rejected") => Err(format!(
            "the membership proposal was rejected ({proposal_id})"
        )),
        _ => {
            let (yes, _) = tally(proposal(&proposals, &proposal_id)?)?;
            let required =
                decision_threshold(proposal(&proposals, &proposal_id)?, validators.len())?;
            Err(format!(
                "ballot cast — {yes} of {required} required approvals; waiting on the other validators ({proposal_id})"
            ))
        }
    }
}

pub(crate) async fn submit_governance(
    backend: &Backend,
    client: &NodeClient,
    payload: Value,
) -> Result<SubmitReceipt, String> {
    let bytes = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
    let frame = backend
        .sign_content_frame(ContentTarget::Governance, hex(&bytes))
        .await
        .map_err(|error| {
            if error == "identity-locked" {
                "unlock your identity to submit governance operations".into()
            } else {
                error
            }
        })?;
    client
        .submit_frame(decode_hex(&frame)?)
        .await
        .map_err(|error| error.to_string())
}

async fn governance_proposals(client: &NodeClient) -> Result<Vec<Value>, String> {
    let reply = client
        .query("governance", Value::String("proposals".into()))
        .await
        .map_err(|error| error.to_string())?;
    Ok(variant_array(&reply, "proposals")?.to_vec())
}

fn proposal<'a>(proposals: &'a [Value], proposal_id: &str) -> Result<&'a Value, String> {
    proposals
        .iter()
        .find(|proposal| proposal.get("proposal_id").and_then(Value::as_str) == Some(proposal_id))
        .ok_or_else(|| format!("proposal {proposal_id} disappeared"))
}

fn mint_proposal_id(proposals: &[Value], subject: &str) -> String {
    let taken: HashSet<&str> = proposals
        .iter()
        .filter_map(|proposal| proposal.get("proposal_id").and_then(Value::as_str))
        .collect();
    let head = format!("leave:{}:", &subject[..subject.len().min(16)]);
    (0..)
        .map(|index| format!("{head}{index}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .expect("an unbounded sequence contains an unused proposal id")
}

fn can_settle_early(proposal: &Value, member_count: usize) -> Result<bool, String> {
    let (yes, total) = tally(proposal)?;
    match proposal.get("voting_rule") {
        Some(Value::String(rule)) if rule == "dynamic_validator_majority" => {
            Ok(yes > member_count as u64 / 2)
        }
        Some(Value::Object(rule)) if rule.contains_key("threshold") => Ok(yes
            >= rule["threshold"]["required_yes"]
                .as_u64()
                .ok_or_else(|| "governance threshold is invalid".to_string())?),
        Some(Value::Object(rule)) if rule.contains_key("participating_majority") => {
            let quorum = rule["participating_majority"]["quorum"]
                .as_u64()
                .ok_or_else(|| "governance quorum is invalid".to_string())?;
            let remaining = electorate_power(proposal, member_count)?
                .checked_sub(yes)
                .ok_or_else(|| "governance vote power exceeds the electorate".to_string())?;
            Ok(total >= quorum && yes > remaining)
        }
        None => Ok(yes > member_count as u64 / 2),
        _ => Err("governance voting rule is invalid".into()),
    }
}

fn decision_threshold(proposal: &Value, member_count: usize) -> Result<u64, String> {
    match proposal.get("voting_rule") {
        Some(Value::String(rule)) if rule == "dynamic_validator_majority" => {
            Ok(member_count as u64 / 2 + 1)
        }
        Some(Value::Object(rule)) if rule.contains_key("threshold") => {
            rule["threshold"]["required_yes"]
                .as_u64()
                .ok_or_else(|| "governance threshold is invalid".into())
        }
        Some(Value::Object(rule)) if rule.contains_key("participating_majority") => {
            rule["participating_majority"]["quorum"]
                .as_u64()
                .ok_or_else(|| "governance quorum is invalid".into())
        }
        None => Ok(member_count as u64 / 2 + 1),
        _ => Err("governance voting rule is invalid".into()),
    }
}

fn tally(proposal: &Value) -> Result<(u64, u64), String> {
    let electorate = proposal
        .get("electorate")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance proposal electorate is invalid".to_string())?;
    let powers: BTreeMap<String, u64> = electorate
        .iter()
        .map(|row| {
            let row = row
                .as_array()
                .filter(|row| row.len() == 2)
                .ok_or_else(|| "governance electorate row is invalid".to_string())?;
            Ok((
                bytes_hex(&value_bytes(&row[0])?),
                row[1].as_u64().unwrap_or(0),
            ))
        })
        .collect::<Result<_, String>>()?;
    let votes = proposal
        .get("votes")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance proposal votes are invalid".to_string())?;
    let mut yes = 0_u64;
    let mut total = 0_u64;
    for row in votes {
        let row = row
            .as_array()
            .filter(|row| row.len() == 2)
            .ok_or_else(|| "governance vote row is invalid".to_string())?;
        let power = if powers.is_empty() {
            1
        } else {
            powers
                .get(&bytes_hex(&value_bytes(&row[0])?))
                .copied()
                .unwrap_or(0)
        };
        total = total
            .checked_add(power)
            .ok_or_else(|| "governance vote power overflowed".to_string())?;
        if row[1].as_bool() == Some(true) {
            yes = yes
                .checked_add(power)
                .ok_or_else(|| "governance yes power overflowed".to_string())?;
        }
    }
    Ok((yes, total))
}

fn electorate_power(proposal: &Value, member_count: usize) -> Result<u64, String> {
    let electorate = proposal
        .get("electorate")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance proposal electorate is invalid".to_string())?;
    if electorate.is_empty() {
        return Ok(member_count as u64);
    }
    electorate.iter().try_fold(0_u64, |total, row| {
        row.as_array()
            .and_then(|row| row.get(1))
            .and_then(Value::as_u64)
            .map(|power| total.saturating_add(power))
            .ok_or_else(|| "governance electorate row is invalid".to_string())
    })
}

async fn query_keys(client: &NodeClient, variant: &str) -> Result<Vec<String>, String> {
    let reply = client
        .query("valset", Value::String(variant.into()))
        .await
        .map_err(|error| error.to_string())?;
    variant_array(&reply, variant)?
        .iter()
        .map(|value| value_key(value).map(|bytes| bytes_hex(&bytes)))
        .collect()
}

fn local_client(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<NodeClient>, String> {
    if node.is_some() {
        return Ok(None);
    }
    workspace
        .map(|workspace| NodeClient::local(workspace.ports.http).map_err(|error| error.to_string()))
        .transpose()
}

fn validate_node_identity(status: &NodeStatus, workspace: &Workspace) -> Result<(), String> {
    if status.public_key.as_deref().is_some_and(|key| {
        !workspace.pubkey.is_empty() && !key.eq_ignore_ascii_case(&workspace.pubkey)
    }) {
        return Err(
            "the process answering this workspace port reports a different node identity".into(),
        );
    }
    Ok(())
}

fn role(workspace: Option<&Workspace>) -> operator::NodeRole {
    match workspace {
        Some(workspace) if workspace.founder => operator::NodeRole::GenesisValidator,
        Some(workspace) if workspace.member => operator::NodeRole::MemberValidator,
        None => operator::NodeRole::RemoteUser,
        Some(_) => operator::NodeRole::Guest,
    }
}

fn module_root(module: &ModuleStatus) -> operator::ModuleRoot {
    operator::ModuleRoot {
        id: module.id.clone(),
        root: module.root.clone(),
        category: match module.category.as_deref() {
            Some("workspace") => operator::ModuleCategory::Workspace,
            Some("developer") => operator::ModuleCategory::Developer,
            Some("automation") => operator::ModuleCategory::Automation,
            _ => operator::ModuleCategory::System,
        },
    }
}

fn parse_logs(tail: &str) -> Vec<operator::LogLine> {
    tail.lines()
        .rev()
        .take(1_000)
        .map(|line| {
            let level = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"]
                .into_iter()
                .find(|level| line.contains(level))
                .unwrap_or("OTHER");
            let timestamp = line.split_whitespace().next().unwrap_or_default();
            operator::LogLine {
                timestamp: timestamp.into(),
                level: level.into(),
                target: line
                    .split_whitespace()
                    .find(|word| word.starts_with("ducktape::"))
                    .unwrap_or_default()
                    .trim_end_matches(':')
                    .into(),
                message: line.into(),
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
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

fn variant_array<'a>(value: &'a Value, variant: &str) -> Result<&'a [Value], String> {
    value
        .get(variant)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("unexpected module reply: wanted {variant}"))
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

fn value_bytes(value: &Value) -> Result<Vec<u8>, String> {
    value
        .as_array()
        .ok_or_else(|| "wire key is not a byte array".to_string())?
        .iter()
        .map(|byte| {
            byte.as_u64()
                .filter(|byte| *byte <= u8::MAX as u64)
                .map(|byte| byte as u8)
                .ok_or_else(|| "wire key contains an invalid byte".to_string())
        })
        .collect()
}

fn value_key(value: &Value) -> Result<Vec<u8>, String> {
    let bytes = value_bytes(value)?;
    if bytes.len() != 32 {
        return Err("wire node key is not 32 bytes".into());
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn bytes_hex(bytes: &[u8]) -> String {
    hex(bytes)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("expected an even-length hexadecimal value".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(|| "expected a hexadecimal value".to_string())
        })
        .collect()
}

fn decode_key(value: &str) -> Result<Vec<u8>, String> {
    let bytes = decode_hex(value)?;
    if bytes.len() != 32 {
        return Err("node key is not 32 bytes".into());
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

fn preferences_path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    let home = home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "could not determine the current user's home directory".to_string())?;
    Ok(home.join(".ducktape").join(PREFERENCES_FILE))
}

fn update_preferences(update: impl FnOnce(&mut DesktopPreferences)) -> Result<(), String> {
    let path = preferences_path()?;
    let _guard = PREFERENCES_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "preferences lock is poisoned".to_string())?;
    let mut preferences = load_preferences_at(&path)?;
    update(&mut preferences);
    save_preferences_at(&path, &preferences)
}

fn load_preferences_at(path: &Path) -> Result<DesktopPreferences, String> {
    let text = match private_fs::read_to_string(path)? {
        Some(text) => text,
        None => return Ok(DesktopPreferences::default()),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return Ok(DesktopPreferences::default()),
    };
    let mode = match value.get("theme").and_then(Value::as_str) {
        Some("dark") => theme::Mode::Dark,
        _ => theme::Mode::Light,
    };
    let accent = value
        .get("accent")
        .and_then(Value::as_u64)
        .filter(|accent| *accent < 5)
        .unwrap_or(0) as usize;
    let notifications = value
        .get("notifications")
        .and_then(|value| notifications_from_json(value).ok())
        .unwrap_or_default();
    Ok(DesktopPreferences {
        mode,
        accent,
        notifications,
    })
}

fn save_preferences_at(path: &Path, preferences: &DesktopPreferences) -> Result<(), String> {
    let value = json!({
        "theme": match preferences.mode { theme::Mode::Light => "light", theme::Mode::Dark => "dark" },
        "accent": preferences.accent,
        "notifications": notifications_json(&preferences.notifications),
    });
    private_fs::write_atomic(
        path,
        &serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?,
    )
}

fn validate_notifications(notifications: &settings::NotificationPrefs) -> Result<(), String> {
    if notifications.muted_channels.len() > MAX_MUTED_CHANNELS
        || notifications
            .muted_channels
            .iter()
            .any(|channel| channel.is_empty() || channel.len() > MAX_CHANNEL_BYTES)
    {
        return Err("muted channel preferences exceed the desktop safety limit".into());
    }
    Ok(())
}

fn notifications_json(preferences: &settings::NotificationPrefs) -> Value {
    json!({
        "enabled": preferences.enabled,
        "mentions": preferences.mentions,
        "replies": preferences.replies,
        "huddles": preferences.huddles,
        "runs": preferences.runs,
        "forge": preferences.forge,
        "governance": preferences.governance,
        "mutedChannels": preferences.muted_channels,
    })
}

fn notifications_from_json(value: &Value) -> Result<settings::NotificationPrefs, String> {
    let defaults = settings::NotificationPrefs::default();
    let bool_value =
        |key: &str, fallback: bool| value.get(key).and_then(Value::as_bool).unwrap_or(fallback);
    let muted_channels = value
        .get("mutedChannels")
        .and_then(Value::as_array)
        .map(|channels| {
            channels
                .iter()
                .map(|channel| {
                    channel
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "muted channel preference is not a string".to_string())
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    let preferences = settings::NotificationPrefs {
        enabled: bool_value("enabled", defaults.enabled),
        mentions: bool_value("mentions", defaults.mentions),
        replies: bool_value("replies", defaults.replies),
        huddles: bool_value("huddles", defaults.huddles),
        runs: bool_value("runs", defaults.runs),
        forge: bool_value("forge", defaults.forge),
        governance: bool_value("governance", defaults.governance),
        muted_channels,
    };
    validate_notifications(&preferences)?;
    Ok(preferences)
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
    fn governance_threshold_uses_frozen_power() {
        let proposal = json!({
            "voting_rule": { "participating_majority": { "quorum": 2 } },
            "electorate": [[[1], 2], [[2], 1]],
            "votes": [[[1], true]]
        });
        assert!(can_settle_early(&proposal, 99).unwrap());
        assert_eq!(decision_threshold(&proposal, 99).unwrap(), 2);
    }

    #[test]
    fn preferences_round_trip_and_validate_muted_channels() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("iced-preferences.json");
        let preferences = DesktopPreferences {
            mode: theme::Mode::Dark,
            accent: 4,
            notifications: settings::NotificationPrefs {
                muted_channels: vec!["general".into()],
                ..settings::NotificationPrefs::default()
            },
        };
        save_preferences_at(&path, &preferences).unwrap();
        assert_eq!(load_preferences_at(&path).unwrap(), preferences);
    }

    #[test]
    fn module_categories_are_fail_closed_to_system() {
        let module = ModuleStatus {
            id: "future".into(),
            root: "00".repeat(32),
            category: Some("unknown".into()),
        };
        assert_eq!(
            module_root(&module).category,
            operator::ModuleCategory::System
        );
    }

    #[test]
    fn metrics_parser_derives_histograms_rates_and_planes() {
        let first = parse_metrics(
            r#"
ducktape_block_height 10
ducktape_blocks_total 20
ducktape_consensus_reachable_validators 3
ducktape_ops_outcome_total{outcome="applied"} 8
ducktape_ops_outcome_total{outcome="rejected"} 2
ducktape_block_apply_latency_seconds_count 10
ducktape_block_apply_latency_seconds_bucket{le="0.1"} 5
ducktape_block_apply_latency_seconds_bucket{le="0.5"} 9
ducktape_block_apply_latency_seconds_bucket{le="+Inf"} 10
ducktape_dataplane_open{service="voice",owner="chat"} 1
ducktape_dataplane_age_seconds{service="voice",owner="chat"} 65
ducktape_dataplane_bytes{service="voice",owner="chat",dir="tx",class="stream"} 1000
ducktape_dataplane_bytes{service="voice",owner="chat",dir="rx",class="stream"} 500
ducktape_statesync_serve_age_seconds{peer="abc"} 8
ducktape_statesync_serve_bytes{peer="abc"} 200
ducktape_statesync_serve_frame_height{peer="abc"} 8
ducktape_statesync_serve_last_request{peer="abc",kind="frames"} 1
"#,
        )
        .unwrap();
        let previous = timed_metrics(&first, 1_000);
        let mut second = first.clone();
        second.blocks_total = 24;
        second
            .planes
            .get_mut(&("voice".into(), "chat".into()))
            .unwrap()
            .tx_bytes = 1_500;
        second.sync_peers.get_mut("abc").unwrap().bytes_tx = 500;
        let snapshot = metrics_snapshot(&second, 3_000, Some(&previous));
        assert_eq!(snapshot.block_height, 10);
        assert_eq!(snapshot.connected_peers, 3);
        assert_eq!(snapshot.blocks_per_second, 2.0);
        assert!((snapshot.apply_p50_ms - 100.0).abs() < 0.001);
        assert_eq!(snapshot.data_planes[0].age, "1m 5s");
        assert_eq!(snapshot.data_planes[0].tx_bytes_per_second, 250.0);
        assert_eq!(snapshot.sync_peers[0].phase, "replaying frames");
        assert_eq!(snapshot.sync_peers[0].tx_bytes_per_second, 150.0);
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

    #[tokio::test]
    async fn commands_return_the_screen_specific_error_event() {
        assert!(matches!(
            execute_operator(None, None, None, operator::Command::LoadMetrics).await,
            operator::ServiceEvent::MetricsLoaded(Ok(None))
        ));
        assert!(matches!(
            execute_operator(None, None, None, operator::Command::LoadNode).await,
            operator::ServiceEvent::NodeLoaded(Ok(None))
        ));
        assert!(matches!(
            execute_settings(
                None,
                None,
                None,
                SettingsContext::default(),
                settings::Command::ForgetWorkspace { force: false },
            )
            .await,
            settings::ServiceEvent::DangerFinished(Err(error))
                if error.contains("backend")
        ));
    }
}
