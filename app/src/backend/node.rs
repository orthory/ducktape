use super::*;

/// The settings pane's facts: where this app points and what identity it
/// holds locally.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct SettingsFacts {
    pub generation: i64,
    pub endpoint: String,
    pub node_key: String,
    pub height: i64,
    pub key_path: String,
    pub key_state: String,
    /// this workspace's directory on this device — the NETWORK card's Data dir.
    pub data_dir: String,
    pub open_tabs: i64,
    /// THE VIEWER'S OWN KEY, full hex — the `me` every membership test needs.
    /// `ChatMember.key` is `member_id(..)` at full width, and `account_id` is a
    /// `short_label` of the identity module's ACCOUNT id, so neither the account
    /// card nor the node key can answer "is this row me". Empty on a device with
    /// no user key, which `post_gate` reads as "not seated" — the honest answer
    /// when there is no identity to seat.
    pub user_key: String,
}

/// The NETWORK card's Data dir row.
/// Load the settings facts: node identity from /v1/status, the local user
/// key's location and state, and the persisted tab count.
pub async fn load_settings_facts(
    rpc: String,
    generation: i64,
) -> Result<SettingsFacts, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status().await?;
        let (key_path, key_state) = match user_key_path() {
            Err(_) => ("(unset)".to_string(), "unlocatable".to_string()),
            Ok(path) => {
                let state = match std::fs::read(&path) {
                    Err(_) => "absent",
                    Ok(bytes) if bytes.starts_with(ENCRYPTED_KEY_PREFIX.as_bytes()) => "encrypted",
                    Ok(_) => "PLAINTEXT — secure it",
                };
                (path.display().to_string(), state.to_string())
            }
        };
        let tabs = load_doc_tabs(rpc.clone()).await;
        let data_dir = workspace_at(&rpc)
            .map(|(_, dir)| dir.display().to_string())
            .or_else(|| ducktape_home().map(|home| home.display().to_string()))
            .unwrap_or_default();
        Ok(SettingsFacts {
            generation,
            endpoint: rpc,
            node_key: short_label(&status.public_key),
            height: i64::try_from(status.height).unwrap_or(i64::MAX),
            key_path,
            key_state,
            data_dir,
            open_tabs: count_i64(tabs.len()),
            user_key: local_user_key()
                .await
                .map(|key| hex_encode(&key))
                .unwrap_or_default(),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Forget this endpoint's persisted doc tabs.
pub async fn clear_doc_tabs(rpc: String) -> bool {
    save_doc_tabs(rpc, Vec::new()).await
}

/// One log line for the operator pane.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct NodeLogLine {
    pub cursor: String,
    pub line: String,
}

/// The node's live log ring as an app stream — reconnects with backoff and
/// resumes from the last cursor, exactly like the module stream.
pub fn node_logs(rpc: String) -> iced::futures::stream::BoxStream<'static, NodeLogLine> {
    struct State {
        rpc: String,
        cursor: Option<String>,
        stream: Option<
            iced::futures::stream::BoxStream<'static, ducktape_rpc::Result<ducktape_rpc::LogLine>>,
        >,
        retry_attempt: u32,
    }
    iced::futures::stream::unfold(
        State {
            rpc,
            cursor: None,
            stream: None,
            retry_attempt: 0,
        },
        |mut state| async move {
            loop {
                if state.stream.is_none() && state.retry_attempt > 0 {
                    tokio::time::sleep(retry_delay(state.retry_attempt)).await;
                }
                if state.stream.is_none() {
                    let Ok(rpc) = rpc_client(&state.rpc) else {
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        continue;
                    };
                    match rpc.log_events(state.cursor.clone()).await {
                        Ok(stream) => state.stream = Some(stream),
                        Err(_) => {
                            state.retry_attempt = state.retry_attempt.saturating_add(1);
                            continue;
                        }
                    }
                }
                match state
                    .stream
                    .as_mut()
                    .expect("stream initialized")
                    .next()
                    .await
                {
                    Some(Ok(line)) => {
                        state.retry_attempt = 0;
                        state.cursor = Some(line.cursor.clone());
                        return Some((
                            NodeLogLine {
                                cursor: line.cursor,
                                line: line.line,
                            },
                            state,
                        ));
                    }
                    Some(Err(_)) | None => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                    }
                }
            }
        },
    )
    .boxed()
}

