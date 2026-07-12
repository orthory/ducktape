//! The registry model and its on-disk io: `~/.ducktape/registry.json` and the
//! path helpers around it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager as _;

// ── The registry model ──────────────────────────────────

/// the ports a workspace's node binds. concrete (never `:0`) so the app knows
/// where to reach the http surface and reachability plane across restarts.
#[derive(Clone, Serialize, Deserialize)]
pub struct Ports {
    /// the encrypted p2p mesh listener.
    pub listen: u16,
    /// the http/ws app surface the webview talks to.
    pub http: u16,
    /// the local json-lines rpc `invite-accept` drives.
    pub rpc: u16,
    /// the UDP WireGuard/rendezvous underlay socket.
    #[serde(default)]
    pub wireguard: Option<u16>,
    /// the UDP invite intro listener.
    #[serde(default)]
    pub invite: Option<u16>,
}

/// one workspace, as stored in the registry and handed to the ui.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    /// filesystem + registry key; a slug of the name, unique per registry.
    pub id: String,
    /// the human label the user gave this workspace locally.
    pub name: String,
    /// the network's chain-id (shared by every member; from the descriptor).
    pub chain_id: String,
    /// this workspace's own identity pubkey, hex.
    pub pubkey: String,
    /// this node founded the network (sole genesis validator) vs joined one.
    pub founder: bool,
    /// this identity is already in the descriptor's validator set — it boots
    /// straight to a validator. false means it will PARK until admitted.
    pub member: bool,
    pub ports: Ports,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Registry {
    /// bumped if the on-disk shape ever changes; 1 today.
    pub(super) version: u32,
    /// the workspace whose node the app currently talks to.
    pub(super) active: Option<String>,
    pub(super) workspaces: Vec<Workspace>,
    /// has the identity-creation mnemonic been shown + re-entered once on
    /// this machine? UX-only (no security weight) — gates whether the
    /// identity gate re-shows the "confirm your recovery phrase" step.
    /// `#[serde(default)]` so a pre-existing `registry.json` (version stays
    /// 1) keeps loading with this defaulting to `false`.
    #[serde(default)]
    pub(super) mnemonic_confirmed: bool,
}

/// the http coordinates `workspace_select` returns to the webview.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub id: String,
    pub http_url: String,
}

// ── Path + registry io ──────────────────────────────────

/// `~/.ducktape` — the registry root. created on demand. `pub(crate)` so
/// [`crate::user_identity`] can locate `user.key` as a sibling of `workspaces/`
/// without duplicating this lookup.
pub(crate) fn root(app: &crate::rt::AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|err| format!("no home dir: {err}"))?;
    Ok(home.join(".ducktape"))
}

pub(super) fn workspaces_dir(app: &crate::rt::AppHandle) -> Result<PathBuf, String> {
    Ok(root(app)?.join("workspaces"))
}

fn registry_path(app: &crate::rt::AppHandle) -> Result<PathBuf, String> {
    Ok(root(app)?.join("registry.json"))
}

fn empty_registry() -> Registry {
    Registry {
        version: 1,
        active: None,
        workspaces: Vec::new(),
        mnemonic_confirmed: false,
    }
}

pub(super) fn load_registry(app: &crate::rt::AppHandle) -> Result<Registry, String> {
    load_registry_at(&registry_path(app)?)
}

/// load + parse the registry, RECOVERING from a corrupt file instead of
/// bricking. a truncated / malformed `registry.json` (a hand-edit, a partial
/// pre-atomic save, a disk error) used to make every command fail — no list, no
/// select, no onboarding — with the only recovery, deleting the file, never
/// surfaced. here a parse error preserves the bad file as `registry.json.bak`
/// and boots the empty first-run state, so the app stays usable and the
/// workspace dirs still on disk can be re-added. a genuine READ error
/// (permissions) still propagates — the boot path lands on the gate with it.
pub(super) fn load_registry_at(path: &Path) -> Result<Registry, String> {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(reg) => Ok(reg),
            Err(err) => {
                let backup = path.with_extension("json.bak");
                let _ = fs::rename(path, &backup);
                eprintln!(
                    "load_registry: {path:?} is corrupt ({err}); backed up to {backup:?}, \
                     starting empty"
                );
                Ok(empty_registry())
            }
        },
        // a missing registry is the first-run empty state, not an error.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(empty_registry()),
        Err(err) => Err(format!("read {path:?}: {err}")),
    }
}

pub(super) fn save_registry(app: &crate::rt::AppHandle, reg: &Registry) -> Result<(), String> {
    let dir = root(app)?;
    fs::create_dir_all(&dir).map_err(|err| format!("create {dir:?}: {err}"))?;
    save_registry_at(&registry_path(app)?, reg)
}

/// write the registry ATOMICALLY: serialize to a sibling temp, fsync it, then
/// rename over the target. a crash / ENOSPC mid-write leaves the OLD registry
/// intact rather than a half-written, unparseable file (the corrupt-registry
/// brick this pairs with [`load_registry_at`]'s recovery to close). rename is
/// atomic within a directory on one filesystem, so a concurrent second instance
/// can at worst lose its own update — never corrupt the file. (A cross-process
/// advisory lock to also prevent the lost update is a deliberate follow-up;
/// same-$HOME multi-instance is an edge case and the fleet isolates $HOME.)
pub(super) fn save_registry_at(path: &Path, reg: &Registry) -> Result<(), String> {
    let text = serde_json::to_string_pretty(reg).map_err(|err| err.to_string())?;
    write_atomic(path, text.as_bytes())
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!("{extension}.tmp"))
        .unwrap_or_else(|| "tmp".into());
    let tmp = path.with_extension(extension);
    {
        use std::io::Write as _;
        let mut file = fs::File::create(&tmp).map_err(|err| format!("create {tmp:?}: {err}"))?;
        file.write_all(bytes)
            .map_err(|err| format!("write {tmp:?}: {err}"))?;
        file.sync_all()
            .map_err(|err| format!("fsync {tmp:?}: {err}"))?;
    }
    fs::rename(&tmp, path).map_err(|err| format!("rename {tmp:?} -> {path:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_registry_recovers_to_empty_and_backs_up() {
        let dir = std::env::temp_dir().join(format!("dt-reg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("registry.json");
        std::fs::write(&path, b"{ this is not valid json").unwrap();
        let reg = load_registry_at(&path).unwrap();
        assert!(reg.workspaces.is_empty(), "recovers to an empty registry");
        assert!(reg.active.is_none());
        assert!(
            path.with_extension("json.bak").exists(),
            "the corrupt file is preserved as .bak"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_registry_is_atomic_and_roundtrips() {
        let dir = std::env::temp_dir().join(format!("dt-reg2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("registry.json");
        let reg = Registry {
            version: 1,
            active: Some("team".into()),
            workspaces: Vec::new(),
            mnemonic_confirmed: false,
        };
        save_registry_at(&path, &reg).unwrap();
        assert!(
            !path.with_extension("json.tmp").exists(),
            "the temp file is consumed by the rename"
        );
        assert_eq!(
            load_registry_at(&path).unwrap().active.as_deref(),
            Some("team")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
