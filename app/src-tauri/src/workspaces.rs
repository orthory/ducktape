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
//! mutations reuse the SAME onboarding verbs the CLI exposes
//! (`init`/`join`/`invite`/`invite-accept`) behind the bounded
//! [`crate::daemon::NodeControl`] actor, so no Tauri handler executes or waits
//! on the node binary and the registry never reimplements identity,
//! descriptors, or governance — it only allocates ports, lays out directories,
//! and remembers the result. NOTE a parked joiner may well serve
//! its http/rpc surface (newer node builds do — every read just answers
//! "parked: no state to serve"), so an answering port is NOT admission;
//! onboarding progress is read back from the stable marker lines the node
//! prints to `daemon.log` — see [`workspace_phase`].

use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager as _;

use crate::daemon::{NodeControl, last_line, require_main_window, run_verb};

const DEFAULT_PRIMARY_COORDINATOR: &str = "p2p.ducktape.byeongsu.dev:3478";

/// the coordinator handed to every spawned node: the deployed public
/// rendezvous by default, overridable via `DUCKTAPE_PRIMARY_COORDINATOR`
/// (passed verbatim — the node CLI accepts `none` to opt out). the no-UI
/// escape hatch for self-hosted coordinators; deliberately not a setting.
fn primary_coordinator() -> String {
    std::env::var("DUCKTAPE_PRIMARY_COORDINATOR")
        .unwrap_or_else(|_| DEFAULT_PRIMARY_COORDINATOR.into())
}

const MAX_WORKSPACE_NAME_BYTES: usize = 128;
const MAX_INVITE_BLOB_BYTES: usize = 256 * 1024;

fn validate_workspace_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("a workspace needs a name".into());
    }
    if name.len() > MAX_WORKSPACE_NAME_BYTES {
        return Err(format!(
            "workspace name is too long ({} bytes; limit {MAX_WORKSPACE_NAME_BYTES})",
            name.len()
        ));
    }
    Ok(())
}

