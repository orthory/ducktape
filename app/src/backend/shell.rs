use super::*;

/// One shell navigation entry. `live` is the capsule's pulse dot.
#[derive(Clone, Debug, PartialEq)]
pub struct NavItem {
    pub id: crate::ShellTab,
    pub title: String,
    pub icon: String,
    pub badge: i64,
    pub active: bool,
    pub live: bool,
}

/// `1 repository` / `3 repositories` — the ONE place a count and its noun are
/// joined. English has no rule that derives the plural (repository/repositories,
/// directory/directories), so both forms are stated by the caller. Every
/// count label goes through here; a bare `format!("{n} agents")` renders the
/// `1 agents` the register-machine subtitles used to show.
pub fn plural(count: i64, one: String, many: String) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

/// A HEADER SUBTITLE IS A MEASUREMENT, AND A DISCONNECTED APP MEASURED NOTHING.
/// Every `*_summary` below folds rows that only a live node can deliver, so with
/// the node down they all read `0 … · 0 …` — a claim about content, asserted off
/// a listing nobody fetched. They take `connected` and say nothing instead, the
/// same trade `count_label` (backend/document.rs) and `member_tier`
/// (backend/roster.rs) already make: silence over a confident zero.
///
/// AND AN ALL-ZERO PAIR IS THE SAME NOISE ONE STEP LATER. Every one of these
/// screens plates the empty case in words — "No proposals yet…", "No agents
/// registered…", "No members here yet…", "Empty directory…" — so a subtitle
/// reading `0 open · 0 settled` over the top of it says the same nothing twice,
/// in digits. #996 settled the rule for the bell and the member count: gate the
/// digit AND its word together. These four were the sites it did not reach.
/// A zero that sits BESIDE a real reading stays — `1 agent · 0 working` is the
/// sentence doing its job.
///
/// `2 humans · 1 agent` — the machine subtitle beside the Members title, and
/// the same reading in Settings' network card.
///
/// IT FOLDS THE ROWS IT IS PRINTED ABOVE. This used to fold the valset queries
/// instead — validators plus residents — while the list under it also draws the
/// registered agents, which hold no valset standing at all. Both numbers were
/// true and the sentence they formed was not: a demo workspace read
/// `1 validator · 0 residents` above two rows, and "residents" is a word the
/// screen never says anywhere else. `is_agent` is the one split the screen
/// itself makes — the Humans / Agents chips are `filter_members` over exactly
/// that field — so these two counts partition the list and sum to the All chip
/// beside them. The validator count is not lost: it is its own chip, and every
/// row carries its role marker.
pub fn members_summary(connected: bool, rows: Vec<MemberRow>) -> String {
    if !connected || rows.is_empty() {
        return String::new();
    }
    let agents = rows.iter().filter(|row| row.is_agent).count();
    let left = plural(
        count_i64(rows.len() - agents),
        "human".into(),
        "humans".into(),
    );
    let right = plural(count_i64(agents), "agent".into(), "agents".into());
    format!("{left} · {right}")
}

/// `4 agents · 2 working` — the Agents title's machine subtitle. `working` is
/// runs in flight, not `AgentStatus::Active`: Active is the registration
/// default and would report every registered agent as busy forever.
pub fn agents_summary(connected: bool, rows: Vec<AgentRow>) -> String {
    if !connected || rows.is_empty() {
        return String::new();
    }
    let working = rows.iter().filter(|row| row.live).count();
    let registered = plural(count_i64(rows.len()), "agent".into(), "agents".into());
    format!("{registered} · {working} working")
}

/// `12 open · 3 settled` — the Approvals title's machine subtitle.
pub fn proposals_summary(connected: bool, rows: Vec<ProposalRow>) -> String {
    if !connected || rows.is_empty() {
        return String::new();
    }
    let open = rows.iter().filter(|row| row.open).count();
    format!("{open} open · {} settled", rows.len() - open)
}

/// `N pending` — the header count, open proposals only.
pub fn pending_label(rows: Vec<ProposalRow>) -> String {
    format!("{} pending", rows.iter().filter(|row| row.open).count())
}

