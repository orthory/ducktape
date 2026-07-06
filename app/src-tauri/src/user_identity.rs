//! user-key custody for the desktop shell. the USER key (`~/.ducktape/user.key`)
//! is machine-per-user, shared by every workspace -- unlike `identity.key`,
//! which is per workspace. the shell never parses or holds the secret: it
//! shells out to `ducktape-node` verbs exactly like workspace keygen, and only
//! signatures/pubkeys cross this boundary.

use serde::Serialize;

use crate::workspaces::{last_line, root, run_verb};

/// the shell's view of this machine's user identity: just the pubkey, hex.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdentity {
    /// this machine's user pubkey, hex — shared across every workspace.
    pub pubkey: String,
}

/// `~/.ducktape/user.key` — a sibling of `workspaces/`, not inside any one of
/// them: this key outlives and is shared by every workspace on the machine.
fn user_key_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(root(app)?.join("user.key"))
}

/// ensure `~/.ducktape/user.key` exists and report this machine's user pubkey.
/// `user-key` creates the file (hex ed25519 seed, 0600) only if it is absent;
/// otherwise it just reads the existing one back — so this is idempotent and
/// safe to call on every launch.
#[tauri::command]
pub fn user_identity_status(app: tauri::AppHandle) -> Result<UserIdentity, String> {
    let node_bin = crate::daemon::resolve_node_bin()?;
    let out = run_verb(
        &node_bin,
        &["user-key", "--out", &user_key_path(&app)?.to_string_lossy()],
    )?;
    Ok(UserIdentity {
        pubkey: last_line(&out),
    })
}

/// sign a `bind_node` `IdentityMsg` binding `node_pub` to this user key at
/// `nonce` — the one-line, ready-to-submit JSON `user-sign-bind` prints.
#[tauri::command]
pub fn user_sign_bind(
    app: tauri::AppHandle,
    chain_id: String,
    node_pub: String,
    nonce: u64,
) -> Result<String, String> {
    let node_bin = crate::daemon::resolve_node_bin()?;
    let out = run_verb(
        &node_bin,
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
    )?;
    Ok(last_line(&out))
}

/// sign an `unbind_node` `IdentityMsg` for `node_pub` at `nonce` — the undo of
/// [`user_sign_bind`], same one-line ready-to-submit JSON shape.
#[tauri::command]
pub fn user_sign_unbind(
    app: tauri::AppHandle,
    chain_id: String,
    node_pub: String,
    nonce: u64,
) -> Result<String, String> {
    let node_bin = crate::daemon::resolve_node_bin()?;
    let out = run_verb(
        &node_bin,
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
    )?;
    Ok(last_line(&out))
}
