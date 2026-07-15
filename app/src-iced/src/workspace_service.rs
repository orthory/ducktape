//! Backend/transport edge for the transport-free workspace screens.

use serde_json::Value;

use crate::backend::{Backend, Workspace as BackendWorkspace};
use crate::screens::workspace::{
    BootErrorKind, BootFailure, Command, LogTail, Phase, PhaseReport, ServiceEvent, Workspace,
};
use crate::transport::NodeClient;

const INCOMPATIBLE_MARKER: &str = "DUCKTAPE_STATE_SCHEMA_INCOMPATIBLE";

pub async fn execute(backend: Option<Backend>, command: Command) -> ServiceEvent {
    let Some(backend) = backend else {
        return error_event(command, "desktop backend is unavailable".into());
    };
    match command {
        Command::LoadWorkspaces => ServiceEvent::WorkspacesLoaded(
            backend
                .list_workspaces()
                .await
                .map(|items| items.into_iter().map(workspace).collect()),
        ),
        Command::LoadJoinCode => ServiceEvent::JoinCodeLoaded(backend.workspace_join_code().await),
        Command::CreateWorkspace { name } => {
            ServiceEvent::WorkspaceCreated(backend.create_workspace(name).await.map(workspace))
        }
        Command::JoinWorkspace { name, invite } => {
            ServiceEvent::WorkspaceJoined(backend.join_workspace(name, invite).await.map(workspace))
        }
        Command::ConnectRemote { url } => {
            let result = match NodeClient::new(&url) {
                Ok(client) => client
                    .status()
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            ServiceEvent::RemoteConnected(result)
        }
        Command::ActivateWorkspace { workspace_id } | Command::RetryWorkspace { workspace_id } => {
            ServiceEvent::WorkspaceActivated {
                workspace_id: workspace_id.clone(),
                result: backend
                    .activate_workspace(workspace_id)
                    .await
                    .map(|_| ())
                    .map_err(boot_failure),
            }
        }
        Command::PollPhase {
            workspace_id,
            delay_ms,
        } => {
            if delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
            ServiceEvent::PhaseLoaded {
                workspace_id: workspace_id.clone(),
                result: backend.workspace_phase(workspace_id).await.and_then(phase),
            }
        }
        Command::CheckJoinReady { workspace_id } => {
            match join_ready(&backend, &workspace_id).await {
                Ok(ready) => ServiceEvent::JoinReady {
                    workspace_id,
                    ready,
                },
                Err(error) => ServiceEvent::PhaseLoaded {
                    workspace_id,
                    result: Ok(PhaseReport {
                        phase: Phase::Fatal,
                        detail: Some(error),
                    }),
                },
            }
        }
        Command::LoadLog { workspace_id } => ServiceEvent::LogLoaded {
            workspace_id: workspace_id.clone(),
            result: backend
                .workspace_log_tail(workspace_id)
                .await
                .map(|log| LogTail {
                    path: log.path,
                    tail: log.tail,
                }),
        },
        Command::CancelJoin { workspace_id } => {
            let result = backend.stop_workspace_node(workspace_id).await;
            ServiceEvent::WorkspacesLoaded(match result {
                Ok(_) => backend
                    .list_workspaces()
                    .await
                    .map(|items| items.into_iter().map(workspace).collect()),
                Err(error) => Err(error),
            })
        }
        Command::ForgetWorkspace {
            workspace_id,
            force,
        } => ServiceEvent::WorkspaceForgot {
            workspace_id: workspace_id.clone(),
            force,
            result: backend
                .forget_workspace(workspace_id, force)
                .await
                .map(|next| next.map(workspace)),
        },
        Command::CopyText(_)
        | Command::ClearCopiedAfter { .. }
        | Command::Connected(_)
        | Command::Dismiss => unreachable!("shell-owned workspace effect"),
    }
}

fn error_event(command: Command, error: String) -> ServiceEvent {
    match command {
        Command::LoadJoinCode => ServiceEvent::JoinCodeLoaded(Err(error)),
        Command::CreateWorkspace { .. } => ServiceEvent::WorkspaceCreated(Err(error)),
        Command::JoinWorkspace { .. } => ServiceEvent::WorkspaceJoined(Err(error)),
        Command::ConnectRemote { .. } => ServiceEvent::RemoteConnected(Err(error)),
        Command::ActivateWorkspace { workspace_id } | Command::RetryWorkspace { workspace_id } => {
            ServiceEvent::WorkspaceActivated {
                workspace_id,
                result: Err(boot_failure(error)),
            }
        }
        Command::PollPhase { workspace_id, .. } | Command::CheckJoinReady { workspace_id } => {
            ServiceEvent::PhaseLoaded {
                workspace_id,
                result: Err(error),
            }
        }
        Command::LoadLog { workspace_id } => ServiceEvent::LogLoaded {
            workspace_id,
            result: Err(error),
        },
        Command::ForgetWorkspace {
            workspace_id,
            force,
        } => ServiceEvent::WorkspaceForgot {
            workspace_id,
            force,
            result: Err(error),
        },
        _ => ServiceEvent::WorkspacesLoaded(Err(error)),
    }
}

async fn join_ready(backend: &Backend, id: &str) -> Result<bool, String> {
    let target = backend
        .list_workspaces()
        .await?
        .into_iter()
        .find(|workspace| workspace.id == id)
        .ok_or_else(|| "workspace no longer exists".to_string())?;
    let client = NodeClient::local(target.ports.http).map_err(|error| error.to_string())?;
    let status = match client.status().await {
        Ok(status) => status,
        Err(_) => return Ok(false),
    };
    if let Some(got) = status.public_key
        && !target.pubkey.is_empty()
        && !got.eq_ignore_ascii_case(&target.pubkey)
    {
        return Err(
            "the process answering this workspace port reports a different node identity".into(),
        );
    }
    for query in ["validators", "residents"] {
        let reply = match client.query("valset", Value::String(query.into())).await {
            Ok(reply) => reply,
            Err(_) => return Ok(false),
        };
        if reply_keys(&reply, query)
            .iter()
            .any(|key| key.eq_ignore_ascii_case(&target.pubkey))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn reply_keys(reply: &Value, variant: &str) -> Vec<String> {
    reply
        .get(variant)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|key| {
            key.as_array()?
                .iter()
                .map(|byte| u8::try_from(byte.as_u64()?).ok())
                .collect::<Option<Vec<_>>>()
        })
        .map(|key| key.into_iter().map(|byte| format!("{byte:02x}")).collect())
        .collect()
}

fn workspace(item: BackendWorkspace) -> Workspace {
    Workspace {
        id: item.id,
        name: item.name,
        chain_id: item.chain_id,
        pubkey: item.pubkey,
        member: item.member,
    }
}

fn phase(report: crate::backend::WorkspacePhaseReport) -> Result<PhaseReport, String> {
    let phase = match report.phase.as_str() {
        "starting" => Phase::Starting,
        "parked" => Phase::Parked,
        "admitted" => Phase::Admitted,
        "synced" => Phase::Synced,
        "promoted" => Phase::Promoted,
        "fatal" => Phase::Fatal,
        _ => return Err("node reported an unknown join phase".into()),
    };
    Ok(PhaseReport {
        phase,
        detail: report.detail,
    })
}

fn boot_failure(reason: String) -> BootFailure {
    BootFailure {
        kind: if reason.contains(INCOMPATIBLE_MARKER) {
            BootErrorKind::IncompatibleWorkspace
        } else {
            BootErrorKind::StartupFailure
        },
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valset_keys_decode_strict_byte_arrays() {
        let reply = serde_json::json!({"validators": [[0, 1, 254, 255], [256], "bad"]});
        assert_eq!(reply_keys(&reply, "validators"), ["0001feff"]);
    }

    #[test]
    fn phase_wire_is_closed() {
        assert!(
            phase(crate::backend::WorkspacePhaseReport {
                phase: "future".into(),
                detail: None,
            })
            .is_err()
        );
    }
}
