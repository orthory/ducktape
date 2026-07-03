//! the multi-workspace registry — the desktop app's front door.
//!
//! a "workspace" is one ducktape network the user founded or joined. each is a
//! self-contained directory under `~/.ducktape/workspaces/<id>/` holding the
//! network-shape node config the `ducktape-node` binary already understands:
//! `network.toml` (the descriptor), `identity.key` (this workspace's own
//! ed25519 secret — identity is PER WORKSPACE), `node.toml`, `storage/`, and
//! `daemon.log`. `~/.ducktape/registry.json` indexes them and records which one
//! is active.
//!
//! every mutation shells out to the SAME onboarding verbs the CLI exposes
//! (`init`/`join`/`invite`/`invite-accept`), so the registry never reimplements
//! identity, descriptors, or governance — it only allocates ports, lays out
//! directories, and remembers the result. a parked joiner serves no http/rpc
//! surface (the node gates both off until it is a validator), so onboarding
//! progress is read back from the stable marker lines the node prints to
//! `daemon.log` — see [`workspace_phase`].

use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::Manager as _;

// ── The registry model ──────────────────────────────────

/// the three ports a workspace's node binds. concrete (never `:0`) so the
/// founder's descriptor carries a stable dial hint and the app knows where to
/// reach the http surface across restarts.
#[derive(Clone, Serialize, Deserialize)]
pub struct Ports {
    /// the encrypted p2p mesh listener (also the advertised dial hint).
    pub listen: u16,
    /// the http/ws app surface the webview talks to.
    pub http: u16,
    /// the local json-lines rpc `invite-accept` drives.
    pub rpc: u16,
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
struct Registry {
    /// bumped if the on-disk shape ever changes; 1 today.
    version: u32,
    /// the workspace whose node the app currently talks to.
    active: Option<String>,
    workspaces: Vec<Workspace>,
}

/// the http coordinates [`workspace_select`] returns to the webview.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub id: String,
    pub http_url: String,
}

/// the onboarding phase [`workspace_phase`] reads back from the node log.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseReport {
    /// one of: starting | parked | admitted | synced | promoted | fatal.
    pub phase: String,
    /// the trailing text of the marker line, for a live status string.
    pub detail: Option<String>,
}

// ── Path + registry io ──────────────────────────────────

/// `~/.ducktape` — the registry root. created on demand.
///
/// overridable via `DUCKTAPE_HOME` (an absolute registry root, used verbatim —
/// not joined with `.ducktape`) so several worktree QA instances can each keep a
/// PRIVATE registry with its own workspaces, ports, and storage instead of
/// sharing the one home-dir registry. unset → today's `~/.ducktape`.
fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(dir));
    }
    let home = app
        .path()
        .home_dir()
        .map_err(|err| format!("no home dir: {err}"))?;
    Ok(home.join(".ducktape"))
}

fn workspaces_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(root(app)?.join("workspaces"))
}

fn registry_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(root(app)?.join("registry.json"))
}

fn load_registry(app: &tauri::AppHandle) -> Result<Registry, String> {
    let path = registry_path(app)?;
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|err| format!("parse {path:?}: {err}")),
        // a missing registry is the first-run empty state, not an error.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Registry {
            version: 1,
            active: None,
            workspaces: Vec::new(),
        }),
        Err(err) => Err(format!("read {path:?}: {err}")),
    }
}

fn save_registry(app: &tauri::AppHandle, reg: &Registry) -> Result<(), String> {
    let dir = root(app)?;
    fs::create_dir_all(&dir).map_err(|err| format!("create {dir:?}: {err}"))?;
    let path = registry_path(app)?;
    let text = serde_json::to_string_pretty(reg).map_err(|err| err.to_string())?;
    fs::write(&path, text).map_err(|err| format!("write {path:?}: {err}"))
}

// ── Helpers ─────────────────────────────────────────────