fn validate_node_pubkey(pubkey: &str) -> Result<(), String> {
    if pubkey.len() != 64 || !pubkey.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("node public key must be exactly 32 bytes of hex".into());
    }
    Ok(())
}

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
struct Registry {
    /// bumped if the on-disk shape ever changes; 1 today.
    version: u32,
    /// the workspace whose node the app currently talks to.
    active: Option<String>,
    workspaces: Vec<Workspace>,
    /// has the identity-creation mnemonic been shown + re-entered once on
    /// this machine? UX-only (no security weight) — gates whether the
    /// identity gate re-shows the "confirm your recovery phrase" step.
    /// `#[serde(default)]` so a pre-existing `registry.json` (version stays
    /// 1) keeps loading with this defaulting to `false`.
    #[serde(default)]
    mnemonic_confirmed: bool,
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

fn workspaces_dir(app: &crate::rt::AppHandle) -> Result<PathBuf, String> {
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

fn load_registry(app: &crate::rt::AppHandle) -> Result<Registry, String> {
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
fn load_registry_at(path: &Path) -> Result<Registry, String> {
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

fn save_registry(app: &crate::rt::AppHandle, reg: &Registry) -> Result<(), String> {
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
fn save_registry_at(path: &Path, reg: &Registry) -> Result<(), String> {
    let text = serde_json::to_string_pretty(reg).map_err(|err| err.to_string())?;
    write_atomic(path, text.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
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

/// [`free_port`]'s UDP twin — the wireguard/invite listeners are UDP, and a
/// free TCP port says nothing about the same UDP port (a collision silently
/// disables the reachability plane). same TOCTOU window, same dedup.
fn free_udp_port(used: &[u16]) -> Result<u16, String> {
    for _ in 0..64 {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0")
            .map_err(|err| format!("probe free udp port: {err}"))?;
        let port = socket.local_addr().map_err(|err| err.to_string())?.port();
        drop(socket);
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    Err("could not find a free localhost udp port".into())
}

/// distinct free ports, avoiding every port already recorded in the
/// registry — a stopped workspace's ports are still ITS ports; handing them to
/// a new workspace would collide the moment both run.
fn allocate_ports(reserved: &[u16]) -> Result<Ports, String> {
    let mut used = reserved.to_vec();
    let listen = free_port(&used)?;
    used.push(listen);
    let http = free_port(&used)?;
    used.push(http);
    let rpc = free_port(&used)?;
    used.push(rpc);
    let wireguard = free_udp_port(&used)?;
    used.push(wireguard);
    let invite = free_udp_port(&used)?;
    Ok(Ports {
        listen,
        http,
        rpc,
        wireguard: Some(wireguard),
        invite: Some(invite),
    })
}

/// every port the registry has already committed to a workspace.
fn reserved_ports(reg: &Registry) -> Vec<u16> {
    reg.workspaces
        .iter()
        .flat_map(|w| {
            [
                Some(w.ports.listen),
                Some(w.ports.http),
                Some(w.ports.rpc),
                w.ports.wireguard,
                w.ports.invite,
            ]
        })
        .flatten()
        .collect()
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
pub fn workspace_list(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
) -> Result<Vec<Workspace>, String> {
    require_main_window(&window)?;
    Ok(load_registry(&app)?.workspaces)
}

/// the active workspace, or null on first run / after none is selected.
#[tauri::command]
pub fn workspace_active(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
) -> Result<Option<Workspace>, String> {
    require_main_window(&window)?;
    let reg = load_registry(&app)?;
    Ok(reg
        .active
        .as_ref()
        .and_then(|id| reg.workspaces.iter().find(|w| &w.id == id).cloned()))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRouteName {
    pub label: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayLocalRoute {
    pub name: GatewayRouteName,
    pub port: u16,
}

/// Bind one globally published gateway route (apex when `label` is null) to an
/// exact loopback port. The port remains node-local and the sidecar persists no
/// URL, host, or executable command.
#[tauri::command]
pub async fn gateway_route_bind(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    label: Option<String>,
    port: u16,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || gateway_route_bind_blocking(app, id, label, port))
        .await
}

fn gateway_route_bind_blocking(
    app: crate::rt::AppHandle,
    id: String,
    label: Option<String>,
    port: u16,
) -> Result<(), String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let mut args = vec![
        "gateway-route-bind".to_string(),
        "--workspace".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    if let Some(label) = label {
        args.extend(["--label".to_string(), label]);
    }
    args.extend(["--port".to_string(), port.to_string()]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_verb(&refs).map(|_| ())
}

#[tauri::command]
pub async fn gateway_route_unbind(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    label: Option<String>,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || gateway_route_unbind_blocking(app, id, label))
        .await
}

fn gateway_route_unbind_blocking(
    app: crate::rt::AppHandle,
    id: String,
    label: Option<String>,
) -> Result<(), String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let mut args = vec![
        "gateway-route-unbind".to_string(),
        "--workspace".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    if let Some(label) = label {
        args.extend(["--label".to_string(), label]);
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_verb(&refs).map(|_| ())
}

#[tauri::command]
pub async fn gateway_route_list(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
) -> Result<Vec<GatewayLocalRoute>, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || gateway_route_list_blocking(app, id))
        .await
}

fn gateway_route_list_blocking(
    app: crate::rt::AppHandle,
    id: String,
) -> Result<Vec<GatewayLocalRoute>, String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let out = run_verb(&[
        "gateway-route-list",
        "--workspace",
        &dir.to_string_lossy(),
    ])?;
    serde_json::from_str(last_line(&out).trim())
        .map_err(|error| format!("gateway-route-list output is not json: {error}"))
}

/// found a NEW network: mint a fresh chain-id + this workspace's identity, seed
/// the genesis validator set with it (a solo 1-validator network usable at
/// once), and record it active. does not spawn — the ui calls
/// [`workspace_select`] next.
#[tauri::command]
pub async fn workspace_create(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    name: String,
) -> Result<Workspace, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_create_blocking(app, name))
        .await
}

fn workspace_create_blocking(app: crate::rt::AppHandle, name: String) -> Result<Workspace, String> {
    let name = name.trim().to_string();
    validate_workspace_name(&name)?;
    let mut reg = load_registry(&app)?;
    let id = unique_id(&name, &reg.workspaces);
    let dir = workspaces_dir(&app)?.join(&id);
    fs::create_dir_all(&dir).map_err(|err| format!("create {dir:?}: {err}"))?;
    let ports = allocate_ports(&reserved_ports(&reg))?;

    // bind the mesh listener dual-stack ([::] accepts BOTH families on a
    // default dual-stack host — 0.0.0.0 could never accept the v6 overlay-ULA
    // dials that ride WireGuard tunnels).
    let listen = format!("[::]:{}", ports.listen);
    let http = format!("127.0.0.1:{}", ports.http);
    let rpc = format!("127.0.0.1:{}", ports.rpc);
    let wireguard = format!(
        "0.0.0.0:{}",
        ports
            .wireguard
            .ok_or("workspace allocator did not assign a wireguard port")?
    );
    let invite = format!(
        "0.0.0.0:{}",
        ports
            .invite
            .ok_or("workspace allocator did not assign an invite port")?
    );
    let dir_s = dir.to_string_lossy().to_string();
    let coordinator = primary_coordinator();
    // desktop-spawned nodes run the TUN-less userspace WireGuard backend
    // (overlay-net ADR phase 4): no /dev/net/tun, no setcap, no host
    // mutation. self-managed configs keep the parse default (`tun`).
    let chain_id = run_verb(&[
        "init",
        "--name",
        &name,
        "--dir",
        &dir_s,
        "--listen",
        &listen,
        "--http",
        &http,
        "--gateway",
        "127.0.0.1:0",
        "--rpc",
        &rpc,
        "--primary-coordinator",
        &coordinator,
        "--wireguard-listen",
        &wireguard,
        "--invite-listen",
        &invite,
        "--wireguard-effect",
        "socket",
    ])
    .map(|out| last_line(&out))?;
    // read the pubkey back off the identity `init` just wrote (keygen reuses an
    // existing key and prints it) rather than parsing verb stderr.
    let pubkey = run_verb(&[
        "keygen",
        "--out",
        &dir.join("identity.key").to_string_lossy(),
    ])
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
pub async fn workspace_join(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    name: String,
    blob: String,
) -> Result<Workspace, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_join_blocking(app, name, blob))
        .await
}

fn workspace_join_blocking(
    app: crate::rt::AppHandle,
    name: String,
    blob: String,
) -> Result<Workspace, String> {
    let name = name.trim().to_string();
    let blob = blob.trim().to_string();
    validate_workspace_name(&name)?;
    if blob.is_empty() {
        return Err("paste the invite blob to join".into());
    }
    if blob.len() > MAX_INVITE_BLOB_BYTES {
        return Err(format!(
            "invite blob is too large ({} bytes; limit {MAX_INVITE_BLOB_BYTES})",
            blob.len()
        ));
    }
    let mut reg = load_registry(&app)?;
    let id = unique_id(&name, &reg.workspaces);
    let dir = workspaces_dir(&app)?.join(&id);
    fs::create_dir_all(&dir).map_err(|err| format!("create {dir:?}: {err}"))?;
    let ports = allocate_ports(&reserved_ports(&reg))?;

    // bind dual-stack, but pass NO --advertised and NO --listen: a joiner
    // needs zero reachability config. `cmd_join` picks the right defaults per
    // invite shape (a WG invite: `[::]` mesh listen + advertised "overlay",
    // dialable over the tunnel) — a guessed public IP here would override
    // that and gossip an unreachable address that poisons working routes.
    // only the ports stay ours so the registry's per-workspace allocation
    // holds: listen rides --listen with the unspecified host.
    let listen = format!("[::]:{}", ports.listen);
    let http = format!("127.0.0.1:{}", ports.http);
    let rpc = format!("127.0.0.1:{}", ports.rpc);
    let wireguard = format!(
        "0.0.0.0:{}",
        ports
            .wireguard
            .ok_or("workspace allocator did not assign a wireguard port")?
    );
    let invite = format!(
        "0.0.0.0:{}",
        ports
            .invite
            .ok_or("workspace allocator did not assign an invite port")?
    );
    let dir_s = dir.to_string_lossy().to_string();
    // join prints this identity's pubkey (for the inviter's admit) on stdout.
    // --wireguard-effect socket: the desktop default (overlay-net ADR phase
    // 4) — a WG-invite join brings the plane up TUN-less, no privileges.
    let pubkey = run_verb(&[
        "join",
        &blob,
        "--dir",
        &dir_s,
        "--listen",
        &listen,
        "--http",
        &http,
        "--gateway",
        "127.0.0.1:0",
        "--rpc",
        &rpc,
        "--wireguard-listen",
        &wireguard,
        "--invite-listen",
        &invite,
        "--wireguard-effect",
        "socket",
    ])
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
pub async fn workspace_invite_blob(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_invite_blob_blocking(app, id))
        .await
}

fn workspace_invite_blob_blocking(app: crate::rt::AppHandle, id: String) -> Result<String, String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&["invite", "--config", &cfg.to_string_lossy()]).map(|out| last_line(&out))
}

/// the join requests parked joiners delivered to this member's running node
/// over the lobby channel — the queue the Members view renders with an
/// "Approve" button (approve = [`workspace_admit`], the normal governance
/// ballot). raw JSON array from the `join-requests` verb, parsed here so the
/// frontend gets typed rows.
#[tauri::command]
pub async fn workspace_join_requests(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
) -> Result<Vec<serde_json::Value>, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_join_requests_blocking(app, id))
        .await
}

fn workspace_join_requests_blocking(
    app: crate::rt::AppHandle,
    id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    let out = run_verb(&["join-requests", "--config", &cfg.to_string_lossy()])?;
    serde_json::from_str(last_line(&out).trim())
        .map_err(|e| format!("join-requests output is not json: {e}"))
}

/// admit a joiner by pubkey: drive the governance AddValidator through THIS
/// running member node's local rpc. the node must be started (a member serves
/// rpc); the joiner's parked node promotes itself once the epoch cuts over.
#[tauri::command]
pub async fn workspace_admit(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_admit_blocking(app, id, pubkey))
        .await
}

fn workspace_admit_blocking(
    app: crate::rt::AppHandle,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    let pubkey = pubkey.trim().to_string();
    validate_node_pubkey(&pubkey)
        .map_err(|_| "paste the joiner's 32-byte identity pubkey to admit".to_string())?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&["invite-accept", &pubkey, "--config", &cfg.to_string_lossy()]).map(|_| ())
}

/// remove a validator by pubkey: drive the governance RemoveValidator through
/// THIS running member node's local rpc. the removal counterpart of
/// [`workspace_admit`] — it opens a removal proposal and casts this node's
/// yes-ballot; the change only takes effect once a strict majority of members
/// approve, and the removed node drops out at the next epoch cutover.
#[tauri::command]
pub async fn workspace_demote(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_demote_blocking(app, id, pubkey))
        .await
}

