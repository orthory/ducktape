//! Workspace creation, activation, and managed-node lifecycle for the iced shell.

#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
use std::io::Read as _;
#[cfg(not(unix))]
use std::io::Write as _;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Backend;
use super::node_control::{self, Supervisor};
use super::private_fs;
use super::workspaces::{self, Registry, Workspace, WorkspacePorts};

const DEFAULT_PRIMARY_COORDINATOR: &str = "p2p.ducktape.byeongsu.dev:3478";
const MAX_WORKSPACE_NAME_BYTES: usize = 128;
const MAX_INVITE_BLOB_BYTES: usize = 256 * 1024;
const LOG_TAIL_BYTES: u64 = 64 * 1024;
pub(super) const STOP_GRACE: Duration = Duration::from_secs(6);
const NODE_STATUS_BYTES: u64 = 64 * 1024;
const NODE_STATUS_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(unix)]
const MAX_PROCESS_ARGV_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceActivation {
    pub id: String,
    pub http_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceNodeStatus {
    pub id: String,
    pub running: bool,
    pub ready: bool,
    pub pid: Option<u32>,
    pub alive: Option<bool>,
    pub uptime_secs: Option<u64>,
    pub binary_path: Option<String>,
    pub data_dir: String,
    pub log_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLogTail {
    pub path: String,
    pub tail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePhaseReport {
    /// `starting | parked | admitted | synced | promoted | fatal`.
    pub phase: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteForms {
    pub short: Option<String>,
    pub blob: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceJoinRequest {
    pub joiner: String,
    pub issuer: String,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
}

impl Backend {
    /// Found a new solo-validator network. The node is started separately by
    /// [`Self::activate_workspace`], matching the existing onboarding flow.
    pub async fn create_workspace(&self, name: String) -> Result<Workspace, String> {
        let root = self.root.clone();
        self.control
            .run(move || create_workspace_blocking(&root, name))
            .await
    }

    #[allow(dead_code)]
    pub async fn found_workspace(&self, name: String) -> Result<Workspace, String> {
        self.create_workspace(name).await
    }

    /// Materialize a remote network from either a full invite blob or a short
    /// coordinator URL. `ducktape-node join` owns both wire formats.
    pub async fn join_workspace(&self, name: String, invite: String) -> Result<Workspace, String> {
        let root = self.root.clone();
        self.control
            .run(move || join_workspace_blocking(&root, name, invite))
            .await
    }

    #[allow(dead_code)]
    pub async fn import_remote_workspace(
        &self,
        name: String,
        invite: String,
    ) -> Result<Workspace, String> {
        self.join_workspace(name, invite).await
    }

    /// Reuse the one staged identity until a join consumes it.
    pub async fn workspace_join_code(&self) -> Result<String, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let workspaces = workspaces_root(&root);
                private_fs::ensure_private_dir(&workspaces)?;
                let staging = workspaces.join(".pending-join");
                private_fs::ensure_private_dir(&staging)?;
                let output = run_owned_verb(&[
                    "keygen".into(),
                    "--dir".into(),
                    staging.to_string_lossy().into_owned(),
                ])?;
                Ok(node_control::last_line(&output))
            })
            .await
    }

    /// Mint a targeted invite using the active workspace's node identity. The
    /// coordinator link is best-effort; the self-contained blob is not.
    pub async fn workspace_invite_blob(
        &self,
        id: String,
        target: String,
    ) -> Result<WorkspaceInviteForms, String> {
        let root = self.root.clone();
        self.control
            .run(move || workspace_invite_blob_blocking(&root, id, target))
            .await
    }

    /// Read verified lobby announcements from this managed workspace's node.
    pub async fn workspace_join_requests(
        &self,
        id: String,
    ) -> Result<Vec<WorkspaceJoinRequest>, String> {
        let root = self.root.clone();
        self.control
            .run(move || workspace_join_requests_blocking(&root, id))
            .await
    }

    /// Make a workspace active after its managed node is known to have started
    /// or an already-running instance has been safely adopted.
    pub async fn activate_workspace(&self, id: String) -> Result<WorkspaceActivation, String> {
        let root = self.root.clone();
        self.control
            .run(move || activate_workspace_blocking(&root, &id))
            .await
    }

    #[allow(dead_code)]
    pub async fn switch_workspace(&self, id: String) -> Result<WorkspaceActivation, String> {
        self.activate_workspace(id).await
    }

    pub async fn start_workspace_node(&self, id: String) -> Result<WorkspaceNodeStatus, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let registry = load_registry(&root)?;
                let workspace = find_workspace(&registry, &id)?.clone();
                start_node(&root, &workspace)?;
                node_status(&root, &workspace)
            })
            .await
    }

    pub async fn stop_workspace_node(&self, id: String) -> Result<WorkspaceNodeStatus, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let registry = load_registry(&root)?;
                let workspace = find_workspace(&registry, &id)?.clone();
                stop_node(
                    &workspace_dir(&root, &workspace.id)?,
                    &workspace.ports,
                    STOP_GRACE,
                )?;
                node_status(&root, &workspace)
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn restart_workspace_node(&self, id: String) -> Result<WorkspaceNodeStatus, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let registry = load_registry(&root)?;
                let workspace = find_workspace(&registry, &id)?.clone();
                stop_node(
                    &workspace_dir(&root, &workspace.id)?,
                    &workspace.ports,
                    STOP_GRACE,
                )?;
                start_node(&root, &workspace)?;
                node_status(&root, &workspace)
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn workspace_node_status(&self, id: String) -> Result<WorkspaceNodeStatus, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let registry = load_registry(&root)?;
                node_status(&root, find_workspace(&registry, &id)?)
            })
            .await
    }

    #[allow(dead_code)]
    pub async fn workspace_node_ready(&self, id: String) -> Result<bool, String> {
        Ok(self.workspace_node_status(id).await?.ready)
    }

    pub async fn forget_workspace(
        &self,
        id: String,
        force: bool,
    ) -> Result<Option<Workspace>, String> {
        let root = self.root.clone();
        self.control
            .run(move || forget_workspace_blocking(&root, &id, force))
            .await
    }

    pub async fn workspace_phase(&self, id: String) -> Result<WorkspacePhaseReport, String> {
        let root = self.root.clone();
        self.control
            .run(move || workspace_phase_blocking(&root, &id))
            .await
    }

    pub async fn workspace_log_tail(&self, id: String) -> Result<WorkspaceLogTail, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let registry = load_registry(&root)?;
                let workspace = find_workspace(&registry, &id)?;
                let path = workspace_dir(&root, &workspace.id)?.join("daemon.log");
                Ok(WorkspaceLogTail {
                    path: path.display().to_string(),
                    tail: workspaces::read_tail(&path, LOG_TAIL_BYTES)?,
                })
            })
            .await
    }
}

