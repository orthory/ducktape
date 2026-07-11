//! user-key custody for the desktop shell. the USER key (`~/.ducktape/user.key`)
//! is machine-per-user, shared by every workspace -- unlike `identity.key`,
//! which is per workspace. the shell never parses or holds the seed: custody
//! stays in `ducktape-node` verbs and only signatures/pubkeys/mnemonics cross
//! this boundary. The password crosses via secret stdin only, never argv/env.
//! Every blocking custody operation runs on [`crate::daemon::NodeControl`], not
//! on a Tauri runtime worker.
//!
//! this module also holds the SESSION PASSWORD CACHE: once the user proves
//! they know the password (create/restore/unlock/encrypt), the shell keeps it
//! in process memory for the rest of the app's run so bind/unbind don't
//! re-prompt on every call. app restart = locked again; there is no disk
//! persistence of the password, ever.

use std::sync::Mutex;

use serde::Serialize;
use zeroize::Zeroize as _;

use crate::daemon::{NodeControl, last_line, require_main_window, run_verb, run_verb_with_stdin};
use crate::workspaces::root;

/// the design spec's floor for NEW passwords (create/restore/encrypt),
/// enforced here too for fast inline feedback -- the `ducktape-node` verbs
/// re-enforce the same floor authoritatively, so a shell-side bug here can
/// never let a too-short password through.
const MIN_PASSWORD_LEN: usize = 8;

fn check_password_len(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    Ok(())
}

// ── The session password cache ──────────────────────────

/// A string whose allocation is overwritten before release. Deliberately does
/// not implement `Debug` or `Display`, so the actor/command layer cannot log a
/// password or recovery phrase by accident.
struct SecretString(String);

impl SecretString {
    fn new(value: String) -> Self {
        Self(value)
    }
}

impl Clone for SecretString {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::ops::Deref for SecretString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        zero_string(&mut self.0);
    }
}

/// the verified password for this machine's user key, cached in process
/// memory ONLY for the life of the app -- never written to disk, never logged.
/// `None` means locked (or an absent/plaintext key that needs no cache).
static SESSION_PASSWORD: Mutex<Option<SecretString>> = Mutex::new(None);

/// Overwrite the allocation with volatile writes before clearing it. Use the
/// audited `zeroize` implementation so the compiler cannot elide the scrub as
/// a dead write during optimization.
fn zero_string(s: &mut String) {
    s.zeroize();
}

/// recover a poisoned lock rather than panicking -- a prior panicked caller
/// must not brick every future identity command in the session.
fn session_lock() -> std::sync::MutexGuard<'static, Option<SecretString>> {
    SESSION_PASSWORD.lock().unwrap_or_else(|e| e.into_inner())
}

/// cache `password` for this session, zeroing whatever was cached before it.
fn cache_store(password: &str) {
    let mut guard = session_lock();
    *guard = Some(SecretString::new(password.to_string()));
}

/// the cached password, if any. `None` means locked -- callers must not
/// substitute an empty string for "no password".
fn cache_peek() -> Option<SecretString> {
    session_lock().clone()
}

/// drop the cached password, zeroing it first. used by the explicit
/// `user_identity_lock` command (a Settings affordance).
fn cache_clear() {
    session_lock().take();
}

// ── Wire types ──────────────────────────────────────────

/// the shell's view of this machine's user identity: just the pubkey, hex.
/// kept for [`user_identity_status`] (legacy, compat-only).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdentity {
    /// this machine's user pubkey, hex — shared across every workspace.
    pub pubkey: String,
}

/// [`user_identity_state`]'s report: the gate-driving state machine value.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityState {
    /// one of: absent | plaintext | locked | unlocked.
    pub state: String,
    /// absent when there is no key on disk yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
    /// the UX-only "confirmed the recovery phrase once" registry flag.
    pub mnemonic_confirmed: bool,
}

/// the pubkey-only success shape (unlock/restore/encrypt).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPubkey {
    pub pubkey: String,
}

/// [`user_identity_create`]'s success shape: the mnemonic is shown exactly
/// once here -- the app must not be able to re-fetch it without a password
/// (that's what [`user_identity_reveal`] is for, and it always re-prompts).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityCreated {
    pub pubkey: String,
    pub mnemonic: String,
}

/// [`user_identity_reveal`]'s success shape.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityMnemonic {
    pub mnemonic: String,
}

// ── Paths ───────────────────────────────────────────────