/// Append a log line to the pane's bounded ring (newest last, 500 kept).
pub fn push_log_line(mut lines: Vec<NodeLogLine>, line: NodeLogLine) -> Vec<NodeLogLine> {
    let duplicate = lines.last().is_some_and(|last| last.cursor == line.cursor);
    if duplicate {
        return lines;
    }
    lines.push(line);
    let excess = lines.len().saturating_sub(500);
    lines.drain(..excess);
    lines
}

/// The pane's visible window: substring-filtered, newest last.
pub fn filter_log_lines(lines: Vec<NodeLogLine>, filter: String) -> Vec<NodeLogLine> {
    let needle = filter.trim().to_lowercase();
    if needle.is_empty() {
        return lines;
    }
    lines
        .into_iter()
        .filter(|line| line.line.to_lowercase().contains(&needle))
        .collect()
}

/// One tracing line, split for the dark log console's three columns.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LogParts {
    pub time: String,
    pub level: String,
    pub message: String,
}

/// Split `2026-07-27T09:12:44.918Z  INFO ducktape::join: admitted` into its
/// three columns. A line that does not carry a level is all message.
pub fn split_log_line(line: String) -> LogParts {
    const LEVELS: [&str; 5] = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
    let mut fields = line.split_whitespace();
    let Some(first) = fields.next() else {
        return LogParts {
            time: String::new(),
            level: String::new(),
            message: line,
        };
    };
    let timestamped =
        first.contains(':') && first.chars().next().is_some_and(|c| c.is_ascii_digit());
    let (time, level_field) = match timestamped {
        true => (first.to_string(), fields.next().unwrap_or_default()),
        false => (String::new(), first),
    };
    if !LEVELS.contains(&level_field) {
        return LogParts {
            time,
            level: String::new(),
            message: line,
        };
    }
    let cut = line
        .find(level_field)
        .map_or(line.len(), |at| at + level_field.len());
    LogParts {
        time,
        level: level_field.to_string(),
        message: line[cut..].trim_start().to_string(),
    }
}

/// The node's consensus/storage facts — everything `/v1/status` publishes that
/// the two-field `Status` type drops, plus the mesh sample's live/total.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct NodeFacts {
    pub generation: i64,
    /// The daemon's build version, verbatim off `/v1/status` (its own
    /// `CARGO_PKG_VERSION`). A build/commit SHA is NOT published anywhere, so
    /// the version line carries the version alone.
    pub version: String,
    pub root_hash: String,
    /// The three consensus facts are OPTION on purpose: `operations.consensus`
    /// is absent on a resident, a joiner and the embedded local daemon
    /// "rather than being filled with misleading zeroes", so a plain i64 would
    /// print a hard 0 as if it were measured.
    pub view: Option<i64>,
    pub quorum: Option<i64>,
    pub reachable_validators: Option<i64>,
    /// These two are under the SAME absent-on-a-resident `operations` object as
    /// the trio above, so they get the same honesty — carried as [`UNMEASURED`]
    /// rather than a plain `0`, which both renderers already print as `—`.
    pub last_finalized_at: i64,
    pub checkpoint_height: i64,
    pub peers_live: i64,
    pub peers_total: i64,
}

