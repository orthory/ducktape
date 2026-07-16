use super::*;

use std::collections::{BTreeMap, BTreeSet};

use futures_util::{StreamExt as _, stream};
use sha2::{Digest as _, Sha256};

use crate::screens::agents;

const MAX_AGENTS: usize = 4_096;
const MAX_CHANNELS: usize = 4_096;
const MAX_WATCHES: usize = 4_096;
const MAX_PENDING_RUNS: usize = 4_096;
const MAX_RECENT_RUNS: usize = 100;
const MAX_USAGE_ROWS: usize = 16_384;
const MAX_NAME_BYTES: usize = 512;
const MAX_CAPABILITY_BYTES: usize = 256;
const MAX_RECORD_BYTES: usize = 4 * 1024;
const MAX_SKILLS: usize = 64;
const MAX_CAP_ENTRIES: usize = 256;
const MAX_CAP_ENTRY_BYTES: usize = 1024;
const KNOWN_ACTIONS: [&str; 7] = [
    "chat.post",
    "chat.post_message",
    "tasks.create",
    "tasks.update_status",
    "pages.comment",
    "pages.set_checked",
    "duckfs.write_text",
];
/// Execute one Agents effect with the exact `agent`, `runs`, `chat`,
/// `capability`, `dispatch`, and `saga` read contracts.
pub async fn execute_agents(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    command: agents::Command,
) -> agents::ServiceEvent {
    use agents::{Command, ServiceEvent};

    match command {
        Command::Load => ServiceEvent::Loaded(load_agents(backend.as_ref(), node.as_ref()).await),
        Command::RefreshCapabilities => {
            ServiceEvent::CapabilitiesLoaded(load_capabilities(node.as_ref()).await)
        }
        Command::RegisterAgent {
            display_name,
            agent_id,
            capability,
            allowed_actions,
            caps,
            skills,
        } => ServiceEvent::WriteFinished(
            register_agent(
                backend.as_ref(),
                node.as_ref(),
                display_name,
                agent_id,
                capability,
                allowed_actions,
                caps,
                skills,
            )
            .await,
        ),
        Command::UpdateAgent {
            agent_id,
            display_name,
            capability,
            allowed_actions,
            caps,
            skills,
        } => ServiceEvent::WriteFinished(
            update_agent(
                backend.as_ref(),
                node.as_ref(),
                agent_id,
                display_name,
                capability,
                allowed_actions,
                caps,
                skills,
            )
            .await,
        ),
        Command::PauseAgent(agent_id) => ServiceEvent::WriteFinished(
            agent_status(backend.as_ref(), node.as_ref(), agent_id, false).await,
        ),
        Command::ResumeAgent(agent_id) => ServiceEvent::WriteFinished(
            agent_status(backend.as_ref(), node.as_ref(), agent_id, true).await,
        ),
        Command::WatchChannel { channel_id, policy } => ServiceEvent::WriteFinished(
            watch_channel(backend.as_ref(), node.as_ref(), channel_id, policy).await,
        ),
        Command::UnwatchChannel(channel_id) => ServiceEvent::WriteFinished(
            unwatch_channel(backend.as_ref(), node.as_ref(), channel_id).await,
        ),
        Command::SetJobWorker(enabled) => ServiceEvent::WriteFinished(
            set_job_worker(backend.as_ref(), node.as_ref(), enabled).await,
        ),
        Command::CancelRun(run_id) => {
            ServiceEvent::WriteFinished(cancel_run(backend.as_ref(), node.as_ref(), run_id).await)
        }
        Command::ReassignRun { run_id, attempt } => ServiceEvent::WriteFinished(
            reassign_run(backend.as_ref(), node.as_ref(), run_id, attempt).await,
        ),
        // Copy-to-clipboard is a shell-side runtime action (`execute_agents`
        // intercepts it before the service runs); the service never sees it.
        Command::CopyText(_) => ServiceEvent::WriteFinished(Ok(())),
    }
}

/// One reconnecting WebSocket for every expanded run log. The subscription
/// identity includes the topic set, so collapsing the last pane drops the
/// receiver and cancels the transport task.
pub fn run_output_subscription(
    origin: String,
    mut dispatch_ids: Vec<String>,
) -> iced::Subscription<agents::RunLogEvent> {
    dispatch_ids.retain(|dispatch_id| validate_digest(dispatch_id).is_ok());
    dispatch_ids.sort();
    dispatch_ids.dedup();
    if dispatch_ids.is_empty() {
        return iced::Subscription::none();
    }
    iced::Subscription::run_with((origin, dispatch_ids), run_output_stream)
}

