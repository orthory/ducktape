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
    pub id: String,
    pub live: bool,
    pub height: i64,
}

/// One wallet row the launch window lists — `ducktape wallet list --json`,
/// verbatim. `state` is the key file's own reading (`encrypted` |
/// `unreadable`), so a row says whether it can sign before anyone types a
/// password into it.
#[derive(Clone, Debug, Hash, PartialEq, serde::Deserialize)]
pub struct WalletInfo {
    pub name: String,
    pub pubkey: String,
    pub state: String,
    pub active: bool,
}

/// The keystore's rows, straight off the CLI's `--json`. `None` is a shape the
/// app does not recognize; an empty list is a keystore with nothing in it.
pub(crate) fn parse_wallet_rows(json: &str) -> Option<Vec<WalletInfo>> {
    serde_json::from_str(json).ok()
}

/// A row, built. Ice reads extern structs but cannot construct one, so the
/// wallet-list test's preset needs this to seed rows the way `optimistic_message`
/// seeds chat ones.
pub fn wallet_info(name: String, pubkey: String, state: String, active: bool) -> WalletInfo {
    WalletInfo {
        name,
        pubkey,
        state,
        active,
    }
}

/// A pubkey at row width: enough hex to recognize an identity by, never the
/// full 64. Empty in, empty out — a row with no reading claims none.
pub fn short_pubkey(pubkey: &str) -> String {
    let head: String = pubkey.chars().take(16).collect();
    match head.len() < pubkey.len() {
        true => format!("{head}…"),
        false => head,
    }
}

/// The network list's one-line reminder of who it is about to sign as.
pub fn active_wallet_label(name: &str) -> String {
    if name.is_empty() {
        return "read-only — no wallet unlocked".to_string();
    }
    format!("signing as {name}")
}

