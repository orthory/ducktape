//! Workspace registry model, validation, and port allocation.

use std::collections::HashSet;
use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::private_fs;

const REGISTRY_VERSION: u32 = 1;
const MAX_WORKSPACE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePorts {
    pub listen: u16,
    pub http: u16,
    pub rpc: u16,
    #[serde(default)]
    pub wireguard: Option<u16>,
    #[serde(default)]
    pub invite: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub chain_id: String,
    pub pubkey: String,
    pub founder: bool,
    pub member: bool,
    pub ports: WorkspacePorts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub workspaces: Vec<Workspace>,
    pub active: Option<Workspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Registry {
    pub(super) version: u32,
    pub(super) active: Option<String>,
    pub(super) workspaces: Vec<Workspace>,
    #[serde(default)]
    pub(super) mnemonic_confirmed: bool,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            active: None,
            workspaces: Vec::new(),
            mnemonic_confirmed: false,
        }
    }
}

pub(super) fn snapshot_at(root: &Path) -> Result<WorkspaceSnapshot, String> {
    let registry = load_registry_at(&root.join("registry.json"))?;
    let active = registry.active.as_ref().and_then(|id| {
        registry
            .workspaces
            .iter()
            .find(|workspace| &workspace.id == id)
            .cloned()
    });
    Ok(WorkspaceSnapshot {
        workspaces: registry.workspaces,
        active,
    })
}

pub(super) fn load_registry_at(path: &Path) -> Result<Registry, String> {
    match private_fs::read_to_string(path)? {
        Some(text) => match parse_registry(&text) {
            Ok(registry) => Ok(registry),
            Err(error) => {
                let backup = path.with_extension("json.bak");
                let _ = fs::rename(path, &backup);
                tracing::warn!(
                    target: "ducktape::shell",
                    path = %path.display(),
                    backup = %backup.display(),
                    reason = "invalid_registry",
                    error = %error,
                    "workspace registry was invalid; starting empty"
                );
                Ok(Registry::default())
            }
        },
        None => Ok(Registry::default()),
    }
}

fn parse_registry(text: &str) -> Result<Registry, String> {
    let registry: Registry =
        serde_json::from_str(text).map_err(|error| format!("parse registry: {error}"))?;
    validate_registry(&registry)?;
    Ok(registry)
}

fn validate_registry(registry: &Registry) -> Result<(), String> {
    if registry.version != REGISTRY_VERSION {
        return Err(format!(
            "unsupported registry version {} (expected {REGISTRY_VERSION})",
            registry.version
        ));
    }

    let mut ids = HashSet::new();
    let mut ports = HashSet::new();
    for workspace in &registry.workspaces {
        validate_workspace(workspace)?;
        if !ids.insert(workspace.id.as_str()) {
            return Err(format!("duplicate workspace id {:?}", workspace.id));
        }
        for port in workspace_ports(&workspace.ports) {
            if !ports.insert(port) {
                return Err(format!("workspace port {port} is assigned more than once"));
            }
        }
    }
    if let Some(active) = registry.active.as_deref() {
        if !ids.contains(active) {
            return Err(format!("active workspace {active:?} does not exist"));
        }
    }
    Ok(())
}

fn validate_workspace(workspace: &Workspace) -> Result<(), String> {
    let id = workspace.id.as_bytes();
    if id.is_empty()
        || !id[0].is_ascii_alphanumeric()
        || !id[id.len() - 1].is_ascii_alphanumeric()
        || !id
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        return Err(format!("unsafe workspace id {:?}", workspace.id));
    }
    let name = workspace.name.trim();
    if name.is_empty() || name.len() > MAX_WORKSPACE_NAME_BYTES {
        return Err(format!("invalid workspace name for {:?}", workspace.id));
    }
    if workspace.chain_id.trim().is_empty() || workspace.pubkey.trim().is_empty() {
        return Err(format!(
            "workspace {:?} is missing its chain or public key",
            workspace.id
        ));
    }
    if workspace_ports(&workspace.ports).any(|port| port == 0) {
        return Err(format!("workspace {:?} contains port zero", workspace.id));
    }
    Ok(())
}

fn workspace_ports(ports: &WorkspacePorts) -> impl Iterator<Item = u16> {
    [
        Some(ports.listen),
        Some(ports.http),
        Some(ports.rpc),
        ports.wireguard,
        ports.invite,
    ]
    .into_iter()
    .flatten()
}

pub(super) fn port_listening(port: u16) -> bool {
    let timeout = Duration::from_millis(200);
    let v4 = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&v4, timeout).is_ok()
        || TcpStream::connect_timeout(&v6, timeout).is_ok()
}