fn workspace_invite_blob_blocking(
    root: &Path,
    id: String,
    target: String,
) -> Result<WorkspaceInviteForms, String> {
    let target = target.trim().to_string();
    if target.len() != 64 || !target.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("an invitee join code is exactly 64 hexadecimal characters".into());
    }
    let registry = load_registry(root)?;
    let workspace = find_workspace(&registry, &id)?;
    let config = workspace_dir(root, &workspace.id)?
        .join("node.toml")
        .display()
        .to_string();
    match node_control::run_verb(&[
        "invite", "--config", &config, "--target", &target, "--short",
    ]) {
        Ok(output) => {
            let short = node_control::last_line(&output);
            let blob = output
                .lines()
                .rev()
                .find(|line| line.trim_start().starts_with('🦆') && !line.contains("://"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if blob.is_empty() {
                return Err("invite command did not return a self-contained invite blob".into());
            }
            Ok(WorkspaceInviteForms {
                short: Some(short),
                blob,
            })
        }
        Err(_) => node_control::run_verb(&["invite", "--config", &config, "--target", &target])
            .map(|output| WorkspaceInviteForms {
                short: None,
                blob: node_control::last_line(&output),
            }),
    }
}

fn workspace_join_requests_blocking(
    root: &Path,
    id: String,
) -> Result<Vec<WorkspaceJoinRequest>, String> {
    let registry = load_registry(root)?;
    let workspace = find_workspace(&registry, &id)?;
    let config = workspace_dir(root, &workspace.id)?
        .join("node.toml")
        .display()
        .to_string();
    let output = node_control::run_verb(&["join-requests", "--config", &config])?;
    serde_json::from_str(node_control::last_line(&output).trim())
        .map_err(|error| format!("join-requests output is not json: {error}"))
}

fn create_workspace_blocking(root: &Path, name: String) -> Result<Workspace, String> {
    let name = validated_name(name)?;
    let mut registry = load_registry(root)?;
    let parent = workspaces_root(root);
    private_fs::ensure_private_dir(&parent)?;
    let id = unique_id(&name, &registry.workspaces, &parent);
    let dir = parent.join(&id);
    private_fs::create_private_dir(&dir)?;

    let result = (|| {
        let ports = workspaces::allocate_ports(&reserved_ports(&registry))?;
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
        let dir_text = dir.display().to_string();
        let coordinator = primary_coordinator();
        let args = [
            "init".to_string(),
            "--name".into(),
            name.clone(),
            "--dir".into(),
            dir_text,
            "--listen".into(),
            listen,
            "--http".into(),
            http,
            "--gateway".into(),
            "127.0.0.1:0".into(),
            "--rpc".into(),
            rpc,
            "--primary-coordinator".into(),
            coordinator,
            "--wireguard-listen".into(),
            wireguard,
            "--invite-listen".into(),
            invite,
            "--wireguard-effect".into(),
            "socket".into(),
        ];
        let chain_id = run_owned_verb(&args).map(|output| node_control::last_line(&output))?;
        let key_path = dir.join("identity.key").display().to_string();
        let pubkey = node_control::run_verb(&["keygen", "--out", &key_path])
            .map(|output| node_control::last_line(&output))?;
        harden_workspace_material(&dir)?;

        let workspace = Workspace {
            id: id.clone(),
            name,
            chain_id,
            pubkey,
            founder: true,
            member: true,
            ports,
        };
        registry.workspaces.push(workspace.clone());
        registry.active = Some(id);
        save_registry(root, &registry)?;
        Ok(workspace)
    })();

    match result {
        Ok(workspace) => Ok(workspace),
        Err(error) => Err(with_cleanup(error, cleanup_fresh_workspace(&dir, None))),
    }
}

fn join_workspace_blocking(root: &Path, name: String, invite: String) -> Result<Workspace, String> {
    let name = validated_name(name)?;
    let invite = invite.trim().to_string();
    if invite.is_empty() {
        return Err("paste the invite blob to join".into());
    }
    if invite.len() > MAX_INVITE_BLOB_BYTES {
        return Err(format!(
            "invite blob is too large ({} bytes; limit {MAX_INVITE_BLOB_BYTES})",
            invite.len()
        ));
    }

    let mut registry = load_registry(root)?;
    let parent = workspaces_root(root);
    private_fs::ensure_private_dir(&parent)?;
    let id = unique_id(&name, &registry.workspaces, &parent);
    let dir = parent.join(&id);
    private_fs::create_private_dir(&dir)?;
    let staged = parent.join(".pending-join").join("identity.key");
    let adopted = match fs::symlink_metadata(&staged) {
        Ok(_) => {
            if let Err(error) = private_fs::harden_private_file(&staged).and_then(|()| {
                fs::rename(&staged, dir.join("identity.key"))
                    .map_err(|error| format!("adopt staged join identity: {error}"))
            }) {
                return Err(with_cleanup(error, cleanup_fresh_workspace(&dir, None)));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(with_cleanup(
                format!("inspect staged join identity: {error}"),
                cleanup_fresh_workspace(&dir, None),
            ));
        }
    };

    let result = (|| {
        let ports = workspaces::allocate_ports(&reserved_ports(&registry))?;
        let listen = format!("[::]:{}", ports.listen);
        let http = format!("127.0.0.1:{}", ports.http);
        let rpc = format!("127.0.0.1:{}", ports.rpc);
        let wireguard = format!(
            "0.0.0.0:{}",
            ports
                .wireguard
                .ok_or("workspace allocator did not assign a wireguard port")?
        );
        let invite_port = format!(
            "0.0.0.0:{}",
            ports
                .invite
                .ok_or("workspace allocator did not assign an invite port")?
        );
        let args = [
            "join".to_string(),
            invite,
            "--dir".into(),
            dir.display().to_string(),
            "--listen".into(),
            listen,
            "--http".into(),
            http,
            "--gateway".into(),
            "127.0.0.1:0".into(),
            "--rpc".into(),
            rpc,
            "--wireguard-listen".into(),
            wireguard,
            "--invite-listen".into(),
            invite_port,
            "--wireguard-effect".into(),
            "socket".into(),
        ];
        let pubkey = run_owned_verb(&args).map(|output| node_control::last_line(&output))?;
        harden_workspace_material(&dir)?;
        let (chain_id, validators) = read_descriptor(&dir)?;
        if let Some(existing) = registry
            .workspaces
            .iter()
            .find(|workspace| workspace.chain_id == chain_id)
        {
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
        registry.workspaces.push(workspace.clone());
        registry.active = Some(id);
        save_registry(root, &registry)?;
        Ok(workspace)
    })();

    match result {
        Ok(workspace) => Ok(workspace),
        Err(error) => {
            let restore = adopted.then_some(staged.as_path());
            Err(with_cleanup(error, cleanup_fresh_workspace(&dir, restore)))
        }
    }
}

fn activate_workspace_blocking(root: &Path, id: &str) -> Result<WorkspaceActivation, String> {
    let mut registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?.clone();
    start_node(root, &workspace)?;
    if registry.active.as_deref() != Some(id) {
        registry.active = Some(id.to_string());
        save_registry(root, &registry)?;
    }
    Ok(WorkspaceActivation {
        id: workspace.id,
        http_url: format!("http://127.0.0.1:{}", workspace.ports.http),
    })
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExistingNodeAction {
    Spawn,
    Adopt(u32),
}

#[cfg(unix)]
fn existing_node_action(
    verified_pids: &[u32],
    listen: bool,
    http: bool,
) -> Result<ExistingNodeAction, &'static str> {
    match (verified_pids, listen, http) {
        ([], false, false) => Ok(ExistingNodeAction::Spawn),
        ([pid], true, true) => Ok(ExistingNodeAction::Adopt(*pid)),
        ([], _, _) => Err("configured ports are held by an unverified process"),
        ([_], _, _) => Err("the verified node is not serving both configured ports"),
        _ => Err("more than one verified node uses this workspace config"),
    }
}

pub(super) fn start_node(root: &Path, workspace: &Workspace) -> Result<(), String> {
    let dir = workspace_dir(root, &workspace.id)?;
    harden_workspace_material(&dir)?;
    let listen = workspaces::port_listening(workspace.ports.listen);
    let http = workspaces::port_listening(workspace.ports.http);

    #[cfg(unix)]
    {
        let pids = workspace_node_pids(&dir)?;
        match existing_node_action(&pids, listen, http) {
            Ok(ExistingNodeAction::Adopt(pid)) => {
                write_atomic(&pidfile(&dir), pid.to_string().as_bytes())?;
                return wait_node_ready(root, workspace);
            }
            Ok(ExistingNodeAction::Spawn) => {}
            Err(reason) => return Err(format!("refusing to adopt {:?}: {reason}", workspace.name)),
        }
    }
    #[cfg(not(unix))]
    if listen || http {
        return Err(format!(
            "refusing to adopt listeners that this platform cannot verify on workspace ports {} and {}",
            workspace.ports.listen, workspace.ports.http
        ));
    }

    let config = fs::canonicalize(dir.join("node.toml"))
        .map_err(|error| format!("resolve workspace node config: {error}"))?;
    let log = dir.join("daemon.log");
    let mut child = node_control::spawn_workspace_node(&config, &log, Some(workspace.ports.http))
        .map_err(|error| {
        format!("the node for {:?} exited on start: {error}", workspace.name)
    })?;
    let pid_path = pidfile(&dir);
    node_control::record_pid_if_alive(&mut child, &pid_path, &workspace.name);
    let stopping = node_control::register_supervised(&dir);
    node_control::watch_node_exit(
        child,
        Supervisor {
            config,
            log,
            http_port: workspace.ports.http,
            listen_port: workspace.ports.listen,
            pidfile: pid_path,
            workspace: workspace.name.clone(),
            stopping,
        },
    );
    if let Err(error) = wait_node_ready(root, workspace) {
        let cleanup = stop_node(&dir, &workspace.ports, STOP_GRACE);
        return Err(with_cleanup(error, cleanup));
    }
    Ok(())
}

pub(super) fn wait_node_ready(root: &Path, workspace: &Workspace) -> Result<(), String> {
    let deadline = std::time::Instant::now() + STOP_GRACE;
    loop {
        let status = node_status(root, workspace)?;
        if status.ready {
            return Ok(());
        }
        if status.alive == Some(false) {
            return Err(format!(
                "the node for {:?} exited during startup",
                workspace.name
            ));
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "the node for {:?} did not become ready before the startup deadline",
                workspace.name
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn forget_workspace_blocking(
    root: &Path,
    id: &str,
    force: bool,
) -> Result<Option<Workspace>, String> {
    let mut registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?.clone();
    let dir = workspace_dir(root, &workspace.id)?;

    match probe_forget(&dir) {
        ForgetVerdict::Safe => {}
        ForgetVerdict::Unconfirmed(_) if force => {}
        ForgetVerdict::Unconfirmed(message) | ForgetVerdict::ConfirmedInSet(message) => {
            return Err(message);
        }
    }

    stop_node(&dir, &workspace.ports, STOP_GRACE)?;
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(
            target: "ducktape::shell",
            workspace = %workspace.id,
            reason = "workspace_directory_remove_failed",
            %error,
            "workspace was forgotten but its stopped data directory could not be removed"
        ),
    }
    registry
        .workspaces
        .retain(|candidate| candidate.id != workspace.id);
    if registry.active.as_deref() == Some(&workspace.id) {
        registry.active = registry
            .workspaces
            .first()
            .map(|candidate| candidate.id.clone());
    }
    save_registry(root, &registry)?;
    Ok(registry.active.as_ref().and_then(|active| {
        registry
            .workspaces
            .iter()
            .find(|workspace| &workspace.id == active)
            .cloned()
    }))
}

fn workspace_phase_blocking(root: &Path, id: &str) -> Result<WorkspacePhaseReport, String> {
    let mut registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?.clone();
    let dir = workspace_dir(root, &workspace.id)?;
    let tail = workspaces::read_tail(&dir.join("daemon.log"), LOG_TAIL_BYTES)?;
    let log_report = classify_phase(&tail);

    if log_report.phase != "fatal"
        && recorded_pid_alive(&dir) == Some(false)
        && !workspaces::port_listening(workspace.ports.listen)
        && !workspaces::port_listening(workspace.ports.http)
    {
        let detail = tail
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "the node exited before it came up".to_string());
        return Ok(WorkspacePhaseReport {
            phase: "fatal".into(),
            detail: Some(detail),
        });
    }
    if log_report.phase == "fatal" {
        return Ok(log_report);
    }

    let config = dir.join("node.toml").display().to_string();
    let report = match node_control::run_verb(&["join-state", "--config", &config]) {
        Ok(output) => parse_join_state(&output).unwrap_or(log_report),
        Err(_) => log_report,
    };
    if report.phase == "promoted"
        && let Some(workspace) = registry
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
        && !workspace.member
    {
        workspace.member = true;
        save_registry(root, &registry)?;
    }
    Ok(report)
}

fn node_status(root: &Path, workspace: &Workspace) -> Result<WorkspaceNodeStatus, String> {
    let dir = workspace_dir(root, &workspace.id)?;
    #[cfg(unix)]
    let pids = workspace_node_pids(&dir)?;
    #[cfg(unix)]
    if pids.len() > 1 {
        return Err(format!(
            "more than one verified node is using the config for {:?}",
            workspace.name
        ));
    }
    #[cfg(unix)]
    let pid = pids.first().copied();
    #[cfg(not(unix))]
    let pid = read_pid(&dir);
    let alive = pid.map(|_| true).or_else(|| read_pid(&dir).map(|_| false));
    let listen = workspaces::port_listening(workspace.ports.listen);
    let http = workspaces::port_listening(workspace.ports.http);
    let ready = alive == Some(true)
        && listen
        && http
        && node_identity_ready(workspace.ports.http, &workspace.pubkey).is_ok();
    let log = dir.join("daemon.log");
    Ok(WorkspaceNodeStatus {
        id: workspace.id.clone(),
        running: alive == Some(true),
        ready,
        pid,
        alive,
        uptime_secs: pid.and_then(node_uptime_secs),
        binary_path: node_control::node_binary_display(),
        data_dir: dir.display().to_string(),
        log_path: log.display().to_string(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedNodeStatus {
    version: String,
    app_hash: String,
    public_key: Option<String>,
}

fn node_identity_ready(http_port: u16, expected_public_key: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .connect_timeout(NODE_STATUS_TIMEOUT)
        .timeout(NODE_STATUS_TIMEOUT)
        .build()
        .map_err(|_| "could not initialize the node identity probe".to_string())?;
    let response = client
        .get(format!("http://127.0.0.1:{http_port}/v1/status"))
        .send()
        .map_err(|_| "the workspace HTTP listener did not answer its identity probe".to_string())?;
    if !response.status().is_success() {
        return Err("the workspace HTTP listener refused its identity probe".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > NODE_STATUS_BYTES)
    {
        return Err("the workspace HTTP listener returned an oversized status".into());
    }
    let mut bytes = Vec::new();
    response
        .take(NODE_STATUS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "the workspace HTTP listener returned an unreadable status".to_string())?;
    if bytes.len() as u64 > NODE_STATUS_BYTES {
        return Err("the workspace HTTP listener returned an oversized status".into());
    }
    let status: ManagedNodeStatus = serde_json::from_slice(&bytes)
        .map_err(|_| "the workspace HTTP listener returned an invalid status".to_string())?;
    validate_node_identity(&status, expected_public_key)
}

fn validate_node_identity(
    status: &ManagedNodeStatus,
    expected_public_key: &str,
) -> Result<(), String> {
    if status.version.is_empty() || status.app_hash.is_empty() {
        return Err("the workspace HTTP listener is not a Ducktape node".into());
    }
    let Some(public_key) = status.public_key.as_deref() else {
        return Err("the workspace HTTP listener did not report its node identity".into());
    };
    if !public_key.eq_ignore_ascii_case(expected_public_key) {
        return Err("the workspace HTTP listener reports a different node identity".into());
    }
    Ok(())
}

fn validated_name(name: String) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("a workspace needs a name".into());
    }
    if name.len() > MAX_WORKSPACE_NAME_BYTES {
        return Err(format!(
            "workspace name is too long ({} bytes; limit {MAX_WORKSPACE_NAME_BYTES})",
            name.len()
        ));
    }
    Ok(name)
}

fn unique_id(name: &str, taken: &[Workspace], parent: &Path) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-').replace("--", "-");
    let base = if trimmed.is_empty() {
        "workspace".to_string()
    } else {
        trimmed
    };
    let available = |candidate: &str| {
        !taken.iter().any(|workspace| workspace.id == candidate) && !parent.join(candidate).exists()
    };
    if available(&base) {
        return base;
    }
    (2..)
        .map(|number| format!("{base}-{number}"))
        .find(|candidate| available(candidate))
        .expect("the natural numbers are not exhausted")
}

fn read_descriptor(dir: &Path) -> Result<(String, Vec<String>), String> {
    #[derive(Deserialize)]
    struct Descriptor {
        chain_id: String,
        #[serde(default)]
        validators: Vec<String>,
    }

    let path = dir.join("network.toml");
    let text = fs::read_to_string(&path).map_err(|error| format!("read {path:?}: {error}"))?;
    let descriptor: Descriptor =
        toml::from_str(&text).map_err(|error| format!("parse {path:?}: {error}"))?;
    Ok((descriptor.chain_id, descriptor.validators))
}

fn run_owned_verb(args: &[String]) -> Result<String, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    node_control::run_verb(&args)
}

fn primary_coordinator() -> String {
    std::env::var("DUCKTAPE_PRIMARY_COORDINATOR")
        .unwrap_or_else(|_| DEFAULT_PRIMARY_COORDINATOR.into())
}

fn workspaces_root(root: &Path) -> PathBuf {
    root.join("workspaces")
}

fn harden_workspace_material(dir: &Path) -> Result<(), String> {
    private_fs::harden_private_dir(dir)?;
    for name in ["node.toml", "network.toml", "identity.key"] {
        private_fs::harden_private_file(&dir.join(name))?;
    }
    Ok(())
}

pub(super) fn workspace_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !safe_workspace_id(id) {
        return Err(format!("unsafe workspace id {id:?}"));
    }
    let parent = workspaces_root(root);
    private_fs::harden_private_dir(&parent)?;
    let dir = parent.join(id);
    harden_workspace_material(&dir)?;
    Ok(dir)
}

fn safe_workspace_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

pub(super) fn find_workspace<'a>(
    registry: &'a Registry,
    id: &str,
) -> Result<&'a Workspace, String> {
    registry
        .workspaces
        .iter()
        .find(|workspace| workspace.id == id)
        .ok_or_else(|| format!("no workspace {id:?}"))
}

fn reserved_ports(registry: &Registry) -> Vec<u16> {
    registry
        .workspaces
        .iter()
        .flat_map(|workspace| {
            [
                Some(workspace.ports.listen),
                Some(workspace.ports.http),
                Some(workspace.ports.rpc),
                workspace.ports.wireguard,
                workspace.ports.invite,
            ]
        })
        .flatten()
        .collect()
}

fn registry_path(root: &Path) -> PathBuf {
    root.join("registry.json")
}

pub(super) fn load_registry(root: &Path) -> Result<Registry, String> {
    workspaces::load_registry_at(&registry_path(root))
}

fn save_registry(root: &Path, registry: &Registry) -> Result<(), String> {
    private_fs::ensure_private_dir(root)?;
    let bytes = serde_json::to_vec_pretty(registry).map_err(|error| error.to_string())?;
    write_atomic(&registry_path(root), &bytes)
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    private_fs::write_atomic(path, bytes)
}

fn cleanup_fresh_workspace(dir: &Path, restore_identity_to: Option<&Path>) -> Result<(), String> {
    if let Some(staged) = restore_identity_to {
        if let Some(parent) = staged.parent() {
            private_fs::ensure_private_dir(parent)?;
        }
        let identity = dir.join("identity.key");
        match fs::symlink_metadata(&identity) {
            Ok(_) => {
                private_fs::harden_private_file(&identity)?;
                fs::rename(&identity, staged)
                    .map_err(|error| format!("restore staged join identity: {error}"))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect fresh workspace identity: {error}")),
        }
    }
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("remove fresh workspace {}: {error}", dir.display())),
    }
}

fn with_cleanup(error: String, cleanup: Result<(), String>) -> String {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => format!("{error}; cleanup also failed: {cleanup}"),
    }
}