fn workspace_demote_blocking(
    app: crate::rt::AppHandle,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    let pubkey = pubkey.trim().to_string();
    validate_node_pubkey(&pubkey)
        .map_err(|_| "provide the validator's 32-byte public key to remove".to_string())?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&["member-remove", &pubkey, "--config", &cfg.to_string_lossy()]).map(|_| ())
}

/// promote a resident into the consensus quorum by pubkey: drive the
/// governance AddValidator through THIS running member node's local rpc. the
/// second, deliberate step of staged admission — [`workspace_admit`] grants
/// resident standing; this seats the (pre-synced, warm) key as a validator at
/// the next epoch cutover. same majority ceremony as every membership change.
#[tauri::command]
pub async fn workspace_promote(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_promote_blocking(app, id, pubkey))
        .await
}

fn workspace_promote_blocking(
    app: crate::rt::AppHandle,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    let pubkey = pubkey.trim().to_string();
    validate_node_pubkey(&pubkey)
        .map_err(|_| "provide the resident's 32-byte public key to promote".to_string())?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&["promote", &pubkey, "--config", &cfg.to_string_lossy()]).map(|_| ())
}

/// revoke resident standing by pubkey: drive the governance RemoveResident
/// through THIS running member node's local rpc. the undo of
/// [`workspace_admit`] — the key drops off the mesh at the next epoch cutover
/// and its node parks again; re-granting is another admit. a seated validator
/// is [`workspace_demote`]'s job (the tiers never overlap).
#[tauri::command]
pub async fn workspace_resident_remove(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_resident_remove_blocking(app, id, pubkey))
        .await
}

fn workspace_resident_remove_blocking(
    app: crate::rt::AppHandle,
    id: String,
    pubkey: String,
) -> Result<(), String> {
    let pubkey = pubkey.trim().to_string();
    validate_node_pubkey(&pubkey)
        .map_err(|_| "provide the resident's 32-byte public key to revoke".to_string())?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&[
        "resident-remove",
        &pubkey,
        "--config",
        &cfg.to_string_lossy(),
    ])
    .map(|_| ())
}

/// REQUEST to leave a network: drive this node's on-chain SELF-removal, and
/// KEEP THE NODE RUNNING. the honest first half of departure — the node must
/// stay up through its own pending removal, because it is a current validator
/// and commonware's fault model needs every validator to sign to finalize the
/// Execute block that completes the removal. tear the node down here and the
/// remaining member(s) can NEVER finalize it — the network halts and this node
/// stays a ghost validator forever.
///
/// so this ONLY runs `member-leave` over the running node's rpc: it opens a
/// RemoveValidator proposal for OUR OWN key and casts our yes-ballot. in a set
/// of two-or-more this stays PENDING until a strict majority of the REMAINING
/// members approve; once they do, the epoch cuts over and this node drops out of
/// the valset — at which point it is safe to [`workspace_forget`] it.
///
/// a solo (n==1) node is refused here by the last-validator guard (you cannot
/// remove the last validator); a lone node just forgets its workspace directly.
/// errors surface to the caller (unlike forget, this is not best-effort — the
/// user asked to submit an on-chain change and deserves to know if it failed).
#[tauri::command]
pub async fn workspace_request_leave(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_request_leave_blocking(app, id))
        .await
}

fn workspace_request_leave_blocking(app: crate::rt::AppHandle, id: String) -> Result<(), String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    run_verb(&["member-leave", "--config", &cfg.to_string_lossy()]).map(|_| ())
}

/// wrap a "couldn't reach/resolve the node" failure into the honest refusal we
/// show when [`workspace_forget`] cannot confirm this node has left the valset.
/// FAIL CLOSED: we would rather strand a user behind a startable node than
/// destroy the identity a still-in-set validator needs to finalize its removal.
fn unconfirmed_forget(detail: String) -> String {
    format!(
        "start the node and finish leaving — we can't confirm this workspace has left the \
         validator set ({detail}), and destroying its identity now could permanently halt the \
         network. bring the node up, request to leave, and wait until the other members approve \
         (you drop out of the set) before forgetting this workspace."
    )
}

/// the verdict of the pre-forget membership probe — drives whether teardown is
/// allowed and, when refused, whether a FORCE forget may override the refusal.
enum ForgetVerdict {
    /// definitively safe to tear down: this node is out of the valset
    /// (`in-set=false`), or a provably solo network (`in-set=true validators=1`,
    /// no peer to strand).
    Safe,
    /// the running node CONFIRMS it is still a current validator of a set of
    /// two-or-more. tearing it down halts quorum and strands the pending removal,
    /// so this refusal is ABSOLUTE — a force forget cannot override a provably
    /// live multi-member validator; request-leave-and-wait first.
    ConfirmedInSet(String),
    /// membership could NOT be confirmed — node down/bricked, rpc error, no node
    /// binary, or a status line we cannot parse. refused by default (fail
    /// closed), but this is the UNCERTAINTY a force forget overrides: a node that
    /// can never start can never finalize a removal, so keeping it only strands
    /// the user with a workspace they can never remove.
    Unconfirmed(String),
}

/// classify a `member-status` stdout line (`in-set=<bool> validators=<n>`) into a
/// [`ForgetVerdict`]. FAILS CLOSED: only a definitively out-of-set or provably
/// solo line is `Safe`; a confirmed in-set set-of-two-or-more is `ConfirmedInSet`;
/// anything we cannot parse into BOTH fields is `Unconfirmed` — an unreadable
/// status is uncertainty, never an authorization to destroy an identity.
fn classify_status(status_line: &str) -> ForgetVerdict {
    let in_set = if status_line.contains("in-set=true") {
        Some(true)
    } else if status_line.contains("in-set=false") {
        Some(false)
    } else {
        None
    };
    let validators = status_line
        .split_whitespace()
        .find_map(|tok| tok.strip_prefix("validators="))
        .and_then(|n| n.parse::<usize>().ok());
    match (in_set, validators) {
        // already left the set, or a provably solo network — safe to forget.
        (Some(false), _) | (Some(true), Some(1)) => ForgetVerdict::Safe,
        // definitively still a current validator of a multi-member set.
        (Some(true), Some(n)) => ForgetVerdict::ConfirmedInSet(format!(
            "this node is still a current validator of {n} — forgetting it now would halt the \
             network's quorum and strand your removal. request to leave first, then wait until \
             the other members approve (you drop out of the set) before forgetting this \
             workspace."
        )),
        // ambiguous/unparseable status — fail closed, do NOT destroy the identity.
        _ => ForgetVerdict::Unconfirmed(
            "couldn't confirm this workspace has left the validator set (the node's membership \
             status was unreadable) — refusing to forget it, because destroying its identity \
             while it may still be a validator could permanently halt the network. bring the \
             node up and finish leaving first."
                .to_string(),
        ),
    }
}

/// probe the RUNNING node for whether tearing this workspace down is safe. any
/// failure to reach, resolve, or read the node's membership collapses to
/// `Unconfirmed` (fail closed) — exactly the uncertainty a force forget may
/// override for a node that can no longer start.
fn probe_forget(dir: &Path) -> ForgetVerdict {
    let cfg = node_toml(dir);
    match run_verb(&["member-status", "--config", &cfg.to_string_lossy()]) {
        Ok(status) => classify_status(&last_line(&status)),
        Err(err) => ForgetVerdict::Unconfirmed(unconfirmed_forget(err)),
    }
}