/// `~/.ducktape/user.key` — a sibling of `workspaces/`, not inside any one of
/// them: this key outlives and is shared by every workspace on the machine.
fn user_key_path(app: &crate::rt::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(root(app)?.join("user.key"))
}

// ── Status parsing ──────────────────────────────────────

/// classify a `user-key status` stdout line into `(state, pubkey)`. a pure
/// string parser pulled out of [`user_identity_state`] so it is cheaply
/// unit-tested without shelling out to the node binary; see the tests below.
///
/// note: the verb's own vocabulary is `absent | plaintext <hex> | encrypted
/// <hex>` -- this maps `encrypted` to the app-facing `"locked"`, which
/// [`user_identity_state`] upgrades to `"unlocked"` when the session cache
/// holds this file's password.
///
/// the mismatch error is SHAPE-ONLY, never content: this only fires when the
/// verb contract has already broken (wrong binary, garbled stdout), which is
/// exactly when we can no longer assume the line is secret-free — and every
/// `Err(String)` here crosses IPC to the frontend.
fn parse_key_status(line: &str) -> Result<(&'static str, Option<String>), String> {
    let line = line.trim();
    if line == "absent" {
        return Ok(("absent", None));
    }
    if let Some(pubkey) = line.strip_prefix("plaintext ") {
        return Ok(("plaintext", Some(pubkey.trim().to_string())));
    }
    if let Some(pubkey) = line.strip_prefix("encrypted ") {
        return Ok(("locked", Some(pubkey.trim().to_string())));
    }
    Err(format!(
        "unrecognized user-key status output ({} chars)",
        line.chars().count()
    ))
}

/// split a multi-line verb stdout into non-empty, trimmed lines, in order.
fn value_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

/// split `user-key init`'s stdout into its `(mnemonic, pubkey)` line pair.
/// a pure helper pulled out of [`user_identity_create`] so the mismatch arm
/// is unit-testable. the error is COUNTS-ONLY, never content: in exactly the
/// malformed-stdout case this guards against, the captured lines may well
/// CONTAIN THE MNEMONIC, and `Err(String)` crosses IPC to the frontend —
/// interpolating `{lines:?}` here would ship the recovery phrase into
/// whatever error toast/log the app renders.
fn parse_init_output(stdout: &str) -> Result<(String, String), String> {
    let lines = value_lines(stdout);
    match lines.as_slice() {
        [mnemonic, pubkey] => Ok((mnemonic.to_string(), pubkey.to_string())),
        other => Err(format!(
            "user-key init: expected mnemonic + pubkey lines, got {} line(s)",
            other.len()
        )),
    }
}

// ── Commands ────────────────────────────────────────────

/// report this machine's user pubkey, LEGACY shape (pubkey-only, no
/// state/lock info) — kept for the Settings "User key" row's backward compat
/// and any external caller still on this shape.
///
/// Re-pointed at `user-key status` (never `--out`/GENERATE): the GENERATE verb
/// errors on a v2 (encrypted) key file, which used to make this command FATAL
/// for every encrypted identity — silently breaking auto-bind and rendering a
/// raw error string in Settings. `status` reads back `absent | plaintext <pub>
/// | encrypted <pub>` regardless of format, via the same [`parse_key_status`]
/// helper [`user_identity_state`] uses. `absent` has no pubkey to report, so
/// it errors here — nothing in the app calls this on an absent key anymore
/// (the identity gate guarantees a key exists before the console, and this
/// command's callers have moved to [`user_identity_state`]).
#[tauri::command]
pub async fn user_identity_status(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
) -> Result<UserIdentity, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_identity_status_blocking(app))
        .await
}

fn user_identity_status_blocking(app: crate::rt::AppHandle) -> Result<UserIdentity, String> {
    let out = run_verb(&[
        "user-key",
        "status",
        "--key",
        &user_key_path(&app)?.to_string_lossy(),
    ])?;
    let (_state, pubkey) = parse_key_status(&last_line(&out))?;
    pubkey
        .map(|pubkey| UserIdentity { pubkey })
        .ok_or_else(|| "no user identity".to_string())
}

/// the identity gate's state machine input: `user-key status` (never touches
/// a password) folded with the session cache (does the shell hold a verified
/// password for this file right now?) and the registry's UX-only
/// mnemonic-confirmed flag.
#[tauri::command]
pub async fn user_identity_state(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
) -> Result<IdentityState, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control.run(move || user_identity_state_blocking(app)).await
}

