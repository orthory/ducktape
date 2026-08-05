use super::*;

/// One row of the launch window's network list. `id` is the row's stable
/// device-local key: the workspace DIRECTORY name for a materialized network,
/// the canonical endpoint for a saved remote. `chain_id` is the network's own
/// identity out of `network.toml` (empty for a remote the device holds no
/// descriptor for) — it is also the exact `-n` selector later CLI calls need.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HubNetwork {
    pub id: String,
    pub chain_id: String,
    pub name: String,
    pub endpoint: String,
    pub kind: String,
    pub last_used: i64,
    /// The liveness reading, merged in by `probe_known_networks`. `probed`
    /// distinguishes "not answered yet" from a measured dead node — the row
    /// must not claim either before the probe returns.
    pub probed: bool,
    pub live: bool,
    pub height: i64,
}

/// One probe answer. Never an error: a node that does not answer IS the
/// reading (`live == false`), not a failure to hide behind a banner.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HubProbe {
    pub generation: i64,
    pub id: String,
    pub live: bool,
    pub height: i64,
}

/// Everything the launch window needs in one boot read: the local user key's
/// state (the login step's discriminant) and the known-network list.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HubState {
    pub key_state: String,
    pub networks: Vec<HubNetwork>,
    pub preselect: String,
}

/// What `user key init` hands back: the 24 recovery words (shown exactly
/// once) and the minted pubkey.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct KeyCreated {
    pub words: String,
    pub pubkey: String,
}

/// `absent` | `encrypted` | `plaintext` | `unlocatable` — the same probe
/// Settings runs, computed in-process (no subprocess, no password).
pub(crate) fn user_key_state() -> String {
    let Ok(path) = user_key_path() else {
        return "unlocatable".into();
    };
    match std::fs::read(&path) {
        Err(_) => "absent".into(),
        Ok(bytes) if bytes.starts_with(ENCRYPTED_KEY_PREFIX.as_bytes()) => "encrypted".into(),
        Ok(_) => "plaintext".into(),
    }
}

/// A network's display name is the human half of its chain id: `demo#a1b2`
/// reads `demo`. A remote row falls back to its endpoint sans scheme.
fn display_name(chain_id: &str, fallback: &str) -> String {
    let named = chain_id.split('#').next().unwrap_or_default();
    if !named.is_empty() {
        return named.to_string();
    }
    fallback
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string()
}

/// The known-network list: every unforgotten workspace directory plus every
/// saved remote endpoint, most recently used first.
pub(crate) fn known_networks() -> Vec<HubNetwork> {
    let prefs = read_prefs();
    let forgotten = forgotten_workspaces();
    let stamps = &prefs["network_last_used"];
    let mut rows: Vec<HubNetwork> = registered_workspaces()
        .into_iter()
        .filter(|(dir_name, _)| !forgotten.contains(dir_name))
        .map(|(dir_name, dir)| {
            let chain_id = node_dir_value(&dir, "network.toml", "chain_id").unwrap_or_default();
            let endpoint = workspace_endpoint(&dir).unwrap_or_default();
            HubNetwork {
                name: display_name(&chain_id, &dir_name),
                chain_id,
                endpoint,
                kind: "local".into(),
                last_used: stamps[&dir_name].as_i64().unwrap_or(0),
                id: dir_name,
                probed: false,
                live: false,
                height: -1,
            }
        })
        .collect();
    let remotes = prefs["saved_remotes"].as_array().cloned().unwrap_or_default();
    for remote in remotes {
        let Some(endpoint) = remote["endpoint"].as_str() else {
            continue;
        };
        let already_local = rows.iter().any(|row| row.endpoint == endpoint);
        if already_local {
            continue;
        }
        rows.push(HubNetwork {
            id: endpoint.to_string(),
            chain_id: String::new(),
            name: display_name("", endpoint),
            endpoint: endpoint.to_string(),
            kind: "remote".into(),
            last_used: stamps[endpoint].as_i64().unwrap_or(0),
            probed: false,
            live: false,
            height: -1,
        });
    }
    rows.sort_by(|a, b| b.last_used.cmp(&a.last_used).then(a.id.cmp(&b.id)));
    rows
}

/// The row the list preselects: the most recently used, else the CLI
/// registry's `active` workspace, else the first row.
fn preselect_id(rows: &[HubNetwork]) -> String {
    let last_used = rows.iter().find(|row| row.last_used > 0);
    if let Some(row) = last_used {
        return row.id.clone();
    }
    let registry_active = registry_active_workspace();
    let active = rows
        .iter()
        .find(|row| Some(row.id.as_str()) == registry_active.as_deref());
    if let Some(row) = active {
        return row.id.clone();
    }
    rows.first().map(|row| row.id.clone()).unwrap_or_default()
}