/// FORGET a workspace: stop its node, delete its directory, and drop its
/// registry entry. the honest second half of departure — the destructive local
/// teardown, GUARDED so it can never brick consensus.
///
/// the guard: a node must NOT tear itself down while it is still a current
/// validator of a set of two-or-more with a pending removal — killing it halts
/// quorum (the remaining members can't finalize without its signature) and
/// strands its on-chain removal. so before touching anything, we ask the running
/// node whether it is still in the valset (`member-status`): if `in-set=true`
/// and `validators>=2`, we REFUSE with guidance to request-leave-and-wait first.
/// a lone validator (`validators=1`, a solo network only this node runs) or an
/// already-removed key (`in-set=false`) is safe — forgetting a solo network just
/// destroys a network no one else is in.
///
/// the guard FAILS CLOSED. destroying `identity.key` is irreversible, so we
/// tear down ONLY when the running node DEFINITIVELY confirms it is not holding a
/// multi-member set's quorum: it has already left the valset (`in-set=false`),
/// or it is a provably solo network (`validators=1`, no peer to strand). on ANY
/// uncertainty — node DOWN, rpc error, unresolvable node binary, or an
/// unparseable status line — we REFUSE. a still-in-set validator whose
/// self-removal is merely PENDING is a RECOVERABLE state (restart it and it can
/// sign the Execute block that finalizes the removal); but forget its
/// `identity.key` and that signature can never be produced, so the removal can
/// never finalize and the remaining member(s) can never reach quorum
/// (commonware `quorum(n)=n` for `n<=3`) — a PERMANENT HALT with a ghost
/// validator. a down node is exactly this uncertain case, so by default we
/// require it to be reachable and confirm it has left before its identity may be
/// destroyed.
///
/// `force` is the escape hatch for a node that can NEVER come up (a bricked
/// recovery, corrupt state — its surface FATALs on every start, so the default
/// guard can never confirm anything and the workspace becomes un-removable). it
/// overrides ONLY the `Unconfirmed` verdict (node down/unreachable/unreadable);
/// it can NOT override a `ConfirmedInSet` verdict — a reachable node that proves
/// it is still a current validator of a multi-member set is never force-torn-down
/// (that would halt a provably-live network). the caller gates `force` behind an
/// explicit, honest confirmation.
///
/// returns the newly-active workspace the registry repointed to, or `None` when
/// none remain.
#[tauri::command]
pub async fn workspace_forget(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    force: bool,
) -> Result<Option<Workspace>, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_forget_blocking(app, id, force))
        .await
}

fn workspace_forget_blocking(
    app: crate::rt::AppHandle,
    id: String,
    force: bool,
) -> Result<Option<Workspace>, String> {
    let mut reg = load_registry(&app)?;
    let ws = find(&reg, &id)?.clone();
    let dir = workspaces_dir(&app)?.join(&ws.id);

    // guard (FAIL CLOSED): confirm — via the RUNNING node's own membership — that
    // tearing this workspace down cannot strand a peer or halt quorum. teardown
    // proceeds ONLY on a definitive `Safe` verdict. a `ConfirmedInSet` refusal is
    // ABSOLUTE (force included): we never destroy the identity.key a provably-live
    // multi-member validator needs to finalize its pending removal. an
    // `Unconfirmed` refusal (node down/bricked/unreadable) is what `force`
    // overrides — a node that can never start can never finalize a removal, so
    // keeping it only strands the user with a workspace they can never remove.
    match probe_forget(&dir) {
        ForgetVerdict::Safe => {}
        ForgetVerdict::Unconfirmed(_) if force => {}
        ForgetVerdict::Unconfirmed(msg) => return Err(msg),
        ForgetVerdict::ConfirmedInSet(msg) => return Err(msg),
    }

    // stop the node FOR REAL before touching anything. the old best-effort
    // http shutdown left parked joiners (no http surface) and wedged nodes
    // running: the detached process survived the forget, kept its ports and
    // its mesh presence, and RE-CREATED `storage/` under the just-deleted
    // directory — the workspace haunted the registry it was removed from. a
    // node that cannot be stopped now honestly refuses the forget instead of
    // manufacturing a zombie.
    stop_workspace_node(&dir, &ws.ports, std::time::Duration::from_secs(6))?;

    // delete the directory, then drop the registry entry and repoint active. a
    // failed rmdir (e.g. a still-open file on windows) must not block forgetting
    // the workspace — the registry entry removal is what matters.
    match fs::remove_dir_all(&dir) {
        // an already-absent directory is a forgotten workspace's natural state.
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => eprintln!("workspace_forget: could not remove {dir:?}: {err}"),
    }
    reg.workspaces.retain(|w| w.id != ws.id);
    if reg.active.as_deref() == Some(&ws.id) {
        reg.active = reg.workspaces.first().map(|w| w.id.clone());
    }
    save_registry(&app, &reg)?;

    let next = reg
        .active
        .as_ref()
        .and_then(|active| reg.workspaces.iter().find(|w| &w.id == active))
        .cloned();
    Ok(next)
}

/// make `id` active and ensure its node is running; return the http url the
/// webview should dial. adopts an already-listening node (idempotent across
/// re-selects and the promotion exec-reboot) instead of double-spawning.
#[tauri::command]
pub async fn workspace_select(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
) -> Result<Selection, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_select_blocking(app, id))
        .await
}

fn workspace_select_blocking(app: crate::rt::AppHandle, id: String) -> Result<Selection, String> {
    let mut reg = load_registry(&app)?;
    let ws = find(&reg, &id)?.clone();
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let http_url = format!("http://127.0.0.1:{}", ws.ports.http);

    // already running? adopt it — never spawn a second process for one
    // workspace. we probe the p2p LISTEN port, not http: the mesh listener is
    // bound in every phase (parked, promoting, validator) on every node build,
    // while http may lag behind, so this is the one dependable liveness port.
    // a second spawn would collide on exactly this port anyway.
    if port_listening(ws.ports.listen) {
        return finish_selection(&app, &mut reg, ws, http_url);
    }

    let log_path = dir.join("daemon.log");
    // spawn AND verify the node survived. a bind conflict, an unparseable
    // node.toml, or a boot panic dies in milliseconds — and used to return Ok
    // with a dead http_url the webview would poll for 10s before giving a
    // generic timeout. spawn_verified reads the real reason back out of
    // daemon.log instead. http is the readiness signal for a member/founder; a
    // parking joiner never serves it, so "still alive after the grace" carries.
    let child =
        crate::daemon::spawn_workspace_node(&node_toml(&dir), &log_path, Some(ws.ports.http))
            .map_err(|failure| {
                format!("the node for \"{}\" exited on start: {failure}", ws.name)
            })?;
    // record the detached pid so teardown can address the process directly —
    // the http shutdown route alone can't reach a parked joiner (no surface).
    // best-effort: a failed write only degrades stop back to the pgrep sweep.
    if let Err(err) = fs::write(pidfile(&dir), child.id().to_string()) {
        eprintln!("workspace_select: could not record node pid: {err}");
    }
    // commit `active` ONLY now the node is confirmed up: a select that fails to
    // start the node must not repoint `active` at a workspace the next boot
    // then can't launch (which would strand the app on that dead workspace).
    finish_selection(&app, &mut reg, ws, http_url)
}

fn finish_selection(
    app: &crate::rt::AppHandle,
    reg: &mut Registry,
    workspace: Workspace,
    http_url: String,
) -> Result<Selection, String> {
    commit_active(app, reg, &workspace.id)?;
    Ok(Selection {
        id: workspace.id,
        http_url,
    })
}

