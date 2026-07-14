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
//!
//! layout: the registry model + io live in [`registry`], port allocation in
//! [`ports`], node teardown + process oracles in [`lifecycle`], log-marker
//! phase classification in [`phase`], and the pre-forget membership guard in
//! [`forget`]. every `#[tauri::command]` stays here.

mod forget;
mod lifecycle;
mod phase;
mod ports;
mod registry;

pub(crate) use lifecycle::port_listening;
pub(crate) use phase::read_tail;
pub(crate) use registry::root;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::daemon::{NodeControl, last_line, require_main_window, run_verb};

use forget::{probe_forget, ForgetVerdict};
use lifecycle::{node_uptime_secs, pidfile, read_pid, recorded_pid_alive, stop_workspace_node};
use phase::{classify, parse_join_state, PhaseReport};
use ports::{allocate_ports, reserved_ports};
use registry::{
    load_registry, save_registry, workspaces_dir, write_atomic, Ports, Registry, Selection,
    Workspace,
};

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

/// shared skeleton of the gateway-route mutations: resolve the workspace dir,
/// then run `verb --workspace <dir> [--label <label>] [--port <port>]`.
fn gateway_route_verb_blocking(
    app: crate::rt::AppHandle,
    id: String,
    verb: &str,
    label: Option<String>,
    port: Option<u16>,
) -> Result<(), String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let mut args = vec![
        verb.to_string(),
        "--workspace".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    if let Some(label) = label {
        args.extend(["--label".to_string(), label]);
    }
    if let Some(port) = port {
        args.extend(["--port".to_string(), port.to_string()]);
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_verb(&refs).map(|_| ())
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
        .run(move || gateway_route_verb_blocking(app, id, "gateway-route-bind", label, Some(port)))
        .await
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
        .run(move || gateway_route_verb_blocking(app, id, "gateway-route-unbind", label, None))
        .await
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
    // adopt the staged join identity (the join code the invitee handed the
    // inviter) so `cmd_join` reuses it and its target self-check passes. the
    // staging slot is consumed exactly once.
    let staged = workspaces_dir(&app)?.join(".pending-join").join("identity.key");
    if staged.exists() {
        fs::rename(&staged, dir.join("identity.key"))
            .map_err(|e| format!("adopt staged join identity: {e}"))?;
    }
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
///
/// The short link is the coordinator-hosted `🦆://<name>/<id>` URL; the full
/// blob is the self-contained fallback that works without any coordinator.
/// `short` is `None` when the coordinator was unreachable/refused — the blob
/// still works.
#[derive(Serialize, Clone)]
pub struct InviteForms {
    pub short: Option<String>,
    pub blob: String,
}

#[tauri::command]
pub async fn workspace_invite_blob(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    target: String,
) -> Result<InviteForms, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_invite_blob_blocking(app, id, target))
        .await
}

fn workspace_invite_blob_blocking(
    app: crate::rt::AppHandle,
    id: String,
    target: String,
) -> Result<InviteForms, String> {
    let target = target.trim().to_string();
    if target.is_empty() {
        return Err(
            "paste the invitee's join code — every invite is locked to the key it admits".into(),
        );
    }
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    let cfg_s = cfg.to_string_lossy().to_string();
    match run_verb(&["invite", "--config", &cfg_s, "--target", &target, "--short"]) {
        Ok(out) => {
            // `--short` prints the full blob line, then the short URL as the
            // LAST line. Recover the blob line (`🦆…`, but never the `🦆://` URL).
            let short = last_line(&out);
            let blob = out
                .lines()
                .rev()
                .find(|l| l.trim_start().starts_with('🦆') && !l.contains("://"))
                .unwrap_or_default()
                .trim()
                .to_string();
            Ok(InviteForms {
                short: Some(short),
                blob,
            })
        }
        // coordinator unreachable/refusing: the full blob must still work.
        Err(_) => {
            run_verb(&["invite", "--config", &cfg_s, "--target", &target]).map(|out| InviteForms {
                short: None,
                blob: last_line(&out),
            })
        }
    }
}

/// the invitee's JOIN CODE: pre-mint the identity a future `workspace_join`
/// will adopt, in a one-slot staging dir, and return its pubkey. the code
/// handed to the inviter IS the key the invite locks to. repeat calls reuse the
/// same staged identity (keygen semantics).
#[tauri::command]
pub async fn workspace_join_code(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
) -> Result<String, String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || {
            let staging = workspaces_dir(&app)?.join(".pending-join");
            fs::create_dir_all(&staging).map_err(|e| format!("create {staging:?}: {e}"))?;
            run_verb(&["keygen", "--dir", &staging.to_string_lossy()]).map(|out| last_line(&out))
        })
        .await
}

