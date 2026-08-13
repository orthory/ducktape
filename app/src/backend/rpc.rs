use super::*;
use ::chat;

pub(crate) async fn signed_write(
    rpc: &RpcClient,
    target: &str,
    payload: Vec<u8>,
    password: String,
) -> Result<(), String> {
    if payload.is_empty() || payload.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{target} transaction exceeds the signed payload limit"
        ));
    }
    let frame = sign_frame(target, &payload, password).await?;
    rpc.submit_frame(frame).await.map_err(Into::into)
}

/// THE session signer: one `ducktape user sign-frame` child, unlocked once,
/// then fed one request line per signed write.
///
/// Opening the key runs argon2id over 64 MiB (`bin/node/src/userkey.rs`), so
/// the per-op process this replaced charged every reaction tap ~200-400 ms of
/// memory-hard KDF plus a spawn — and five taps in a row (reactions skip
/// `mutation_phase` on purpose) fanned out five concurrent 64 MiB jobs against
/// the render thread. The posture is unchanged by construction: the app
/// already holds the password in state, and the private key still lives only
/// in the child's address space.
static SIGNER: tokio::sync::Mutex<Option<Signer>> = tokio::sync::Mutex::const_new(None);

/// How long a signer that just failed a request gets to exit before its
/// stderr is written off. Short on purpose — the request already spent
/// `RPC_TIMEOUT`, and this is only here to name the cause.
const SIGNER_EXIT_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) struct Signer {
    /// The password this child was unlocked with. A different one is a
    /// different seat (a re-login, or a restored key), so it gets its own
    /// child instead of signing under the key this one holds.
    password: Zeroizing<String>,
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    frames: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
}

impl Signer {
    /// Spawn the child and hand it the password — the session's one argon2id
    /// pass. A bad password does not fail here: the child dies on it and the
    /// first request reports its stderr through [`Signer::reap`].
    ///
    /// The binary and key path are arguments rather than reads of
    /// `ducktape_binary()` / `user_key_path()` so the session can be driven
    /// against a stub signer without touching this process's environment.
    pub(super) async fn unlock(
        binary: PathBuf,
        key: PathBuf,
        password: Zeroizing<String>,
    ) -> Result<Self, String> {
        require_encrypted_key(&key)?;
        let input = Zeroizing::new(password_line(&password)?);
        let mut command = tokio::process::Command::new(binary);
        command
            .arg("user")
            .arg("sign-frame")
            .arg("--key")
            .arg(&key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            format!(
                "could not start the ducktape signer ({error}); build node-bin or set DUCKTAPE_BIN"
            )
        })?;
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err("ducktape signer pipes are unavailable".into());
        };
        stdin
            .write_all(&input)
            .await
            .map_err(|error| format!("could not unlock the signer: {error}"))?;
        Ok(Self {
            password,
            child,
            stdin,
            frames: tokio::io::BufReader::new(stdout).lines(),
        })
    }

    /// One request line out, one frame-hex line back.
    pub(super) async fn sign(&mut self, request: &str) -> Result<Vec<u8>, String> {
        self.stdin
            .write_all(request.as_bytes())
            .await
            .map_err(|error| format!("could not send payload to signer: {error}"))?;
        let answer = tokio::time::timeout(RPC_TIMEOUT, self.frames.next_line())
            .await
            .map_err(|_| "ducktape signer timed out".to_string())?
            .map_err(|error| format!("ducktape signer failed: {error}"))?;
        let frame_hex = answer.ok_or_else(|| "ducktape signer returned no frame".to_string())?;
        hex_decode(frame_hex.trim())
    }

    /// Close the pipe and collect the child's exit — its stderr is the only
    /// place a session-level refusal (wrong password, unreadable key) is
    /// spelled out.
    async fn reap(self) -> Option<String> {
        drop(self.stdin);
        drop(self.frames);
        let output = tokio::time::timeout(SIGNER_EXIT_TIMEOUT, self.child.wait_with_output())
            .await
            .ok()?
            .ok()?;
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.trim().is_empty() {
            return None;
        }
        Some(format!(
            "ducktape signer refused the transaction: {}",
            bounded_detail(&detail)
        ))
    }
}