/// set `id` as the registry's active workspace, persisting only on a change.
/// pulled out of [`workspace_select`] so both the adopt and the fresh-spawn
/// success paths commit `active` at the same point — after the node is known
/// to be up, never before.
fn commit_active(app: &crate::rt::AppHandle, reg: &mut Registry, id: &str) -> Result<(), String> {
    if reg.active.as_deref() != Some(id) {
        reg.active = Some(id.to_string());
        save_registry(app, reg)?;
    }
    Ok(())
}

/// read this workspace's onboarding phase back from `daemon.log`. a parked
/// joiner serves no http/rpc, so its log is the only progress signal; the
/// webview treats a successful `/v1/status` as the authoritative "ready" and
/// only falls back to this while the surface is still down.
#[tauri::command]
pub fn workspace_phase(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    id: String,
) -> Result<PhaseReport, String> {
    require_main_window(&window)?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let tail = read_tail(&dir.join("daemon.log"), 64 * 1024)?;
    let report = classify(&tail);
    // classify only knows the node's stdout markers, so a node that crashed on
    // boot for a reason it printed no known marker for — a bind conflict, a
    // config parse error, an abort — reads as "starting"/"parked" FOREVER: a
    // cheerful spinner over a corpse. cross-check the process. if the pid WE
    // recorded is gone and neither port is held, the node is dead, not slow;
    // report fatal with the last log line as the best reason we have. (once the
    // node answers /v1/status the webview stops polling this, so a live node
    // never reaches here; a healthy parked joiner keeps its pid + listen port.)
    if report.phase != "fatal"
        && recorded_pid_alive(&dir) == Some(false)
        && !port_listening(ws.ports.listen)
        && !port_listening(ws.ports.http)
    {
        let detail = tail
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "the node exited before it came up".to_string());
        return Ok(PhaseReport {
            phase: "fatal".into(),
            detail: Some(detail),
        });
    }
    Ok(report)
}

/// the path + tail of a workspace's `daemon.log`, so the ui can show the real
/// startup reason and offer an "open log" affordance instead of stranding the
/// developer with a generic timeout. reuses [`read_tail`].
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogTail {
    pub path: String,
    pub tail: String,
}

#[tauri::command]
pub fn workspace_log_tail(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    id: String,
) -> Result<LogTail, String> {
    require_main_window(&window)?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let log_path = workspaces_dir(&app)?.join(&ws.id).join("daemon.log");
    Ok(LogTail {
        path: log_path.display().to_string(),
        tail: read_tail(&log_path, 64 * 1024)?,
    })
}

/// the running node's operational identity for the Node → Logs tab: its pid +
/// liveness, uptime, the resolved node binary, and the workspace's data + log
/// paths. every process field is best-effort — a workspace whose node we did
/// NOT spawn (adopted or already-listening) has no pidfile, so `pid`/`alive`/
/// `uptime_secs` come back `None` and the row renders "—" rather than lying.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFacts {
    /// the detached node's pid, from the pidfile we recorded on spawn.
    pub pid: Option<u32>,
    /// `kill -0` on that pid; `None` when there is no pidfile to check.
    pub alive: Option<bool>,
    /// elapsed running time in seconds (`ps -o etime`); unix-only, best-effort.
    pub uptime_secs: Option<u64>,
    /// the `ducktape-node` binary path the daemon flow resolves to.
    pub binary_path: Option<String>,
    /// this workspace's on-disk directory (`~/.ducktape/workspaces/<id>`).
    pub data_dir: String,
    /// the `daemon.log` inside `data_dir` — the same file `workspace_log_tail`
    /// reads, surfaced here so the row can show the path the viewer follows.
    pub log_path: String,
}

#[tauri::command]
pub fn workspace_runtime_facts(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    id: String,
) -> Result<RuntimeFacts, String> {
    require_main_window(&window)?;
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let pid = read_pid(&dir);
    let log_path = dir.join("daemon.log");
    Ok(RuntimeFacts {
        pid,
        alive: recorded_pid_alive(&dir),
        uptime_secs: pid.and_then(node_uptime_secs),
        binary_path: crate::daemon::node_binary_display(),
        log_path: log_path.display().to_string(),
        data_dir: dir.display().to_string(),
    })
}

/// has the identity-creation mnemonic been confirmed once on this machine?
/// `pub(crate)` so [`crate::user_identity::user_identity_state`] can fold
/// this UX flag into its reported state without reaching into `Registry`'s
/// otherwise-private fields.
pub(crate) fn mnemonic_confirmed(app: &crate::rt::AppHandle) -> Result<bool, String> {
    Ok(load_registry(app)?.mnemonic_confirmed)
}

/// persist the mnemonic-confirmed flag, idempotently (no write if already
/// set). `pub(crate)` so [`crate::user_identity::user_identity_restore`] can
/// set it directly (a restore counts as confirmed — the words were just
/// typed back in), alongside the [`user_identity_confirm_mnemonic`] command
/// below for the create-flow's explicit confirmation step.
pub(crate) fn set_mnemonic_confirmed(app: &crate::rt::AppHandle) -> Result<(), String> {
    let mut reg = load_registry(app)?;
    if !reg.mnemonic_confirmed {
        reg.mnemonic_confirmed = true;
        save_registry(app, &reg)?;
    }
    Ok(())
}

/// mark the identity-creation mnemonic confirmed (shown once, re-entered
/// correctly) — a persisted, UX-only flag with no security weight: it only
/// stops the identity gate from re-showing the confirmation step on future
/// launches. lives here (not `user_identity.rs`) because it is purely a
/// `Registry` mutation, same as every other workspace-registry command.
#[tauri::command]
pub async fn user_identity_confirm_mnemonic(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control.run(move || set_mnemonic_confirmed(&app)).await
}

// ── Phase classification ────────────────────────────────