#[allow(dead_code)]
pub(super) fn allocate_ports(reserved: &[u16]) -> Result<WorkspacePorts, String> {
    let mut used = reserved.to_vec();
    let listen = free_tcp_port(&used)?;
    used.push(listen);
    let http = free_tcp_port(&used)?;
    used.push(http);
    let rpc = free_tcp_port(&used)?;
    used.push(rpc);
    let wireguard = free_udp_port(&used)?;
    used.push(wireguard);
    let invite = free_udp_port(&used)?;
    Ok(WorkspacePorts {
        listen,
        http,
        rpc,
        wireguard: Some(wireguard),
        invite: Some(invite),
    })
}

fn free_tcp_port(used: &[u16]) -> Result<u16, String> {
    for _ in 0..64 {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("probe free tcp port: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        drop(listener);
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    Err("could not find a free tcp port".into())
}

fn free_udp_port(used: &[u16]) -> Result<u16, String> {
    for _ in 0..64 {
        let socket = UdpSocket::bind("127.0.0.1:0")
            .map_err(|error| format!("probe free udp port: {error}"))?;
        let port = socket
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        drop(socket);
        if !used.contains(&port) {
            return Ok(port);
        }
    }
    Err("could not find a free udp port".into())
}

pub(super) fn read_tail(path: &Path, max: u64) -> Result<String, String> {
    let mut file = match private_fs::open_private_read(path)? {
        Some(file) => file,
        None => return Ok(String::new()),
    };
    let len = file.metadata().map_err(|error| error.to_string())?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace(id: &str, listen: u16) -> Workspace {
        Workspace {
            id: id.into(),
            name: "Team".into(),
            chain_id: format!("chain-{id}"),
            pubkey: "11".repeat(32),
            founder: true,
            member: true,
            ports: WorkspacePorts {
                listen,
                http: listen + 1,
                rpc: listen + 2,
                wireguard: Some(listen + 3),
                invite: Some(listen + 4),
            },
        }
    }

    fn registry_json(workspaces: Vec<Workspace>, active: Option<&str>) -> String {
        serde_json::to_string(&Registry {
            version: REGISTRY_VERSION,
            active: active.map(str::to_string),
            workspaces,
            mnemonic_confirmed: false,
        })
        .unwrap()
    }

    fn scratch(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ducktape-iced-registry-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn registry_parses_wire_shape_and_resolves_active() {
        let text = registry_json(vec![workspace("alpha", 31_000)], Some("alpha"));
        let registry = parse_registry(&text).unwrap();
        assert_eq!(registry.workspaces[0].chain_id, "chain-alpha");
        let root = scratch("active");
        fs::write(root.join("registry.json"), text).unwrap();
        let snapshot = snapshot_at(&root).unwrap();
        assert_eq!(
            snapshot.active.as_ref().map(|item| item.id.as_str()),
            Some("alpha")
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn registry_rejects_path_like_ids_and_dangling_active() {
        for id in ["../escape", "a/b", "a\\b", "-leading", "UPPER"] {
            let error = parse_registry(&registry_json(vec![workspace(id, 31_000)], Some(id)))
                .expect_err("unsafe id must fail");
            assert!(error.contains("unsafe workspace id"), "{error}");
        }
        let error = parse_registry(&registry_json(
            vec![workspace("alpha", 31_000)],
            Some("ghost"),
        ))
        .expect_err("dangling active must fail");
        assert!(error.contains("does not exist"), "{error}");
    }

    #[test]
    fn registry_rejects_duplicate_ids_and_ports() {
        let duplicate_id = registry_json(
            vec![workspace("alpha", 31_000), workspace("alpha", 32_000)],
            Some("alpha"),
        );
        assert!(
            parse_registry(&duplicate_id)
                .expect_err("duplicate id")
                .contains("duplicate workspace id")
        );

        let duplicate_port = registry_json(
            vec![workspace("alpha", 31_000), workspace("beta", 31_004)],
            Some("alpha"),
        );
        assert!(
            parse_registry(&duplicate_port)
                .expect_err("overlapping port")
                .contains("assigned more than once")
        );
    }

    #[test]
    fn invalid_registry_is_backed_up_and_recovers_empty() {
        let root = scratch("backup");
        let path = root.join("registry.json");
        fs::write(&path, "{not json").unwrap();
        let snapshot = snapshot_at(&root).unwrap();
        assert!(snapshot.workspaces.is_empty());
        assert!(snapshot.active.is_none());
        assert!(path.with_extension("json.bak").exists());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn allocated_ports_avoid_reserved_and_each_other() {
        let reserved = [40_000, 40_001, 40_002];
        let ports = allocate_ports(&reserved).unwrap();
        let allocated: Vec<u16> = workspace_ports(&ports).collect();
        assert!(allocated.iter().all(|port| !reserved.contains(port)));
        let unique: HashSet<u16> = allocated.iter().copied().collect();
        assert_eq!(unique.len(), allocated.len());
    }
}