/// The settled half of the register — the RECENTLY FINALIZED column.
pub fn settled_proposals(rows: Vec<ProposalRow>) -> Vec<ProposalRow> {
    rows.into_iter().filter(|row| !row.open).collect()
}

/// One seat per REQUIRED signature, filled for each approval already in —
/// the quorum dots. Capped so a large threshold does not overflow the card.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct QuorumSeat {
    pub filled: bool,
}

pub fn quorum_dots(approvals: i64, required: i64) -> Vec<QuorumSeat> {
    let seats = required.clamp(0, 12) as usize;
    (0..seats)
        .map(|seat| QuorumSeat {
            filled: (seat as i64) < approvals,
        })
        .collect()
}

/// `3 / 4` — the tally, one mono run.
pub fn tally_label(approvals: i64, required: i64) -> String {
    format!("{approvals} / {required}")
}

/// `tally_label` for two readings that are ALREADY rendered — the consensus
/// trio off `/v1/status` is optional per field, so each arrives as its own
/// `optional_number` string (`—` when the node reports nothing). Joining the
/// numbers instead would mean carrying them as `i64` and printing a measured
/// `0` for "not reported".
pub fn reading_pair(left: impl AsRef<str>, right: impl AsRef<str>) -> String {
    format!("{} / {}", left.as_ref(), right.as_ref())
}

/// `near` one vote from quorum (or past it), else `far` — success vs meta ink.
pub fn tally_tone(approvals: i64, required: i64) -> String {
    match approvals >= required.saturating_sub(1) {
        true => "near".into(),
        false => "far".into(),
    }
}

/// `3 approvals · 1 more for quorum`, or `quorum met`.
pub fn tally_note(approvals: i64, required: i64) -> String {
    let remaining = required.saturating_sub(approvals);
    if remaining <= 0 {
        return "quorum met".into();
    }
    let have = plural(approvals, "approval".into(), "approvals".into());
    format!("{have} · {remaining} more for quorum")
}

/// The approve button leans forward at the last vote: `Approve →`.
pub fn approve_label(approvals: i64, required: i64) -> String {
    match approvals + 1 >= required {
        true => "Approve →".into(),
        false => "Approve".into(),
    }
}

/// The kind pill's two tones: an access-class action reads `access`.
pub fn proposal_kind_tone(action: String) -> String {
    let access = matches!(
        action.as_str(),
        "add_validator" | "add_resident" | "remove_validator" | "remove_resident" | "grant_client"
    );
    match access {
        true => "access".into(),
        false => "neutral".into(),
    }
}

/// How many proposals are still open — the count the rail pins to Approvals.
pub fn open_proposals(rows: Vec<ProposalRow>) -> i64 {
    rows.iter().filter(|row| row.open).count() as i64
}

/// The rail's navigation: nine collaboration surfaces plus the node operator
/// surface, with the active pane flagged. `settings` is not here because the
/// rail pins it to its own footer beside the account avatar.
pub fn shell_nav(tab: crate::ShellTab, approvals: i64, agent_live: bool) -> Vec<NavItem> {
    [
        (crate::ShellTab::Chat, "Chat", "nav-chat"),
        (crate::ShellTab::Shell, "Shell", "code-slash"),
        (crate::ShellTab::Pages, "Pages", "nav-pages"),
        (crate::ShellTab::Forge, "Forge", "nav-forge"),
        (crate::ShellTab::Agents, "Agents", "nav-agents"),
        (crate::ShellTab::Files, "Files", "nav-files"),
        (crate::ShellTab::Explorer, "Explorer", "nav-explorer"),
        (crate::ShellTab::Node, "Node", "node"),
        (crate::ShellTab::Members, "Members", "nav-members"),
        (crate::ShellTab::Governance, "Approvals", "shield-check"),
    ]
    .into_iter()
    .map(|(id, title, icon)| NavItem {
        id,
        title: title.into(),
        icon: icon.into(),
        badge: if id == crate::ShellTab::Governance {
            approvals
        } else {
            0
        },
        active: id == tab,
        live: id == crate::ShellTab::Forge && agent_live,
    })
    .collect()
}

