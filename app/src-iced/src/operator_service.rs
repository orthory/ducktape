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

mod gateway;
mod metrics;
mod modules;
mod node;
mod sandbox;
mod settings_adapter;

pub use settings_adapter::{DesktopPreferences, SettingsContext};

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
            node::load(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::LoadGateway => ServiceEvent::GatewayLoaded(
            gateway::load(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::LoadGatewayRoute(key) => ServiceEvent::GatewayRouteLoaded(
            gateway::load_route(node.as_ref(), workspace.as_ref(), &key).await,
        ),
        Command::LoadModules => {
            ServiceEvent::ModulesLoaded(modules::load(node.as_ref(), workspace.as_ref()).await)
        }
        Command::StartNode => ServiceEvent::ActionFinished {
            screen: Screen::Node,
            result: node::managed_action(backend.as_ref(), workspace.as_ref(), true).await,
        },
        Command::StopNode => ServiceEvent::ActionFinished {
            screen: Screen::Node,
            result: node::managed_action(backend.as_ref(), workspace.as_ref(), false).await,
        },
        Command::PauseMetrics(_) => ServiceEvent::ActionFinished {
            screen: Screen::Metrics,
            result: Ok(()),
        },
        Command::LoadMetrics => {
            ServiceEvent::MetricsLoaded(metrics::load(node.as_ref(), workspace.as_ref()).await)
        }
        Command::LoadSandbox | Command::CheckSandbox => ServiceEvent::SandboxLoaded(
            sandbox::load(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::ApplySandbox(mode) => ServiceEvent::ActionFinished {
            screen: Screen::Sandbox,
            result: sandbox::apply(backend.as_ref(), workspace.as_ref(), mode).await,
        },
        Command::StartSandboxSetup { check, agent } => ServiceEvent::ActionFinished {
            screen: Screen::Sandbox,
            result: sandbox::start_setup(node.as_ref(), workspace.as_ref(), &check, &agent).await,
        },
        Command::SaveGatewayRoute(draft) => ServiceEvent::ActionFinished {
            screen: Screen::Gateway,
            result: gateway::save_route(backend.as_ref(), node.as_ref(), workspace.as_ref(), draft)
                .await,
        },
        Command::RemoveGatewayRoute(key) => ServiceEvent::ActionFinished {
            screen: Screen::Gateway,
            result: gateway::remove_route(
                backend.as_ref(),
                node.as_ref(),
                workspace.as_ref(),
                &key,
            )
            .await,
        },
        Command::CreateGatewayStarter(draft) => ServiceEvent::ActionFinished {
            screen: Screen::Gateway,
            result: gateway::create_starter(
                backend.as_ref(),
                node.as_ref(),
                workspace.as_ref(),
                &draft,
            )
            .await,
        },
        Command::CheckGatewayHealth(key) => ServiceEvent::GatewayHealthChecked(
            gateway::check_health(node.as_ref(), workspace.as_ref(), &key).await,
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
    settings_adapter::execute(backend, node, workspace, context, command).await
}

pub fn load_preferences() -> Result<DesktopPreferences, String> {
    settings_adapter::load_preferences()
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

fn variant_array<'a>(value: &'a Value, variant: &str) -> Result<&'a [Value], String> {
    value
        .get(variant)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("unexpected module reply: wanted {variant}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_categories_are_fail_closed_to_system() {
        let module = ModuleStatus {
            id: "future".into(),
            root: "00".repeat(32),
            category: Some("unknown".into()),
        };
        assert_eq!(
            modules::module_root(&module).category,
            operator::ModuleCategory::System
        );
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
