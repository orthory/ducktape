//! Managed node-local gateway route bindings.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::Backend;
use super::node_control;
use super::workspace_service::{find_workspace, load_registry, workspace_dir};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayRouteName {
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatewayLocalRoute {
    pub name: GatewayRouteName,
    pub port: u16,
}

impl Backend {
    pub async fn gateway_route_list(&self, id: String) -> Result<Vec<GatewayLocalRoute>, String> {
        let root = self.root.clone();
        self.control
            .run(move || gateway_route_list_blocking(&root, &id))
            .await
    }

    pub async fn gateway_route_bind(
        &self,
        id: String,
        label: Option<String>,
        port: u16,
    ) -> Result<(), String> {
        validate_route_name(label.as_deref())?;
        if port == 0 {
            return Err("gateway loopback port must be 1..65535".into());
        }
        let root = self.root.clone();
        self.control
            .run(move || {
                gateway_route_verb_blocking(&root, &id, "gateway-route-bind", label, Some(port))
            })
            .await
    }

    pub async fn gateway_route_unbind(
        &self,
        id: String,
        label: Option<String>,
    ) -> Result<(), String> {
        validate_route_name(label.as_deref())?;
        let root = self.root.clone();
        self.control
            .run(move || {
                gateway_route_verb_blocking(&root, &id, "gateway-route-unbind", label, None)
            })
            .await
    }
}

fn gateway_route_verb_blocking(
    root: &Path,
    id: &str,
    verb: &str,
    label: Option<String>,
    port: Option<u16>,
) -> Result<(), String> {
    let registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?;
    let dir = workspace_dir(root, &workspace.id)?;
    let mut args = vec![
        verb.to_string(),
        "--workspace".into(),
        dir.display().to_string(),
    ];
    if let Some(label) = label {
        args.extend(["--label".into(), label]);
    }
    if let Some(port) = port {
        args.extend(["--port".into(), port.to_string()]);
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    node_control::run_verb(&refs).map(|_| ())
}

fn gateway_route_list_blocking(root: &Path, id: &str) -> Result<Vec<GatewayLocalRoute>, String> {
    let registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?;
    let dir = workspace_dir(root, &workspace.id)?;
    let output =
        node_control::run_verb(&["gateway-route-list", "--workspace", &dir.to_string_lossy()])?;
    let line = node_control::last_line(&output);
    let routes: Vec<GatewayLocalRoute> = serde_json::from_str(line.trim())
        .map_err(|error| format!("gateway-route-list output is not valid JSON: {error}"))?;
    if routes.len() > 256 {
        return Err("gateway local-route list exceeds the desktop safety limit".into());
    }
    for route in &routes {
        validate_route_name(route.name.label.as_deref())?;
        if route.port == 0 {
            return Err("gateway local-route list contains port zero".into());
        }
    }
    Ok(routes)
}

fn validate_route_name(label: Option<&str>) -> Result<(), String> {
    if let Some(label) = label
        && (label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    {
        return Err("gateway route label is invalid".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_labels_are_dns_safe() {
        assert!(validate_route_name(None).is_ok());
        assert!(validate_route_name(Some("docs-v2")).is_ok());
        assert!(validate_route_name(Some("../docs")).is_err());
        assert!(validate_route_name(Some("Upper")).is_err());
    }
}