/// the join requests parked joiners delivered to this member's running node
/// over the lobby channel — the queue the Members view renders with an
/// "Approve" button (approve = the account-signed `admitMember` governance
/// ceremony). raw JSON array from the `join-requests` verb, parsed here so the
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
    // workspace. the p2p LISTEN port is the primary probe (bound in every
    // phase — parked, promoting, validator — while http may lag at boot; a
    // second spawn would collide on exactly this port), but http is probed TOO:
    // a mesh listener bound to a non-loopback interface is invisible to a
    // loopback probe, and missing a live node here is what spawns the
    // address-in-use crash loop (epic QA BUG-1). http is always loopback for an
    // app-managed node, so either port answering means "adopt".
    if port_listening(ws.ports.listen) || port_listening(ws.ports.http) {
        // record the adopted node's pid (verified by exe/cmdline sweep) so the
        // control phase reads a LIVE pid instead of a stale corpse's — a stale
        // pidfile otherwise shows "Start" for a node we are connected to
        // (epic QA BUG-3).
        #[cfg(unix)]
        if let Some(pid) = lifecycle::live_workspace_node_pid(&dir) {
            let _ = fs::write(pidfile(&dir), pid.to_string());
        }
        return finish_selection(&app, &mut reg, ws, http_url);
    }

    let log_path = dir.join("daemon.log");
    // spawn AND verify the node survived. a bind conflict, an unparseable
    // node.toml, or a boot panic dies in milliseconds — and used to return Ok
    // with a dead http_url the webview would poll for 10s before giving a
    // generic timeout. spawn_verified reads the real reason back out of
    // daemon.log instead. http is the readiness signal for a member/founder; a
    // parking joiner never serves it, so "still alive after the grace" carries.
    let mut child =
        crate::daemon::spawn_workspace_node(&node_toml(&dir), &log_path, Some(ws.ports.http))
            .map_err(|failure| {
                format!("the node for \"{}\" exited on start: {failure}", ws.name)
            })?;
    // record the detached pid so teardown can address the process directly —
    // the http shutdown route alone can't reach a parked joiner (no surface).
    // only a STILL-ALIVE child's pid is persisted (epic QA BUG-3).
    crate::daemon::record_pid_if_alive(&mut child, &pidfile(&dir), &ws.name);
    // keep the handle instead of dropping it: it is the only thing that can ever
    // report HOW the node died (and reap it). the supervisor also revives a node
    // that crashes — validator uptime for a user who knows nothing of daemons —
    // unless a deliberate stop raised its intent flag. see `daemon::watch_node_exit`.
    let stopping = crate::daemon::register_supervised(&dir);
    crate::daemon::watch_node_exit(
        child,
        crate::daemon::Supervisor {
            config: node_toml(&dir),
            log: log_path.clone(),
            http_port: ws.ports.http,
            listen_port: ws.ports.listen,
            pidfile: pidfile(&dir),
            workspace: ws.name.clone(),
            stopping,
        },
    );
    // commit `active` ONLY now the node is confirmed up: a select that fails to
    // start the node must not repoint `active` at a workspace the next boot
    // then can't launch (which would strand the app on that dead workspace).
    finish_selection(&app, &mut reg, ws, http_url)
}

/// Atomically apply one sandbox mode and restart the managed node. A failed
/// boot restores the old config and attempts to bring the old node back before
/// returning the error, so a bad apply does not strand the workspace.
#[tauri::command]
pub async fn workspace_sandbox_apply(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
    mode: String,
) -> Result<(), String> {
    require_main_window(&window)?;
    let control = control.inner().clone();
    control
        .run(move || workspace_sandbox_apply_blocking(app, id, mode))
        .await
}