fn run_output_stream(
    source: &(String, Vec<String>),
) -> impl iced::futures::Stream<Item = agents::RunLogEvent> + use<> {
    use iced::futures::SinkExt as _;

    let (origin, dispatch_ids) = source.clone();
    iced::stream::channel(64, async move |mut output| {
        let client = match NodeClient::new(&origin) {
            Ok(client) => client,
            Err(error) => {
                let _ = output
                    .send(agents::RunLogEvent::Unavailable {
                        dispatch_id: None,
                        reason: error.to_string(),
                    })
                    .await;
                return;
            }
        };
        let topics = dispatch_ids
            .iter()
            .map(|dispatch_id| format!("run-output:{dispatch_id}"))
            .collect();
        let mut source = match client.subscribe(topics, BTreeMap::new()) {
            Ok(source) => source,
            Err(error) => {
                let _ = output
                    .send(agents::RunLogEvent::Unavailable {
                        dispatch_id: None,
                        reason: error.to_string(),
                    })
                    .await;
                return;
            }
        };
        while let Some(event) = source.recv().await {
            let event = match event {
                crate::transport::StreamEvent::Connected => agents::RunLogEvent::Connected,
                crate::transport::StreamEvent::Disconnected(reason) => {
                    agents::RunLogEvent::Disconnected(reason)
                }
                crate::transport::StreamEvent::Frame(frame) => match frame {
                    crate::transport::ServerFrame::Tail {
                        topic,
                        cursor,
                        item,
                    } => {
                        let Some(dispatch_id) = topic.strip_prefix("run-output:") else {
                            continue;
                        };
                        let Ok(cursor) = cursor.parse::<u64>() else {
                            continue;
                        };
                        let Some(stream) = item.get("stream").and_then(Value::as_str) else {
                            continue;
                        };
                        let stream = match stream {
                            "stdout" => agents::RunStream::Stdout,
                            "stderr" => agents::RunStream::Stderr,
                            _ => continue,
                        };
                        let Some(text) = item.get("line").and_then(Value::as_str) else {
                            continue;
                        };
                        agents::RunLogEvent::Line {
                            dispatch_id: dispatch_id.to_owned(),
                            cursor,
                            stream,
                            text: text.to_owned(),
                        }
                    }
                    crate::transport::ServerFrame::Lagged { topic, cursor } => {
                        let Some(dispatch_id) = topic.strip_prefix("run-output:") else {
                            continue;
                        };
                        let Ok(cursor) = cursor.parse::<u64>() else {
                            continue;
                        };
                        agents::RunLogEvent::Lagged {
                            dispatch_id: dispatch_id.to_owned(),
                            cursor,
                        }
                    }
                    crate::transport::ServerFrame::Error { topic, detail, .. } => {
                        agents::RunLogEvent::Unavailable {
                            dispatch_id: topic.strip_prefix("run-output:").map(str::to_owned),
                            reason: detail,
                        }
                    }
                    _ => continue,
                },
            };
            if output.send(event).await.is_err() {
                return;
            }
        }
    })
}