/// a workspace id from a display name: lowercase, dash-separated, non-empty,
/// and unique within the registry (a `-2`, `-3`… suffix on collision).
fn unique_id(name: &str, taken: &[Workspace]) -> String {
    let base: String = {
        let slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let trimmed = slug.trim_matches('-').replace("--", "-");
        if trimmed.is_empty() {
            "workspace".into()
        } else {
            trimmed
        }
    };
    if !taken.iter().any(|w| w.id == base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.iter().any(|w| &w.id == candidate))
        .expect("the natural numbers are not exhausted")
}

/// grab a free localhost port by binding `:0` and reading the assignment back.
/// a local single-user TOCTOU window we accept — the node rebinds it moments
/// later. `used` avoids handing out the same port twice in one allocation.
fn free_port(used: &[u16]) -> Result<u16, String> {
    for _ in 0..64 {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|err| format!("probe free port: {err}"))?;
        let port = listener.local_addr().map_err(|err| err.to_string())?.port();
        drop(listener);
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    Err("could not find a free localhost port".into())
}

/// three distinct free ports, avoiding every port already recorded in the
/// registry — a stopped workspace's ports are still ITS ports; handing them to
/// a new workspace would collide the moment both run.
fn allocate_ports(reserved: &[u16]) -> Result<Ports, String> {
    let mut used = reserved.to_vec();
    let listen = free_port(&used)?;
    used.push(listen);
    let http = free_port(&used)?;
    used.push(http);
    let rpc = free_port(&used)?;
    Ok(Ports { listen, http, rpc })
}

/// every port the registry has already committed to a workspace.
fn reserved_ports(reg: &Registry) -> Vec<u16> {
    reg.workspaces
        .iter()
        .flat_map(|w| [w.ports.listen, w.ports.http, w.ports.rpc])
        .collect()
}

/// run a `ducktape-node` onboarding verb to completion and return its stdout
/// (trimmed). the verbs print the datum (chain-id, pubkey, invite blob) to
/// stdout and human guidance to stderr; a non-zero exit surfaces stderr.
fn run_verb(node_bin: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(node_bin)
        .args(args)
        .output()
        .map_err(|err| format!("run ducktape-node {}: {err}", args.first().unwrap_or(&"")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!(
                "ducktape-node {} exited {}",
                args.first().unwrap_or(&""),
                out.status
            )
        } else {
            detail.to_string()
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// the last non-empty line of a verb's stdout — the datum (verbs may print a
/// trailing summary line; the payload is always last).
fn last_line(stdout: &str) -> String {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .next_back()
        .unwrap_or("")
        .to_string()
}

/// pull `chain_id` and `validators` out of a written `network.toml`.
fn read_descriptor(dir: &Path) -> Result<(String, Vec<String>), String> {
    #[derive(Deserialize)]
    struct Descriptor {
        chain_id: String,
        #[serde(default)]
        validators: Vec<String>,
    }
    let path = dir.join("network.toml");
    let text = fs::read_to_string(&path).map_err(|err| format!("read {path:?}: {err}"))?;
    let d: Descriptor = toml::from_str(&text).map_err(|err| format!("parse {path:?}: {err}"))?;
    Ok((d.chain_id, d.validators))
}

fn node_toml(dir: &Path) -> PathBuf {
    dir.join("node.toml")
}

fn find<'a>(reg: &'a Registry, id: &str) -> Result<&'a Workspace, String> {
    reg.workspaces
        .iter()
        .find(|w| w.id == id)
        .ok_or_else(|| format!("no workspace {id:?}"))
}

// ── Commands ────────────────────────────────────────────

/// every workspace in the registry, in creation order.
#[tauri::command]
pub fn workspace_list(app: tauri::AppHandle) -> Result<Vec<Workspace>, String> {
    Ok(load_registry(&app)?.workspaces)
}

/// the active workspace, or null on first run / after none is selected.
#[tauri::command]
pub fn workspace_active(app: tauri::AppHandle) -> Result<Option<Workspace>, String> {
    let reg = load_registry(&app)?;
    Ok(reg
        .active
        .as_ref()
        .and_then(|id| reg.workspaces.iter().find(|w| &w.id == id).cloned()))
}

/// found a NEW network: mint a fresh chain-id + this workspace's identity, seed
/// the genesis validator set with it (a solo 1-validator network usable at
/// once), and record it active. does not spawn — the ui calls
/// [`workspace_select`] next.
#[tauri::command]
pub fn workspace_create(app: tauri::AppHandle, name: String) -> Result<Workspace, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("a workspace needs a name".into());
    }
    let node_bin = crate::daemon::resolve_node_bin()?;
    let mut reg = load_registry(&app)?;
    let id = unique_id(&name, &reg.workspaces);
    let dir = workspaces_dir(&app)?.join(&id);
    fs::create_dir_all(&dir).map_err(|err| format!("create {dir:?}: {err}"))?;
    let ports = allocate_ports(&reserved_ports(&reg))?;

    let listen = format!("127.0.0.1:{}", ports.listen);
    let http = format!("127.0.0.1:{}", ports.http);
    let rpc = format!("127.0.0.1:{}", ports.rpc);
    let dir_s = dir.to_string_lossy().to_string();
    let chain_id = run_verb(
        &node_bin,
        &[
            "init",
            "--name",
            &name,
            "--dir",
            &dir_s,
            "--listen",
            &listen,
            "--advertised",
            &listen,
            "--http",
            &http,
            "--rpc",
            &rpc,
        ],
    )
    .map(|out| last_line(&out))?;
    // read the pubkey back off the identity `init` just wrote (keygen reuses an
    // existing key and prints it) rather than parsing verb stderr.
    let pubkey = run_verb(
        &node_bin,
        &[
            "keygen",
            "--out",
            &dir.join("identity.key").to_string_lossy(),
        ],
    )
    .map(|out| last_line(&out))?;

    let workspace = Workspace {
        id: id.clone(),
        name,
        chain_id,
        pubkey,
        founder: true,
        member: true,
        ports,
    };
    reg.workspaces.push(workspace.clone());
    reg.active = Some(id);
    save_registry(&app, &reg)?;
    Ok(workspace)
}

