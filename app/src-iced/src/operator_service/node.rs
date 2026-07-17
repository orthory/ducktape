use super::*;

pub(super) async fn load(
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
        .map(|status| status.modules.iter().map(modules::module_root).collect())
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

pub(super) async fn managed_action(
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

fn role(workspace: Option<&Workspace>) -> operator::NodeRole {
    match workspace {
        Some(workspace) if workspace.founder => operator::NodeRole::GenesisValidator,
        Some(workspace) if workspace.member => operator::NodeRole::MemberValidator,
        None => operator::NodeRole::RemoteUser,
        Some(_) => operator::NodeRole::Guest,
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
