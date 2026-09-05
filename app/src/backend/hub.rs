use super::*;

/// One row of the launch window's network list. `id` is the row's stable
/// device-local key: the chain id for a materialized network (the same id the
/// CLI registry lists), the canonical endpoint for a saved remote. `chain_id`
/// is the network's own identity out of `network.toml` (empty for a remote the
/// device holds no descriptor for) — it is also the exact `-n` selector later
/// CLI calls need.
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

/// One wallet row the launch window lists, straight off `keystore::wallet`.
/// `state` is the key file's own reading (`encrypted` | `unreadable`), so a
/// row says whether it can sign before anyone types a password into it.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct WalletInfo {
    pub name: String,
    pub pubkey: String,
    pub state: String,
    pub active: bool,
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

/// `unlocatable` when this device has no key path to read at all, else the
/// keystore's own reading of that file (`absent` | `encrypted` | `unreadable`)
/// — the same classification the wallet listing shows, computed in-process (no
/// subprocess, no password).
pub(crate) fn user_key_state() -> String {
    let Ok(path) = user_key_path() else {
        return "unlocatable".into();
    };
    keystore::userkey::key_file_state(&path).as_str().into()
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
        .filter(|(chain_id, _)| !forgotten.contains(chain_id))
        .map(|(chain_id, dir)| {
            let endpoint = workspace_endpoint(&dir).unwrap_or_default();
            HubNetwork {
                name: display_name(&chain_id, &chain_id),
                endpoint,
                kind: "local".into(),
                last_used: stamps[&chain_id].as_i64().unwrap_or(0),
                id: chain_id.clone(),
                chain_id,
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

/// Which step the launch window opens on: an empty keystore means the
/// password step (the device key is minted under it), anything else lands on
/// the wallet list — the unlock surface. An unreadable key renders its refusal
/// plate on its own row rather than on a separate screen.
pub fn hub_entry_step(wallets: Vec<WalletInfo>) -> crate::HubStep {
    if wallets.is_empty() {
        crate::HubStep::Password
    } else {
        crate::HubStep::Wallets
    }
}

/// The name an auto-minted device key gets: this host's name, in the
/// keystore's grammar; `device` when the host has none to give. Asked of
/// the kernel (`gethostname`), not `/etc/hostname` — macOS has no such file
/// and a GUI app inherits no `HOSTNAME`.
pub fn device_key_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let name = keystore::wallet::sanitize_name(host.trim());
    if name.is_empty() {
        return "device".to_string();
    }
    name
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

/// What the status item's "Open" row has to do, as the discriminant the
/// handler branches on once. Closing a window no longer ends the process, so
/// the daemon can be running with both slots empty — and [`window_target`] on
/// an empty slot names a FRESH id, whose focus is a no-op, which would make a
/// raise-only row do nothing at all. With nothing tracked, open.
pub fn tray_open_action(
    console: Option<iced::window::Id>,
    onboarding: Option<iced::window::Id>,
) -> crate::TrayOpen {
    match console.is_none() && onboarding.is_none() {
        true => crate::TrayOpen::Open,
        false => crate::TrayOpen::Raise,
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

/// The keystore's rows. `DUCKTAPE_USER_KEY` bypasses the keystore with one
/// synthetic row so rigs and huddle lanes get the same single screen.
/// A failure is returned, never flattened to an empty list: "no wallets" sends
/// the launch window to the create ceremony, and sending someone who HAS
/// wallets there because a directory would not read is a lie with no way back.
async fn wallet_rows() -> Result<Vec<WalletInfo>, String> {
    if env_key_override() {
        return Ok(vec![WalletInfo {
            name: ENV_WALLET.into(),
            pubkey: String::new(),
            state: user_key_state(),
            active: true,
        }]);
    }
    let listed = keystore::wallet::list(&duck_home()?)?;
    Ok(listed
        .into_iter()
        .map(|row| WalletInfo {
            name: row.name,
            pubkey: row.pubkey,
            state: row.state.to_string(),
            active: row.active,
        })
        .collect())
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
    // This read is also what runs the keystore's legacy `user.key` adoption on
    // a first post-upgrade boot, and it is a directory listing rather than a
    // subprocess now — so the macOS Gatekeeper assessment the launch window
    // used to absorb here (3.2 s on a freshly built ~1 GB debug `ducktape`)
    // simply is not paid: nothing on the key path execs anything.
    let (wallets, wallets_error) = match wallet_rows().await {
        Ok(rows) => (rows, String::new()),
        Err(cause) => {
            // The detail can name a path — it reaches the screen, never the
            // log ring. The token is the fact.
            tracing::warn!(
                target: "ducktape::app",
                reason = "wallet_list_failed",
                "the keystore listing failed; the launch window shows the refusal"
            );
            (Vec::new(), user_error(cause))
        }
    };
    let networks = known_networks();
    let forgotten = forgotten_workspaces();
    let hidden = registered_workspaces()
        .into_iter()
        .filter(|(chain_id, _)| forgotten.contains(chain_id))
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
/// does not. 3s — a liveness dot must not hang the list.
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
        Some((chain_id, _)) => chain_id,
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

/// Run one keystore ceremony OFF the async runtime.
///
/// Every one of them spends an argon2id pass over 64 MiB — 200-400 ms of
/// memory-hard work. On a tokio worker that is 400 ms during which the UI's
/// other tasks do not run, and this app's tasks are what repaint it.
async fn in_the_keystore<T: Send + 'static>(
    ceremony: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(ceremony)
        .await
        .map_err(|_| "the keystore operation did not finish".to_string())?
}

/// BEGIN this device's key: pick its name and its 24 words, and write
/// NOTHING. The seed lives only in [`MINTED_PHRASE`] until
/// [`confirm_recovery_phrase`] reads three of the words back and seals the
/// key file — so a key file that exists is a key whose only backup someone
/// has confirmed they hold, and a ceremony abandoned halfway leaves no
/// half-founded identity behind for the account screens to find.
///
/// The password is checked HERE (an 8-char floor is not worth learning after
/// writing 24 words down) and the name is claimed here too — after the host
/// (`-2`… on a collision). Returns the wallet name the seal will use.
pub async fn create_device_key(password: String) -> Result<String, AppError> {
    async {
        require_password(&password)?;
        let duck = duck_home()?;
        let base = device_key_name();
        let candidates =
            std::iter::once(base.clone()).chain((2..10).map(|n| format!("{base}-{n}")));
        let name = candidates
            .into_iter()
            .find(|name| !keystore::wallet::key_file(&duck, name).exists())
            .ok_or_else(|| {
                "this host already holds nine device keys — pick one in the wallet list".to_string()
            })?;
        let mut seed = Zeroizing::new([0u8; 32]);
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, seed.as_mut_slice());
        hold_minted_phrase(
            name.clone(),
            Zeroizing::new(keystore::userkey::mnemonic_of_seed(&seed)),
        )?;
        Ok(name)
    }
    .await
    .map_err(app_error)
}

// ============================================================================
// the recovery-phrase ceremony — show the 24 words once, then confirm three
// ============================================================================

/// THE ONLY COPY of a phrase whose key does not exist yet, alive between
/// [`create_device_key`] and the confirm that seals it. It is deliberately
/// NOT app state: the screen asks for its rows, the confirm asks whether
/// three typed words match, and neither answer hands the phrase to anything
/// that could log, snapshot or persist it. The rows the screen draws are the
/// one copy that leaves — a render-time `String` per word, gone with the
/// step. Sealing drops the phrase: the words are shown once, and this app
/// has no verb that shows them again (`ducktape user key reveal` reads them
/// back off the key file, which is the thing the phrase is a backup FOR).
static MINTED_PHRASE: std::sync::Mutex<Option<MintedPhrase>> = std::sync::Mutex::new(None);

struct MintedPhrase {
    /// the wallet name the confirm will seal these words under.
    name: String,
    words: Zeroizing<String>,
    /// the three 1-based positions this ceremony asks back, ascending.
    asked: [usize; 3],
}

/// One row of the phrase screen's two-column grid. Ice cannot index a list,
/// so the pairing (`1`/`13`, `2`/`14`, …) is done here — twelve rows fit the
/// launch window without a scroll, twenty-four do not.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PhraseRow {
    pub left_number: String,
    pub left_word: String,
    pub right_number: String,
    pub right_word: String,
}

/// Stash a phrase for the ceremony and pick the words it will ask back.
///
/// A phrase too short to ask three distinct words out of is refused rather
/// than padded: `mnemonic_of_seed` is always 24, so the only way here is a
/// caller that has stopped handing over a mnemonic, and a prompt that asks
/// for position 1 three times would hide that instead of showing it.
fn hold_minted_phrase(name: String, words: Zeroizing<String>) -> Result<(), String> {
    use rand::seq::SliceRandom as _;
    let count = words.split_whitespace().count();
    let mut positions: Vec<usize> = (1..=count).collect();
    positions.shuffle(&mut rand::rngs::OsRng);
    let [first, second, third, ..] = positions[..] else {
        return Err(format!("a recovery phrase is 24 words, not {count}"));
    };
    let mut asked = [first, second, third];
    asked.sort_unstable();
    let mut held = MINTED_PHRASE.lock().expect("minted recovery phrase");
    *held = Some(MintedPhrase { name, words, asked });
    Ok(())
}

/// The phrase screen's rows, or none when no ceremony is in flight — the
/// phrase step is reachable only straight off a mint.
pub fn phrase_rows() -> Vec<PhraseRow> {
    let held = MINTED_PHRASE.lock().expect("minted recovery phrase");
    held.as_ref()
        .map(|phrase| phrase_rows_of(&phrase.words))
        .unwrap_or_default()
}

/// The pairing itself, over any phrase — the screen's own test drives this
/// with a FIXED mnemonic so a capture never carries a real one.
pub fn phrase_rows_of(words: &str) -> Vec<PhraseRow> {
    let words: Vec<&str> = words.split_whitespace().collect();
    let half = words.len().div_ceil(2);
    (0..half)
        .map(|row| PhraseRow {
            left_number: format!("{}", row + 1),
            left_word: words[row].to_string(),
            right_number: format!("{}", row + half + 1),
            right_word: words.get(row + half).copied().unwrap_or("").to_string(),
        })
        .collect()
}

/// "5, 12 and 20" — the positions, in the sentence the two screens name them
/// in. Ice cannot concatenate, so both sentences are built here.
fn asked_label(asked: &[usize; 3]) -> String {
    format!("{}, {} and {}", asked[0], asked[1], asked[2])
}

/// What the confirm step asks for, or empty when no ceremony is in flight.
pub fn recovery_prompt() -> String {
    let held = MINTED_PHRASE.lock().expect("minted recovery phrase");
    let Some(phrase) = held.as_ref() else {
        return String::new();
    };
    format!(
        "Type words {} — in that order, separated by spaces.",
        asked_label(&phrase.asked)
    )
}

/// THE CEREMONY'S ONE GATE, decided and nothing else: the three words back,
/// at the positions [`recovery_prompt`] named, case-insensitively. Hands the
/// seal below the wallet to write; the refusal names the positions and never
/// a word.
fn confirmed_phrase(answer: &str) -> Result<(String, Zeroizing<String>), String> {
    let held = MINTED_PHRASE.lock().expect("minted recovery phrase");
    let Some(phrase) = held.as_ref() else {
        return Err("there is no recovery phrase waiting to be confirmed".to_string());
    };
    let words: Vec<&str> = phrase.words.split_whitespace().collect();
    let typed: Vec<&str> = answer.split_whitespace().collect();
    let all_three_typed = typed.len() == phrase.asked.len();
    let every_word_matches = phrase.asked.iter().zip(typed.iter()).all(|(at, typed)| {
        words
            .get(at - 1)
            .is_some_and(|word| word.eq_ignore_ascii_case(typed))
    });
    let confirmed = all_three_typed && every_word_matches;
    if !confirmed {
        return Err(format!(
            "Those are not words {}. Check the phrase you wrote down.",
            asked_label(&phrase.asked)
        ));
    }
    Ok((phrase.name.clone(), phrase.words.clone()))
}

/// THE END OF THE CEREMONY: three right words seal the key they back.
/// The seal is `keystore::wallet::import` — the very call `ducktape wallet
/// import <name>` makes, so the phrase on screen restores this identity byte
/// for byte, here or on another machine.
///
/// A miss (or a seal that fails) keeps the phrase and the step, so a typo
/// costs a retry and not the account; a pass drops it, and nothing in this
/// app can show it again. The pointer write is not allowed to fail the call:
/// it degrades to a warning, and the user lands on a wallet that exists but
/// is not active, which the wallet list can still fix.
pub async fn confirm_recovery_phrase(answer: String, password: String) -> Result<String, AppError> {
    let answer = Zeroizing::new(answer);
    let (name, words) = confirmed_phrase(&answer).map_err(app_error)?;
    let pubkey = async {
        let duck = duck_home()?;
        let sealing = {
            let (name, password) = (name.clone(), Zeroizing::new(password));
            in_the_keystore(move || keystore::wallet::import(&duck, &name, &words, &password))
        };
        sealing.await
    }
    .await
    .map_err(app_error)?;
    end_the_ceremony();
    if activate_wallet(&name).await.is_err() {
        tracing::warn!(
            target: "ducktape::app",
            reason = "wallet_activate_failed",
            "the minted wallet is not the active one; pick it in the launch window"
        );
    }
    set_local_user_key(hex_decode(&pubkey).ok()).await;
    Ok(pubkey)
}

/// Let the words go, the moment the key they back exists.
fn end_the_ceremony() {
    let mut held = MINTED_PHRASE.lock().expect("minted recovery phrase");
    *held = None;
}

/// Re-seal an identity from its 24 words under a new password. Returns the
/// pubkey — the same identity those words were minted as.
pub async fn restore_user_key(
    name: String,
    words: ui_lang_runtime::Secret,
    password: String,
) -> Result<String, AppError> {
    async {
        let duck = duck_home()?;
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
        let importing = {
            let (name, password) = (name.clone(), Zeroizing::new(password));
            in_the_keystore(move || keystore::wallet::import(&duck, &name, &normalized, &password))
        };
        let pubkey = importing.await?;
        activate_wallet(&name).await?;
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(pubkey)
    }
    .await
    .map_err(app_error)
}

/// Unlock the NAMED wallet: a decrypt that succeeds iff `password` opens it,
/// followed by the pointer write that makes it the wallet this device signs
/// with. The pubkey the decrypt just proved seeds the in-process identity
/// cache — without it the first hydrate re-reads what this derivation already
/// paid 64 MiB to learn.
pub async fn unlock_wallet(name: String, password: String) -> Result<String, AppError> {
    async {
        let path = wallet_key_path(&name)?;
        require_password(&password)?;
        let opening = {
            let password = Zeroizing::new(password);
            in_the_keystore(move || keystore::userkey::open_user_key_at(&path, &password))
        };
        let pubkey = hex_encode(opening.await?.public_key().as_ref());
        activate_wallet(&name).await?;
        set_local_user_key(hex_decode(&pubkey).ok()).await;
        Ok(pubkey)
    }
    .await
    .map_err(app_error)
}

/// The active-pointer write. The env override names no keystore row, so it has
/// no pointer to move.
async fn activate_wallet(name: &str) -> Result<(), String> {
    keystore::wallet::valid_name(name)?;
    if env_key_override() && name == ENV_WALLET {
        return Ok(());
    }
    keystore::wallet::activate(&duck_home()?, name)
}

/// The console's Settings re-unlock, which knows a password and nothing else:
/// it re-proves the wallet this session is already signing with.
pub async fn unlock_user_key(password: String) -> Result<String, AppError> {
    let name = active_or_env_wallet().map_err(app_error)?;
    unlock_wallet(name, password).await
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

    /// A device key is named after the host, in the keystore's grammar, and
    /// never comes out empty.
    #[test]
    fn a_device_key_is_named_after_the_host_and_never_empty() {
        let name = device_key_name();
        assert!(!name.is_empty());
        assert!(keystore::wallet::valid_name(&name).is_ok(), "{name}");
    }

    /// The keystore decides both the entry step and the preselected row: an
    /// empty one is the password step, and the active wallet is the row the
    /// list opens on.
    #[test]
    fn entry_step_and_preselect_follow_the_keystore() {
        assert!(matches!(hub_entry_step(vec![]), crate::HubStep::Password));
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

    /// A wallet name is a path segment: `..` or `/` would walk the key file
    /// out of the keystore. The check lives in `keystore_key_path`, so a name
    /// that never passed through a person — the `active` pointer file's
    /// contents — is gated on the same terms.
    #[test]
    fn a_wallet_name_is_never_a_path() {
        for name in ["env", "default", "alice2", "a.b_c-d", "0"] {
            assert!(
                keystore::wallet::valid_name(name).is_ok(),
                "{name} should be valid"
            );
        }
        let refused = [
            "",
            "-h",
            "..",
            "../x",
            "a/b",
            "/etc/passwd",
            "Alice",
            ".hidden",
        ];
        for name in refused {
            // both the wallet-facing join and the pointer-derived one, which
            // is what every unlock and `user_key_state` resolve through.
            assert!(wallet_key_path(name).is_err(), "{name} built a path");
            let refusal = keystore_key_path(name).expect_err("built a path");
            assert!(
                refusal.contains("wallet name"),
                "unnamed refusal: {refusal}"
            );
        }
        assert!(keystore::wallet::valid_name(&"a".repeat(41)).is_ok());
        assert!(keystore::wallet::valid_name(&"a".repeat(42)).is_err());
    }

    /// A FIXED phrase — never a minted one, so nothing here can leak a real
    /// key's backup into a test log.
    const TEST_PHRASE: &str = "abandon amount liar amount expire adjust cage candy arch gather drum bullet absurd math era live bid rhythm alien crouch range attend journey unaware";

    /// the entropy [`TEST_PHRASE`] encodes: `00 01 02 … 1f`. A fixture, not a
    /// key — every phrase in this file's tests and in the screen captures is
    /// this one, so no minted phrase can reach a log or a PNG.
    const TEST_SEED: [u8; 32] = {
        let mut seed = [0u8; 32];
        let mut byte = 0;
        while byte < 32 {
            seed[byte] = byte as u8;
            byte += 1;
        }
        seed
    };

    /// The grid pairs 1↔13, 2↔14 … 12↔24, which is what lets the launch
    /// window show all 24 words without a scroll.
    #[test]
    fn the_phrase_grid_pairs_the_halves() {
        let rows = phrase_rows_of(TEST_PHRASE);
        assert_eq!(rows.len(), 12);
        assert_eq!(rows[0].left_number, "1");
        assert_eq!(rows[0].left_word, "abandon");
        assert_eq!(rows[0].right_number, "13");
        assert_eq!(rows[0].right_word, "absurd");
        assert_eq!(rows[11].left_number, "12");
        assert_eq!(rows[11].left_word, "bullet");
        assert_eq!(rows[11].right_number, "24");
        assert_eq!(rows[11].right_word, "unaware");
        assert!(phrase_rows_of("").is_empty());
    }

    /// THE WHOLE CEREMONY, in one test because [`MINTED_PHRASE`] is a
    /// process-global slot and two tests racing over it prove nothing: the
    /// mint hands the words over, the prompt names three positions, a wrong
    /// answer is refused WITHOUT naming a word and keeps the phrase, the
    /// right answer hands back the wallet to seal, and the phrase is gone the
    /// moment the seal is done with it.
    ///
    /// The gate is driven, not [`confirm_recovery_phrase`] itself: the seal
    /// writes a key file into the real `~/.ducktape`, which is not a test's
    /// to touch. What the seal does with what the gate returns is asserted
    /// below instead — the phrase IS a `ducktape wallet import` phrase.
    #[test]
    fn the_confirm_gate_ends_the_ceremony() {
        hold_minted_phrase(
            "device-test".to_string(),
            Zeroizing::new(TEST_PHRASE.into()),
        )
        .expect("a 24-word fixture");
        assert!(
            hold_minted_phrase("too-short".to_string(), Zeroizing::new("one two".into())).is_err()
        );
        assert_eq!(phrase_rows().len(), 12);
        let asked = MINTED_PHRASE
            .lock()
            .expect("minted recovery phrase")
            .as_ref()
            .expect("a phrase is held")
            .asked;
        let prompt = recovery_prompt();
        assert!(prompt.contains(&asked_label(&asked)), "{prompt}");
        let words: Vec<&str> = TEST_PHRASE.split_whitespace().collect();
        let right = asked
            .iter()
            .map(|at| words[at - 1])
            .collect::<Vec<_>>()
            .join(" ");

        let refusal = confirmed_phrase("nope nope nope").expect_err("three wrong words pass");
        assert!(refusal.contains(&asked_label(&asked)), "{refusal}");
        for word in &words {
            assert!(
                !refusal.contains(word),
                "the refusal leaked a word: {refusal}"
            );
        }
        // a miss keeps the phrase: the retry is still possible.
        assert_eq!(phrase_rows().len(), 12);
        // and so is a short answer.
        assert!(confirmed_phrase(words[asked[0] - 1]).is_err());

        // CASE IS NOT THE TEST — the words are, in order.
        let (name, sealing) =
            confirmed_phrase(&right.to_uppercase()).expect("the right words in the right order");
        assert_eq!(name, "device-test");
        assert_eq!(sealing.as_str(), TEST_PHRASE);
        // WHAT THE SEAL IS HANDED is what `ducktape wallet import <name>`
        // eats — the same `keystore::wallet::import` call, so the words on
        // screen restore this identity — and it decodes to exactly the seed
        // the mint drew, which is the whole claim the phrase makes.
        let seed = keystore::userkey::seed_of_mnemonic(&sealing).expect("a bip39 phrase");
        assert_eq!(seed, TEST_SEED);
        assert_eq!(keystore::userkey::mnemonic_of_seed(&seed), TEST_PHRASE);

        end_the_ceremony();
        assert!(phrase_rows().is_empty(), "the phrase outlived its ceremony");
        assert_eq!(recovery_prompt(), "");
        assert!(confirmed_phrase(&right).is_err());
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