async fn sign_frame(target: &str, payload: &[u8], password: String) -> Result<Vec<u8>, String> {
    let password = Zeroizing::new(password);
    let request = format!("{target} {} {}\n", next_sequence(), hex_encode(payload));
    // The lock IS the semaphore of 1: a burst of reactions queues on one
    // child instead of fanning out, and it is what keeps the request lines
    // and the frame lines on the child's pipes paired.
    let mut session = SIGNER.lock().await;
    let seated = session
        .as_ref()
        .is_some_and(|signer| signer.password == password);
    if !seated {
        *session = Some(Signer::unlock(ducktape_binary(), user_key_path()?, password).await?);
    }
    let signer = session.as_mut().expect("the session was seated above");
    let fault = match signer.sign(&request).await {
        Ok(frame) => return Ok(frame),
        Err(fault) => fault,
    };
    // A signer that failed one request never serves a second: retire it here
    // so the next write unlocks a fresh one.
    let Some(refusal) = session.take() else {
        return Err(fault);
    };
    Err(refusal.reap().await.unwrap_or(fault))
}

/// Retire the session signer — the `Lock` button's other half. Clearing
/// `password` in state stops the app from signing; this stops the CHILD from
/// being able to.
pub async fn lock_signer() -> bool {
    SIGNER.lock().await.take().is_some()
}

/// The password as the signer's first stdin line, rejecting what the
/// line-delimited stdin contract cannot carry.
pub(crate) fn password_line(password: &str) -> Result<Vec<u8>, String> {
    let invalid_password = password.len() > 16 * 1024
        || password
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, 0 | b'\n' | b'\r'));
    if invalid_password {
        return Err("key password is too long or contains a line delimiter".into());
    }
    if password.is_empty() {
        return Err("the local user key is locked; enter its password".into());
    }
    let mut input = Vec::with_capacity(password.len() + 1);
    input.extend_from_slice(password.as_bytes());
    input.push(b'\n');
    Ok(input)
}

/// The cached identity: `None` = not read yet, `Some(reading)` = the disk's
/// answer. A plain cache would freeze the launch state for process life —
/// the launch window can MINT the key in-process now, so its creators refresh
/// this through [`set_local_user_key`] instead of demanding a restart.
static LOCAL_USER_KEY: tokio::sync::RwLock<Option<Option<Vec<u8>>>> =
    tokio::sync::RwLock::const_new(None);

pub(crate) async fn local_user_key() -> Option<Vec<u8>> {
    if let Some(reading) = LOCAL_USER_KEY.read().await.clone() {
        return reading;
    }
    let reading = read_local_user_key().await;
    *LOCAL_USER_KEY.write().await = Some(reading.clone());
    reading
}

/// Replace the cached identity — called by the key ceremonies (init/restore)
/// with the pubkey the CLI just printed.
pub(crate) async fn set_local_user_key(reading: Option<Vec<u8>>) {
    *LOCAL_USER_KEY.write().await = Some(reading);
}

/// The cached identity WITHOUT waiting — the update thread's synchronous
/// folds cannot await. `None` covers the cold cache and a held write lock;
/// by any reaction tap the cache is warm (every hydrate reads it first).
pub(crate) fn cached_user_key() -> Option<Vec<u8>> {
    LOCAL_USER_KEY
        .try_read()
        .ok()
        .and_then(|reading| reading.clone().flatten())
}