/// JOIN an existing network from an invite blob: materialize the workspace
/// (descriptor + this identity + config) and record it. a non-member identity
/// will PARK when started until a member admits it — that is surfaced by
/// [`workspace_phase`], not here.
#[tauri::command]
pub fn workspace_join(
    app: tauri::AppHandle,
    name: String,
    blob: String,
) -> Result<Workspace, String> {
    let name = name.trim().to_string();
    let blob = blob.trim().to_string();
    if name.is_empty() {
        return Err("a workspace needs a name".into());
    }
    if blob.is_empty() {
        return Err("paste the invite blob to join".into());
    }
    let node_bin = crate::daemon::resolve_node_bin()?;
    let mut reg = load_registry(&app)?;
    let id = unique_id(&name, &reg.workspaces);
    let dir = workspaces_dir(&app)?.join(&id);
    fs::create_dir_all(&dir).map_err(|err| format!("create {dir:?}: {err}"))?;
    let ports = allocate_ports(&reserved_ports(&reg))?;

    let listen = format!("127.0.0.1:{}", ports.listen);
    let http = format!("127.0.0.1:{}", ports.http);
    let rpc = format!("127.0.0.1:{}", ports.rpc);
    let dir_s = dir.to_string_lossy().to_string();
    // join prints this identity's pubkey (for the inviter's admit) on stdout.
    let pubkey = run_verb(
        &node_bin,
        &[
            "join",
            &blob,
            "--dir",
            &dir_s,
            "--listen",
            &listen,
            "--advertised",
            &listen,
            "--http",
            &http,
            "--rpc",
            &rpc,
        ],
    )
    .map(|out| last_line(&out))?;

    let (chain_id, validators) = read_descriptor(&dir)?;
    // refuse joining the same network into two workspaces — one dir per chain.
    if let Some(existing) = reg.workspaces.iter().find(|w| w.chain_id == chain_id) {
        // undo the fresh dir so a rejected join leaves nothing behind.
        let _ = fs::remove_dir_all(&dir);
        return Err(format!(
            "already joined this network as {:?}",
            existing.name
        ));
    }
    let member = validators.contains(&pubkey);

    let workspace = Workspace {
        id: id.clone(),
        name,
        chain_id,
        pubkey,
        founder: false,
        member,
        ports,
    };
    reg.workspaces.push(workspace.clone());
    reg.active = Some(id);
    save_registry(&app, &reg)?;
    Ok(workspace)
}