/// Everything the launch window needs in one boot read: the keystore's wallet
/// rows (the login step's discriminant), why the listing is empty when it
/// failed rather than being empty, the known-network list, and how many
/// forgotten workspaces are still on disk — `hidden` is what keeps forget
/// from being a one-way door nobody can see.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HubState {
    pub wallets: Vec<WalletInfo>,
    pub wallets_error: String,
    pub networks: Vec<HubNetwork>,
    pub preselect: String,
    pub hidden: i64,
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
    let remotes = prefs["saved_remotes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
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

/// Which step the launch window opens on: an empty keystore means the create
/// ceremony, anything else lands on the wallet list — the unlock surface. An
/// unreadable key renders its refusal plate on its own row rather than on a
/// separate screen.
pub fn hub_entry_step(wallets: Vec<WalletInfo>) -> crate::HubStep {
    if wallets.is_empty() {
        crate::HubStep::Create
    } else {
        crate::HubStep::Wallets
    }
}

/// The row the wallet list preselects: the active wallet, else the first.
pub fn preselect_wallet(wallets: Vec<WalletInfo>) -> String {
    wallets
        .iter()
        .find(|row| row.active)
        .or_else(|| wallets.first())
        .map(|row| row.name.clone())
        .unwrap_or_default()
}

/// A refreshed keystore keeps the row the user picked when it survived, else
/// falls back to the fresh preselection — the same ruling
/// [`refreshed_hub_selection`] makes for networks. The refresh can land while
/// someone is typing into the selected row's password field, and re-picking
/// under them unmounts that field mid-word.
pub fn refreshed_wallet_selection(
    wallets: Vec<WalletInfo>,
    current: String,
    preselect: String,
) -> String {
    let survives = wallets.iter().any(|row| row.name == current);
    match survives {
        true => current,
        false => preselect,
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
pub fn password_problem(password: &str, confirm: &str) -> String {
    if password.chars().count() < 8 {
        return "The password needs at least 8 characters.".into();
    }
    if password != confirm {
        return "The passwords do not match.".into();
    }
    String::new()
}

/// The close/focus target for a window that may not be open. `Some(id)` names
/// it; `None` yields a fresh id that names NO window, and iced drops a
/// `window::Action::Close` for an id its manager does not hold (iced_winit
/// `lib.rs`). That no-op IS how an ice handler — which has no if-blocks and
/// whose window tasks are terminal — spells a conditional close. It is also
/// the only way to reach `target=`, which demands `window-id`, not
/// `window-id?`.
pub fn window_target(current: Option<iced::window::Id>) -> iced::window::Id {
    current.unwrap_or_else(iced::window::Id::unique)
}

/// [`window_target`] gated on a bool: while `keep` holds, yields a fresh id
/// (a no-op close); once it does not, names the window. How a branch-free
/// fold spells "close the huddle window only if the huddle ended".
pub fn window_target_unless(keep: bool, current: Option<iced::window::Id>) -> iced::window::Id {
    if keep {
        iced::window::Id::unique()
    } else {
        window_target(current)
    }
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

/// the synthetic row a `DUCKTAPE_USER_KEY` override renders as.
const ENV_WALLET: &str = "env";

/// Is the keystore bypassed? The `DUCKTAPE_USER_KEY` override is what
/// SYNTHESIZES the `env` row, so it is also the only condition under which the
/// name `env` means that row. Without this test a keystore wallet legitimately
/// named `env` would resolve to the override's file — the app would show one
/// identity and sign as another.
fn env_key_override() -> bool {
    std::env::var_os("DUCKTAPE_USER_KEY").is_some()
}

/// The keystore read is on the launch window's critical path: it decides
/// whether the first screen is the create ceremony or the wallet list, and
/// nothing renders until it answers. 10s, not `CLI_TIMEOUT`'s 120 — a wedged
/// binary must become a visible refusal, not two minutes of `loading`.
const WALLET_LIST_TIMEOUT: Duration = Duration::from_secs(10);

/// The keystore's rows. `DUCKTAPE_USER_KEY` bypasses the keystore with one
/// synthetic row so rigs and huddle lanes get the same single screen.
/// A failure is returned, never flattened to an empty list: "no wallets" sends
/// the launch window to the create ceremony, and sending someone who HAS
/// wallets there because the CLI was missing is a lie with no way back.
async fn wallet_rows() -> Result<Vec<WalletInfo>, String> {
    if env_key_override() {
        return Ok(vec![WalletInfo {
            name: ENV_WALLET.into(),
            pubkey: String::new(),
            state: user_key_state(),
            active: true,
        }]);
    }
    let listed = tokio::time::timeout(
        WALLET_LIST_TIMEOUT,
        ducktape_cli_raw(&["wallet", "list", "--json"], None, Vec::new()),
    )
    .await
    .map_err(|_| "listing the keystore timed out".to_string())?;
    let stdout = listed?;
    parse_wallet_rows(&stdout).ok_or_else(|| "the keystore listing is unreadable".to_string())
}

/// The named wallet's key file — `env` names the override path, and only while
/// the override is what put that row on screen.
fn wallet_key_path(name: &str) -> Result<PathBuf, String> {
    match env_key_override() && name == ENV_WALLET {
        true => user_key_path(),
        false => keystore_key_path(name),
    }
}

/// Whose password the console's Settings re-unlock is about: the override, else
/// the keystore's active wallet.
fn active_or_env_wallet() -> Result<String, String> {
    if env_key_override() {
        return Ok(ENV_WALLET.to_string());
    }
    let name = active_wallet_name()?;
    if name.is_empty() {
        return Err("no active wallet — pick one in the launch window".to_string());
    }
    Ok(name)
}

pub async fn hub_state() -> HubState {
    // `wallet list` is ALSO the CLI pre-warm, and it is now awaited rather
    // than fired and forgotten. On macOS the FIRST exec of a freshly built
    // binary pays Gatekeeper's whole-file assessment — measured 3.2 s on an
    // M-series mini for the ~1 GB debug `ducktape`, 0.01 s once cached — and
    // every rebuild mints a new binary, so every `make dev` session's first
    // unlock (or first signed write) ate it. Paying it HERE means the launch
    // window wears the wait once, on a screen that has nothing to type into
    // yet, instead of on the user's first click. It is also what runs the
    // keystore's legacy `user.key` adoption on a first post-upgrade boot.
    let (wallets, wallets_error) = match wallet_rows().await {
        Ok(rows) => (rows, String::new()),
        Err(cause) => {
            // The detail carries the CLI's stderr, which can name a path — it
            // reaches the screen, never the log ring. The token is the fact.
            tracing::warn!(
                target: "ducktape::app",
                reason = "wallet_list_failed",
                "the keystore listing failed; the launch window shows the refusal"
            );
            (Vec::new(), cause)
        }
    };
    let networks = known_networks();
    let forgotten = forgotten_workspaces();
    let hidden = registered_workspaces()
        .into_iter()
        .filter(|(dir_name, _)| forgotten.contains(dir_name))
        .count() as i64;
    HubState {
        wallets,
        wallets_error,
        preselect: preselect_id(&networks),
        networks,
        hidden,
    }
}

/// Empty the forgotten-workspaces tombstone list — every hidden local
/// network reappears in the picker. The one door back: a forgotten dir
/// cannot be re-joined (the workspace already exists on disk), so without
/// this a forget was irreversible from the UI.
pub async fn restore_hidden_networks() -> bool {
    let mut prefs = read_prefs();
    prefs["forgotten_workspaces"] = serde_json::json!([]);
    write_prefs(&prefs)
}

/// Merge one probe answer into the list by row id.
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
pub fn network_run_hint(row: &HubNetwork) -> String {
    if row.kind != "local" {
        return "node unreachable".into();
    }
    let selector = match row.chain_id.is_empty() {
        true => &row.id,
        false => &row.chain_id,
    };
    format!("not running · ducktape node run -n {selector}")
}

/// Probe every known network's endpoint, emitting one reading per row as it
/// answers. Bounded: one `/v1/status` with a short timeout per endpoint.
pub fn probe_known_networks() -> iced::futures::stream::BoxStream<'static, HubProbe> {
    use iced::futures::StreamExt;
    let probes = known_networks().into_iter().map(move |row| async move {
        let reading = probe_endpoint(&row.endpoint).await;
        HubProbe {
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
            let mut remotes = prefs["saved_remotes"]
                .as_array()
                .cloned()
                .unwrap_or_default();
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
        let remotes = prefs["saved_remotes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
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

/// `wallet new <name>` — mint a named wallet under `password`. Returns the 24
/// recovery words and pubkey; the fresh wallet becomes the active one and the
/// in-process key cache is refreshed, so the new identity signs without a
/// restart.
///
/// THE WORDS COME BACK FIRST. Once `wallet new` returns, a sealed key exists on
/// disk whose ONLY backup is the phrase in this stdout — an error after that
/// point destroys the phrase and leaves the key. So the pointer write is not
/// allowed to fail this call: it degrades to a warning, and the user lands on
/// a wallet that is minted but not active, which the list can still fix.
pub async fn create_user_key(name: String, password: String) -> Result<KeyCreated, AppError> {
    let created = async {
        check_wallet_name(&name)?;
        let stdout = ducktape_cli(&["wallet", "new", &name], None, password).await?;
        let mut lines = stdout.lines().filter(|line| !line.trim().is_empty()).rev();
        let pubkey = lines.next().unwrap_or_default().trim().to_string();
        let words = lines.next().unwrap_or_default().trim().to_string();
        if words.split_whitespace().count() != 24 {
            return Err("the key tool did not return a 24-word recovery phrase".to_string());
        }
        Ok(KeyCreated { words, pubkey })
    }
    .await
    .map_err(app_error)?;
    if activate_wallet(&name).await.is_err() {
        tracing::warn!(
            target: "ducktape::app",
            reason = "wallet_activate_failed",
            "the minted wallet is not the active one; pick it in the launch window"
        );
    }
    set_local_user_key(hex_decode(&created.pubkey).ok()).await;
    Ok(created)
}

/// `wallet import <name>` — re-seal an identity from its 24 words under a new
/// password. Returns the pubkey.
pub async fn restore_user_key(
    name: String,
    words: ui_lang_runtime::Secret,
    mut password: String,
) -> Result<String, AppError> {
    async {
        check_wallet_name(&name)?;
        let normalized = Zeroizing::new(
            words
                .expose()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
        if normalized.split(' ').count() != 24 {
            return Err("a recovery phrase is exactly 24 words".to_string());
        }
        let password_line = secret_line(&password);
        password.zeroize();
        let input = secret_line(&normalized)?
            .into_iter()
            .chain(password_line?)
            .collect::<Vec<u8>>();
        let stdout = ducktape_cli_raw(&["wallet", "import", &name], None, input).await?;
        let pubkey = last_line(&stdout)?;
        activate_wallet(&name).await?;
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(pubkey)
    }
    .await
    .map_err(app_error)
}

/// Unlock the NAMED wallet: a decrypt probe that succeeds iff `password` opens
/// it, followed by the pointer write that makes it the wallet every keyless
/// verb signs with. The pubkey the probe just proved seeds the in-process
/// identity cache — without it the first hydrate pays a `user key status`
/// subprocess to re-read what this derivation already paid 64 MiB to learn.
pub async fn unlock_wallet(name: String, password: String) -> Result<String, AppError> {
    async {
        let path = wallet_key_path(&name)?;
        let stdout =
            ducktape_cli(&["user", "key", "unlock", "--key"], Some(&path), password).await?;
        let pubkey = last_line(&stdout)?;
        activate_wallet(&name).await?;
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(pubkey)
    }
    .await
    .map_err(app_error)
}

/// `wallet use <name>` — the active pointer write. The env override names no
/// keystore row, so it has no pointer to move.
async fn activate_wallet(name: &str) -> Result<(), String> {
    check_wallet_name(name)?;
    if env_key_override() && name == ENV_WALLET {
        return Ok(());
    }
    ducktape_cli_raw(&["wallet", "use", name], None, Vec::new())
        .await
        .map(|_| ())
}

/// The console's Settings re-unlock, which knows a password and nothing else:
/// it re-proves the wallet this session is already signing with.
pub async fn unlock_user_key(password: String) -> Result<String, AppError> {
    let name = active_or_env_wallet().map_err(app_error)?;
    unlock_wallet(name, password).await
}

/// One secret as the CLI's stdin field: the bytes plus the newline, refusing
/// embedded delimiters the same way `password_line` does.
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

async fn ducktape_cli(
    args: &[&str],
    path: Option<&Path>,
    password: String,
) -> Result<String, String> {
    let mut password = password;
    let input = secret_line(&password);
    password.zeroize();
    ducktape_cli_raw(args, path, input?).await
}

/// Run `ducktape <args…> [path]` with secrets piped over stdin (the CLI's only
/// secret channel), returning full stdout. `path` is the trailing argument the
/// `user key` verbs take after their `--out`/`--key` flag; the `wallet` verbs
/// name their key by wallet name and pass `None`.
async fn ducktape_cli_raw(
    args: &[&str],
    path: Option<&Path>,
    input: Vec<u8>,
) -> Result<String, String> {
    let input = Zeroizing::new(input);
    // The VERB names itself in a refusal, not the argv: `--key`/`--json` are
    // plumbing the person reading the error did not type and cannot act on.
    let verb = args
        .iter()
        .take_while(|arg| !arg.starts_with('-'))
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .args(args)
        .args(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        format!(
            "could not start the ducktape key tool ({error}); build node-bin or set DUCKTAPE_BIN"
        )
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
        .map_err(|_| format!("ducktape {verb} timed out"))?
        .map_err(|error| format!("ducktape {verb} failed: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ducktape {verb} refused: {}",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(actives: &[(&str, bool)]) -> Vec<WalletInfo> {
        actives
            .iter()
            .map(|(name, active)| WalletInfo {
                name: name.to_string(),
                pubkey: String::new(),
                state: "encrypted".into(),
                active: *active,
            })
            .collect()
    }

    /// The keystore decides both the entry step and the preselected row: an
    /// empty one is the create ceremony, and the active wallet is the row the
    /// list opens on.
    #[test]
    fn entry_step_and_preselect_follow_the_keystore() {
        assert!(matches!(hub_entry_step(vec![]), crate::HubStep::Create));
        assert!(matches!(
            hub_entry_step(rows(&[("a", false)])),
            crate::HubStep::Wallets
        ));
        assert_eq!(preselect_wallet(rows(&[("a", false), ("b", true)])), "b");
        assert_eq!(preselect_wallet(rows(&[("a", false)])), "a");
        assert_eq!(preselect_wallet(vec![]), "");
    }

    /// A refresh that lands while someone is on the wallet list must not
    /// re-pick under them — only a selection whose row is GONE falls back.
    #[test]
    fn a_refresh_keeps_the_row_the_user_picked() {
        let listed = rows(&[("a", false), ("b", true)]);
        assert_eq!(
            refreshed_wallet_selection(listed.clone(), "a".into(), "b".into()),
            "a"
        );
        assert_eq!(
            refreshed_wallet_selection(listed, "gone".into(), "b".into()),
            "b"
        );
        assert_eq!(
            refreshed_wallet_selection(vec![], "a".into(), String::new()),
            ""
        );
    }

    /// `wallet list --json` verbatim, extra fields and all.
    #[test]
    fn wallet_rows_parse_the_list_json() {
        let json = r#"[{"name":"demo","pubkey":"ab","state":"encrypted","active":true,"path":"/x/demo.key"}]"#;
        let rows = parse_wallet_rows(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "demo");
        assert!(rows[0].active);
        assert!(parse_wallet_rows("not json").is_none());
    }

    /// A wallet name is an argv word AND a path segment. Both readings are
    /// refused before either one is built: a leading `-` would be read as a
    /// clap flag (`wallet use -h` prints help and exits 0 — a refusal wearing
    /// a success), and `..`/`/` would walk the key path out of the keystore.
    /// The check lives in `keystore_key_path`, so a name that never passed
    /// through a person — the `active` pointer file's contents — is gated too.
    #[test]
    fn a_wallet_name_is_never_a_flag_or_a_path() {
        for name in ["env", "default", "alice2", "a.b_c-d", "0"] {
            assert!(check_wallet_name(name).is_ok(), "{name} should be valid");
        }
        let refused = [
            "",
            "-h",
            "--json",
            "..",
            "../x",
            "a/b",
            "/etc/passwd",
            "Alice",
            ".hidden",
        ];
        for name in refused {
            assert!(check_wallet_name(name).is_err(), "{name} should be refused");
            // both the wallet-facing join and the pointer-derived one, which
            // is what every signer spawn and `user_key_state` resolve through.
            assert!(wallet_key_path(name).is_err(), "{name} built a path");
            let refusal = keystore_key_path(name).expect_err("built a path");
            assert!(
                refusal.contains("wallet name"),
                "unnamed refusal: {refusal}"
            );
        }
        assert!(check_wallet_name(&"a".repeat(41)).is_ok());
        assert!(check_wallet_name(&"a".repeat(42)).is_err());
    }

    /// A row's pubkey is shortened, never invented.
    #[test]
    fn short_pubkey_and_wallet_label_say_only_what_they_know() {
        assert_eq!(short_pubkey(""), "");
        assert_eq!(short_pubkey("abcd"), "abcd");
        assert_eq!(
            short_pubkey(&"a".repeat(64)),
            format!("{}…", "a".repeat(16))
        );
        assert_eq!(active_wallet_label("demo"), "signing as demo");
        assert_eq!(active_wallet_label(""), "read-only — no wallet unlocked");
    }
}