fn user_identity_state_blocking(app: crate::rt::AppHandle) -> Result<IdentityState, String> {
    let out = run_verb(&[
        "user-key",
        "status",
        "--key",
        &user_key_path(&app)?.to_string_lossy(),
    ])?;
    let (raw_state, pubkey) = parse_key_status(&last_line(&out))?;
    let state = if raw_state == "locked" && cache_peek().is_some() {
        "unlocked"
    } else {
        raw_state
    };
    Ok(IdentityState {
        state: state.to_string(),
        pubkey,
        mnemonic_confirmed: crate::workspaces::mnemonic_confirmed(&app)?,
    })
}

/// create a brand-new identity: `user-key init` mints a fresh seed, encrypts
/// it with `password`, and prints the mnemonic THEN the pubkey (two stdout
/// lines — `last_line` alone won't do here). caches `password` on success;
/// leaves the registry's `mnemonic_confirmed` flag false (the caller still
/// owes the user the confirm-3-words step).
#[tauri::command]
pub async fn user_identity_create(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    password: String,
) -> Result<IdentityCreated, String> {
    let password = SecretString::new(password);
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_identity_create_blocking(app, password))
        .await
}

fn user_identity_create_blocking(
    app: crate::rt::AppHandle,
    password: SecretString,
) -> Result<IdentityCreated, String> {
    check_password_len(&password)?;
    let out = run_verb_with_stdin(
        &[
            "user-key",
            "init",
            "--out",
            &user_key_path(&app)?.to_string_lossy(),
        ],
        &[&password],
    )?;
    let (mnemonic, pubkey) = parse_init_output(&out)?;
    cache_store(&password);
    Ok(IdentityCreated { pubkey, mnemonic })
}

/// restore an identity from its 24-word mnemonic: `user-key restore` derives
/// the same seed, encrypts it with the new `password`, and prints the pubkey.
/// caches `password`; marks `mnemonic_confirmed` true (the user just proved
/// they hold the words by typing them in).
#[tauri::command]
pub async fn user_identity_restore(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    mnemonic: String,
    password: String,
) -> Result<IdentityPubkey, String> {
    let mnemonic = SecretString::new(mnemonic);
    let password = SecretString::new(password);
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_identity_restore_blocking(app, mnemonic, password))
        .await
}

fn user_identity_restore_blocking(
    app: crate::rt::AppHandle,
    mnemonic: SecretString,
    password: SecretString,
) -> Result<IdentityPubkey, String> {
    check_password_len(&password)?;
    let out = run_verb_with_stdin(
        &[
            "user-key",
            "restore",
            "--out",
            &user_key_path(&app)?.to_string_lossy(),
        ],
        &[&mnemonic, &password],
    )?;
    let pubkey = last_line(&out);
    cache_store(&password);
    crate::workspaces::set_mnemonic_confirmed(&app)?;
    Ok(IdentityPubkey { pubkey })
}

/// unlock an existing encrypted identity: `user-key unlock` is pure
/// verification (nothing persists) -- a wrong password errors and the cache
/// is left untouched. caches `password` only once the verb confirms it.
#[tauri::command]
pub async fn user_identity_unlock(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    password: String,
) -> Result<IdentityPubkey, String> {
    let password = SecretString::new(password);
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_identity_unlock_blocking(app, password))
        .await
}

fn user_identity_unlock_blocking(
    app: crate::rt::AppHandle,
    password: SecretString,
) -> Result<IdentityPubkey, String> {
    let out = run_verb_with_stdin(
        &[
            "user-key",
            "unlock",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
        ],
        &[&password],
    )?;
    let pubkey = last_line(&out);
    cache_store(&password);
    Ok(IdentityPubkey { pubkey })
}

/// reveal the 24-word mnemonic. ALWAYS uses the password the caller just
/// supplied -- the session cache is NEVER consulted here, by design: reveal
/// is the one action the spec says must always re-prompt, however recently
/// the identity was unlocked.
#[tauri::command]
pub async fn user_identity_reveal(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    password: String,
) -> Result<IdentityMnemonic, String> {
    let password = SecretString::new(password);
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_identity_reveal_blocking(app, password))
        .await
}

fn user_identity_reveal_blocking(
    app: crate::rt::AppHandle,
    password: SecretString,
) -> Result<IdentityMnemonic, String> {
    let out = run_verb_with_stdin(
        &[
            "user-key",
            "reveal",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
        ],
        &[&password],
    )?;
    Ok(IdentityMnemonic {
        mnemonic: last_line(&out),
    })
}