fn pidfile(dir: &Path) -> PathBuf {
    dir.join("node.pid")
}

fn read_pid(dir: &Path) -> Option<u32> {
    fs::read_to_string(pidfile(dir)).ok()?.trim().parse().ok()
}

fn recorded_pid_alive(dir: &Path) -> Option<bool> {
    let pid = read_pid(dir)?;
    #[cfg(unix)]
    return Some(verified_workspace_process_alive(pid, dir));
    #[cfg(not(unix))]
    return Some(pid != 0);
}

#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
struct ManagedProcessSpec {
    executable: PathBuf,
    config: PathBuf,
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessSnapshot {
    executable: PathBuf,
    argv: Vec<OsString>,
}

#[cfg(unix)]
fn managed_process_spec(dir: &Path) -> Result<ManagedProcessSpec, String> {
    let executable = node_control::node_binary_path()
        .ok_or_else(|| "the trusted Ducktape node sidecar could not be resolved".to_string())?;
    let config_path = dir.join("node.toml");
    private_fs::harden_private_file(&config_path)?;
    let config = fs::canonicalize(config_path)
        .map_err(|error| format!("resolve workspace node config: {error}"))?;
    Ok(ManagedProcessSpec { executable, config })
}

#[cfg(unix)]
fn managed_argv_matches(argv: &[OsString], config: &Path) -> bool {
    argv.len() == 3 && argv[1] == "--config" && Path::new(&argv[2]) == config
}

#[cfg(unix)]
fn process_snapshot_matches(snapshot: &ProcessSnapshot, spec: &ManagedProcessSpec) -> bool {
    snapshot.executable == spec.executable && managed_argv_matches(&snapshot.argv, &spec.config)
}

#[cfg(target_os = "linux")]
fn process_snapshot(pid: u32) -> Option<ProcessSnapshot> {
    use std::os::unix::ffi::OsStringExt as _;

    let process = PathBuf::from(format!("/proc/{pid}"));
    let executable = fs::canonicalize(process.join("exe")).ok()?;
    let mut bytes = Vec::new();
    fs::File::open(process.join("cmdline"))
        .ok()?
        .take(MAX_PROCESS_ARGV_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_PROCESS_ARGV_BYTES {
        return None;
    }
    let mut fields: Vec<&[u8]> = bytes.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    let argv = fields
        .into_iter()
        .map(|field| OsString::from_vec(field.to_vec()))
        .collect();
    Some(ProcessSnapshot { executable, argv })
}

#[cfg(target_os = "macos")]
fn process_snapshot(pid: u32) -> Option<ProcessSnapshot> {
    use std::os::unix::ffi::OsStringExt as _;

    const PROC_PIDPATHINFO_MAXSIZE: usize = 4 * 1024;
    const CTL_KERN: i32 = 1;
    const KERN_PROCARGS2: i32 = 49;

    let mut executable_bytes = vec![0u8; PROC_PIDPATHINFO_MAXSIZE];
    // SAFETY: proc_pidpath writes at most the provided buffer size and retains no pointer.
    let executable_len = unsafe {
        libc::proc_pidpath(
            i32::try_from(pid).ok()?,
            executable_bytes.as_mut_ptr().cast(),
            PROC_PIDPATHINFO_MAXSIZE as u32,
        )
    };
    if executable_len <= 0 {
        return None;
    }
    executable_bytes.truncate(executable_len as usize);
    let executable = fs::canonicalize(PathBuf::from(OsString::from_vec(executable_bytes))).ok()?;

    let mut mib = [CTL_KERN, KERN_PROCARGS2, i32::try_from(pid).ok()?];
    let mut length = 0usize;
    // SAFETY: the first call only asks the kernel for the required buffer length.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length < std::mem::size_of::<i32>()
        || length as u64 > MAX_PROCESS_ARGV_BYTES
    {
        return None;
    }
    let mut bytes = vec![0u8; length];
    // SAFETY: sysctl writes at most `length` bytes into the owned buffer.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length > bytes.len()
    {
        return None;
    }
    bytes.truncate(length);
    let argc = i32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?);
    if argc <= 0 || argc > 4_096 {
        return None;
    }
    let mut offset = std::mem::size_of::<i32>();
    offset += bytes.get(offset..)?.iter().position(|byte| *byte == 0)? + 1;
    while bytes.get(offset) == Some(&0) {
        offset += 1;
    }
    let mut argv = Vec::with_capacity(argc as usize);
    for _ in 0..argc {
        let end = offset + bytes.get(offset..)?.iter().position(|byte| *byte == 0)?;
        argv.push(OsString::from_vec(bytes[offset..end].to_vec()));
        offset = end + 1;
    }
    Some(ProcessSnapshot { executable, argv })
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_snapshot(_pid: u32) -> Option<ProcessSnapshot> {
    None
}

#[cfg(unix)]
fn verified_workspace_process_alive(pid: u32, dir: &Path) -> bool {
    managed_process_spec(dir).is_ok_and(|spec| {
        process_snapshot(pid).is_some_and(|snapshot| process_snapshot_matches(&snapshot, &spec))
    })
}

#[cfg(unix)]
fn process_ids() -> Result<Vec<u32>, String> {
    #[cfg(target_os = "linux")]
    {
        let entries = fs::read_dir("/proc").map_err(|error| format!("scan /proc: {error}"))?;
        Ok(entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_string_lossy().parse().ok())
            .collect())
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/bin/ps")
            .args(["-axo", "pid="])
            .output()
            .map_err(|error| format!("list processes: {error}"))?;
        if !output.status.success() {
            return Err("process listing failed".into());
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .filter_map(|pid| pid.parse().ok())
            .collect())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Err("managed-node process verification is unsupported on this Unix platform".into())
}

#[cfg(unix)]
fn workspace_node_pids(dir: &Path) -> Result<Vec<u32>, String> {
    let spec = managed_process_spec(dir)?;
    let mut candidates = read_pid(dir).into_iter().collect::<Vec<_>>();
    candidates.extend(process_ids()?);
    candidates.sort_unstable();
    candidates.dedup();
    let current = std::process::id();
    Ok(candidates
        .into_iter()
        .filter(|pid| {
            *pid != current
                && process_snapshot(*pid)
                    .is_some_and(|snapshot| process_snapshot_matches(&snapshot, &spec))
        })
        .collect())
}

#[cfg(unix)]
fn signal_verified_pid(pid: u32, dir: &Path, signal: libc::c_int) -> Result<bool, String> {
    let spec = managed_process_spec(dir)?;
    if !process_snapshot(pid).is_some_and(|snapshot| process_snapshot_matches(&snapshot, &spec)) {
        return Ok(false);
    }
    // SAFETY: the PID was re-snapshotted immediately above; kill retains no pointer.
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        Ok(true)
    } else {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(false)
        } else {
            Err(format!("signal verified workspace node {pid}: {error}"))
        }
    }
}