/// Which step the launch window opens on: no key on disk means the create
/// ceremony, anything else lands on unlock — a plaintext or unlocatable key
/// renders its refusal plate there rather than on a separate screen.
pub fn hub_entry_step(key_state: String) -> String {
    match key_state.as_str() {
        "absent" => "create".into(),
        _ => "unlock".into(),
    }
}

/// A refreshed list keeps the user's selection when its row survived, else
/// falls back to the fresh preselection.
pub fn refreshed_hub_selection(
    networks: Vec<HubNetwork>,
    current: String,
    preselect: String,
) -> String {
    let survives = networks.iter().any(|row| row.id == current);
    match survives {
        true => current,
        false => preselect,
    }
}

/// The selected row's endpoint, or empty when the selection no longer names
/// a row (a forget can race the click).
pub fn selected_network_endpoint(networks: Vec<HubNetwork>, id: String) -> String {
    networks
        .into_iter()
        .find(|row| row.id == id)
        .map(|row| row.endpoint)
        .unwrap_or_default()
}

/// The create ceremony's refusal, or empty when the pair is acceptable —
/// the same floor the CLI enforces (8 scalar chars).
pub fn password_problem(password: String, confirm: String) -> String {
    if password.chars().count() < 8 {
        return "The password needs at least 8 characters.".into();
    }
    if password != confirm {
        return "The passwords do not match.".into();
    }
    String::new()
}

/// Clear a tracked window id when it is the one that closed.
pub fn without_window(
    current: Option<iced::window::Id>,
    closed: iced::window::Id,
) -> Option<iced::window::Id> {
    match current == Some(closed) {
        true => None,
        false => current,
    }
}

pub async fn hub_state() -> HubState {
    let networks = known_networks();
    HubState {
        key_state: user_key_state(),
        preselect: preselect_id(&networks),
        networks,
    }
}

/// Merge one probe answer into the list — by row id, generations already
/// checked by the handler.
pub fn apply_network_probe(networks: Vec<HubNetwork>, probe: HubProbe) -> Vec<HubNetwork> {
    networks
        .into_iter()
        .map(|mut row| {
            if row.id == probe.id {
                row.probed = true;
                row.live = probe.live;
                row.height = probe.height;
            }
            row
        })
        .collect()
}

/// The command that starts a dead local network's node — the honest row
/// subtitle, same doctrine as provisioning's `blocked` step.
pub fn network_run_hint(row: HubNetwork) -> String {
    if row.kind != "local" {
        return "node unreachable".into();
    }
    let selector = match row.chain_id.is_empty() {
        true => row.id,
        false => row.chain_id,
    };
    format!("not running · ducktape node run -n {selector}")
}

/// Probe every known network's endpoint, emitting one reading per row as it
/// answers. Bounded: one `/v1/status` with a short timeout per endpoint.
pub fn probe_known_networks(
    generation: i64,
) -> iced::futures::stream::BoxStream<'static, HubProbe> {
    use iced::futures::StreamExt;
    let probes = known_networks().into_iter().map(move |row| async move {
        let reading = probe_endpoint(&row.endpoint).await;
        HubProbe {
            generation,
            id: row.id,
            live: reading.is_some(),
            height: reading.unwrap_or(-1),
        }
    });
    iced::futures::stream::iter(probes)
        .buffer_unordered(8)
        .boxed()
}

/// One bounded status read: the height when the node answers, `None` when it
/// does not. 3s, not `RPC_TIMEOUT` — a liveness dot must not hang the list.
async fn probe_endpoint(endpoint: &str) -> Option<i64> {
    if endpoint.is_empty() {
        return None;
    }
    let client = rpc_client(endpoint).ok()?;
    let status = tokio::time::timeout(Duration::from_secs(3), client.status())
        .await
        .ok()?
        .ok()?;
    Some(status.height as i64)
}

/// Stamp a network's last-used time and — for an endpoint no workspace
/// directory serves — remember it as a saved remote. Called on every
/// successful console connect; best-effort, a failed write costs only the
/// next boot's sort order.
pub async fn remember_network(rpc: String) -> bool {
    let endpoint = canonical_endpoint(rpc);
    if endpoint.is_empty() {
        return false;
    }
    let now = unix_now();
    let mut prefs = read_prefs();
    let key = match workspace_at(&endpoint) {
        Some((dir_name, _)) => dir_name,
        None => {
            let mut remotes = prefs["saved_remotes"].as_array().cloned().unwrap_or_default();
            let known = remotes
                .iter()
                .any(|remote| remote["endpoint"].as_str() == Some(endpoint.as_str()));
            if !known {
                remotes.push(serde_json::json!({ "endpoint": endpoint }));
                prefs["saved_remotes"] = serde_json::json!(remotes);
            }
            endpoint.clone()
        }
    };
    prefs["network_last_used"][&key] = serde_json::json!(now);
    write_prefs(&prefs)
}

