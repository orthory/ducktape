//! Host-side sandbox preflight probes for the Node view's onboarding section.
//!
//! Read-only. It inspects the active workspace's `node.toml` for the serving
//! opt-in + backend mode, then probes the LOCAL host for the platform backend's
//! readiness (binary present, base image pulled, cgroup v2 delegation). It
//! never mutates config: turning serving on/off is guided TOML the operator
//! pastes into `node.toml` (the app has no config-write path — see the Sandbox
//! tab), matching the "existing patterns only" scope of the onboarding phase.
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

/// `bin --version` → present + version line, or an honest not-found.
fn probe_binary(bin: &str) -> ProbeResult {
    match Command::new(bin).arg("--version").output() {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let line = stdout.lines().next().unwrap_or("").trim();
            if line.is_empty() {
                ProbeResult::ok(format!("{bin} present"))
            } else {
                ProbeResult::ok(line.to_string())
            }
        }
        Ok(_) => ProbeResult::fail(format!("{bin} present but `--version` failed")),
        Err(_) => ProbeResult::fail(format!("{bin} not found on PATH")),
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
    let image = cfg
        .sandbox_image
        .clone()
        .unwrap_or_else(|| DEFAULT_SANDBOX_IMAGE.to_string());
    let mode = cfg.sandbox.clone().unwrap_or_default();

    let (os, backend, backend_binary, base_image, cgroup_delegation) =
        if cfg!(target_os = "linux") {
            (
                "linux",
                "podman",
                Some(probe_binary("podman")),
                Some(probe_image("podman", &image)),
                Some(probe_cgroup_delegation()),
            )
        } else if cfg!(target_os = "macos") {
            // tart is phase 2 (needs a real-Mac pass): probe presence honestly,
            // leave the podman-shaped image/cgroup checks not-applicable.
            ("macos", "tart", Some(probe_binary("tart")), None, None)
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