async fn load_agents(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
) -> Result<Option<agents::AgentData>, String> {
    let Some(client) = node else {
        return Ok(None);
    };
    let agents_query = client.query("agent", Value::String("agents".into()));
    let capabilities_query = client.query("capability", Value::String("all".into()));
    let channels_query = client.query("chat", Value::String("channels".into()));
    let watches_query = client.query("runs", Value::String("watches".into()));
    let pending_query = client.query("runs", Value::String("pending_runs".into()));
    let recent_query = client.query("runs", Value::String("recent_runs".into()));
    let usage_query = client.view("saga", json!({ "usage": {} }));
    let status_query = client.status();
    let (
        agents_reply,
        capabilities_reply,
        channels_reply,
        watches_reply,
        pending_reply,
        recent_reply,
        usage,
        status,
    ) = tokio::join!(
        agents_query,
        capabilities_query,
        channels_query,
        watches_query,
        pending_query,
        recent_query,
        usage_query,
        status_query,
    );
    let agents_reply = agents_reply.map_err(|error| error.to_string())?;
    let channels_reply = channels_reply.map_err(|error| error.to_string())?;
    let watches_reply = watches_reply.map_err(|error| error.to_string())?;
    let pending_reply = pending_reply.map_err(|error| error.to_string())?;
    let current_height = status.map_err(|error| error.to_string())?.height;
    // The usage index is node-local derived state and may be absent on an old
    // node. Keep that optional read quiet, matching the former UsageCard.
    let usage = usage
        .ok()
        .map(|reply| parse_usage(&reply))
        .transpose()?
        .filter(|usage| usage.requests != 0);
    let identity_key = current_identity_key(backend).await?;
    let (recent_runs, recent_runs_error) = match recent_reply {
        Ok(reply) => match parse_recent_runs(&reply) {
            Ok(runs) => (runs, None),
            Err(error) => (Vec::new(), Some(error)),
        },
        Err(error) => (Vec::new(), Some(error.to_string())),
    };

    let agent_rows = variant_array(&agents_reply, "agents", MAX_AGENTS)?;
    let roster: Vec<_> = agent_rows
        .iter()
        .map(parse_agent_record)
        .collect::<Result<_, _>>()?;
    let (capabilities, capability_status) = match capabilities_reply {
        Ok(reply) => match parse_capabilities(&reply) {
            Ok(capabilities) => (capabilities, agents::CapabilityStatus::Ready),
            Err(_) => (Vec::new(), agents::CapabilityStatus::Error),
        },
        Err(_) => (Vec::new(), agents::CapabilityStatus::Error),
    };
    let channels = parse_channels(&channels_reply)?;
    let watches = parse_watches(&watches_reply)?;
    let pending_rows = variant_array(&pending_reply, "pending_runs", MAX_PENDING_RUNS)?;
    let mut parsed_pending: Vec<_> = pending_rows
        .iter()
        .map(parse_pending_wire)
        .collect::<Result<_, _>>()?;
    parsed_pending.sort_by_key(|pending| std::cmp::Reverse(pending.created_at));
    let dispatch_ids = parsed_pending
        .iter()
        .map(|pending| pending.dispatch_id.clone())
        .collect::<Vec<_>>();
    let leases = stream::iter(dispatch_ids)
        .map(|dispatch_id| {
            let client = client.clone();
            async move {
                let reply = client
                    .query(
                        "dispatch",
                        json!({
                            "dispatch": {
                                "receiver": "runs",
                                "dispatch_id": dispatch_id
                            }
                        }),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                parse_dispatch_lease(&reply)
            }
        })
        .buffered(16)
        .collect::<Vec<_>>()
        .await;
    let mut pending_runs = Vec::with_capacity(parsed_pending.len());
    for (wire, lease) in parsed_pending.into_iter().zip(leases) {
        let lease = lease.unwrap_or_default();
        pending_runs.push(agents::PendingRun {
            run_id: wire.run_id,
            dispatch_id: wire.dispatch_id,
            agent_id: wire.agent_id,
            channel_id: wire.channel_id,
            anchor_sequence: wire.anchor_sequence,
            job_id: wire.job_id,
            created_at: format_stamp(wire.created_at),
            requested_by_me: identity_key
                .as_ref()
                .is_some_and(|key| wire.requester_external.as_ref() == Some(key)),
            attempt: lease.attempt,
            lease_remaining: lease
                .expires_at
                .map(|expires| expires.saturating_sub(current_height)),
            pending: false,
        });
    }
    Ok(Some(agents::AgentData {
        agents: roster,
        capabilities,
        capability_status,
        channels,
        watches,
        pending_runs,
        recent_runs,
        recent_runs_error,
        usage,
        job_worker_pending: false,
    }))
}

async fn load_capabilities(node: Option<&NodeClient>) -> Result<Vec<String>, String> {
    let reply = query(node, "capability", Value::String("all".into())).await?;
    parse_capabilities(&reply)
}

#[allow(clippy::too_many_arguments)]
async fn register_agent(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    display_name: String,
    agent_id: String,
    capability: String,
    allowed_actions: Vec<String>,
    caps: agents::ResourceCaps,
    skills: Vec<agents::SkillRef>,
) -> Result<(), String> {
    validate_agent_record(
        &agent_id,
        &display_name,
        &capability,
        &allowed_actions,
        &caps,
        &skills,
    )?;
    submit_signed(
        backend,
        node,
        ContentTarget::Agent,
        json!({
            "register_agent": {
                "agent_id": agent_id,
                "display_name": display_name.trim(),
                "capability": capability.trim(),
                "allowed_actions": canonical_actions(allowed_actions)?,
                "caps": caps_value(&caps)?,
                "skills": skills_value(&skills)?
            }
        }),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn update_agent(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    agent_id: String,
    display_name: String,
    capability: String,
    allowed_actions: Vec<String>,
    caps: agents::ResourceCaps,
    skills: Vec<agents::SkillRef>,
) -> Result<(), String> {
    validate_agent_record(
        &agent_id,
        &display_name,
        &capability,
        &allowed_actions,
        &caps,
        &skills,
    )?;
    submit_signed(
        backend,
        node,
        ContentTarget::Agent,
        json!({
            "update_agent": {
                "agent_id": agent_id,
                "display_name": display_name.trim(),
                "capability": capability.trim(),
                "allowed_actions": canonical_actions(allowed_actions)?,
                "caps": caps_value(&caps)?,
                "skills": skills_value(&skills)?
            }
        }),
    )
    .await
}

async fn agent_status(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    agent_id: String,
    active: bool,
) -> Result<(), String> {
    validate_agent_id(&agent_id)?;
    let payload = if active {
        json!({ "resume_agent": { "agent_id": agent_id } })
    } else {
        json!({ "pause_agent": { "agent_id": agent_id } })
    };
    submit_signed(backend, node, ContentTarget::Agent, payload).await
}

async fn watch_channel(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    channel_id: String,
    policy: agents::TurnPolicy,
) -> Result<(), String> {
    validate_channel_id(&channel_id)?;
    let policy = match policy {
        agents::TurnPolicy::Mention => Value::String("mention".into()),
        agents::TurnPolicy::All => Value::String("all".into()),
        agents::TurnPolicy::RoundRobin => Value::String("round_robin".into()),
        agents::TurnPolicy::Assigned(agent_id) => {
            validate_agent_id(&agent_id)?;
            json!({ "assigned": agent_id })
        }
    };
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "watch_channel": { "channel_id": channel_id, "policy": policy } }),
    )
    .await
}