/// Load the node facts from the raw status document plus the peer sample.
/// A section the node omits for its role stays `None` — the status projection
/// leaves it out rather than filling it with misleading numbers, and so do we.
pub async fn load_node_facts(rpc: String, generation: i64) -> Result<NodeFacts, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status_json().await?;
        let operations = &status["operations"];
        let consensus = &operations["consensus"];
        let peers = client.peers().await.unwrap_or_default();
        let peers = peers["peers"].as_array().cloned().unwrap_or_default();
        Ok(NodeFacts {
            generation,
            version: status["version"].as_str().unwrap_or_default().to_string(),
            root_hash: status["root_hash"].as_str().unwrap_or_default().to_string(),
            view: consensus["view"].as_i64(),
            quorum: consensus["quorum"].as_i64(),
            reachable_validators: consensus["reachable_validators"].as_i64(),
            last_finalized_at: operations["last_finalized_at"]
                .as_i64()
                .unwrap_or(UNMEASURED),
            checkpoint_height: operations["storage"]["checkpoint_height"]
                .as_i64()
                .unwrap_or(UNMEASURED),
            peers_live: count_i64(
                peers
                    .iter()
                    .filter(|peer| peer["live"].as_bool().unwrap_or(false))
                    .count(),
            ),
            peers_total: count_i64(peers.len()),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// What an `operations` reading the node did not publish carries.
///
/// The rule is already written twice — `NodeFacts`'s consensus trio is
/// `Option` "rather than being filled with misleading zeroes", and `state.ice`
/// says an absent reading "must print `—`, never a measured `0`". The two
/// `i64` fields beside them had no way to say it, because `0` is a legal
/// height and a legal timestamp.
///
/// NEGATIVE is that way: `height_label` already renders `< 0` as `h —`, so
/// this reuses a contract the renderer had rather than inventing one. Naming
/// it keeps the `-1` from reading as arithmetic at the fill site.
pub const UNMEASURED: i64 = -1;

/// A consensus fact the node did not publish for this role reads `—`, never a
/// zero. The view has no way to branch on an absent value itself.
pub fn optional_number(value: Option<i64>) -> String {
    match value {
        Some(number) => grouped_digits(number),
        None => "—".into(),
    }
}

/// One peer row.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PeerRow {
    pub key: String,
    pub height: i64,
    pub live: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PeersData {
    pub generation: i64,
    pub peers: Vec<PeerRow>,
}

/// Load the peers standing view.
pub async fn load_peers(rpc: String, generation: i64) -> Result<PeersData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let reply = rpc.peers().await?;
        let peers = reply["peers"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|peer| PeerRow {
                key: short_label(peer["key"].as_str().unwrap_or_default()),
                height: peer["height"].as_i64().unwrap_or(0),
                live: peer["live"].as_bool().unwrap_or(false),
            })
            .collect();
        Ok(PeersData { generation, peers })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// One registered module, as the node itself reports it.
///
/// There is no MARKETPLACE behind this row and there cannot be: a publisher, a
/// verification badge, an install count and a catalog description exist in no
/// module, no index and no manifest. This is the INSTALLED/RUNTIME truth —
/// what is registered, at which code, with which swap pending.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ModuleRow {
    pub id: String,
    /// `workspace` | `developer` | `automation` | `system` — the presentation
    /// category the status projection attaches by id. Never consensus state.
    pub category: String,
    /// The module's own state root, short form.
    pub root: String,
    /// The active component's sha256, short form. Empty when this network runs
    /// no lifecycle module (the daemon's default set does not).
    pub code_hash: String,
    /// The scheduled swap's target hash, short form; empty when none is armed.
    pub pending_hash: String,
    /// The pending swap's activation height (0 when none is armed).
    pub activation_height: i64,
    /// Validators that have verified the pending bytes locally.
    pub readiness: i64,
    /// The pending swap has full coverage and will activate at its height.
    pub ready: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ModulesData {
    pub generation: i64,
    pub rows: Vec<ModuleRow>,
}

/// The registered module set: `/v1/status` publishes id, root and category for
/// every module, and the lifecycle module (where a network runs one) adds the
/// active code hash and any armed swap.
///
/// The lifecycle half is BEST EFFORT on purpose — the daemon's default module
/// set has no `lifecycle`, and a network without one still has a real,
/// complete registered set to show.
pub async fn load_modules(rpc: String, generation: i64) -> Result<ModulesData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status_json().await?;
        let code = module_code_by_id(&client).await;
        let rows = status["modules"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|module| {
                let id = module["id"].as_str().unwrap_or_default().to_string();
                let lifecycle = code.get(&id);
                let pending =
                    lifecycle.map_or(serde_json::Value::Null, |entry| entry["pending"].clone());
                ModuleRow {
                    category: module["category"].as_str().unwrap_or_default().to_string(),
                    root: short_digest(module["root"].as_str().unwrap_or_default()),
                    code_hash: lifecycle
                        .map(|entry| {
                            short_digest(&hex_encode(&json_bytes(&entry["active_code_hash"])))
                        })
                        .unwrap_or_default(),
                    pending_hash: short_digest(&hex_encode(&json_bytes(&pending["code_hash"]))),
                    activation_height: pending["activation_height"].as_i64().unwrap_or(0),
                    readiness: count_i64(
                        pending["readiness"]
                            .as_array()
                            .map_or(0, |signals| signals.len()),
                    ),
                    ready: pending["ready"].as_bool().unwrap_or(false),
                    id,
                }
            })
            .collect();
        Ok(ModulesData { generation, rows })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// `LifecycleQuery::ModuleStatus` keyed by module id, empty when this network
/// runs no lifecycle module.
async fn module_code_by_id(client: &RpcClient) -> BTreeMap<String, serde_json::Value> {
    let Ok(reply) = client
        .query::<_, serde_json::Value>("lifecycle", &serde_json::json!("module_status"))
        .await
    else {
        return BTreeMap::new();
    };
    reply["module_status"]["modules"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let id = entry["module_id"].as_str()?.to_string();
            Some((id, entry))
        })
        .collect()
}

/// One curated skill of an agent: the ref's name and whether it loads as
/// persona (`LoadMode::Always`) or on demand.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentSkill {
    pub name: String,
    pub always: bool,
}

