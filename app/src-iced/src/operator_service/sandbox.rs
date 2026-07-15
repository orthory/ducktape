use super::*;

pub(super) async fn load(
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

pub(super) async fn apply(
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

pub(super) async fn start_setup(
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