/// Does the pane `tab` mounts actually read `plane`'s rows?
///
/// THE TAB-SWITCH GATE. Every tab move used to refetch members, governance,
/// agents and account — four `/v1/query` round trips per click, on the way into
/// panes that render none of them. The refetch is not what keeps those planes
/// fresh either: `plane_live_hit` already refetches each one when ITS module
/// commits (`live.rs`), which is both cheaper and earlier. So a tab move only
/// needs the plane its destination screen is about to draw.
///
/// The titlebar chips (tier, approvals, agent dot, account name) read all four
/// from state, and state is what the connect load and the live-plane lane fill
/// — no chip depends on a tab click.
pub fn tab_reads_plane(tab: crate::ShellTab, plane: String) -> bool {
    match plane.as_str() {
        // the tier badge, the admin gate and the forge write gate all read the
        // roster, so five panes draw it.
        "members" => matches!(
            tab,
            crate::ShellTab::Members
                | crate::ShellTab::Governance
                | crate::ShellTab::Forge
                | crate::ShellTab::Node
                | crate::ShellTab::Settings
        ),
        "governance" => tab == crate::ShellTab::Governance,
        "agents" => tab == crate::ShellTab::Agents,
        // Settings draws the account card; Forge draws the org "about".
        "account" => matches!(tab, crate::ShellTab::Settings | crate::ShellTab::Forge),
        _ => false,
    }
}

/// The demo registry, when this machine has one (`ops/demo-seed.sh` is its
/// only writer). Read per call, not cached: the launch window switches
/// networks in-process, so no registry reading may outlive a boot.
fn demo_registry() -> Option<serde_json::Value> {
    let path = ducktape_home()?.join("registry.json");
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn registry_active_entry() -> Option<serde_json::Value> {
    let registry = demo_registry()?;
    let active = registry.get("active")?.as_str()?;
    registry
        .get("workspaces")?
        .as_array()?
        .iter()
        .find(|workspace| workspace.get("id").and_then(|id| id.as_str()) == Some(active))
        .cloned()
}

/// The registry's `active` workspace id — the launch list's preselection
/// hint on a demo-seeded machine.
pub(crate) fn registry_active_workspace() -> Option<String> {
    demo_registry()?.get("active")?.as_str().map(str::to_string)
}

/// The active workspace's name, from the CLI's registry. The app and
/// the CLI name the same workspace, so the titlebar says `demo`, not an IP.
fn active_workspace_name() -> Option<String> {
    let workspace = registry_active_entry()?;
    workspace
        .get("name")
        .or_else(|| workspace.get("id"))
        .and_then(|name| name.as_str())
        .map(str::to_string)
}

/// The active workspace's http endpoint, from the same registry the titlebar
/// name comes from. This is what makes a bare `make dev` connect: with no
/// `DUCKTAPE_NODE` and an empty endpoint field the app fell back to a
/// hardcoded port while the seeded node listened wherever `node init` picked
/// its ports — so every first boot opened on "Could not connect" over a
/// perfectly healthy node the registry knew the address of.
pub(crate) fn registered_endpoint() -> Option<String> {
    let workspace = registry_active_entry()?;
    let http = workspace.get("ports")?.get("http")?.as_u64()?;
    Some(format!("http://127.0.0.1:{http}"))
}

/// `$DUCKTAPE_HOME`, else `~/.ducktape` — the same resolution the user key uses.
pub(crate) fn ducktape_home() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Some(PathBuf::from(root));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".ducktape"))
}

/// The CLI's workspace registry: `<ducktape home>/workspaces`. `node init` and
/// `node join` materialize one directory per network in here, so the directory
/// listing IS the registry — there is no index file to keep in sync.
fn workspaces_root() -> Option<PathBuf> {
    ducktape_home().map(|home| home.join("workspaces"))
}