/// migrate a legacy plaintext identity to encrypted (v2): `user-key encrypt`
/// rewrites the file in place and prints the (unchanged) pubkey. caches
/// `password` on success, same as create/unlock. marks `mnemonic_confirmed`
/// true, same as [`user_identity_restore`]: a legacy key predates the
/// shown-once mnemonic ceremony entirely, so there is no confirm step this
/// user could ever complete -- forcing the create-flow's confirm loop on
/// someone who just secured a pre-existing identity would trap them behind a
/// gate with no way through.
#[tauri::command]
pub async fn user_identity_encrypt(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    password: String,
) -> Result<IdentityPubkey, String> {
    let password = SecretString::new(password);
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_identity_encrypt_blocking(app, password))
        .await
}

fn user_identity_encrypt_blocking(
    app: crate::rt::AppHandle,
    password: SecretString,
) -> Result<IdentityPubkey, String> {
    check_password_len(&password)?;
    let out = run_verb_with_stdin(
        &[
            "user-key",
            "encrypt",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
        ],
        &[&password],
    )?;
    let pubkey = last_line(&out);
    cache_store(&password);
    crate::workspaces::set_mnemonic_confirmed(&app)?;
    Ok(IdentityPubkey { pubkey })
}

/// drop the session-cached password (a Settings "lock" affordance). the next
/// bind/unbind on an encrypted key will need a fresh unlock.
#[tauri::command]
pub async fn user_identity_lock(
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control.run(user_identity_lock_blocking).await
}

fn user_identity_lock_blocking() -> Result<(), String> {
    cache_clear();
    Ok(())
}

/// resolve the stdin lines a `user-sign-bind`/`user-sign-unbind` invocation
/// needs: probes `user-key status` (never the key bytes -- the shell stays
/// crypto/format-agnostic) rather than sniffing the file directly. plaintext
/// and absent keys need no password (legacy behavior, unchanged); an
/// encrypted key needs the session-cached password, or the caller gets the
/// exact `"identity-locked"` string the app reacts to.
fn signing_stdin(app: &crate::rt::AppHandle) -> Result<Vec<SecretString>, String> {
    let out = run_verb(&[
        "user-key",
        "status",
        "--key",
        &user_key_path(app)?.to_string_lossy(),
    ])?;
    let (state, _pubkey) = parse_key_status(&last_line(&out))?;
    if state != "locked" {
        return Ok(Vec::new());
    }
    match cache_peek() {
        Some(password) => Ok(vec![password]),
        None => Err("identity-locked".to_string()),
    }
}

/// sign a `bind_node` `IdentityMsg` binding `node_pub` to this user key at
/// `nonce` — the one-line, ready-to-submit JSON `user-sign-bind` prints. an
/// encrypted key with no cached password fails with `identity-locked`
/// (exact string) instead of hanging on a stdin the caller never provides.
#[tauri::command]
pub async fn user_sign_bind(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    chain_id: String,
    node_pub: String,
    nonce: u64,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_sign_bind_blocking(app, chain_id, node_pub, nonce))
        .await
}

fn user_sign_bind_blocking(
    app: crate::rt::AppHandle,
    chain_id: String,
    node_pub: String,
    nonce: u64,
) -> Result<String, String> {
    let stdin_lines = signing_stdin(&app)?;
    let stdin_refs: Vec<&str> = stdin_lines.iter().map(SecretString::as_ref).collect();
    let out = run_verb_with_stdin(
        &[
            "user-sign-bind",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
            "--chain-id",
            &chain_id,
            "--node-pub",
            &node_pub,
            "--nonce",
            &nonce.to_string(),
        ],
        &stdin_refs,
    )?;
    Ok(last_line(&out))
}

/// sign an `unbind_node` `IdentityMsg` for `node_pub` at `nonce` — the undo of
/// [`user_sign_bind`], same one-line ready-to-submit JSON shape and the same
/// `identity-locked` gating on an encrypted, uncached key.
///
/// consumed by the Account view's Nodes card (the lost-device "Unbind"
/// affordance) via store/account-ops.ts.
#[tauri::command]
pub async fn user_sign_unbind(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    chain_id: String,
    node_pub: String,
    nonce: u64,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_sign_unbind_blocking(app, chain_id, node_pub, nonce))
        .await
}