async fn read_local_user_key() -> Option<Vec<u8>> {
    let key = user_key_path().ok()?;
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("key")
        .arg("status")
        .arg("--key")
        .arg(key)
        .kill_on_drop(true);
    let output = tokio::time::timeout(RPC_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() || output.stdout.len() > 256 {
        return None;
    }
    parse_user_key_status(std::str::from_utf8(&output.stdout).ok()?)
}

pub(crate) fn parse_user_key_status(status: &str) -> Option<Vec<u8>> {
    let mut fields = status.split_whitespace();
    match fields.next()? {
        "encrypted" => {}
        _ => return None,
    }
    let key = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    public_key(key, "local user key").ok()
}

/// The client-local UI prefs file (doc tabs, per-endpoint) — sibling to the
/// user key: `$DUCKTAPE_HOME/app-prefs.json`, else `~/.ducktape/app-prefs.json`.
/// Never wire state: purely this device's view preferences.
fn prefs_path() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Some(PathBuf::from(root).join("app-prefs.json"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape/app-prefs.json"))
}

pub(crate) fn read_prefs() -> serde_json::Value {
    let Some(path) = prefs_path() else {
        return serde_json::json!({});
    };
    std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(crate) fn write_prefs(prefs: &serde_json::Value) -> bool {
    let Some(path) = prefs_path() else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(bytes) = serde_json::to_vec_pretty(prefs) else {
        return false;
    };
    std::fs::write(&path, bytes).is_ok()
}

/// The persisted appearance override — `"light"` / `"dark"`, or empty when
/// this device follows the OS. DEVICE-global, not per-endpoint: appearance is
/// a property of the person's eyes and room, not of a workspace.
pub async fn load_appearance() -> String {
    match read_prefs()["appearance"].as_str() {
        Some("light") => "light".into(),
        Some("dark") => "dark".into(),
        _ => String::new(),
    }
}

pub async fn save_appearance(mode: String) -> bool {
    let known = mode == "light" || mode == "dark";
    if !known {
        return false;
    }
    let mut prefs = read_prefs();
    prefs["appearance"] = serde_json::json!(mode);
    write_prefs(&prefs)
}

/// This endpoint's persisted doc tabs (open page ids, in open order).
pub async fn load_doc_tabs(rpc: String) -> Vec<String> {
    let prefs = read_prefs();
    prefs["doc_tabs"][&rpc]
        .as_array()
        .map(|tabs| {
            tabs.iter()
                .filter_map(|tab| tab.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Persist this endpoint's doc tabs. Best-effort: a failed write only costs
/// tab restoration on the next boot.
pub async fn save_doc_tabs(rpc: String, tabs: Vec<String>) -> bool {
    let mut prefs = read_prefs();
    prefs["doc_tabs"][&rpc] = serde_json::json!(tabs);
    write_prefs(&prefs)
}

/// Add a page to the doc-tab strip (idempotent, keeps open order).
pub fn doc_tabs_with(mut tabs: Vec<String>, page_id: String) -> Vec<String> {
    if page_id.is_empty() || tabs.contains(&page_id) {
        return tabs;
    }
    tabs.push(page_id);
    tabs
}

/// Close one tab.
pub fn doc_tabs_without(mut tabs: Vec<String>, page_id: String) -> Vec<String> {
    tabs.retain(|tab| *tab != page_id);
    tabs
}

/// One rendered doc tab.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct DocTab {
    pub id: String,
    pub title: String,
    pub active: bool,
}

/// The rendered tab strip: open tabs that still exist, titled from the page
/// list, the active one flagged.
pub fn doc_tab_rows(tabs: Vec<String>, pages: Vec<PageItem>, active: String) -> Vec<DocTab> {
    tabs.into_iter()
        .filter_map(|tab| {
            let page = pages.iter().find(|page| page.id == tab)?;
            Some(DocTab {
                title: page.title.clone(),
                active: tab == active,
                id: tab,
            })
        })
        .collect()
}

/// Drop tabs whose page is gone. `doc_tab_rows` already resolves every tab
/// against the live page list when it draws, so a dead id is invisible in the
/// bar — but the PERSISTED list kept them forever, and Settings counts that
/// list: `Open page tabs 11` beside a bar showing two. Pruning where the pages
/// land keeps the stored list and its count honest.
pub fn doc_tabs_pruned(tabs: Vec<String>, pages: Vec<PageItem>) -> Vec<String> {
    tabs.into_iter()
        .filter(|tab| pages.iter().any(|page| page.id == *tab))
        .collect()
}

/// The tab to activate after closing one: the last remaining tab, or empty.
pub fn next_doc_tab(tabs: Vec<String>, closed: String, active: String) -> String {
    if closed != active {
        return active;
    }
    tabs.into_iter()
        .rev()
        .find(|tab| *tab != closed)
        .unwrap_or_default()
}

pub(crate) fn user_key_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DUCKTAPE_USER_KEY") {
        return Ok(path.into());
    }
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(root).join("user.key"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape/user.key"))
        .ok_or_else(|| "cannot locate local user.key; set DUCKTAPE_USER_KEY".to_string())
}

pub(crate) fn require_encrypted_key(path: &std::path::Path) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    if metadata.len() > MAX_KEY_FILE_BYTES {
        return Err("local user key file is unexpectedly large".into());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let mut prefix = [0; ENCRYPTED_KEY_PREFIX.len()];
    let read = file
        .read(&mut prefix)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let encrypted = read == prefix.len() && prefix == ENCRYPTED_KEY_PREFIX.as_bytes();
    prefix.zeroize();
    if encrypted {
        Ok(())
    } else {
        Err("local user key must use the encrypted v1 format".into())
    }
}

pub(crate) fn ducktape_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("DUCKTAPE_BIN") {
        return path.into();
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(sibling) = current.parent().map(|parent| parent.join("ducktape"))
        && sibling.is_file()
    {
        return sibling;
    }
    PathBuf::from("ducktape")
}

pub(crate) fn bounded_text(value: String, field: &str, limit: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > limit || value.chars().any(|character| character == '\0') {
        return Err(format!("{field} must be between 1 and {limit} bytes"));
    }
    Ok(value.to_string())
}

pub(crate) fn bounded_exact_text(
    value: String,
    field: &str,
    limit: usize,
) -> Result<String, String> {
    let invalid = value.len() > limit || value.chars().any(|character| character == '\0');
    if invalid {
        return Err(format!(
            "{field} must be at most {limit} bytes and contain no NUL"
        ));
    }
    Ok(value)
}

pub(crate) fn required_id(value: String, subject: &str) -> Result<String, String> {
    bounded_text(value, &format!("{subject} id"), 512)
}

pub(crate) fn public_key(value: &str, field: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    let expected = chat::HUDDLE_NODE_KEY_BYTES * 2;
    if value.len() != expected {
        return Err(format!("{field} must be {expected} hexadecimal characters"));
    }
    hex_decode(value).map_err(|_| format!("{field} must be hexadecimal"))
}

pub(crate) fn positive_sequence(value: i64) -> Result<u64, String> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "message sequence must be positive".into())
}

/// One voice at the surface. The global banner prints whatever reaches an
/// `AppError`/`HydrationError`, so the known developer diagnostics — signer
/// spawn chatter, key paths, argv timeouts, serde parse positions — translate
/// to a user sentence here. A module's own sentence flows through untouched.
pub(crate) fn user_error(message: String) -> String {
    let signer_broken = message.contains("signer") || message.contains("DUCKTAPE_BIN");
    if signer_broken {
        return "The signing helper failed to run. Check the ducktape install in Settings.".into();
    }
    let key_unreadable = message.contains("local user key");
    if key_unreadable {
        return "This device's user key is missing or unreadable. Check Settings.".into();
    }
    // The key tool's AEAD cannot tell a wrong password from a damaged key file,
    // so one sentence has to serve both; init/restore/unlock all refuse through
    // here, and the raw `FATAL: …` line is the app's first screen otherwise.
    let password_refused = message.contains("corrupt or wrong password");
    if password_refused {
        return "That password did not open this device's key. Check it and try again.".into();
    }
    let node_slow = message.contains("timed out");
    if node_slow {
        return "The node did not answer in time. Retry in a moment.".into();
    }
    let reply_garbled = message.contains("invalid type")
        || message.contains("while parsing")
        || message.contains("at line ")
        || message.contains("non-UTF-8");
    if reply_garbled {
        return "The node sent a reply this app could not read. Reload and retry.".into();
    }
    message
}

pub(crate) fn app_error(message: String) -> AppError {
    message.into()
}

pub(crate) fn committed_error(message: String) -> AppError {
    AppError {
        message: user_error(message),
        committed: true,
    }
}

pub(crate) fn retry_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(4);
    Duration::from_secs(1_u64 << exponent)
}

pub(crate) fn number_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(crate) fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