/// Every registered workspace as `(chain id, directory)`: a directory holding
/// a `node.toml` is a workspace, whatever else it contains.
pub(crate) fn registered_workspaces() -> Vec<(String, PathBuf)> {
    let Some(root) = workspaces_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut workspaces: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.join("node.toml").is_file())
        .filter_map(|dir| {
            let name = dir.file_name()?.to_str()?.to_string();
            Some((name, dir))
        })
        .collect();
    workspaces.sort();
    workspaces
}

/// One top-level value out of a workspace file (`key = "value"`). `node.toml`
/// and `network.toml` are both written key-per-line by the CLI, so this reads
/// them without a toml parser the app would otherwise not need.
pub(crate) fn node_dir_value(dir: &Path, file: &str, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(file)).ok()?;
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| name.trim() == key)
        .map(|(_, value)| {
            value
                .split('#')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches(['"', '\''])
                .to_string()
        })
}

/// This workspace's app endpoint, from its `http_listen`.
pub(crate) fn workspace_endpoint(dir: &Path) -> Option<String> {
    node_dir_value(dir, "node.toml", "http_listen").map(|listen| format!("http://{listen}"))
}

/// The registered workspace this app is pointed at, matched on the endpoint it
/// is actually connected to.
pub(crate) fn workspace_at(rpc: &str) -> Option<(String, PathBuf)> {
    let endpoint = canonical_endpoint(rpc.to_string());
    registered_workspaces()
        .into_iter()
        .find(|(_, dir)| workspace_endpoint(dir).as_deref() == Some(endpoint.as_str()))
}

/// Workspaces this device has been told to forget — device-local, never wire
/// state. The directories stay on disk; the console simply stops offering them.
pub(crate) fn forgotten_workspaces() -> Vec<String> {
    read_prefs()["forgotten_workspaces"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// What `node init` / `node join` hand back: the network's id, where it
/// materialized, and the endpoint this app should connect to.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct WorkspaceInit {
    pub chain_id: String,
    pub workspace: String,
    pub rpc: String,
}

/// Materialize this device's workspace from an invite blob:
/// `ducktape node join <blob>`.
pub async fn join_network(blob: ui_lang_runtime::Secret) -> Result<WorkspaceInit, AppError> {
    async {
        let blob = blob.expose().trim();
        let valid = !blob.is_empty()
            && blob.len() <= 64 * 1024
            && !blob.chars().any(|character| character == '\0');
        if !valid {
            return Err("invite must be between 1 and 65536 bytes".into());
        }
        // `join` reports progress on stderr, so the workspace it materialized
        // is identified by diffing the registry around the call.
        let before: BTreeSet<String> = registered_workspaces()
            .into_iter()
            .map(|(chain_id, _)| chain_id)
            .collect();
        ducktape_cli(&["node", "join", blob]).await?;
        let chain_id = registered_workspaces()
            .into_iter()
            .map(|(chain_id, _)| chain_id)
            .find(|chain_id| !before.contains(chain_id))
            .ok_or_else(|| "the invite did not materialize a workspace".to_string())?;
        workspace_init(&chain_id)
    }
    .await
    .map_err(app_error)
}

/// Mint a single-use bearer invite for a workspace: `ducktape node invite`
/// prints the `🦆…` blob on stdout. This WRITES (it folds this member's dial
/// hint into the descriptor), so it is not a read-only probe.
pub async fn mint_invite(workspace: String, ttl_days: i64) -> Result<String, AppError> {
    async {
        let ttl = ttl_days.clamp(1, 365).to_string();
        ducktape_cli(&["node", "invite", "-n", &workspace, "--ttl-days", &ttl]).await
    }
    .await
    .map_err(app_error)
}

/// The workspace facts of a freshly registered chain id.
fn workspace_init(chain_id: &str) -> Result<WorkspaceInit, String> {
    let (chain_id, dir) = registered_workspaces()
        .into_iter()
        .find(|(id, _)| id == chain_id)
        .ok_or_else(|| format!("{chain_id} is not in the workspace registry"))?;
    let rpc = workspace_endpoint(&dir)
        .ok_or_else(|| "the new workspace has no node.toml http_listen".to_string())?;
    Ok(WorkspaceInit {
        chain_id,
        workspace: dir.display().to_string(),
        rpc,
    })
}

/// Run one `ducktape` verb and return its stdout's last non-empty line — the
/// CLI's machine value (diagnostics ride stderr).
async fn ducktape_cli(args: &[&str]) -> Result<String, String> {
    // Name the `<noun> <verb>` head only. The tail is values, and `node join`
    // carries the whole invite blob — hundreds of characters that, echoed into
    // the banner's fixed column, push the CLI's actual reason off the bottom.
    let verb = args[..args.len().min(2)].join(" ");
    let mut command = tokio::process::Command::new(ducktape_binary());
    command.args(args).kill_on_drop(true);
    let output = tokio::time::timeout(CLI_TIMEOUT, command.output())
        .await
        .map_err(|_| format!("ducktape {verb} timed out"))?
        .map_err(|error| {
            format!(
                "could not start the ducktape CLI ({error}); build node-bin or set DUCKTAPE_BIN"
            )
        })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ducktape {verb} refused: {}",
            bounded_detail(&detail)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .ok_or_else(|| format!("ducktape {verb} returned nothing"))
}

/// One provisioning step. `state` is `done` | `running` | `pending` | `blocked`.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ProvisionStep {
    pub index: i64,
    pub label: String,
    pub state: String,
    /// `state == "done"`, as a Copy field. The onboarding handler has to decide
    /// whether the phase advances BEFORE it moves the step into the reading,
    /// and reading `state` there would move the String out from under it.
    pub settled: bool,
}