async fn unwatch_channel(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    channel_id: String,
) -> Result<(), String> {
    validate_channel_id(&channel_id)?;
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "unwatch_channel": { "channel_id": channel_id } }),
    )
    .await
}

async fn set_job_worker(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    enabled: bool,
) -> Result<(), String> {
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "enable_job_worker": { "enabled": enabled } }),
    )
    .await
}

async fn cancel_run(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    run_id: String,
) -> Result<(), String> {
    validate_run_id(&run_id)?;
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "cancel_run": { "run_id": run_id } }),
    )
    .await
}

async fn reassign_run(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    run_id: String,
    attempt: u32,
) -> Result<(), String> {
    validate_run_id(&run_id)?;
    if attempt > 32 {
        return Err("run attempt is outside the supported bound".into());
    }
    submit_signed(
        backend,
        node,
        ContentTarget::Runs,
        json!({ "reassign_run": { "run_id": run_id, "attempt": attempt } }),
    )
    .await
}

struct PendingWire {
    run_id: String,
    dispatch_id: String,
    agent_id: String,
    channel_id: String,
    anchor_sequence: u64,
    job_id: Option<String>,
    created_at: u64,
    requester_external: Option<String>,
}

#[derive(Debug, Default)]
struct DispatchLease {
    attempt: u32,
    expires_at: Option<u64>,
}

fn parse_agent_record(value: &Value) -> Result<agents::AgentRecord, String> {
    let id = bounded_string(value, "agent_id", 63)?;
    validate_agent_id(&id)?;
    let display_name = bounded_string(value, "display_name", MAX_NAME_BYTES)?;
    let capability = bounded_string(value, "capability", MAX_CAPABILITY_BYTES)?;
    let allowed_actions = string_array(value, "allowed_actions", KNOWN_ACTIONS.len(), 64)?;
    let allowed_actions = canonical_actions(allowed_actions)?;
    let status = match value.get("status").and_then(Value::as_str) {
        Some("active") => agents::AgentStatus::Active,
        Some("paused") => agents::AgentStatus::Paused,
        _ => return Err("agent returned an invalid status".into()),
    };
    let created_at = required_u64(value, "created_at")?;
    let updated_at = required_u64(value, "updated_at")?;
    let caps = value
        .get("caps")
        .filter(|caps| !caps.is_null())
        .map(parse_caps)
        .transpose()?
        .unwrap_or_default();
    let skills = value
        .get("skills")
        .filter(|skills| !skills.is_null())
        .map(parse_skills)
        .transpose()?
        .unwrap_or_default();
    Ok(agents::AgentRecord {
        id,
        owner: parse_owner(
            value
                .get("owner")
                .ok_or_else(|| "agent is missing its owner".to_string())?,
        )?,
        display_name,
        capability,
        allowed_actions,
        status,
        created_at: format_stamp(created_at),
        updated_at: format_stamp(updated_at),
        caps,
        skills,
        pending: false,
    })
}

