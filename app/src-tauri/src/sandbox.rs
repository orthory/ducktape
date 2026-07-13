//! Host-side sandbox preflight probes and config editing for the Sandbox page.
//!
//! It inspects the active workspace's `node.toml` for the serving opt-in +
//! backend mode, probes the LOCAL host for backend readiness, and owns the pure
//! formatting-preserving config edit used by the guarded workspace apply flow.
//!
//! Probing runs subprocesses on the host, the same pattern the workspace and
//! forge commands already use (`ducktape-node` verbs, `git`). The probes only
//! describe the machine THIS desktop runs on, so the Sandbox tab gates the call
//! on the app owning a local managed node; otherwise it renders every item as
//! "unknown — run preflight on the node host".

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use serde::Serialize;
use tauri::Manager as _;

/// Default rootless-podman base image (spec §5), used when `node.toml` pins
/// none. Kept in sync with the TS `DEFAULT_SANDBOX_IMAGE`.
const DEFAULT_SANDBOX_IMAGE: &str = "docker.io/library/node:22-slim";
const DEFAULT_TART_IMAGE: &str = "ghcr.io/cirruslabs/macos-sonoma-base:latest";

/// The sandbox-relevant slice of `node.toml`. Every field is optional so a
/// node predating the capability keys parses cleanly as "serving off, no mode".
#[derive(Default, Deserialize)]
struct SandboxConfig {
    #[serde(default)]
    announce_capabilities: bool,
    #[serde(default)]
    sandbox: Option<String>,
    #[serde(default)]
    sandbox_image: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    ok: bool,
    detail: String,
}

impl ProbeResult {
    fn ok(detail: impl Into<String>) -> Self {
        Self { ok: true, detail: detail.into() }
    }
    fn fail(detail: impl Into<String>) -> Self {
        Self { ok: false, detail: detail.into() }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxPreflight {
    /// Host OS bucket: `"linux" | "macos" | "other"`.
    os: String,
    /// The platform's sandbox backend binary: `"podman" | "tart" | ""`.
    backend: String,
    /// Resolved base image (node.toml override, else the podman default).
    image: String,
    /// `node.toml` `announce_capabilities` — the serving opt-in.
    announce_capabilities: bool,
    /// `node.toml` `sandbox` mode: `"direct" | "podman" | "tart" | ""` (unset).
    mode: String,
    /// Backend binary presence; `None` when no backend applies to this OS.
    backend_binary: Option<ProbeResult>,
    /// Base image pulled; `None` off the podman path (e.g. macOS/tart).
    base_image: Option<ProbeResult>,
    /// cgroup v2 cpu+memory delegation; Linux only, else `None`.
    cgroup_delegation: Option<ProbeResult>,
}

/// Read the active workspace's `node.toml` sandbox keys, tolerating every
/// missing-file / parse case as an all-default config (serving off).
fn config_for(app: &crate::rt::AppHandle, id: &str) -> SandboxConfig {
    let Ok(home) = app.path().home_dir() else {
        return SandboxConfig::default();
    };
    let path = home.join(".ducktape").join("workspaces").join(id).join("node.toml");
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        Err(_) => SandboxConfig::default(),
    }
}

/// Apply one UI sandbox choice without rewriting unrelated node config or its
/// comments. `off` only disables announcing, so applying the previous backend
/// again keeps an operator's custom image/capacity values.
pub(crate) fn config_with_mode(text: &str, mode: &str) -> Result<String, String> {
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|err| format!("parse node.toml: {err}"))?;
    if mode == "off" {
        doc["announce_capabilities"] = toml_edit::value(false);
        return Ok(doc.to_string());
    }

    let image = match mode {
        "direct" => None,
        "podman" => Some(DEFAULT_SANDBOX_IMAGE),
        "tart" => Some(DEFAULT_TART_IMAGE),
        other => return Err(format!("unsupported sandbox mode {other:?}")),
    };
    let same_backend = doc.get("sandbox").and_then(|item| item.as_str()) == Some(mode);
    doc["announce_capabilities"] = toml_edit::value(true);
    doc["sandbox"] = toml_edit::value(mode);
    if let Some(default_image) = image {
        if !same_backend || doc.get("sandbox_image").is_none() {
            doc["sandbox_image"] = toml_edit::value(default_image);
        }
        if !same_backend || doc.get("sandbox_cores").is_none() {
            doc["sandbox_cores"] = toml_edit::value(2);
        }
        if !same_backend || doc.get("sandbox_mem_gb").is_none() {
            doc["sandbox_mem_gb"] = toml_edit::value(4);
        }
    } else {
        doc.as_table_mut().remove("sandbox_image");
        doc.as_table_mut().remove("sandbox_cores");
        doc.as_table_mut().remove("sandbox_mem_gb");
    }
    Ok(doc.to_string())
}

/// `bin --version` → present + version line, or an honest not-found.
fn probe_binary(bin: &str) -> ProbeResult {
    probe_binary_arg(bin, "--version")
}

fn probe_binary_arg(bin: &str, version_arg: &str) -> ProbeResult {
    match Command::new(bin).arg(version_arg).output() {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let line = stdout.lines().next().unwrap_or("").trim();
            if line.is_empty() {
                ProbeResult::ok(format!("{bin} present"))
            } else {
                ProbeResult::ok(line.to_string())
            }
        }
        Ok(_) => ProbeResult::fail(format!("{bin} present but `{version_arg}` failed")),
        Err(_) => ProbeResult::fail(format!("{bin} not found on PATH")),
    }
}