/// The five provisioning steps. Steps 1-3 are facts of the materialized
/// workspace; steps 4-5 are a REAL `/v1/status` poll, because the app attaches
/// to a node it does not supervise — when nothing answers, the step goes
/// `blocked` and its label says which command starts it.
pub fn provision_progress(
    workspace: String,
    rpc: String,
) -> iced::futures::stream::BoxStream<'static, ProvisionStep> {
    struct State {
        dir: Option<PathBuf>,
        chain_id: String,
        rpc: String,
        step: usize,
        attempts: u32,
    }
    let found = registered_workspaces()
        .into_iter()
        .find(|(chain_id, dir)| *chain_id == workspace || dir.display().to_string() == workspace);
    let (chain_id, dir) = match found {
        Some((chain_id, dir)) => (chain_id, Some(dir)),
        None => (workspace, None),
    };
    Box::pin(iced::futures::stream::unfold(
        State {
            dir,
            chain_id,
            rpc,
            step: 0,
            attempts: 0,
        },
        |mut state| async move {
            // the workspace's own facts, then the node's own answer.
            match state.step {
                0 => {
                    state.step = 1;
                    let home = ducktape_home()
                        .map(|home| home.display().to_string())
                        .unwrap_or_else(|| "~/.ducktape".into());
                    Some((
                        registered_step(
                            1,
                            &format!("Workspace registered · {home}"),
                            state.dir.is_some(),
                        ),
                        state,
                    ))
                }
                1 => {
                    state.step = 2;
                    let key = state
                        .dir
                        .as_deref()
                        .and_then(workspace_identity)
                        .unwrap_or_default();
                    let known = !key.is_empty();
                    Some((
                        registered_step(2, &format!("Admin keypair · {key}"), known),
                        state,
                    ))
                }
                2 => {
                    state.step = 3;
                    let ready = state
                        .dir
                        .as_ref()
                        .is_some_and(|dir| dir.join("network.toml").is_file());
                    // No tail: this step proves only that `network.toml` exists,
                    // and what a member later copies is an opaque invite blob with
                    // no URI form — the artifact's "invite links available" promised
                    // a link nothing in this flow mints.
                    Some((registered_step(3, "Workspace ready", ready), state))
                }
                3 => {
                    // the app attaches to a node it does not supervise: the
                    // only honest readiness signal is the node answering.
                    let up = match rpc_client(&state.rpc) {
                        Ok(client) => client.status().await.is_ok(),
                        Err(_) => false,
                    };
                    if up {
                        state.step = 4;
                        return Some((registered_step(4, "Local node starting", true), state));
                    }
                    state.attempts += 1;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let stalled = state.attempts >= PROVISION_PATIENCE;
                    let step = match stalled {
                        false => ProvisionStep {
                            index: 4,
                            label: "Local node starting".into(),
                            state: "running".into(),
                            settled: false,
                        },
                        true => ProvisionStep {
                            index: 4,
                            label: format!(
                                "Start the node · ducktape node run -n {}",
                                state.chain_id
                            ),
                            state: "blocked".into(),
                            settled: false,
                        },
                    };
                    Some((step, state))
                }
                4 => {
                    let listen = state
                        .dir
                        .as_deref()
                        .and_then(|dir| node_dir_value(dir, "node.toml", "http_listen"))
                        .unwrap_or_else(|| state.rpc.clone());
                    state.step = 5;
                    Some((
                        ProvisionStep {
                            index: 5,
                            label: format!("Node API listening · {listen}"),
                            state: "done".into(),
                            settled: true,
                        },
                        state,
                    ))
                }
                // every step has reported; the console takes over.
                _ => None,
            }
        },
    ))
}