/// One granted capability, in the `CapRequest` vocabulary: the request name
/// and the resource it names (empty for the argument-less grants).
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentCap {
    pub label: String,
    pub arg: String,
}

/// One registered agent, rendered. Everything here already rides
/// `AgentRecord` — the registry reply carries the whole record.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub initials: String,
    pub capability: String,
    pub status: String,
    /// the decoded `SagaOrigin::External` key hex, empty for module/system owners.
    pub owner_key: String,
    /// that key resolved against the member roster, else the origin's variant tag.
    pub owner_handle: String,
    pub created_at: i64,
    pub is_mine: bool,
    /// this agent holds a RUN in flight right now — the runs module's pending
    /// register, NOT `status`. `AgentStatus` is only Active|Paused and Active
    /// is the registration default, so it says "not paused", never "working".
    pub live: bool,
    pub tools: i64,
    pub secrets: i64,
    pub subagent_budget: i64,
    pub allowed_actions: Vec<String>,
    pub skills: Vec<AgentSkill>,
    pub caps: Vec<AgentCap>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentsData {
    pub generation: i64,
    pub agents: Vec<AgentRow>,
}

/// The owner origin, decoded: `("<key hex>", "<handle>")`. An external origin
/// carries raw key bytes; a module/system origin has no key at all and reads
/// as its own name.
fn agent_owner(owner: &serde_json::Value) -> (String, String) {
    let Some(tagged) = owner.as_object() else {
        let name = owner.as_str().unwrap_or_default().to_string();
        return (String::new(), name);
    };
    let Some((variant, payload)) = tagged.iter().next() else {
        return (String::new(), String::new());
    };
    if variant != "external" {
        let name = payload.as_str().unwrap_or(variant.as_str()).to_string();
        return (String::new(), name);
    }
    let key = hex_encode(&json_bytes(payload));
    let handle = short_label(&key);
    (key, handle)
}

/// `ResourceCaps` flattened into the `CapRequest` names the console chips.
fn agent_caps(caps: &serde_json::Value) -> Vec<AgentCap> {
    let mut chips = Vec::new();
    for (field, label) in [
        ("forge_read", "ForgeRead"),
        ("forge_push", "ForgePush"),
        ("duckfs_read", "DuckfsRead"),
        ("duckfs_write", "DuckfsWrite"),
        ("tools", "Tool"),
        ("secrets", "Secret"),
        ("pages_write", "PagesWrite"),
    ] {
        for value in caps[field].as_array().cloned().unwrap_or_default() {
            chips.push(AgentCap {
                label: label.into(),
                arg: value.as_str().unwrap_or_default().to_string(),
            });
        }
    }
    if caps["subagent_budget"].as_i64().unwrap_or(0) > 0 {
        chips.push(AgentCap {
            label: "SpawnSubagent".into(),
            arg: String::new(),
        });
    }
    chips
}