fn parse_owner(value: &Value) -> Result<agents::Owner, String> {
    if value.as_str() == Some("system") {
        return Ok(agents::Owner::System);
    }
    if let Some(bytes) = value.get("external") {
        return Ok(agents::Owner::External(bytes_hex(bytes)?));
    }
    if let Some(module) = value.get("module").and_then(Value::as_str) {
        validate_bounded_text(module, "owner module", MAX_NAME_BYTES, false)?;
        return Ok(agents::Owner::Module(module.to_owned()));
    }
    Err("agent returned an invalid owner".into())
}

fn parse_caps(value: &Value) -> Result<agents::ResourceCaps, String> {
    Ok(agents::ResourceCaps {
        forge_read: optional_string_array(value, "forge_read")?,
        forge_push: optional_string_array(value, "forge_push")?,
        duckfs_read: optional_string_array(value, "duckfs_read")?,
        duckfs_write: optional_string_array(value, "duckfs_write")?,
        tools: optional_string_array(value, "tools")?,
        secrets: optional_string_array(value, "secrets")?,
        pages_write: optional_string_array(value, "pages_write")?,
        subagent_budget: value
            .get("subagent_budget")
            .and_then(Value::as_u64)
            .map(|value| {
                u32::try_from(value).map_err(|_| "subagent budget exceeds u32".to_string())
            })
            .transpose()?,
    })
}

fn parse_skills(value: &Value) -> Result<Vec<agents::SkillRef>, String> {
    let rows = value
        .as_array()
        .ok_or_else(|| "agent returned an invalid skills list".to_string())?;
    if rows.len() > MAX_SKILLS {
        return Err("agent returned too many skills".into());
    }
    rows.iter()
        .map(|skill| {
            let name = bounded_string(skill, "name", MAX_NAME_BYTES)?;
            let source_prefix = bounded_string(skill, "source_prefix", MAX_CAP_ENTRY_BYTES)?;
            if name.is_empty() || source_prefix.is_empty() {
                return Err("skill name and source prefix must be non-empty".into());
            }
            let snapshot = optional_string(skill, "source_snapshot", MAX_NAME_BYTES)?;
            if snapshot.as_deref() == Some("") {
                return Err("skill snapshot must be non-empty when present".into());
            }
            let load = match skill.get("load").and_then(Value::as_str) {
                None | Some("on_demand") => agents::LoadMode::OnDemand,
                Some("always") => agents::LoadMode::Always,
                _ => return Err("agent returned an invalid skill load mode".into()),
            };
            Ok(agents::SkillRef {
                name,
                source_prefix,
                snapshot,
                load,
            })
        })
        .collect()
}

fn parse_capabilities(reply: &Value) -> Result<Vec<String>, String> {
    let rows = variant_array(reply, "all", MAX_AGENTS)?;
    let mut tags = BTreeSet::new();
    for row in rows {
        let pair = row
            .as_array()
            .filter(|row| row.len() == 2)
            .ok_or_else(|| "capability registry returned an invalid row".to_string())?;
        let _ = bytes_hex(&pair[0])?;
        let announced = pair[1]
            .as_array()
            .ok_or_else(|| "capability registry returned invalid tags".to_string())?;
        if announced.len() > MAX_CAP_ENTRIES {
            return Err("capability registry row exceeds the desktop limit".into());
        }
        for tag in announced {
            let tag = tag
                .as_str()
                .ok_or_else(|| "capability registry tag is not text".to_string())?;
            validate_bounded_text(tag, "capability", MAX_CAPABILITY_BYTES, false)?;
            tags.insert(tag.to_owned());
        }
    }
    Ok(tags.into_iter().collect())
}

fn parse_channels(reply: &Value) -> Result<Vec<agents::Channel>, String> {
    variant_array(reply, "channels", MAX_CHANNELS)?
        .iter()
        .filter(|channel| {
            !channel
                .get("archived")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && !channel
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id.contains(':'))
        })
        .map(|channel| {
            let id = bounded_string(channel, "id", MAX_NAME_BYTES)?;
            validate_channel_id(&id)?;
            Ok(agents::Channel {
                id,
                name: bounded_string(channel, "name", MAX_NAME_BYTES)?,
            })
        })
        .collect()
}