/// Drop a row from the list. A local network is hidden the way Settings
/// already hides one (`forgotten_workspaces` — the directory survives); a
/// saved remote is simply removed.
pub async fn forget_network(id: String, kind: String) -> bool {
    let mut prefs = read_prefs();
    if kind == "remote" {
        let remotes = prefs["saved_remotes"].as_array().cloned().unwrap_or_default();
        let kept: Vec<_> = remotes
            .into_iter()
            .filter(|remote| remote["endpoint"].as_str() != Some(id.as_str()))
            .collect();
        prefs["saved_remotes"] = serde_json::json!(kept);
        return write_prefs(&prefs);
    }
    let mut forgotten = forgotten_workspaces();
    if !forgotten.contains(&id) {
        forgotten.push(id);
    }
    prefs["forgotten_workspaces"] = serde_json::json!(forgotten);
    write_prefs(&prefs)
}

/// `user key init` — mint this device's user key under `password`. Returns
/// the 24 recovery words and pubkey; refreshes the in-process key cache so
/// the fresh identity is visible without a restart.
pub async fn create_user_key(password: String) -> Result<KeyCreated, AppError> {
    async {
        let path = user_key_path()?;
        let stdout = user_key_cli(&["init", "--out"], &path, password).await?;
        let mut lines = stdout.lines().filter(|line| !line.trim().is_empty()).rev();
        let pubkey = lines.next().unwrap_or_default().trim().to_string();
        let words = lines.next().unwrap_or_default().trim().to_string();
        if words.split_whitespace().count() != 24 {
            return Err("the key tool did not return a 24-word recovery phrase".to_string());
        }
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(KeyCreated { words, pubkey })
    }
    .await
    .map_err(app_error)
}

/// `user key restore` — re-seal an identity from its 24 words under a new
/// password. Returns the pubkey.
pub async fn restore_user_key(words: String, password: String) -> Result<String, AppError> {
    async {
        let normalized = words.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.split(' ').count() != 24 {
            return Err("a recovery phrase is exactly 24 words".to_string());
        }
        let path = user_key_path()?;
        let input = secret_line(&normalized)?
            .into_iter()
            .chain(secret_line(&password)?)
            .collect::<Vec<u8>>();
        let stdout = user_key_cli_raw(&["restore", "--out"], &path, input).await?;
        let pubkey = last_line(&stdout)?;
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(pubkey)
    }
    .await
    .map_err(app_error)
}

/// `user key unlock` — a pure decrypt probe: succeeds iff `password` opens
/// this device's key. Nothing persists; the login step's verifier.
pub async fn unlock_user_key(password: String) -> Result<String, AppError> {
    async {
        let path = user_key_path()?;
        let stdout = user_key_cli(&["unlock", "--key"], &path, password).await?;
        last_line(&stdout)
    }
    .await
    .map_err(app_error)
}

/// One secret as the CLI's stdin field: the bytes plus the newline, refusing
/// embedded delimiters the same way `signing_input` does.
fn secret_line(value: &str) -> Result<Vec<u8>, String> {
    let invalid = value.len() > 16 * 1024
        || value
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, 0 | b'\n' | b'\r'));
    if invalid {
        return Err("a secret field is too long or contains a line delimiter".into());
    }
    if value.is_empty() {
        return Err("enter the key password".into());
    }
    let mut line = Vec::with_capacity(value.len() + 1);
    line.extend_from_slice(value.as_bytes());
    line.push(b'\n');
    Ok(line)
}

fn last_line(stdout: &str) -> Result<String, String> {
    stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .ok_or_else(|| "the key tool returned nothing".to_string())
}

async fn user_key_cli(args: &[&str], path: &Path, password: String) -> Result<String, String> {
    let mut password = password;
    let input = secret_line(&password);
    password.zeroize();
    user_key_cli_raw(args, path, input?).await
}

/// Run `ducktape user key <verb> --out|--key <path>` with secrets piped over
/// stdin (the CLI's only secret channel), returning full stdout.
async fn user_key_cli_raw(args: &[&str], path: &Path, input: Vec<u8>) -> Result<String, String> {
    let input = Zeroizing::new(input);
    let (verb, path_flag) = match args {
        [verb, flag] => (*verb, *flag),
        _ => return Err("malformed key verb".into()),
    };
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("key")
        .arg(verb)
        .arg(path_flag)
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        format!("could not start the ducktape key tool ({error}); build node-bin or set DUCKTAPE_BIN")
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ducktape key tool stdin is unavailable".to_string())?;
    stdin
        .write_all(&input)
        .await
        .map_err(|error| format!("could not reach the ducktape key tool: {error}"))?;
    drop(stdin);
    let output = tokio::time::timeout(CLI_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| format!("ducktape user key {verb} timed out"))?
        .map_err(|error| format!("ducktape user key {verb} failed: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ducktape user key {verb} refused: {}",
            bounded_detail(&detail)
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "the key tool returned non-UTF-8".into())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}