#[cfg(unix)]
fn kill_verified_pid(pid: u32, dir: &Path, grace: Duration) -> Result<(), String> {
    if !signal_verified_pid(pid, dir, libc::SIGTERM)? {
        return Ok(());
    }
    let deadline = std::time::Instant::now() + grace;
    while verified_workspace_process_alive(pid, dir) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if verified_workspace_process_alive(pid, dir) {
        signal_verified_pid(pid, dir, libc::SIGKILL)?;
        let deadline = std::time::Instant::now() + grace;
        while verified_workspace_process_alive(pid, dir) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn node_uptime_secs(pid: u32) -> Option<u64> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "etime="])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| parse_etime(String::from_utf8_lossy(&output.stdout).trim()))
        .flatten()
}

#[cfg(not(unix))]
fn node_uptime_secs(_pid: u32) -> Option<u64> {
    None
}

fn parse_etime(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (days, hms) = match raw.split_once('-') {
        Some((days, rest)) => (days.parse::<u64>().ok()?, rest),
        None => (0, raw),
    };
    let mut fields = hms.split(':').rev();
    let seconds = fields.next()?.parse::<u64>().ok()?;
    let minutes = fields.next()?.parse::<u64>().ok()?;
    let hours = match fields.next() {
        Some(hours) => hours.parse::<u64>().ok()?,
        None => 0,
    };
    if fields.next().is_some() || seconds >= 60 || minutes >= 60 {
        return None;
    }
    Some(((days * 24 + hours) * 60 + minutes) * 60 + seconds)
}