fn parse_watches(reply: &Value) -> Result<Vec<agents::Watch>, String> {
    variant_array(reply, "watches", MAX_WATCHES)?
        .iter()
        .map(|watch| {
            let channel_id = bounded_string(watch, "channel_id", MAX_NAME_BYTES)?;
            validate_channel_id(&channel_id)?;
            let policy = watch
                .get("policy")
                .ok_or_else(|| "runs returned a watch without a policy".to_string())?;
            let policy = match policy.as_str() {
                Some("mention") => agents::TurnPolicy::Mention,
                Some("all") => agents::TurnPolicy::All,
                Some("round_robin") => agents::TurnPolicy::RoundRobin,
                _ => {
                    let assigned = policy
                        .get("assigned")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "runs returned an invalid watch policy".to_string())?;
                    validate_agent_id(assigned)?;
                    agents::TurnPolicy::Assigned(assigned.to_owned())
                }
            };
            Ok(agents::Watch {
                channel_id,
                policy,
                pending: false,
            })
        })
        .collect()
}

fn parse_pending_wire(value: &Value) -> Result<PendingWire, String> {
    let run_id = bounded_string(value, "run_id", MAX_NAME_BYTES)?;
    validate_run_id(&run_id)?;
    let dispatch_id = bounded_string(value, "dispatch_id", 64)?;
    validate_digest(&dispatch_id)?;
    let agent_id = bounded_string(value, "agent_id", 63)?;
    validate_agent_id(&agent_id)?;
    let channel_id = bounded_string(value, "channel_id", MAX_NAME_BYTES)?;
    if !channel_id.is_empty() {
        validate_channel_id(&channel_id)?;
    }
    let requester_external = value
        .get("requester")
        .and_then(|requester| requester.get("external"))
        .map(bytes_hex)
        .transpose()?;
    Ok(PendingWire {
        run_id,
        dispatch_id,
        agent_id,
        channel_id,
        anchor_sequence: required_u64(value, "anchor_seq")?,
        job_id: optional_string(value, "job_id", MAX_NAME_BYTES)?,
        created_at: required_u64(value, "created_at")?,
        requester_external,
    })
}

fn parse_recent_runs(reply: &Value) -> Result<Vec<agents::RunRecord>, String> {
    variant_array(reply, "recent_runs", MAX_RECENT_RUNS)?
        .iter()
        .map(|record| {
            let run_id = bounded_string(record, "run_id", MAX_NAME_BYTES)?;
            validate_run_id(&run_id)?;
            let agent_id = bounded_string(record, "agent_id", 63)?;
            validate_agent_id(&agent_id)?;
            let channel_id = bounded_string(record, "channel_id", MAX_NAME_BYTES)?;
            if !channel_id.is_empty() {
                validate_channel_id(&channel_id)?;
            }
            let outcome = match record.get("outcome").and_then(Value::as_str) {
                Some("delivered") => agents::RunOutcome::Delivered,
                Some("failed") => agents::RunOutcome::Failed,
                _ => return Err("runs returned an invalid delivered-run outcome".into()),
            };
            let executing_node = bounded_string(record, "executing_node", 64)?;
            if executing_node != "unknown" {
                validate_digest(&executing_node)?;
            }
            let pr_number = record.get("pr_number").and_then(Value::as_u64);
            if pr_number == Some(0) {
                return Err("runs returned an invalid pull request number".into());
            }
            Ok(agents::RunRecord {
                dispatch_id: hex_encode(&Sha256::digest(run_id.as_bytes())),
                run_id,
                agent_id,
                channel_id,
                anchor_sequence: required_u64(record, "anchor_seq")?,
                outcome,
                degraded: record
                    .get("degraded")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| "runs returned a record without degraded state".to_string())?,
                created_at: required_u64(record, "created_at")?,
                delivered_at: required_u64(record, "delivered_at")?,
                executing_node,
                output_ref: optional_string(record, "output_ref", MAX_NAME_BYTES)?,
                pr_number,
            })
        })
        .collect()
}

fn parse_dispatch_lease(reply: &Value) -> Result<DispatchLease, String> {
    let Some(dispatch) = reply.get("dispatch") else {
        return Err("dispatch returned an invalid reply".into());
    };
    if dispatch.is_null() {
        return Ok(DispatchLease::default());
    }
    Ok(DispatchLease {
        attempt: dispatch
            .get("attempt")
            .and_then(Value::as_u64)
            .map(|attempt| {
                u32::try_from(attempt).map_err(|_| "dispatch attempt exceeds u32".to_string())
            })
            .transpose()?
            .unwrap_or(0),
        expires_at: dispatch.get("lease_expires_at").and_then(Value::as_u64),
    })
}