/// the one-line invite blob to hand a friend, refreshed with this member's dial
/// hint. requires the workspace to have been founded/joined (it reads config).
#[tauri::command]
pub fn workspace_invite_blob(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let node_bin = crate::daemon::resolve_node_bin()?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&node_bin, &["invite", "--config", &cfg.to_string_lossy()]).map(|out| last_line(&out))
}

/// admit a joiner by pubkey: drive the governance AddValidator through THIS
/// running member node's local rpc. the node must be started (a member serves
/// rpc); the joiner's parked node promotes itself once the epoch cuts over.
#[tauri::command]
pub fn workspace_admit(app: tauri::AppHandle, id: String, pubkey: String) -> Result<(), String> {
    let pubkey = pubkey.trim().to_string();
    if pubkey.is_empty() {
        return Err("paste the joiner's identity pubkey to admit".into());
    }
    let node_bin = crate::daemon::resolve_node_bin()?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(
        &node_bin,
        &["invite-accept", &pubkey, "--config", &cfg.to_string_lossy()],
    )
    .map(|_| ())
}

/// make `id` active and ensure its node is running; return the http url the
/// webview should dial. adopts an already-listening node (idempotent across
/// re-selects and the promotion exec-reboot) instead of double-spawning.
#[tauri::command]
pub fn workspace_select(app: tauri::AppHandle, id: String) -> Result<Selection, String> {
    let node_bin = crate::daemon::resolve_node_bin()?;
    let mut reg = load_registry(&app)?;
    let ws = find(&reg, &id)?.clone();
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let http_url = format!("http://127.0.0.1:{}", ws.ports.http);

    if reg.active.as_deref() != Some(&ws.id) {
        reg.active = Some(ws.id.clone());
        save_registry(&app, &reg)?;
    }

    // already running? adopt it — never spawn a second process for one
    // workspace. we probe the p2p LISTEN port, not http: a parked joiner serves
    // no http yet but its mesh listener is bound the whole time, so this is the
    // one port that is up in every phase (parked, promoting, validator). a
    // second spawn would collide on exactly this port anyway.
    if port_listening(ws.ports.listen) {
        return Ok(Selection {
            id: ws.id,
            http_url,
        });
    }

    let log_path = dir.join("daemon.log");
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|err| format!("open {log_path:?}: {err}"))?;
    let log_err = log.try_clone().map_err(|err| err.to_string())?;
    let mut cmd = Command::new(&node_bin);
    cmd.arg("--config")
        .arg(node_toml(&dir))
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);
    crate::daemon::detach(&mut cmd);
    cmd.spawn()
        .map_err(|err| format!("spawn ducktape-node: {err}"))?;
    Ok(Selection {
        id: ws.id,
        http_url,
    })
}

/// read this workspace's onboarding phase back from `daemon.log`. a parked
/// joiner serves no http/rpc, so its log is the only progress signal; the
/// webview treats a successful `/v1/status` as the authoritative "ready" and
/// only falls back to this while the surface is still down.
#[tauri::command]
pub fn workspace_phase(app: tauri::AppHandle, id: String) -> Result<PhaseReport, String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let log_path = workspaces_dir(&app)?.join(&ws.id).join("daemon.log");
    let tail = read_tail(&log_path, 64 * 1024)?;
    Ok(classify(&tail))
}

// ── Phase classification ────────────────────────────────