fn user_sign_unbind_blocking(
    app: crate::rt::AppHandle,
    chain_id: String,
    node_pub: String,
    nonce: u64,
) -> Result<String, String> {
    let stdin_lines = signing_stdin(&app)?;
    let stdin_refs: Vec<&str> = stdin_lines.iter().map(SecretString::as_ref).collect();
    let out = run_verb_with_stdin(
        &[
            "user-sign-unbind",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
            "--chain-id",
            &chain_id,
            "--node-pub",
            &node_pub,
            "--nonce",
            &nonce.to_string(),
        ],
        &stdin_refs,
    )?;
    Ok(last_line(&out))
}

/// Sign one bounded global gateway route statement. The sidecar parses the
/// exact typed statement and returns a ready-to-submit `GatewayMsg`; this is
/// purpose-specific and never exposes a generic signing oracle.
#[tauri::command]
pub async fn user_sign_gateway_route(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    statement: String,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_sign_gateway_route_blocking(app, statement))
        .await
}

fn user_sign_gateway_route_blocking(
    app: crate::rt::AppHandle,
    statement: String,
) -> Result<String, String> {
    let stdin_lines = signing_stdin(&app)?;
    let stdin_refs: Vec<&str> = stdin_lines.iter().map(SecretString::as_ref).collect();
    let out = run_verb_with_stdin(
        &[
            "user-sign-gateway-route",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
            "--statement",
            &statement,
        ],
        &stdin_refs,
    )?;
    Ok(last_line(&out))
}

/// sign this machine's ed25519 POSSESSION proof for joining `account_id` as a
/// new member at `nonce` — the `MemberProof` JSON a new device produces to
/// prove it holds its key. Pair it with this machine's pubkey (from
/// [`user_identity_state`]); an existing member then feeds both to
/// [`user_sign_add_member`]. `identity-locked` (exact string) on an encrypted,
/// uncached key.
#[tauri::command]
pub async fn user_sign_possession(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    chain_id: String,
    account_id: String,
    nonce: u64,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_sign_possession_blocking(app, chain_id, account_id, nonce))
        .await
}

fn user_sign_possession_blocking(
    app: crate::rt::AppHandle,
    chain_id: String,
    account_id: String,
    nonce: u64,
) -> Result<String, String> {
    let stdin_lines = signing_stdin(&app)?;
    let stdin_refs: Vec<&str> = stdin_lines.iter().map(SecretString::as_ref).collect();
    let out = run_verb_with_stdin(
        &[
            "user-sign-possession",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
            "--chain-id",
            &chain_id,
            "--account-id",
            &account_id,
            "--nonce",
            &nonce.to_string(),
        ],
        &stdin_refs,
    )?;
    Ok(last_line(&out))
}

/// sign an add-member AUTHORIZER certificate and assemble the ready-to-submit
/// `IdentityMsg::AddMemberKey` JSON: the local user key (an existing member)
/// consents to admitting `new_pub` (of `new_kind`, optional `label`) over the
/// same add-member preimage `possession` was signed against. `possession` is
/// the new key's own proof — from [`user_sign_possession`] on another device,
/// or the native FIDO2 transport for a passkey. `identity-locked` on an
/// encrypted, uncached key.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn user_sign_add_member(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    chain_id: String,
    account_id: String,
    new_pub: String,
    new_kind: String,
    nonce: u64,
    possession: String,
    label: Option<String>,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || {
            user_sign_add_member_blocking(
                app, chain_id, account_id, new_pub, new_kind, nonce, possession, label,
            )
        })
        .await
}

#[allow(clippy::too_many_arguments)]
fn user_sign_add_member_blocking(
    app: crate::rt::AppHandle,
    chain_id: String,
    account_id: String,
    new_pub: String,
    new_kind: String,
    nonce: u64,
    possession: String,
    label: Option<String>,
) -> Result<String, String> {
    let stdin_lines = signing_stdin(&app)?;
    let stdin_refs: Vec<&str> = stdin_lines.iter().map(SecretString::as_ref).collect();
    let key = user_key_path(&app)?.to_string_lossy().into_owned();
    let nonce = nonce.to_string();
    let mut args: Vec<&str> = vec![
        "user-sign-add-member",
        "--key",
        &key,
        "--chain-id",
        &chain_id,
        "--account-id",
        &account_id,
        "--new-key",
        &new_pub,
        "--new-kind",
        &new_kind,
        "--nonce",
        &nonce,
        "--possession",
        &possession,
    ];
    if let Some(label) = &label {
        args.push("--label");
        args.push(label);
    }
    let out = run_verb_with_stdin(&args, &stdin_refs)?;
    Ok(last_line(&out))
}