fn parse_usage(reply: &Value) -> Result<agents::Usage, String> {
    let rows = variant_array(reply, "usage", MAX_USAGE_ROWS)?;
    let mut usage = agents::Usage {
        requests: 0,
        failed: 0,
        duration_blocks: 0,
        input_tokens: 0,
        output_tokens: 0,
    };
    for row in rows {
        let runs = required_u64(row, "runs")?;
        usage.requests = usage.requests.saturating_add(runs);
        if !row
            .get("outcomeOk")
            .and_then(Value::as_bool)
            .ok_or_else(|| "usage row is missing outcomeOk".to_string())?
        {
            usage.failed = usage.failed.saturating_add(runs);
        }
        usage.duration_blocks = usage
            .duration_blocks
            .saturating_add(required_u64(row, "totalDurationBlocks")?);
        usage.input_tokens = usage
            .input_tokens
            .saturating_add(required_u64(row, "inputTokens")?);
        usage.output_tokens = usage
            .output_tokens
            .saturating_add(required_u64(row, "outputTokens")?);
    }
    Ok(usage)
}

fn validate_agent_record(
    agent_id: &str,
    display_name: &str,
    capability: &str,
    actions: &[String],
    caps: &agents::ResourceCaps,
    skills: &[agents::SkillRef],
) -> Result<(), String> {
    validate_agent_id(agent_id)?;
    validate_bounded_text(
        display_name.trim(),
        "agent display name",
        MAX_NAME_BYTES,
        false,
    )?;
    validate_bounded_text(
        capability.trim(),
        "agent capability",
        MAX_CAPABILITY_BYTES,
        false,
    )?;
    let actions = canonical_actions(actions.to_vec())?;
    if actions.is_empty() {
        return Err("an agent needs at least one allowed action".into());
    }
    let caps = caps_value(caps)?;
    let skills = skills_value(skills)?;
    let record = json!({
        "agent_id": agent_id,
        "display_name": display_name.trim(),
        "capability": capability.trim(),
        "allowed_actions": actions,
        "caps": caps,
        "skills": skills
    });
    if serde_json::to_vec(&record)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_RECORD_BYTES
    {
        return Err("agent record exceeds the 4 KiB consensus limit".into());
    }
    Ok(())
}

fn canonical_actions(actions: Vec<String>) -> Result<Vec<String>, String> {
    let mut actions = actions;
    if actions.len() > KNOWN_ACTIONS.len() {
        return Err("agent has too many allowed actions".into());
    }
    for action in &actions {
        if !KNOWN_ACTIONS.contains(&action.as_str()) {
            return Err(format!("unknown agent action: {action}"));
        }
    }
    actions.sort();
    actions.dedup();
    Ok(actions)
}

fn caps_value(caps: &agents::ResourceCaps) -> Result<Value, String> {
    let mut object = serde_json::Map::new();
    for (key, values) in [
        ("forge_read", &caps.forge_read),
        ("forge_push", &caps.forge_push),
        ("duckfs_read", &caps.duckfs_read),
        ("duckfs_write", &caps.duckfs_write),
        ("tools", &caps.tools),
        ("secrets", &caps.secrets),
        ("pages_write", &caps.pages_write),
    ] {
        if values.len() > MAX_CAP_ENTRIES {
            return Err(format!("{key} contains too many grants"));
        }
        let mut canonical = BTreeSet::new();
        for value in values {
            validate_bounded_text(value, key, MAX_CAP_ENTRY_BYTES, false)?;
            canonical.insert(value.clone());
        }
        if !canonical.is_empty() {
            object.insert(key.into(), json!(canonical.into_iter().collect::<Vec<_>>()));
        }
    }
    if let Some(budget) = caps.subagent_budget
        && budget != 0
    {
        object.insert("subagent_budget".into(), json!(budget));
    }
    Ok(Value::Object(object))
}