/// A step whose fact is either established or missing.
fn registered_step(index: i64, label: &str, established: bool) -> ProvisionStep {
    ProvisionStep {
        index,
        label: label.to_string(),
        state: match established {
            true => "done".into(),
            false => "blocked".into(),
        },
        settled: established,
    }
}

/// The workspace's own node identity, short — `network.toml` seats it as the
/// founding validator, so a fresh network's admin key is readable there.
fn workspace_identity(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("network.toml")).ok()?;
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("validators"))?;
    let key = line.split('"').nth(1)?;
    Some(short_label(key))
}

/// Forget this workspace on THIS DEVICE: it stops being offered by the shell
/// and its view prefs are dropped. The directory, the identity and the chain
/// are untouched — this is not a leave-the-network op.
pub async fn forget_workspace(rpc: String) -> Result<bool, AppError> {
    let Some((chain_id, _)) = workspace_at(&rpc) else {
        return Err(app_error(
            "this endpoint is not one of this device's registered workspaces".into(),
        ));
    };
    let mut prefs = read_prefs();
    let mut forgotten = forgotten_workspaces();
    if !forgotten.contains(&chain_id) {
        forgotten.push(chain_id);
    }
    prefs["forgotten_workspaces"] = serde_json::json!(forgotten);
    if let Some(tabs) = prefs["doc_tabs"].as_object_mut() {
        tabs.remove(&canonical_endpoint(rpc));
    }
    Ok(write_prefs(&prefs))
}

/// The titlebar's chain label: the workspace serving the CONNECTED endpoint
/// (the launch window may have picked any known network, so the registry's
/// `active` cannot answer), then the demo registry's name, then the bound
/// account, then the endpoint's host, then the product name.
pub fn network_label(account_name: impl AsRef<str>, rpc: impl AsRef<str>) -> String {
    let connected = workspace_at(rpc.as_ref()).map(|(dir_name, dir)| {
        let chain_id = node_dir_value(&dir, "network.toml", "chain_id").unwrap_or_default();
        let named = chain_id.split('#').next().unwrap_or_default();
        match named.is_empty() {
            true => dir_name,
            false => named.to_string(),
        }
    });
    if let Some(workspace) = connected {
        return workspace;
    }
    if let Some(workspace) = active_workspace_name() {
        return workspace;
    }
    let named = account_name.as_ref().trim();
    if !named.is_empty() {
        return named.to_string();
    }
    let host = rpc
        .as_ref()
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    if host.is_empty() {
        return "Ducktape".into();
    }
    host.to_string()
}