fn probe_tart_toolchain() -> ProbeResult {
    let tart = probe_binary("tart");
    if !tart.ok {
        return tart;
    }
    let sshpass = probe_binary_arg("sshpass", "-V");
    if !sshpass.ok {
        return ProbeResult {
            ok: false,
            detail: format!(
                "{}; sshpass is required for guest execution ({})",
                tart.detail, sshpass.detail
            ),
        };
    }
    ProbeResult {
        ok: true,
        detail: format!("{}; {}", tart.detail, sshpass.detail),
    }
}

/// `podman image exists <image>` — exit 0 present, non-zero absent.
fn probe_image(bin: &str, image: &str) -> ProbeResult {
    match Command::new(bin).args(["image", "exists", image]).status() {
        Ok(status) if status.success() => ProbeResult::ok(format!("{image} present")),
        Ok(_) => ProbeResult::fail(format!("{image} not pulled — run `{bin} pull {image}`")),
        Err(_) => ProbeResult::fail(format!("{bin} not runnable")),
    }
}

/// cgroup v2 cpu+memory delegation for the current systemd user session.
///
/// ponytail: reads the app process's own delegated controllers as a proxy for
/// the rootless-podman session (both live in the operator's user slice).
/// Upgrade to per-uid slice inspection only if the app ever runs outside the
/// operator's session.
fn probe_cgroup_delegation() -> ProbeResult {
    if !Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        return ProbeResult::fail("cgroup v2 (unified hierarchy) not mounted");
    }
    let cur = match fs::read_to_string("/proc/self/cgroup") {
        Ok(text) => text,
        Err(err) => return ProbeResult::fail(format!("read /proc/self/cgroup: {err}")),
    };
    // A cgroup v2 process has a single `0::<path>` line.
    let rel = cur.lines().find_map(|l| l.strip_prefix("0::")).unwrap_or("").trim();
    let controllers = fs::read_to_string(format!("/sys/fs/cgroup{rel}/cgroup.controllers"))
        .unwrap_or_default();
    let have: Vec<&str> = controllers.split_whitespace().collect();
    let has_cpu = have.contains(&"cpu");
    let has_mem = have.contains(&"memory");
    if has_cpu && has_mem {
        ProbeResult::ok(format!("cpu + memory delegated ({})", have.join(" ")))
    } else {
        ProbeResult::fail(format!(
            "cpu/memory not delegated to the user session (have: {})",
            if have.is_empty() { "none".to_string() } else { have.join(" ") }
        ))
    }
}

/// Probe the local host's readiness to serve sandboxed agent work + report the
/// active workspace's serving opt-in. `id` is the active workspace id.
#[tauri::command]
pub fn sandbox_preflight(
    app: crate::rt::AppHandle,
    id: String,
) -> Result<SandboxPreflight, String> {
    let cfg = config_for(&app, &id);
    let mode = cfg.sandbox.clone().unwrap_or_default();
    let image = cfg.sandbox_image.clone().unwrap_or_else(|| {
        if mode == "tart" {
            DEFAULT_TART_IMAGE
        } else {
            DEFAULT_SANDBOX_IMAGE
        }
        .to_string()
    });

    let (os, backend, backend_binary, base_image, cgroup_delegation) = if cfg!(target_os = "linux")
    {
        (
            "linux",
            "podman",
            Some(probe_binary("podman")),
            Some(probe_image("podman", &image)),
            Some(probe_cgroup_delegation()),
        )
    } else if cfg!(target_os = "macos") && mode == "podman" {
        (
            "macos",
            "podman",
            Some(probe_binary("podman")),
            Some(probe_image("podman", &image)),
            None,
        )
    } else if cfg!(target_os = "macos") {
        // Tart uses a VM image and has no cgroup probe.
        ("macos", "tart", Some(probe_tart_toolchain()), None, None)
    } else {
        ("other", "", None, None, None)
    };

    Ok(SandboxPreflight {
        os: os.to_string(),
        backend: backend.to_string(),
        image,
        announce_capabilities: cfg.announce_capabilities,
        mode,
        backend_binary,
        base_image,
        cgroup_delegation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_mode_edit_preserves_unrelated_config_and_comments() {
        let original = "# keep me\nnetwork = \"network.toml\"\nlisten = \"127.0.0.1:1\"\n";
        let updated = config_with_mode(original, "podman").unwrap();
        assert!(updated.starts_with("# keep me\n"));
        assert!(updated.contains("listen = \"127.0.0.1:1\""));
        assert!(updated.contains("announce_capabilities = true"));
        assert!(updated.contains("sandbox = \"podman\""));
        assert!(updated.contains(DEFAULT_SANDBOX_IMAGE));
        assert!(config_with_mode(original, "docker").is_err());
    }

    #[test]
    fn off_keeps_backend_tuning_for_reenable() {
        let original =
            "announce_capabilities = true\nsandbox = \"podman\"\nsandbox_image = \"custom\"\n";
        let off = config_with_mode(original, "off").unwrap();
        assert!(off.contains("announce_capabilities = false"));
        let enabled = config_with_mode(&off, "podman").unwrap();
        assert!(enabled.contains("sandbox_image = \"custom\""));
    }
}