fn skills_value(skills: &[agents::SkillRef]) -> Result<Value, String> {
    if skills.len() > MAX_SKILLS {
        return Err(format!("an agent may curate at most {MAX_SKILLS} skills"));
    }
    let rows = skills
        .iter()
        .map(|skill| {
            validate_bounded_text(&skill.name, "skill name", MAX_NAME_BYTES, false)?;
            validate_bounded_text(
                &skill.source_prefix,
                "skill source prefix",
                MAX_CAP_ENTRY_BYTES,
                false,
            )?;
            if !skill.source_prefix.starts_with('/') {
                return Err("skill source prefix must be an absolute DuckFS path".into());
            }
            if let Some(snapshot) = skill.snapshot.as_deref() {
                validate_bounded_text(snapshot, "skill snapshot", MAX_NAME_BYTES, false)?;
            }
            Ok(json!({
                "name": skill.name,
                "source_prefix": skill.source_prefix,
                "source_snapshot": skill.snapshot,
                "load": match skill.load {
                    agents::LoadMode::Always => "always",
                    agents::LoadMode::OnDemand => "on_demand",
                }
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Value::Array(rows))
}

fn validate_agent_id(agent_id: &str) -> Result<(), String> {
    if agent_id.is_empty()
        || agent_id.len() > 63
        || agent_id.starts_with('-')
        || agent_id.ends_with('-')
        || !agent_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("agent id must be a 1..=63 byte lowercase DNS label".into());
    }
    Ok(())
}

fn validate_channel_id(channel_id: &str) -> Result<(), String> {
    validate_bounded_text(channel_id, "channel id", MAX_NAME_BYTES, false)?;
    if channel_id.contains('\0') {
        return Err("channel id contains an unsupported character".into());
    }
    Ok(())
}

fn validate_run_id(run_id: &str) -> Result<(), String> {
    validate_bounded_text(run_id, "run id", MAX_NAME_BYTES, false)
}

fn string_array(
    value: &Value,
    field: &str,
    max_items: usize,
    max_bytes: usize,
) -> Result<Vec<String>, String> {
    let rows = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("record is missing {field}"))?;
    if rows.len() > max_items {
        return Err(format!("{field} contains too many entries"));
    }
    rows.iter()
        .map(|row| {
            let text = row
                .as_str()
                .ok_or_else(|| format!("{field} contains a non-text entry"))?;
            validate_bounded_text(text, field, max_bytes, false)?;
            Ok(text.to_owned())
        })
        .collect()
}

fn optional_string_array(value: &Value, field: &str) -> Result<Vec<String>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(_) => string_array(value, field, MAX_CAP_ENTRIES, MAX_CAP_ENTRY_BYTES),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_and_usage_replies_are_strictly_bounded() {
        let capabilities = parse_capabilities(&json!({
            "all": [[[1, 2, 3], ["codex", "claude"]], [[4], ["codex"]]]
        }))
        .unwrap();
        assert_eq!(capabilities, vec!["claude", "codex"]);

        let usage = parse_usage(&json!({
            "usage": [{
                "outcomeOk": false,
                "runs": 2,
                "totalDurationBlocks": 7,
                "inputTokens": 11,
                "outputTokens": 5
            }]
        }))
        .unwrap();
        assert_eq!(usage.requests, 2);
        assert_eq!(usage.failed, 2);
        assert_eq!(usage.duration_blocks, 7);
        assert_eq!(usage.input_tokens + usage.output_tokens, 16);
    }

    #[test]
    fn recent_runs_keep_the_wire_evidence() {
        let recent = parse_recent_runs(&json!({
            "recent_runs": [{
                "run_id": "run-1",
                "agent_id": "triage",
                "channel_id": "general",
                "anchor_seq": 4,
                "outcome": "delivered",
                "degraded": true,
                "created_at": 10,
                "delivered_at": 13,
                "executing_node": "unknown",
                "output_ref": "feature@deadbeef",
                "pr_number": 12
            }]
        }))
        .unwrap();
        assert_eq!(recent[0].outcome, agents::RunOutcome::Delivered);
        assert_eq!(recent[0].dispatch_id.len(), 64);
        assert_eq!(recent[0].pr_number, Some(12));
    }

    #[test]
    fn agent_records_reject_unknown_actions_and_oversized_caps() {
        let caps = agents::ResourceCaps::default();
        assert!(
            validate_agent_record(
                "helper",
                "Helper",
                "codex",
                &["chat.post".into()],
                &caps,
                &[],
            )
            .is_ok()
        );
        assert!(
            validate_agent_record(
                "helper",
                "Helper",
                "codex",
                &["shell.root".into()],
                &caps,
                &[],
            )
            .is_err()
        );
        assert!(validate_agent_id("Bad_Agent").is_err());

        let caps = agents::ResourceCaps {
            tools: (0..=MAX_CAP_ENTRIES)
                .map(|index| format!("tool-{index}"))
                .collect(),
            ..agents::ResourceCaps::default()
        };
        assert!(caps_value(&caps).is_err());
    }
}