fn workspace_sandbox_apply_blocking(
    app: crate::rt::AppHandle,
    id: String,
    mode: String,
) -> Result<(), String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?.clone();
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let config = node_toml(&dir);
    let original = fs::read_to_string(&config)
        .map_err(|err| format!("read {config:?}: {err}"))?;
    let updated = crate::sandbox::config_with_mode(&original, &mode)?;
    if updated == original {
        return Ok(());
    }

    stop_workspace_node(&dir, &ws.ports, std::time::Duration::from_secs(6))?;
    if let Err(err) = write_atomic(&config, updated.as_bytes()) {
        let recovery = workspace_select_blocking(app, id)
            .map(|_| "the previous node was restarted".to_string())
            .unwrap_or_else(|restart| format!("the previous node also failed to restart: {restart}"));
        return Err(format!("apply sandbox config: {err}; {recovery}"));
    }

    if let Err(apply) = workspace_select_blocking(app.clone(), id.clone()) {
        let recovery = match write_atomic(&config, original.as_bytes()) {
            Ok(()) => workspace_select_blocking(app, id)
                .map(|_| "the previous config was restored and restarted".to_string())
                .unwrap_or_else(|restart| {
                    format!("the previous config was restored but failed to restart: {restart}")
                }),
            Err(restore) => format!("the previous config could not be restored: {restore}"),
        };
        return Err(format!("restart with sandbox mode {mode:?}: {apply}; {recovery}"));
    }
    Ok(())
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

/// read this workspace's onboarding phase. the AUTHORITATIVE source for the
/// positive ladder (`parked → admitted → synced → promoted`) is the node's
/// `join-state` rpc, which derives from committed standing and so is
/// restart-proof: a re-syncing resident reports `admitted`/`synced` even when
/// its fresh `daemon.log` has lost the admission markers — the bug where a
/// joined network read as "admission not claimed". the log + process liveness
/// remain the source for the two edges the rpc cannot report: `starting`
/// (rpc not up yet) and `fatal` (a crashed or gate-rejected node that exited
/// before it could answer).
#[tauri::command]
pub async fn workspace_phase(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, NodeControl>,
    id: String,
) -> Result<PhaseReport, String> {
    require_main_window(&window)?;
    // routed through the node-control actor like every other verb-calling
    // command: the join-state read spawns a subprocess whose rpc could block
    // up to the verb timeout, and that must never run on the command-dispatch
    // thread or wedge a Tauri worker. onboarding contention is low (the join
    // has already returned before phase polling begins).
    let control = control.inner().clone();
    control
        .run(move || workspace_phase_blocking(app, id))
        .await
}

fn workspace_phase_blocking(
    app: crate::rt::AppHandle,
    id: String,
) -> Result<PhaseReport, String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let dir = workspaces_dir(&app)?.join(&ws.id);
    let tail = read_tail(&dir.join("daemon.log"), 64 * 1024)?;
    let log_report = classify(&tail);
    // a dead process is fatal regardless of any stale phase: if the pid WE
    // recorded is gone and neither port is held, the node exited (a bind
    // conflict, a config parse error, a gate-rejected join's exit(1)) — report
    // fatal with the last log line as the best reason. a healthy node keeps its
    // pid + a listen port, so a live node never trips this.
    if log_report.phase != "fatal"
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
    // a FATAL/panic already in the log is terminal — a rejected or crashed join
    // exits before the rpc can answer, so trust the log over a stale rpc read.
    if log_report.phase == "fatal" {
        return Ok(log_report);
    }
    // AUTHORITATIVE positive ladder from the node's rpc (read-only, no lock).
    // unreachable rpc (still booting, or briefly down mid-resync) falls back to
    // the log's best guess.
    let cfg = node_toml(&dir);
    let report = match crate::daemon::run_verb(&["join-state", "--config", &cfg.to_string_lossy()]) {
        Ok(out) => parse_join_state(&out).unwrap_or(log_report),
        Err(_) => log_report,
    };
    // the registry's `member` flag was cached from the descriptor snapshot at
    // join time and never refreshed — so a joiner later PROMOTED to validator
    // read `member=false` forever (couldn't vote / admin). `phase=="promoted"`
    // is the authoritative, restart-proof "this node is a validator" signal, so
    // adopt it here. MONOTONIC-UP: only ever flips false→true on a confirmed
    // promotion; a transient rpc blip yields a lesser phase and simply doesn't
    // write, never demoting a validator on a flicker.
    if report.phase == "promoted" {
        refresh_member_standing(&app, &id)?;
    }
    Ok(report)
}

/// persist that a workspace's node now holds validator standing, idempotently
/// (no write if already a member) and monotonic-up (never clears the flag) —
/// mirrors [`set_mnemonic_confirmed`]. keyed off the authoritative `promoted`
/// join-state phase; the founder path already sets `member` at create.
fn refresh_member_standing(app: &crate::rt::AppHandle, id: &str) -> Result<(), String> {
    let mut reg = load_registry(app)?;
    if registry::mark_member(&mut reg, id) {
        save_registry(app, &reg)?;
    }
    Ok(())
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
}