/// map the node's stable stdout markers to a phase. the log only appends and,
/// within a boot, prints these markers in phase order — so the latest
/// non-regressing marker is the current phase. fatal still wins when it is
/// latest, but late `joining:` retry noise cannot move an already admitted /
/// synced / promoted boot back to the first step.
fn classify(log: &str) -> PhaseReport {
    // (phase, marker substring). the strings are a contract with
    // bin/node/src/main.rs (asserted by bin/node/tests/invite_e2e.rs).
    // "parked" is the phase id the webview already maps; since auto-
    // redemption the underlying markers read "joining:" (no member approval
    // step — the invite redeems itself).
    const MARKERS: &[(&str, &str)] = &[
        ("parked", "joiner mode:"),
        ("parked", "joining:"),
        ("admitted", "admitted at epoch"),
        ("admitted", "resident: standing granted"),
        ("synced", "synced app_hash="),
        ("synced", "resident: pre-synced boundary"),
        ("promoted", "promoted:"),
        ("fatal", "FATAL"),
        // a raw Rust panic on boot ("thread 'main' panicked at …") prints no
        // node marker — catch it so a crashed node stops reading as "starting".
        ("fatal", "panicked at"),
    ];
    let mut latest: Option<(&str, String)> = None;
    for line in log.lines() {
        if let Some((phase, _)) = MARKERS.iter().find(|(_, needle)| line.contains(needle)) {
            if *phase == "parked"
                && matches!(
                    latest.as_ref().map(|(phase, _)| *phase),
                    Some("admitted" | "synced" | "promoted")
                )
            {
                continue;
            }
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
pub(crate) fn read_tail(path: &Path, max: u64) -> Result<String, String> {
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
pub(crate) fn port_listening(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// is the node WE spawned for this workspace still alive? reads the pidfile
/// [`workspace_select`] wrote and signal-0s it. `None` when there is no pidfile
/// (never spawned by us, or an adopted node whose pid we don't own) — the
/// caller must not infer death from an absent pidfile.
fn recorded_pid_alive(dir: &Path) -> Option<bool> {
    let raw = fs::read_to_string(pidfile(dir)).ok()?;
    let pid = raw.trim();
    if pid.is_empty() {
        return None;
    }
    Some(pid_alive(pid))
}

/// the recorded pid as a number, or `None` when there is no (parseable)
/// pidfile — an adopted or never-spawned node. mirrors [`recorded_pid_alive`]'s
/// contract but yields the pid itself, for the runtime-facts row.
fn read_pid(dir: &Path) -> Option<u32> {
    fs::read_to_string(pidfile(dir)).ok()?.trim().parse().ok()
}

/// elapsed running time of `pid` in seconds, via `ps -o etime`. unix only —
/// `ps` is this module's portable-enough process oracle (see [`cmdline_of`]).
/// `None` when the process is gone or the field can't be parsed.
#[cfg(unix)]
fn node_uptime_secs(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "etime="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_etime(String::from_utf8_lossy(&out.stdout).trim())
}

#[cfg(not(unix))]
fn node_uptime_secs(_pid: u32) -> Option<u64> {
    None
}

/// parse a `ps -o etime` field ("[[dd-]hh:]mm:ss") into whole seconds. returns
/// `None` for any shape it doesn't recognize (blank, out-of-range, too many
/// colon groups) rather than guessing.
#[cfg(any(unix, test))]
fn parse_etime(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (days, hms) = match raw.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0u64, raw),
    };
    let mut fields = hms.split(':').rev();
    let secs: u64 = fields.next()?.parse().ok()?;
    let mins: u64 = fields.next()?.parse().ok()?;
    let hours: u64 = match fields.next() {
        Some(h) => h.parse().ok()?,
        None => 0,
    };
    // a valid etime is at most dd-hh:mm:ss — a fourth colon group is malformed.
    if fields.next().is_some() || secs >= 60 || mins >= 60 {
        return None;
    }
    Some(((days * 24 + hours) * 60 + mins) * 60 + secs)
}

/// unix `kill -0 <pid>`: succeeds iff the process exists. shells out to match
/// the rest of this module's teardown path (no libc dep in this crate).
#[cfg(unix)]
fn pid_alive(pid: &str) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn pid_alive(_pid: &str) -> bool {
    true // best-effort on non-unix; the dev box is linux.
}

// ── Stopping a workspace's node for real ────────────────

/// the pidfile [`workspace_select`] records next to `daemon.log` after a spawn,
/// so teardown can address the detached process directly.
fn pidfile(dir: &Path) -> PathBuf {
    dir.join("node.pid")
}

/// the full command line of a live process, or `None` when it is gone (or the
/// platform can't tell). unix only — `ps` is the one portable-enough oracle.
#[cfg(unix)]
fn cmdline_of(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!line.is_empty()).then_some(line)
}

/// pids of LIVE processes that are verifiably THIS workspace's node: the
/// recorded pidfile pid plus a `pgrep -f` sweep for the workspace dir (a
/// wiped-and-recreated registry loses pidfiles; the sweep still finds those
/// zombies). every candidate is verified against its actual command line
/// before it may be killed — a recycled pid must never take an innocent
/// process down.
#[cfg(unix)]
fn workspace_node_pids(dir: &Path) -> Vec<u32> {
    let marker = dir.to_string_lossy().to_string();
    let mut candidates: Vec<u32> = Vec::new();
    if let Some(pid) = fs::read_to_string(pidfile(dir))
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
    {
        candidates.push(pid);
    }
    if let Ok(out) = Command::new("pgrep").args(["-f", &marker]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                candidates.push(pid);
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    let ours = std::process::id();
    candidates
        .into_iter()
        .filter(|pid| *pid != ours)
        .filter(|pid| cmdline_of(*pid).is_some_and(|cmd| cmd.contains(&marker)))
        .collect()
}

/// is `pid` a LIVE process? a zombie counts as dead: the shell never reaps its
/// spawned nodes, so a killed child lingers as `Z` — and `kill -0` keeps
/// succeeding on it, which would burn the whole TERM+KILL grace on an
/// already-dead process. read the state instead of probing signalability.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let Ok(out) = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "stat="])
        .output()
    else {
        return false;
    };
    let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
    out.status.success() && !stat.is_empty() && !stat.starts_with('Z')
}

/// TERM then (after `grace`) KILL `pid`, waiting for it to exit. best-effort —
/// the caller confirms the outcome by port, not by our signals landing.
#[cfg(unix)]
fn kill_pid(pid: u32, grace: std::time::Duration) {
    let alive = process_alive;
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stderr(Stdio::null())
        .status();
    let deadline = std::time::Instant::now() + grace;
    while alive(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if alive(pid) {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stderr(Stdio::null())
            .status();
        let deadline = std::time::Instant::now() + grace;
        while alive(pid) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// stop this workspace's node FOR REAL: ask nicely over http first
/// (/v1/shutdown), then kill every verified process of this workspace, then
/// CONFIRM its ports are released. `Err` when something still holds a port —
/// the caller must NOT delete state a live process would just re-create (the
/// zombie-workspace resurrection this replaces).
fn stop_workspace_node(
    dir: &Path,
    ports: &Ports,
    grace: std::time::Duration,
) -> Result<(), String> {
    // graceful first: the node exits its whole process on this route. a node
    // already down, or a parked joiner serving no http, just fails the connect.
    post_shutdown(ports.http);

    #[cfg(unix)]
    {
        // give the graceful exit a moment before reaching for signals.
        let deadline = std::time::Instant::now() + grace;
        while ports_held(ports) && std::time::Instant::now() < deadline {
            let pids = workspace_node_pids(dir);
            if pids.is_empty() {
                break;
            }
            for pid in pids {
                kill_pid(pid, grace);
            }
        }
        // sweep any survivor once more even if the ports never showed as held
        // (a parked joiner binds only its mesh listener; a fatal-looping node
        // may hold nothing at all between restarts).
        for pid in workspace_node_pids(dir) {
            kill_pid(pid, grace);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir; // no pid oracle here — the port check below still gates.
    }

    // the honest gate: something still answering on this workspace's ports
    // means the node is NOT stopped, whatever the signals claimed.
    let deadline = std::time::Instant::now() + grace;
    while ports_held(ports) {
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "this workspace's node is still running (a listener still holds port {} or {}) \
                 and could not be stopped — aborting so it can't haunt a deleted workspace. \
                 stop the process manually, then try again.",
                ports.listen, ports.http
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = fs::remove_file(pidfile(dir));
    Ok(())
}

/// Stop the active workspace's local node while the desktop app exits. This is
/// best-effort at the UI boundary, but the underlying stop still verifies pids
/// and ports so a half-stopped process is visible in stderr instead of silently
/// surviving.
pub(crate) fn stop_active_workspace_node(app: &crate::rt::AppHandle) -> Result<(), String> {
    stop_active_workspace_node_at(&root(app)?, std::time::Duration::from_secs(2))
}

fn stop_active_workspace_node_at(root: &Path, grace: std::time::Duration) -> Result<(), String> {
    let reg = load_registry_at(&root.join("registry.json"))?;
    let Some(active) = reg.active.as_deref() else {
        return Ok(());
    };
    let Some(ws) = reg.workspaces.iter().find(|ws| ws.id == active) else {
        return Ok(());
    };
    let dir = root.join("workspaces").join(&ws.id);
    stop_workspace_node(&dir, &ws.ports, grace)
}

/// is anything still listening on the ports this workspace owns? the mesh
/// listener is bound in every phase (parked included); http only once serving.
fn ports_held(ports: &Ports) -> bool {
    port_listening(ports.listen) || port_listening(ports.http)
}

/// best-effort "stop this node": POST /v1/shutdown to its http surface over a
/// raw tcp write. the port addresses the node (mirroring the webview's
/// `shutdownNode` in node-bootstrap.ts), and the node exits the whole process
/// on this route. a node already down, or a parked joiner that serves no http,
/// just fails the connect — [`stop_workspace_node`] escalates from here.
fn post_shutdown(http_port: u16) {
    use std::io::Write as _;
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    let addr = SocketAddr::from(([127, 0, 0, 1], http_port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
        return;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let req = format!(
        "POST /v1/shutdown HTTP/1.1\r\nHost: 127.0.0.1:{http_port}\r\n\
         Content-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.write_all(req.as_bytes());
    let _ = stream.flush();
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
                wireguard: None,
                invite: None,
            },
        }];
        assert_eq!(unique_id("My Team", &taken), "my-team-2");
        assert_eq!(unique_id("***", &taken), "workspace");
    }

    #[test]
    fn primary_coordinator_reads_the_env_override() {
        // one test owns the var end-to-end — process env is shared across
        // parallel tests, so set + assert + remove stay in a single test.
        unsafe { std::env::set_var("DUCKTAPE_PRIMARY_COORDINATOR", "127.0.0.1:9999") };
        assert_eq!(primary_coordinator(), "127.0.0.1:9999");
        unsafe { std::env::remove_var("DUCKTAPE_PRIMARY_COORDINATOR") };
        assert_eq!(primary_coordinator(), DEFAULT_PRIMARY_COORDINATOR);
    }

    #[test]
    fn parse_etime_handles_every_field_width() {
        assert_eq!(parse_etime("05"), None); // ss alone is not a valid etime
        assert_eq!(parse_etime("01:05"), Some(65)); // mm:ss
        assert_eq!(parse_etime("02:01:05"), Some(2 * 3600 + 65)); // hh:mm:ss
        assert_eq!(parse_etime("3-02:01:05"), Some(3 * 86_400 + 2 * 3600 + 65)); // dd-hh:mm:ss
        assert_eq!(parse_etime("  01:05  "), Some(65)); // ps pads the field
    }

    #[test]
    fn parse_etime_rejects_malformed() {
        assert_eq!(parse_etime(""), None);
        assert_eq!(parse_etime("nope"), None);
        assert_eq!(parse_etime("99:99"), None); // out-of-range mm/ss
        assert_eq!(parse_etime("1:2:3:4"), None); // too many groups
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
        let log = "[node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] joining: awaiting redemption (epoch 0 has 1 validators)\n";
        let r = classify(log);
        assert_eq!(r.phase, "parked");
        assert!(r.detail.unwrap().contains("awaiting redemption"));
    }

    #[test]
    fn classify_empty_is_starting() {
        assert_eq!(classify("").phase, "starting");
    }

    #[test]
    fn classify_recovers_from_a_stale_fatal() {
        // an old fatal, then a restart that reparks and promotes on the same
        // appended log — the latest line wins, not the scariest one.
        let log = "[node ab] FATAL: still no standing after 900 attempts\n\
                   [node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] joining: awaiting redemption (epoch 0 has 1 validators)\n\
                   [node ab] promoted: validator at epoch 1 boundary 4 — rebooting\n";
        assert_eq!(classify(log).phase, "promoted");
    }

    #[test]
    fn classify_does_not_regress_after_sync_retry_noise() {
        let log = "[node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] admitted at epoch 1 boundary 4 — syncing 16 modules\n\
                   [node ab] synced app_hash=deadbeef\n\
                   [node ab] joining: redemption not landed yet (or the mesh is unreachable) — \
                   the announce keeps retrying and a member node redeems it automatically. \
                   retrying (server error: no finalized boundary to serve yet)\n";
        let report = classify(log);
        assert_eq!(report.phase, "synced");
        assert!(report.detail.as_deref().unwrap_or("").contains("app_hash"));
    }

    #[test]
    fn classify_resident_presync_as_synced() {
        let log = "[node ab] joiner mode: announcing this key with the invite token\n\
                   [node ab] resident: standing granted — following boundaries and serving local reads\n\
                   [node ab] resident: pre-synced boundary 9 app_hash=deadbeef\n\
                   [node ab] joining: redemption not landed yet (or the mesh is unreachable) — \
                   the announce keeps retrying and a member node redeems it automatically. \
                   retrying (server error: no finalized boundary to serve yet)\n";
        let report = classify(log);
        assert_eq!(report.phase, "synced");
        assert!(
            report
                .detail
                .as_deref()
                .unwrap_or("")
                .contains("pre-synced")
        );
    }

    #[test]
    fn classify_flags_a_raw_panic_as_fatal() {
        // a boot panic prints no node marker; the "panicked at" catch-all must
        // still classify it fatal so the join room stops spinning over a corpse.
        let log = "[node ab] joiner mode: parking on the mesh\n\
                   thread 'main' panicked at bin/node/src/main.rs:42:9:\n\
                   called `Result::unwrap()` on an `Err` value: AddrInUse\n";
        let report = classify(log);
        assert_eq!(report.phase, "fatal");
        assert!(
            report.detail.as_deref().unwrap_or("").contains("panicked"),
            "detail: {:?}",
            report.detail
        );
    }

    #[test]
    fn classify_ignores_ordinary_log_lines() {
        // an ordinary info line must not trip any phase — only real markers do.
        let log = "[node ab] listening on 127.0.0.1:8844\n\
                   [node ab] indexed 12 blocks\n";
        assert_eq!(classify(log).phase, "starting");
    }

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

    #[test]
    fn classify_status_allows_a_departed_or_solo_node() {
        // already removed from the set: safe regardless of the reported count.
        assert!(matches!(
            classify_status("in-set=false validators=3"),
            ForgetVerdict::Safe
        ));
        assert!(matches!(
            classify_status("in-set=false validators=1"),
            ForgetVerdict::Safe
        ));
        // a provably solo network: no peer to strand, forgetting just drops it.
        assert!(matches!(
            classify_status("in-set=true validators=1"),
            ForgetVerdict::Safe
        ));
    }

    #[test]
    fn classify_status_confirms_a_still_in_set_validator() {
        // in-set of a set of two-or-more: forgetting would strand the pending
        // removal and halt quorum. must be ConfirmedInSet (never force-overridable)
        // and name the count.
        let verdict = classify_status("in-set=true validators=2");
        match &verdict {
            ForgetVerdict::ConfirmedInSet(msg) => {
                assert!(msg.contains("still a current validator of 2"), "{msg}")
            }
            _ => panic!("expected ConfirmedInSet for a set of two"),
        }
        assert!(matches!(
            classify_status("in-set=true validators=3"),
            ForgetVerdict::ConfirmedInSet(_)
        ));
    }

    #[test]
    fn classify_status_is_unconfirmed_on_an_unparseable_status() {
        // FAIL CLOSED: any line we can't read into BOTH fields is Unconfirmed —
        // an unknown membership can never authorize destroying the identity by
        // itself (only an explicit force may override this uncertainty).
        for line in [
            "",
            "in-set=true",             // count missing
            "validators=2",            // membership missing
            "in-set=true validators=", // count unparseable
            "connection refused",      // not a status line at all
            "in-set=maybe validators=two",
        ] {
            assert!(
                matches!(classify_status(line), ForgetVerdict::Unconfirmed(_)),
                "expected Unconfirmed for {line:?}"
            );
        }
    }

    #[test]
    fn allocated_ports_avoid_reserved() {
        let reserved = [40000u16, 40001, 40002];
        let p = allocate_ports(&reserved).unwrap();
        let got = [
            p.listen,
            p.http,
            p.rpc,
            p.wireguard.expect("wireguard port"),
            p.invite.expect("invite port"),
        ];
        for port in got {
            assert!(!reserved.contains(&port));
        }
        for (idx, port) in got.iter().enumerate() {
            assert!(
                !got[..idx].contains(port),
                "allocated duplicate port {port}"
            );
        }
    }

    #[test]
    fn reserved_ports_includes_reachability_ports() {
        let reg = Registry {
            version: 1,
            active: None,
            workspaces: vec![Workspace {
                id: "team".into(),
                name: "Team".into(),
                chain_id: "chain".into(),
                pubkey: "key".into(),
                founder: true,
                member: true,
                ports: Ports {
                    listen: 40000,
                    http: 40001,
                    rpc: 40002,
                    wireguard: Some(40003),
                    invite: Some(40004),
                },
            }],
            mnemonic_confirmed: false,
        };
        let got = reserved_ports(&reg);
        for port in [40000, 40001, 40002, 40003, 40004] {
            assert!(got.contains(&port));
        }
    }

    // ── stop_workspace_node: the forget teardown must be REAL ──
    //
    // the old best-effort http shutdown left parked/wedged nodes running after
    // a forget; the detached survivor kept its ports and re-created `storage/`
    // under the deleted directory. these pin the repaired contract: verified
    // processes of the workspace die, innocents are never signalled, and a
    // port still held after teardown refuses instead of lying.
    #[cfg(unix)]
    mod stop {
        use super::*;
        use std::time::Duration;

        /// a scratch workspace dir; its path is what pid verification matches.
        fn scratch_dir(tag: &str) -> PathBuf {
            let dir = std::env::temp_dir()
                .join(format!("ducktape-stop-test-{}-{tag}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            dir
        }

        /// a long-lived stand-in node whose command line embeds `dir` (the
        /// trailing `$0` argument keeps `sh` from exec-replacing itself with
        /// `sleep`, which would drop the marker from the command line).
        fn spawn_fake_node(dir: &Path) -> std::process::Child {
            Command::new("sh")
                .arg("-c")
                .arg("sleep 30; : \"$0\"")
                .arg(dir.join("node.toml"))
                .spawn()
                .unwrap()
        }

        /// wait for OUR child to be reaped dead (kill -0 lies for zombies).
        fn died(child: &mut std::process::Child, within: Duration) -> bool {
            let deadline = std::time::Instant::now() + within;
            while std::time::Instant::now() < deadline {
                if child.try_wait().unwrap().is_some() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            false
        }

        /// closed-at-probe-time ports — nothing should be listening on them.
        fn closed_ports() -> Ports {
            let listen = free_port(&[]).unwrap();
            let http = free_port(&[listen]).unwrap();
            Ports {
                listen,
                http,
                rpc: 0,
                wireguard: None,
                invite: None,
            }
        }

        #[test]
        fn kills_the_recorded_pid() {
            let dir = scratch_dir("pidfile");
            let mut child = spawn_fake_node(&dir);
            fs::write(pidfile(&dir), child.id().to_string()).unwrap();

            stop_workspace_node(&dir, &closed_ports(), Duration::from_millis(600)).unwrap();

            assert!(
                died(&mut child, Duration::from_secs(2)),
                "the pidfile-recorded node process must be stopped"
            );
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn sweeps_a_zombie_with_no_pidfile() {
            // a wiped-and-recreated registry loses pidfiles; the command-line
            // sweep must still find and stop the workspace's process.
            let dir = scratch_dir("sweep");
            let mut child = spawn_fake_node(&dir);

            stop_workspace_node(&dir, &closed_ports(), Duration::from_millis(600)).unwrap();

            assert!(
                died(&mut child, Duration::from_secs(2)),
                "a zombie found by command line must be stopped"
            );
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn a_term_killed_zombie_child_does_not_burn_the_grace() {
            // the shell never reaps its spawned nodes, so a TERM-killed child
            // lingers as a zombie — and `kill -0` keeps SUCCEEDING on zombies.
            // liveness must read the process STATE, or every teardown burns
            // the full TERM+KILL grace on an already-dead process (observed
            // live: an 18s forget).
            let dir = scratch_dir("zombie");
            let mut child = spawn_fake_node(&dir);
            fs::write(pidfile(&dir), child.id().to_string()).unwrap();

            let started = std::time::Instant::now();
            stop_workspace_node(&dir, &closed_ports(), Duration::from_secs(3)).unwrap();
            let elapsed = started.elapsed();

            assert!(
                died(&mut child, Duration::from_secs(2)),
                "the node must be stopped"
            );
            assert!(
                elapsed < Duration::from_secs(2),
                "teardown burned the kill grace on a zombie: {elapsed:?}"
            );
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn never_kills_an_unverified_pid() {
            // a recycled pid recorded in a stale pidfile belongs to someone
            // else now — its command line has no trace of this workspace, so
            // it must survive the teardown untouched.
            let dir = scratch_dir("innocent");
            let mut innocent = Command::new("sh")
                .arg("-c")
                .arg("sleep 30")
                .spawn()
                .unwrap();
            fs::write(pidfile(&dir), innocent.id().to_string()).unwrap();

            stop_workspace_node(&dir, &closed_ports(), Duration::from_millis(600)).unwrap();

            assert!(
                innocent.try_wait().unwrap().is_none(),
                "an unverified pid must never be signalled"
            );
            innocent.kill().unwrap();
            innocent.wait().unwrap();
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn refuses_while_a_port_is_still_held() {
            // something unstoppable still listening on the workspace's port
            // means teardown MUST refuse — deleting state under a live process
            // is exactly the zombie-resurrection bug this replaces.
            let dir = scratch_dir("held");
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let held = listener.local_addr().unwrap().port();
            let ports = Ports {
                listen: held,
                http: free_port(&[held]).unwrap(),
                rpc: 0,
                wireguard: None,
                invite: None,
            };

            let err = stop_workspace_node(&dir, &ports, Duration::from_millis(400))
                .expect_err("a held port must refuse the teardown");
            assert!(err.contains("still running"), "{err}");
            drop(listener);
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn stops_the_active_workspace_on_app_exit() {
            let root = scratch_dir("app-exit-root");
            let id = "active";
            let dir = root.join("workspaces").join(id);
            fs::create_dir_all(&dir).unwrap();
            let ports = closed_ports();
            let mut child = spawn_fake_node(&dir);
            fs::write(pidfile(&dir), child.id().to_string()).unwrap();

            let reg = Registry {
                version: 1,
                active: Some(id.into()),
                workspaces: vec![Workspace {
                    id: id.into(),
                    name: "Active".into(),
                    chain_id: "test".into(),
                    pubkey: "pub".into(),
                    founder: true,
                    member: true,
                    ports,
                }],
                mnemonic_confirmed: false,
            };
            save_registry_at(&root.join("registry.json"), &reg).unwrap();

            stop_active_workspace_node_at(&root, Duration::from_millis(600)).unwrap();

            assert!(
                died(&mut child, Duration::from_secs(2)),
                "the app-exit hook must stop the active workspace node"
            );
            assert!(
                !pidfile(&dir).exists(),
                "the active workspace pidfile should be cleared after shutdown"
            );
            let _ = fs::remove_dir_all(&root);
        }
    }
}