/// A consensus stamp at or above this is unix MILLIS, not a block height.
///
/// `consensus_time` is stamped `= height` by the validator lane
/// (bin/noded/src/index.rs) and `= unix_millis()` by a single-writer noded
/// (bin/noded/src/main.rs), and every module record time is that value. No
/// chain reaches 10^12 blocks and no unix-millis clock has ever been below it,
/// so the two lanes are told apart by the magnitude alone. Rendering the millis
/// lane as a height is how `h 1,753,622,400,000` reaches the screen.
const MILLIS_LANE_FLOOR: i64 = 1_000_000_000_000;

/// The wall clock a stamp carries when it came off the unix-millis lane.
fn wall_clock_seconds(stamp: i64) -> Option<i64> {
    match stamp >= MILLIS_LANE_FLOOR {
        true => Some(stamp / 1_000),
        false => None,
    }
}

/// The titlebar's machine value: `h 84,912`, grouped the way the artifact
/// writes heights. A height the node has not reported yet reads `h —`.
pub fn height_label(height: i64) -> String {
    if height < 0 {
        return "h —".into();
    }
    format!("h {}", grouped_digits(height))
}

/// The same `h 84,912` run under the name the record-meta call sites use, where
/// the artifact printed a wall clock the validator lane cannot supply. One
/// renderer on purpose — the two names mark the two slots, not two formats.
pub fn height_label_short(height: i64) -> String {
    height_label(height)
}

/// The honest renderer for a consensus-stamped record time: `412 blocks ago`,
/// `1 block ago`, `this block` — or, on the unix-millis lane, the real elapsed
/// wall clock. A record with no stamp prints nothing.
pub fn height_ago(then_height: i64, now_height: i64, wall_now: i64) -> String {
    if then_height <= 0 {
        return String::new();
    }
    if let Some(seconds) = wall_clock_seconds(then_height) {
        return relative_time(seconds, wall_now);
    }
    let elapsed = now_height.saturating_sub(then_height);
    match elapsed {
        blocks if blocks <= 0 => "this block".into(),
        1 => "1 block ago".into(),
        blocks => format!("{} blocks ago", grouped_digits(blocks)),
    }
}