pub(super) fn stop_node(dir: &Path, ports: &WorkspacePorts, grace: Duration) -> Result<(), String> {
    node_control::mark_stopping(dir);

    #[cfg(unix)]
    {
        let pids = workspace_node_pids(dir)?;
        if pids.is_empty() && ports_held(ports) {
            return Err(format!(
                "refusing to signal an unverified listener on workspace port {} or {}",
                ports.listen, ports.http
            ));
        }
        for pid in pids {
            kill_verified_pid(pid, dir, grace)?;
        }
    }
    #[cfg(not(unix))]
    post_shutdown(ports.http);

    let deadline = std::time::Instant::now() + grace;
    while ports_held(ports) {
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "this workspace's node is still running (a listener still holds port {} or {}) \
                 and could not be stopped — stop the process manually, then try again",
                ports.listen, ports.http
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    #[cfg(unix)]
    if !workspace_node_pids(dir)?.is_empty() {
        return Err("this workspace's verified node process is still running — stop it manually, then try again".into());
    }
    let _ = fs::remove_file(pidfile(dir));
    node_control::clear_supervised(dir);
    Ok(())
}

fn ports_held(ports: &WorkspacePorts) -> bool {
    workspaces::port_listening(ports.listen) || workspaces::port_listening(ports.http)
}

#[cfg(not(unix))]
fn post_shutdown(http_port: u16) {
    use std::net::{SocketAddr, TcpStream};

    let address = SocketAddr::from(([127, 0, 0, 1], http_port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let request = format!(
        "POST /v1/admin/shutdown HTTP/1.1\r\nHost: 127.0.0.1:{http_port}\r\n\
         Content-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.write_all(request.as_bytes());
    let _ = stream.flush();
}

enum ForgetVerdict {
    Safe,
    ConfirmedInSet(String),
    Unconfirmed(String),
}

fn probe_forget(dir: &Path) -> ForgetVerdict {
    let config = dir.join("node.toml").display().to_string();
    match node_control::run_verb(&["member-status", "--config", &config]) {
        Ok(output) => classify_forget_status(&node_control::last_line(&output)),
        Err(error) => ForgetVerdict::Unconfirmed(format!(
            "start the node and finish leaving — we can't confirm this workspace has left the \
             validator set ({error}), and destroying its identity now could permanently halt the \
             network"
        )),
    }
}

fn classify_forget_status(status: &str) -> ForgetVerdict {
    let in_set = if status.contains("in-set=true") {
        Some(true)
    } else if status.contains("in-set=false") {
        Some(false)
    } else {
        None
    };
    let validators = status
        .split_whitespace()
        .find_map(|token| token.strip_prefix("validators="))
        .and_then(|count| count.parse::<usize>().ok());
    match (in_set, validators) {
        (Some(false), _) | (Some(true), Some(1)) => ForgetVerdict::Safe,
        (Some(true), Some(count)) => ForgetVerdict::ConfirmedInSet(format!(
            "this node is still a current validator of {count} — request to leave first, then \
             wait until the other members approve before forgetting this workspace"
        )),
        _ => ForgetVerdict::Unconfirmed(
            "couldn't confirm this workspace has left the validator set; start it and finish \
             leaving before forgetting it"
                .into(),
        ),
    }
}

fn classify_phase(log: &str) -> WorkspacePhaseReport {
    const MARKERS: &[(&str, &str)] = &[
        ("parked", "joiner mode:"),
        ("parked", "joining:"),
        ("admitted", "ADMITTED at height"),
        ("admitted", "admitted at epoch"),
        ("admitted", "resident: standing granted"),
        ("synced", "synced app_hash="),
        ("synced", "resident: pre-synced boundary"),
        ("promoted", "promoted:"),
        ("fatal", "FATAL"),
        ("fatal", "panicked at"),
    ];
    let mut latest: Option<(&str, String)> = None;
    for line in log.lines() {
        if let Some((phase, _)) = MARKERS.iter().find(|(_, marker)| line.contains(marker)) {
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
                .map(|(_, detail)| detail)
                .unwrap_or(line)
                .trim()
                .to_string();
            latest = Some((phase, detail));
        }
    }
    match latest {
        Some((phase, detail)) => WorkspacePhaseReport {
            phase: phase.into(),
            detail: Some(detail),
        },
        None => WorkspacePhaseReport {
            phase: "starting".into(),
            detail: None,
        },
    }
}

fn parse_join_state(output: &str) -> Option<WorkspacePhaseReport> {
    let value: serde_json::Value = serde_json::from_str(output.trim()).ok()?;
    let phase = value.get("phase")?.as_str()?;
    if !matches!(phase, "parked" | "admitted" | "synced" | "promoted") {
        return None;
    }
    let detail = value
        .get("detail")
        .and_then(|detail| detail.as_str())
        .filter(|detail| !detail.is_empty())
        .map(
            |detail| match value.get("height").and_then(|height| height.as_u64()) {
                Some(height) => format!("{detail} (height {height})"),
                None => detail.to_string(),
            },
        );
    Some(WorkspacePhaseReport {
        phase: phase.into(),
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ducktape-iced-workspace-service-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn slug_is_safe_unique_and_does_not_reuse_an_orphan_directory() {
        let parent = scratch("slug");
        fs::create_dir(parent.join("my-team")).unwrap();
        assert_eq!(unique_id("My Team", &[], &parent), "my-team-2");
        assert!(safe_workspace_id("my-team-2"));
        for unsafe_id in ["../team", "team/name", "-team", "TEAM"] {
            assert!(!safe_workspace_id(unsafe_id), "{unsafe_id}");
        }
        fs::remove_dir_all(parent).ok();
    }

    #[test]
    fn registry_save_roundtrips_the_existing_wire_contract_atomically() {
        let root = scratch("registry");
        let registry = Registry {
            version: 1,
            active: Some("team".into()),
            workspaces: vec![Workspace {
                id: "team".into(),
                name: "Team".into(),
                chain_id: "ducktape#team".into(),
                pubkey: "11".repeat(32),
                founder: true,
                member: true,
                ports: WorkspacePorts {
                    listen: 31_000,
                    http: 31_001,
                    rpc: 31_002,
                    wireguard: Some(31_003),
                    invite: Some(31_004),
                },
            }],
            mnemonic_confirmed: true,
        };
        save_registry(&root, &registry).unwrap();
        let text = fs::read_to_string(registry_path(&root)).unwrap();
        assert!(text.contains("\"chainId\""));
        assert!(text.contains("\"mnemonicConfirmed\""));
        assert!(!root.join("registry.json.tmp").exists());
        let loaded = load_registry(&root).unwrap();
        assert_eq!(loaded.active.as_deref(), Some("team"));
        assert!(loaded.mnemonic_confirmed);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn failed_join_cleanup_restores_the_invite_bound_staged_identity() {
        let root = scratch("restore-identity");
        let dir = root.join("workspaces/team");
        let staged = root.join("workspaces/.pending-join/identity.key");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("identity.key"), b"invite-bound-key").unwrap();
        cleanup_fresh_workspace(&dir, Some(&staged)).unwrap();
        assert_eq!(fs::read(&staged).unwrap(), b"invite-bound-key");
        assert!(!dir.exists());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn phase_is_monotonic_past_join_retry_noise_and_fatal_is_visible() {
        let log = "[node] joining: awaiting redemption\n\
                   [node] admitted at epoch 1 boundary 4\n\
                   [node] synced app_hash=deadbeef\n\
                   [node] joining: retrying\n";
        assert_eq!(classify_phase(log).phase, "synced");
        assert_eq!(
            classify_phase("thread 'main' panicked at node.rs:1").phase,
            "fatal"
        );
    }

    #[test]
    fn join_state_accepts_only_the_authoritative_positive_ladder() {
        let report =
            parse_join_state(r#"{"phase":"synced","detail":"serving reads","height":9}"#).unwrap();
        assert_eq!(report.detail.as_deref(), Some("serving reads (height 9)"));
        assert!(parse_join_state(r#"{"phase":"fatal"}"#).is_none());
        assert!(parse_join_state("null").is_none());
    }

    #[test]
    fn forget_guard_fails_closed_and_force_cannot_reclassify_known_membership() {
        assert!(matches!(
            classify_forget_status("in-set=false validators=3"),
            ForgetVerdict::Safe
        ));
        assert!(matches!(
            classify_forget_status("in-set=true validators=1"),
            ForgetVerdict::Safe
        ));
        assert!(matches!(
            classify_forget_status("in-set=true validators=2"),
            ForgetVerdict::ConfirmedInSet(_)
        ));
        assert!(matches!(
            classify_forget_status("connection refused"),
            ForgetVerdict::Unconfirmed(_)
        ));
    }

    #[test]
    fn elapsed_time_parser_handles_ps_shapes_and_rejects_malformed_values() {
        assert_eq!(parse_etime("01:05"), Some(65));
        assert_eq!(parse_etime("02:01:05"), Some(7_265));
        assert_eq!(parse_etime("3-02:01:05"), Some(266_465));
        assert_eq!(parse_etime("05"), None);
        assert_eq!(parse_etime("99:99"), None);
    }

    #[cfg(unix)]
    #[test]
    fn process_matching_requires_exact_executable_and_argv_tokens() {
        let executable = PathBuf::from("/opt/ducktape-node");
        let config = PathBuf::from("/home/user/.ducktape/workspaces/team/node.toml");
        let spec = ManagedProcessSpec {
            executable: executable.clone(),
            config: config.clone(),
        };
        let exact = ProcessSnapshot {
            executable: executable.clone(),
            argv: vec![
                executable.as_os_str().to_owned(),
                "--config".into(),
                config.as_os_str().to_owned(),
            ],
        };
        assert!(process_snapshot_matches(&exact, &spec));

        let mut wrong_executable = exact.clone();
        wrong_executable.executable = PathBuf::from("/tmp/ducktape-node");
        assert!(!process_snapshot_matches(&wrong_executable, &spec));

        for argv in [
            vec![
                executable.as_os_str().to_owned(),
                "--config".into(),
                "/home/user/.ducktape/workspaces/team-2/node.toml".into(),
            ],
            vec![
                executable.as_os_str().to_owned(),
                format!("--config={}", config.display()).into(),
            ],
            vec![
                executable.as_os_str().to_owned(),
                "member-status".into(),
                "--config".into(),
                config.as_os_str().to_owned(),
            ],
        ] {
            assert!(!managed_argv_matches(&argv, &config));
        }
    }

    #[cfg(unix)]
    #[test]
    fn adoption_requires_one_verified_process_and_both_ports() {
        assert_eq!(
            existing_node_action(&[], false, false),
            Ok(ExistingNodeAction::Spawn)
        );
        assert_eq!(
            existing_node_action(&[7], true, true),
            Ok(ExistingNodeAction::Adopt(7))
        );
        for (pids, listen, http) in [
            (Vec::new(), true, false),
            (Vec::new(), false, true),
            (Vec::new(), true, true),
            (vec![7], true, false),
            (vec![7], false, true),
            (vec![7, 8], true, true),
        ] {
            assert!(
                existing_node_action(&pids, listen, http).is_err(),
                "accepted pids={pids:?} listen={listen} http={http}"
            );
        }
    }

    #[test]
    fn node_identity_is_mandatory_and_exact() {
        let expected = "11".repeat(32);
        let wire: ManagedNodeStatus = serde_json::from_value(serde_json::json!({
            "version": "0.1.0",
            "appHash": "aa".repeat(32),
            "publicKey": expected.clone(),
        }))
        .unwrap();
        assert!(validate_node_identity(&wire, &"11".repeat(32)).is_ok());

        let status = |public_key| ManagedNodeStatus {
            version: "0.1.0".into(),
            app_hash: "aa".repeat(32),
            public_key,
        };
        assert!(validate_node_identity(&status(Some(expected.clone())), &expected).is_ok());
        assert!(validate_node_identity(&status(None), &expected).is_err());
        assert!(validate_node_identity(&status(Some("22".repeat(32))), &expected).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn workspace_paths_refuse_a_symlinked_workspace_root() {
        use std::os::unix::fs::symlink;

        let root = scratch("workspace-symlink");
        let outside = scratch("workspace-symlink-outside");
        symlink(&outside, root.join("workspaces")).unwrap();
        assert!(workspace_dir(&root, "team").is_err());
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(outside).ok();
    }
}
