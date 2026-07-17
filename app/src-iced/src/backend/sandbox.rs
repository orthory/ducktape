//! Local sandbox probes and rollback-safe managed workspace configuration.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use super::Backend;
use super::workspace_service::{
    STOP_GRACE, find_workspace, load_registry, start_node, stop_node, wait_node_ready,
    workspace_dir, write_atomic,
};

const DEFAULT_PODMAN_IMAGE: &str = "docker.io/library/node:22-slim";
const DEFAULT_TART_IMAGE: &str = "ghcr.io/cirruslabs/macos-sonoma-base:latest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxChoice {
    Off,
    Podman,
    Tart,
}

impl SandboxChoice {
    fn value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Podman => "podman",
            Self::Tart => "tart",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPreflight {
    pub os: String,
    pub backend: String,
    pub image: String,
    pub announce_capabilities: bool,
    pub mode: String,
    pub backend_binary: Option<ProbeResult>,
    pub base_image: Option<ProbeResult>,
    pub cgroup_delegation: Option<ProbeResult>,
}

#[derive(Default, Deserialize)]
struct SandboxConfig {
    #[serde(default)]
    announce_capabilities: bool,
    sandbox: Option<String>,
    sandbox_image: Option<String>,
}

impl Backend {
    pub async fn sandbox_preflight(&self, id: String) -> Result<SandboxPreflight, String> {
        let root = self.root.clone();
        self.control
            .run(move || sandbox_preflight_blocking(&root, &id))
            .await
    }