/// map the node's stable stdout markers to a phase. the log only appends and,
/// within a boot, prints these markers in phase order — so the LAST line that
/// matches any marker is the current phase. last-match (not highest-rank) is
/// deliberate: an old `FATAL` from a prior boot must not outrank a later
/// successful restart that reparks and promotes on the same appended log.
fn classify(log: &str) -> PhaseReport {
    // (phase, marker substring). the strings are a contract with
    // bin/node/src/main.rs (asserted by bin/node/tests/invite_e2e.rs).
    const MARKERS: &[(&str, &str)] = &[
        ("parked", "joiner mode: parking"),
        ("parked", "parked:"),
        ("admitted", "admitted at epoch"),
        ("synced", "synced app_hash="),
        ("promoted", "promoted:"),
        ("fatal", "FATAL"),
        ("fatal", "not admitted after"),
    ];
    let mut latest: Option<(&str, String)> = None;
    for line in log.lines() {
        if let Some((phase, _)) = MARKERS.iter().find(|(_, needle)| line.contains(needle)) {
            let detail = line
                .split_once("] ")
                .map(|(_, rest)| rest)
                .unwrap_or(line)
                .trim()
                .to_string();
            latest = Some((phase, detail));
        }
    }
    match latest {
        Some((phase, detail)) => PhaseReport {
            phase: phase.to_string(),
            detail: Some(detail),
        },
        // no marker yet: a founder never parks, so this is just early boot.
        None => PhaseReport {
            phase: "starting".into(),
            detail: None,
        },
    }
}

/// the last `max` bytes of a file as lossy utf-8; empty string if absent.
fn read_tail(path: &Path, max: u64) -> Result<String, String> {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(err) => return Err(format!("open {path:?}: {err}")),
    };
    let len = file.metadata().map_err(|err| err.to_string())?.len();
    let start = len.saturating_sub(max);
    file.seek(SeekFrom::Start(start))
        .map_err(|err| err.to_string())?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|err| err.to_string())?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// is something accepting connections on this localhost port right now? used as
/// a liveness probe for an already-running workspace node.
fn port_listening(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_unique_and_safe() {
        let taken = vec![];
        assert_eq!(unique_id("My Team!", &taken), "my-team");
        let taken = vec![Workspace {
            id: "my-team".into(),
            name: "My Team".into(),
            chain_id: "x".into(),
            pubkey: "y".into(),
            founder: true,
            member: true,
            ports: Ports {
                listen: 1,
                http: 2,
                rpc: 3,
            },
        }];
        assert_eq!(unique_id("My Team", &taken), "my-team-2");
        assert_eq!(unique_id("***", &taken), "workspace");
    }

    #[test]
    fn classify_ranks_latest_phase() {
        let log = "[node ab] joiner mode: parking on the mesh\n\
                   [node ab] parked: awaiting admission (epoch 0 has 1 validators)\n\
                   [node ab] admitted at epoch 1 boundary 4 — syncing 16 modules\n\
                   [node ab] synced app_hash=deadbeef\n\
                   [node ab] promoted: validator at epoch 1 boundary 4 — rebooting\n";
        let r = classify(log);
        assert_eq!(r.phase, "promoted");
    }

    #[test]
    fn classify_parked_holds_until_admitted() {
        let log = "[node ab] joiner mode: parking on the mesh\n\
                   [node ab] parked: awaiting admission (epoch 0 has 1 validators)\n";
        let r = classify(log);
        assert_eq!(r.phase, "parked");
        assert!(r.detail.unwrap().contains("awaiting admission"));
    }

    #[test]
    fn classify_empty_is_starting() {
        assert_eq!(classify("").phase, "starting");
    }

    #[test]
    fn classify_recovers_from_a_stale_fatal() {
        // an old fatal, then a restart that reparks and promotes on the same
        // appended log — the latest line wins, not the scariest one.
        let log = "[node ab] FATAL: still not admitted after 900 attempts\n\
                   [node ab] joiner mode: parking on the mesh\n\
                   [node ab] parked: awaiting admission (epoch 0 has 1 validators)\n\
                   [node ab] promoted: validator at epoch 1 boundary 4 — rebooting\n";
        assert_eq!(classify(log).phase, "promoted");
    }

    #[test]
    fn allocated_ports_avoid_reserved() {
        let reserved = [40000u16, 40001, 40002];
        let p = allocate_ports(&reserved).unwrap();
        for got in [p.listen, p.http, p.rpc] {
            assert!(!reserved.contains(&got));
        }
        assert_ne!(p.listen, p.http);
        assert_ne!(p.http, p.rpc);
        assert_ne!(p.listen, p.rpc);
    }
}