/// Load the agent roster from the canonical registry, each row marked with
/// whether THIS device's user key is its owner.
pub async fn load_agents(rpc: String, generation: i64) -> Result<AgentsData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let local = local_user_key().await.map(|key| hex_encode(&key));
        let reply: serde_json::Value = client.query("agent", &serde_json::json!("agents")).await?;
        let working = agents_with_a_run_in_flight(&client).await;
        let agents = reply["agents"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|record| {
                let status = tagged_name(&record["status"]);
                let (owner_key, owner_handle) = agent_owner(&record["owner"]);
                let name = record["display_name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let caps = &record["caps"];
                let id = record["agent_id"].as_str().unwrap_or_default().to_string();
                AgentRow {
                    live: working.contains(&id),
                    initials: initials_of(&name),
                    capability: record["capability"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    created_at: record["created_at"].as_i64().unwrap_or(0),
                    is_mine: local.as_deref().is_some_and(|key| key == owner_key),
                    tools: count_i64(caps["tools"].as_array().map_or(0, Vec::len)),
                    secrets: count_i64(caps["secrets"].as_array().map_or(0, Vec::len)),
                    subagent_budget: caps["subagent_budget"].as_i64().unwrap_or(0),
                    allowed_actions: record["allowed_actions"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|action| action.as_str().map(str::to_string))
                        .collect(),
                    skills: record["skills"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|skill| AgentSkill {
                            name: skill["name"].as_str().unwrap_or_default().to_string(),
                            always: skill["load"].as_str() == Some("always"),
                        })
                        .collect(),
                    caps: agent_caps(caps),
                    id,
                    name,
                    status,
                    owner_key,
                    owner_handle,
                }
            })
            .collect();
        Ok(AgentsData { generation, agents })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The agents holding a run in flight, from the runs module's pending
/// register — the ONLY place in the product that knows an agent is working.
/// A node that cannot answer the query reports nobody working, never everybody.
async fn agents_with_a_run_in_flight(rpc: &RpcClient) -> BTreeSet<String> {
    let Ok(reply) = rpc
        .query::<_, serde_json::Value>("runs", &serde_json::json!("pending_runs"))
        .await
    else {
        return BTreeSet::new();
    };
    let Some(pending) = reply["pending_runs"].as_array() else {
        return BTreeSet::new();
    };
    pending
        .iter()
        .filter_map(|run| run["agent_id"].as_str().map(str::to_string))
        .collect()
}

/// Whether any agent is engaging work right now — the rail's Forge pulse dot.
pub fn any_agent_active(rows: Vec<AgentRow>) -> bool {
    rows.iter().any(|row| row.live)
}

/// One run of one agent: the RECENT RUNS card, the agent live chip and the
/// Explorer RUN hit all read this row.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct RunRow {
    pub run_id: String,
    pub agent_id: String,
    pub outcome: String,
    pub running: bool,
    /// A consensus counter (the creation block), NOT a unix stamp — render it
    /// with `height_ago`/`height_label_short`, never with `relative_time`.
    pub created_at: i64,
    /// what the run PRODUCED, in one line: `RunRecord` carries `pr_number` and
    /// `output_ref` and this is the only surface that reads them.
    pub summary: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentRunsData {
    pub generation: i64,
    pub runs: Vec<RunRow>,
}

/// What a settled run produced, in one line: the forge PR it moved, else the
/// output ref it wrote, else how it ended. Both fields ride `RunRecord`
/// (crates/modules/apps/runs/src/interface.rs) — nothing here is invented.
fn run_summary(record: &serde_json::Value, outcome: &str) -> String {
    if let Some(number) = record["pr_number"].as_u64() {
        return format!("pr #{number}");
    }
    match record["output_ref"].as_str() {
        Some(output) if !output.is_empty() => output.to_string(),
        _ => outcome.to_string(),
    }
}

/// This agent's runs: the pending (RUNNING) entries first, then the delivered
/// ring newest-first. Two queries because the runs module keeps in-flight
/// correlation and settled history in two separate projections.
pub async fn load_agent_runs(
    rpc: String,
    agent_id: String,
    generation: i64,
) -> Result<AgentRunsData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let pending: serde_json::Value = client
            .query("runs", &serde_json::json!("pending_runs"))
            .await?;
        let recent: serde_json::Value = client
            .query("runs", &serde_json::json!("recent_runs"))
            .await?;
        let wanted = |record: &serde_json::Value| {
            agent_id.is_empty() || record["agent_id"].as_str() == Some(agent_id.as_str())
        };
        let mut runs: Vec<RunRow> = pending["pending_runs"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(wanted)
            .map(|record| RunRow {
                run_id: record["run_id"].as_str().unwrap_or_default().to_string(),
                agent_id: record["agent_id"].as_str().unwrap_or_default().to_string(),
                outcome: "running".into(),
                running: true,
                created_at: record["created_at"].as_i64().unwrap_or(0),
                summary: record["channel_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect();
        runs.extend(
            recent["recent_runs"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(wanted)
                .map(|record| {
                    let outcome = tagged_name(&record["outcome"]);
                    RunRow {
                        run_id: record["run_id"].as_str().unwrap_or_default().to_string(),
                        agent_id: record["agent_id"].as_str().unwrap_or_default().to_string(),
                        running: false,
                        created_at: record["created_at"].as_i64().unwrap_or(0),
                        summary: run_summary(&record, &outcome),
                        outcome,
                    }
                }),
        );
        Ok(AgentRunsData { generation, runs })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Pause or resume one agent — owner-gated at the module, not quorum-gated.
pub async fn set_agent_status(
    rpc: String,
    password: String,
    agent_id: String,
    paused: bool,
) -> Result<bool, AppError> {
    async {
        let agent_id = required_id(agent_id, "agent")?;
        let rpc = rpc_client(&rpc)?;
        // `AgentMsg` is snake_case-tagged serde over `sdk::wire` (plain JSON);
        // the app does not depend on the agent crate, so the two owner-gated
        // verbs are written as their wire form.
        let verb = match paused {
            true => "pause_agent",
            false => "resume_agent",
        };
        let payload = serde_json::json!({ verb: { "agent_id": agent_id } });
        signed_write(&rpc, "agent", encode_wire(&payload), password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The local account picture: whether THIS NODE is bound, and the account's
/// public face.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AccountData {
    pub generation: i64,
    pub bound: bool,
    pub account_id: String,
    pub display_name: String,
    pub bio: String,
    pub members: i64,
    pub nodes: i64,
}

/// Load the account this node is bound to (via the canonical resolver).
pub async fn load_account(rpc: String, generation: i64) -> Result<AccountData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let node_key_hex = client.status().await?.public_key;
        let node_key: Vec<u8> = (0..node_key_hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&node_key_hex[i..i + 2], 16).ok())
            .collect();
        let reply: serde_json::Value = client
            .query(
                "identity",
                &serde_json::json!({ "of_node": { "node_key": node_key } }),
            )
            .await?;
        let account = &reply["account"];
        if account.is_null() {
            return Ok(AccountData {
                generation,
                bound: false,
                account_id: String::new(),
                display_name: String::new(),
                bio: String::new(),
                members: 0,
                nodes: 0,
            });
        }
        let id_bytes: Vec<u8> = account["account_id"]
            .as_array()
            .map(|bytes| {
                bytes
                    .iter()
                    .filter_map(|byte| byte.as_u64().map(|byte| byte as u8))
                    .collect()
            })
            .unwrap_or_default();
        Ok(AccountData {
            generation,
            bound: true,
            account_id: short_label(&hex_encode(&id_bytes)),
            display_name: account["display_name"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            bio: account["bio"].as_str().unwrap_or_default().to_string(),
            members: count_i64(account["members"].as_array().map_or(0, |m| m.len())),
            nodes: count_i64(account["nodes"].as_array().map_or(0, |n| n.len())),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Rename the account this node is bound to (origin-gated: the bound node
/// itself is the authority).
pub async fn set_account_name(
    rpc: String,
    password: String,
    display_name: String,
) -> Result<bool, AppError> {
    async {
        let display_name = bounded_text(display_name, "display name", 128)?;
        let client = rpc_client(&rpc)?;
        signed_write(
            &client,
            "identity",
            identity::encode_msg(&identity::IdentityMsg::SetAccountName { display_name }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}