    pub async fn apply_workspace_sandbox(
        &self,
        id: String,
        choice: SandboxChoice,
    ) -> Result<(), String> {
        let root = self.root.clone();
        self.control
            .run(move || apply_workspace_sandbox_blocking(&root, &id, choice))
            .await
    }
}

fn sandbox_preflight_blocking(root: &Path, id: &str) -> Result<SandboxPreflight, String> {
    let registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?;
    let path = workspace_dir(root, &workspace.id)?.join("node.toml");
    let config: SandboxConfig = fs::read_to_string(&path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default();
    let mode = config.sandbox.unwrap_or_default();
    let image = config.sandbox_image.unwrap_or_else(|| {
        if mode == "tart" {
            DEFAULT_TART_IMAGE
        } else {
            DEFAULT_PODMAN_IMAGE
        }
        .into()
    });
    let (os, backend, backend_binary, base_image, cgroup_delegation) = if cfg!(target_os = "linux")
    {
        (
            "linux",
            "podman",
            Some(probe_binary("podman", "--version")),
            Some(probe_image("podman", &image)),
            Some(probe_cgroup_delegation()),
        )
    } else if cfg!(target_os = "macos") && mode == "podman" {
        (
            "macos",
            "podman",
            Some(probe_binary("podman", "--version")),
            Some(probe_image("podman", &image)),
            None,
        )
    } else if cfg!(target_os = "macos") {
        ("macos", "tart", Some(probe_tart()), None, None)
    } else {
        ("other", "", None, None, None)
    };
    Ok(SandboxPreflight {
        os: os.into(),
        backend: backend.into(),
        image,
        announce_capabilities: config.announce_capabilities,
        mode,
        backend_binary,
        base_image,
        cgroup_delegation,
    })
}

fn apply_workspace_sandbox_blocking(
    root: &Path,
    id: &str,
    choice: SandboxChoice,
) -> Result<(), String> {
    let registry = load_registry(root)?;
    let workspace = find_workspace(&registry, id)?.clone();
    let dir = workspace_dir(root, &workspace.id)?;
    let path = dir.join("node.toml");
    let original =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let updated = config_with_mode(&original, choice)?;
    if updated == original {
        return Ok(());
    }

    stop_node(&dir, &workspace.ports, STOP_GRACE)?;
    if let Err(apply) = write_atomic(&path, updated.as_bytes()) {
        let recovery = start_node(root, &workspace)
            .and_then(|()| wait_node_ready(root, &workspace))
            .map(|()| "the previous node was restarted".to_string())
            .unwrap_or_else(|error| format!("the previous node also failed to restart: {error}"));
        return Err(format!("apply sandbox config: {apply}; {recovery}"));
    }
    if let Err(apply) =
        start_node(root, &workspace).and_then(|()| wait_node_ready(root, &workspace))
    {
        let _ = stop_node(&dir, &workspace.ports, STOP_GRACE);
        let recovery = match write_atomic(&path, original.as_bytes()) {
            Ok(()) => start_node(root, &workspace)
                .and_then(|()| wait_node_ready(root, &workspace))
                .map(|()| "the previous config was restored and restarted".to_string())
                .unwrap_or_else(|error| {
                    format!("the previous config was restored but failed to restart: {error}")
                }),
            Err(error) => format!("the previous config could not be restored: {error}"),
        };
        return Err(format!(
            "restart with sandbox mode {:?}: {apply}; {recovery}",
            choice.value()
        ));
    }
    Ok(())
}

fn config_with_mode(text: &str, choice: SandboxChoice) -> Result<String, String> {
    let parsed: toml::Value =
        toml::from_str(text).map_err(|error| format!("parse node.toml: {error}"))?;
    let current = parsed.get("sandbox").and_then(toml::Value::as_str);
    let same_backend = current == Some(choice.value());
    let mut edits = vec![(
        "announce_capabilities",
        (choice != SandboxChoice::Off).to_string(),
    )];
    if choice != SandboxChoice::Off {
        edits.push(("sandbox", format!("{:?}", choice.value())));
        let image = if choice == SandboxChoice::Tart {
            DEFAULT_TART_IMAGE
        } else {
            DEFAULT_PODMAN_IMAGE
        };
        if !same_backend || parsed.get("sandbox_image").is_none() {
            edits.push(("sandbox_image", format!("{image:?}")));
        }
        if !same_backend || parsed.get("sandbox_cores").is_none() {
            edits.push(("sandbox_cores", "2".into()));
        }
        if !same_backend || parsed.get("sandbox_mem_gb").is_none() {
            edits.push(("sandbox_mem_gb", "4".into()));
        }
    }
    let mut output = text.to_string();
    for (key, value) in edits {
        output = set_top_level(&output, key, &value);
    }
    // Verify our formatting-preserving editor still produced valid TOML and
    // that every requested value landed at the top level.
    let verified: toml::Value =
        toml::from_str(&output).map_err(|error| format!("edited node.toml is invalid: {error}"))?;
    let announced = verified
        .get("announce_capabilities")
        .and_then(toml::Value::as_bool);
    if announced != Some(choice != SandboxChoice::Off)
        || (choice != SandboxChoice::Off
            && verified.get("sandbox").and_then(toml::Value::as_str) != Some(choice.value()))
    {
        return Err("sandbox config edit did not apply the requested mode".into());
    }
    Ok(output)
}

fn set_top_level(text: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let first_table = lines
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    if let Some(index) = lines[..first_table].iter().position(|line| {
        line.split_once('=')
            .is_some_and(|(candidate, _)| candidate.trim() == key)
    }) {
        let indent = lines[index].len() - lines[index].trim_start().len();
        lines[index] = format!("{}{key} = {value}", " ".repeat(indent));
    } else {
        lines.insert(first_table, format!("{key} = {value}"));
    }
    let mut output = lines.join("\n");
    if text.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn probe_binary(binary: &str, arg: &str) -> ProbeResult {
    match Command::new(binary).arg(arg).output() {
        Ok(output) if output.status.success() => {
            let line = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            ProbeResult {
                ok: true,
                detail: if line.is_empty() {
                    format!("{binary} present")
                } else {
                    line
                },
            }
        }
        Ok(_) => ProbeResult {
            ok: false,
            detail: format!("{binary} present but `{arg}` failed"),
        },
        Err(_) => ProbeResult {
            ok: false,
            detail: format!("{binary} not found on PATH"),
        },
    }
}

fn probe_tart() -> ProbeResult {
    let tart = probe_binary("tart", "--version");
    if !tart.ok {
        return tart;
    }
    let sshpass = probe_binary("sshpass", "-V");
    ProbeResult {
        ok: sshpass.ok,
        detail: if sshpass.ok {
            format!("{}; {}", tart.detail, sshpass.detail)
        } else {
            format!(
                "{}; sshpass is required for guest execution ({})",
                tart.detail, sshpass.detail
            )
        },
    }
}

fn probe_image(binary: &str, image: &str) -> ProbeResult {
    match Command::new(binary)
        .args(["image", "exists", image])
        .status()
    {
        Ok(status) if status.success() => ProbeResult {
            ok: true,
            detail: format!("{image} present"),
        },
        Ok(_) => ProbeResult {
            ok: false,
            detail: format!("{image} not pulled — run `{binary} pull {image}`"),
        },
        Err(_) => ProbeResult {
            ok: false,
            detail: format!("{binary} not runnable"),
        },
    }
}

fn probe_cgroup_delegation() -> ProbeResult {
    if !Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        return ProbeResult {
            ok: false,
            detail: "cgroup v2 (unified hierarchy) not mounted".into(),
        };
    }
    let current = match fs::read_to_string("/proc/self/cgroup") {
        Ok(text) => text,
        Err(error) => {
            return ProbeResult {
                ok: false,
                detail: format!("read /proc/self/cgroup: {error}"),
            };
        }
    };
    let relative = current
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap_or("")
        .trim();
    let controllers = fs::read_to_string(format!("/sys/fs/cgroup{relative}/cgroup.controllers"))
        .unwrap_or_default();
    let available: Vec<&str> = controllers.split_whitespace().collect();
    let ok = available.contains(&"cpu") && available.contains(&"memory");
    ProbeResult {
        ok,
        detail: if ok {
            format!("cpu + memory delegated ({})", available.join(" "))
        } else {
            format!(
                "cpu/memory not delegated to the user session (have: {})",
                if available.is_empty() {
                    "none".into()
                } else {
                    available.join(" ")
                }
            )
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_edit_preserves_comments_and_custom_same_backend_values() {
        let original = "# keep me\nnetwork = \"network.toml\"\nsandbox = \"podman\"\nsandbox_image = \"custom\"\n";
        let updated = config_with_mode(original, SandboxChoice::Podman).unwrap();
        assert!(updated.starts_with("# keep me\n"));
        assert!(updated.contains("network = \"network.toml\""));
        assert!(updated.contains("sandbox_image = \"custom\""));
        assert!(updated.contains("announce_capabilities = true"));
    }

    #[test]
    fn off_keeps_backend_tuning_for_reenable() {
        let original =
            "announce_capabilities = true\nsandbox = \"podman\"\nsandbox_image = \"custom\"\n";
        let off = config_with_mode(original, SandboxChoice::Off).unwrap();
        assert!(off.contains("announce_capabilities = false"));
        let enabled = config_with_mode(&off, SandboxChoice::Podman).unwrap();
        assert!(enabled.contains("sandbox_image = \"custom\""));
    }
}