/// sign a remove-member certificate and print the ready-to-submit
/// `IdentityMsg::RemoveMemberKey` JSON: the local user key (a member) evicts
/// `target_key` from `account_id` at `nonce`. Any member may remove any member
/// except the last one. `identity-locked` on an encrypted, uncached key.
#[tauri::command]
pub async fn user_sign_remove_member(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    chain_id: String,
    account_id: String,
    target_key: String,
    nonce: u64,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || user_sign_remove_member_blocking(app, chain_id, account_id, target_key, nonce))
        .await
}

fn user_sign_remove_member_blocking(
    app: crate::rt::AppHandle,
    chain_id: String,
    account_id: String,
    target_key: String,
    nonce: u64,
) -> Result<String, String> {
    let stdin_lines = signing_stdin(&app)?;
    let stdin_refs: Vec<&str> = stdin_lines.iter().map(SecretString::as_ref).collect();
    let out = run_verb_with_stdin(
        &[
            "user-sign-remove-member",
            "--key",
            &user_key_path(&app)?.to_string_lossy(),
            "--chain-id",
            &chain_id,
            "--account-id",
            &account_id,
            "--target-key",
            &target_key,
            "--nonce",
            &nonce.to_string(),
        ],
        &stdin_refs,
    )?;
    Ok(last_line(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_status_reads_absent() {
        assert!(matches!(parse_key_status("absent"), Ok(("absent", None))));
        // tolerate the trailing newline/whitespace a verb's stdout may carry.
        assert!(matches!(
            parse_key_status("  absent \n"),
            Ok(("absent", None))
        ));
    }

    #[test]
    fn parse_key_status_reads_plaintext() {
        let (state, pubkey) = parse_key_status("plaintext deadbeef").unwrap();
        assert_eq!(state, "plaintext");
        assert_eq!(pubkey.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn parse_key_status_reads_encrypted_as_locked() {
        let (state, pubkey) = parse_key_status("encrypted deadbeef").unwrap();
        assert_eq!(state, "locked");
        assert_eq!(pubkey.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn parse_key_status_rejects_garbage_without_echoing_it() {
        assert!(parse_key_status("").is_err());
        // the mismatch error must be shape-only: a broken verb contract is
        // exactly when the line can no longer be assumed secret-free, and
        // this Err(String) crosses IPC to the frontend.
        let err = parse_key_status("corrupt hunter2-secret").unwrap_err();
        assert!(
            !err.contains("hunter2"),
            "error echoed stdout content: {err}"
        );
    }

    #[test]
    fn value_lines_drops_blank_lines_and_trims() {
        assert_eq!(value_lines("  one \n\n two\n"), vec!["one", "two"]);
    }

    #[test]
    fn parse_init_output_splits_mnemonic_then_pubkey() {
        let (mnemonic, pubkey) = parse_init_output("word word word\n\ndeadbeef\n").unwrap();
        assert_eq!(mnemonic, "word word word");
        assert_eq!(pubkey, "deadbeef");
    }

    #[test]
    fn parse_init_output_mismatch_reports_counts_never_content() {
        // one line, three lines, zero lines: all rejected — and the error
        // must NEVER quote the lines, which in this failure mode may contain
        // the mnemonic (the Err crosses IPC to the frontend).
        for (stdout, want_count) in [
            ("abandon ability able mnemonic-words", "1 line(s)"),
            ("extra\nabandon ability able\ndeadbeef", "3 line(s)"),
            ("", "0 line(s)"),
        ] {
            let err = parse_init_output(stdout).unwrap_err();
            assert!(err.contains(want_count), "want {want_count:?} in: {err}");
            assert!(
                !err.contains("abandon") && !err.contains("deadbeef"),
                "error echoed stdout content: {err}"
            );
        }
    }

    #[test]
    fn zero_string_scrubs_and_clears_the_value() {
        let mut s = String::from("hunter2 🔒");
        zero_string(&mut s);
        assert!(s.is_empty());
    }

    #[test]
    fn session_cache_round_trips_and_clears() {
        // these tests share the process-global SESSION_PASSWORD, so this one
        // test exercises the whole store/peek/clear lifecycle rather than
        // relying on run-order isolation between separate #[test] fns.
        cache_clear();
        assert!(cache_peek().is_none());
        cache_store("correct horse battery staple");
        let cached = cache_peek();
        assert_eq!(cached.as_deref(), Some("correct horse battery staple"));
        cache_store("replacement password");
        let cached = cache_peek();
        assert_eq!(cached.as_deref(), Some("replacement password"));
        cache_clear();
        assert!(cache_peek().is_none());
    }
}