/// A non-negative count with thousands separators: `84,912`.
pub(crate) fn grouped_digits(value: i64) -> String {
    let digits = value.max(0).to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        let boundary = index > 0 && (digits.len() - index).is_multiple_of(3);
        if boundary {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// TWO uppercase letters for a 28px+ avatar plate: the initials of the first
/// two words, else the first two alphanumerics of one word.
pub fn initials_of(name: impl AsRef<str>) -> String {
    let name = name.as_ref();
    let words: Vec<&str> = name.split_whitespace().take(2).collect();
    if words.len() == 2 {
        let letters: String = words
            .iter()
            .filter_map(|word| word.chars().find(char::is_ascii_alphanumeric))
            .collect();
        if letters.chars().count() == 2 {
            return letters.to_uppercase();
        }
    }
    let letters: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(2)
        .collect();
    match letters.is_empty() {
        true => "?".into(),
        false => letters.to_uppercase(),
    }
}

/// `2h ago` / `40m ago` / `just now`, for a genuine UNIX-SECONDS stamp.
///
/// In this app exactly two values qualify, both off `/v1/status`:
/// `NodeFacts.last_finalized_at` and `operations.phase_since`. NEVER call it on
/// a module record's time — the consensus validator stamps `consensus_time =
/// height` (bin/noded/src/index.rs) and a single-writer noded stamps unix
/// MILLIS, so a record time is a block height, not seconds. Render those with
/// [`height_ago`] / [`height_label_short`].
pub fn relative_time(unix_seconds: i64, wall_now: i64) -> String {
    // [`UNMEASURED`] and "this record carries no stamp" are different facts and
    // print differently: the first is a reading the node never published and
    // owes the reader a `—`, the second is a record that legitimately has no
    // time and prints nothing rather than an em dash on every row.
    if unix_seconds < 0 {
        return "—".into();
    }
    if unix_seconds == 0 {
        return String::new();
    }
    let elapsed = wall_now.saturating_sub(unix_seconds);
    if elapsed < 60 {
        return "just now".into();
    }
    let (value, unit) = duration_parts(elapsed);
    format!("{value}{unit} ago")
}

/// `expires in 412 blocks`; a passed deadline reads `expired`. A governance
/// deadline is `consensus_time + voting_period`, so on the validator lane it is
/// a HEIGHT and the remaining span is counted in blocks — never in hours. On
/// the unix-millis lane the same field genuinely is a clock, and `height` is
/// not comparable to it at all, so that lane is counted against the wall.
pub fn expires_in_blocks(deadline_height: i64, height: i64, wall_now: i64) -> String {
    if let Some(seconds) = wall_clock_seconds(deadline_height) {
        let remaining = seconds.saturating_sub(wall_now);
        if remaining <= 0 {
            return "expired".into();
        }
        let (value, unit) = duration_parts(remaining);
        return format!("expires in {value}{unit}");
    }
    let remaining = deadline_height.saturating_sub(height);
    match remaining {
        blocks if blocks <= 0 => "expired".into(),
        1 => "expires in 1 block".into(),
        blocks => format!("expires in {} blocks", grouped_digits(blocks)),
    }
}

/// A span in seconds as its largest whole unit: `(45, "m")`, `(23, "h")`.
fn duration_parts(seconds: i64) -> (i64, &'static str) {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    match seconds {
        span if span < HOUR => (span / MINUTE, "m"),
        span if span < DAY => (span / HOUR, "h"),
        span => (span / DAY, "d"),
    }
}

// A wall clock (`14:32`) and a day divider (`Today`) are DELIBERATELY absent:
// a module record's stamp is a block height on a validator network and unix
// millis on a single-writer node, so neither could be rendered honestly. The
// artifact's clock is divergence, not a gap — see height_ago/height_label_short.

/// Elapsed `mm:ss` for the huddle pills and panel.
pub fn mmss(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// The wall clock, unix seconds.
pub(crate) fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

pub fn current_wall_seconds() -> i64 {
    now_seconds()
}

/// A serde-tagged enum's variant name, whether it rode as a bare string
/// (unit variant) or as a single-key object (payload variant).
pub(crate) fn tagged_name(value: &serde_json::Value) -> String {
    value.as_str().map(str::to_string).unwrap_or_else(|| {
        value
            .as_object()
            .and_then(|tagged| tagged.keys().next().cloned())
            .unwrap_or_default()
    })
}

/// A serde `Vec<u8>` as it arrives over JSON: an array of numbers.
pub(crate) fn json_bytes(value: &serde_json::Value) -> Vec<u8> {
    value
        .as_array()
        .map(|bytes| {
            bytes
                .iter()
                .filter_map(|byte| byte.as_u64().map(|byte| byte as u8))
                .collect()
        })
        .unwrap_or_default()
}

/// A module payload in its wire form — `sdk::wire` is serde_json bytes.
pub(crate) fn encode_wire(payload: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(payload).unwrap_or_default()
}

/// The first grapheme of a display name, upper-cased, for an avatar plate.
/// The single-glyph avatar label for a name — EMPTY when there is no name.
///
/// It used to fall back to `?`, and the only principal that reaches the
/// fallback is an account nobody has named yet: its avatar sits in the corner
/// of the module rail, and a `?` in a circle there does not read as "unnamed",
/// it reads as HELP. `PrincipalPlate` draws an empty string as a bare plate,
/// which is what an identity with no name looks like.
pub fn initial_of(name: impl AsRef<str>) -> String {
    name.as_ref()
        .trim()
        .chars()
        .next()
        .map(|first| first.to_uppercase().to_string())
        .unwrap_or_default()
}

/// The local user's inbox queue, when a key exists.
///
/// An inbox member IS an origin's actor string (`sdk::Origin::actor_string`),
/// and the module now refuses a MarkRead/Clear naming any queue but the
/// submitter's own — so this is not a display handle, it is the identity the
/// signed frame will carry. It must be derived, never spelled.
pub(crate) async fn local_member() -> Option<String> {
    local_user_key()
        .await
        .map(|key| sdk::Origin::External(key).actor_string())
}
