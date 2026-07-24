//! a minimal libpod (podman) REST client over the node-owned rootless unix
//! socket — the sandbox no longer shells out to the `podman` CLI. Only the
//! handful of endpoints a provider run needs are implemented
//! (create/start/attach/wait/resize/kill/remove), hand-rolled over a
//! `tokio::net::UnixStream` so it pulls no HTTP client dependency and the
//! response parser + attach demux are unit-testable without a running podman.
//!
//! LIVE-VALIDATED against real rootless podman (pasta netns on native Linux):
//! create+inspect confirms every `SpecGenerator` field takes effect (work_dir,
//! mounts+RW, netns=pasta, dropped NET_ADMIN/NET_RAW in `.BoundingCaps`, cpu/mem
//! limits, annotations); attach returns `101 UPGRADED` and the
//! raw-stdin/framed-stdout demux round-trips; the egress ruleset, installed via
//! `nsenter -n` + `nft` into the container netns (netns only — the hook already
//! runs in podman's rootless userns), blocks the LAN + tailnet (incl. tailnet
//! DNS) while pasta's host address + DNS forwarder + the public internet stay
//! reachable. Pure logic is also unit-tested (HTTP parse, chunked decode, attach
//! demux, spec JSON, ruleset order) so it stays green without podman.
//!
//! netns backend: `pasta` — podman 6's only rootless backend (slirp4netns was
//! removed) and the `passt` package on older hosts. Using it explicitly (not
//! `"private"`) makes the run's host + DNS addresses the fixed pasta defaults
//! ([`PASTA_HOST`] / [`PASTA_DNS`]) the egress hook keys on.
//!
//! Path hiding lives here too: [`plan_mounts`] maps every host path to a
//! NEUTRAL `/ducktape/...` guest path and [`translate`] rewrites env/argv to
//! match, so a lent-credential guest never sees the operator's real paths.
//! Network isolation is the egress allowlist [`egress_nftables`] installed in
//! the container netns by the node binary's `__egress-hook`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

/// the libpod API version prefix. podman accepts any `>=` version it supports;
/// pinning a concrete one keeps the request line stable.
const API: &str = "/v5.0.0/libpod";

/// the neutral container root every host path is mounted under. a guest sees
/// `/ducktape/workspace`, `/ducktape/home/...`, `/ducktape/bin/<name>` — never
/// the real host path, so the operator's identity and layout stay hidden.
pub(crate) const GUEST_ROOT: &str = "/ducktape";

// ---------------------------------------------------------------------------
// neutral mount plan + path translation (Part A: hide host paths)
// ---------------------------------------------------------------------------

/// one host→container bind mount, rendered into a `SpecGenerator` mount and
/// used by [`translate`] to rewrite host-path substrings in env/argv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    pub(crate) host: PathBuf,
    pub(crate) guest: PathBuf,
    pub(crate) read_only: bool,
}

/// the neutral guest layout for one run: the bind mounts plus the three guest
/// paths the caller needs to build the spec (workdir, bin, home).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MountPlan {
    pub(crate) mounts: Vec<Mount>,
    pub(crate) guest_workdir: PathBuf,
    pub(crate) guest_bin: PathBuf,
    pub(crate) guest_home: PathBuf,
}

/// build the neutral `/ducktape/*` mount plan from a run's HOST paths. every
/// mapping hides the host side:
/// - `workdir` → `/ducktape/workspace` (rw, the cwd)
/// - `bin`     → `/ducktape/bin/<filename>` (ro)
/// - each `rw_dir` (CLI auth/state under `home`) → `/ducktape/home/<rel>` (rw)
/// - a FILE in `ro_paths` (the workspace-parent context doc) → `/ducktape/<name>`,
///   one level above the workspace so `../<name>` still resolves
/// - every other `ro_path` (PATH dirs, skills tree) → `/ducktape/ro<i>` (ro)
pub(crate) fn plan_mounts(
    workdir: &Path,
    bin: &Path,
    ro_paths: &[PathBuf],
    rw_dirs: &[PathBuf],
    home: &Path,
) -> MountPlan {
    let root = Path::new(GUEST_ROOT);
    let guest_workdir = root.join("workspace");
    let bin_name = bin.file_name().unwrap_or_else(|| std::ffi::OsStr::new("bin"));
    let guest_bin = root.join("bin").join(bin_name);
    let guest_home = root.join("home");

    let mut mounts = vec![
        Mount {
            host: workdir.to_path_buf(),
            guest: guest_workdir.clone(),
            read_only: false,
        },
        Mount {
            host: bin.to_path_buf(),
            guest: guest_bin.clone(),
            read_only: true,
        },
    ];
    for dir in rw_dirs {
        // spec.rs guaranteed these live under HOME; a defensive fallback keeps
        // the basename if a stray one does not, rather than leaking the path.
        let rel = dir.strip_prefix(home).unwrap_or_else(|_| {
            Path::new(dir.file_name().unwrap_or_else(|| std::ffi::OsStr::new("state")))
        });
        mounts.push(Mount {
            host: dir.to_path_buf(),
            guest: guest_home.join(rel),
            read_only: false,
        });
    }
    for (i, path) in ro_paths.iter().enumerate() {
        // the ONE file among ro_paths is the workspace-parent context doc; it
        // must land beside the workspace (parent of cwd) so `../<name>` resolves.
        // every other entry is a directory (PATH binding / skills tree).
        let guest = if path.is_file() {
            let name = path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("context"));
            root.join(name)
        } else {
            root.join(format!("ro{i}"))
        };
        mounts.push(Mount {
            host: path.to_path_buf(),
            guest,
            read_only: true,
        });
    }
    MountPlan {
        mounts,
        guest_workdir,
        guest_bin,
        guest_home,
    }
}

/// rewrite every host-path substring in `value` to its neutral guest path.
/// longest host prefix first, so a nested mount (an auth dir under HOME) wins
/// over its parent. covers whole-value argv paths AND paths embedded in a
/// larger string (the codex `projects."<workdir>"` TOML key). `home` maps to
/// the guest home so any stray `$HOME`-prefixed value is sanitized even though
/// HOME itself is never bind-mounted.
pub(crate) fn translate(value: &str, mounts: &[Mount], home: &Path, guest_home: &Path) -> String {
    let mut pairs: Vec<(String, String)> = mounts
        .iter()
        .map(|m| {
            (
                m.host.to_string_lossy().into_owned(),
                m.guest.to_string_lossy().into_owned(),
            )
        })
        .collect();
    pairs.push((
        home.to_string_lossy().into_owned(),
        guest_home.to_string_lossy().into_owned(),
    ));
    // longest host prefix first so nested mounts win.
    pairs.sort_by_key(|(host, _)| std::cmp::Reverse(host.len()));
    let mut out = value.to_string();
    for (host, guest) in pairs {
        if !host.is_empty() {
            out = out.replace(&host, &guest);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// egress allowlist ruleset (Part B: restrict network)
// ---------------------------------------------------------------------------

/// the nftables ruleset installed inside a run's container netns by the node
/// binary's `__egress-hook` — PURE, so its ordering is unit-tested without
/// podman. ORDER is load-bearing: the broker/node + DNS accepts precede the
/// private-range drop, so those specific host:port pairs survive even though
/// their IPs sit inside dropped ranges. `100.64.0.0/10` is the tailnet/CGNAT
/// block; `fc00::/7` covers IPv6 ULA + tailnet-v6. Everything else (the public
/// internet, v4 and v6) falls through to `policy accept` — the broker still
/// mediates all provider-API traffic regardless.
///
/// `host_ip` is pasta's `host.containers.internal` ([`PASTA_HOST`], the
/// link-local the container reaches this node's broker + RPC at). `resolver_ip`
/// is pasta's DNS forwarder ([`PASTA_DNS`]). DNS is scoped to THAT ip only, NOT
/// `dport 53` universally: pasta copies the host's resolv.conf into the
/// container, so on a tailnet box it also lists the Tailscale MagicDNS resolvers
/// (100.100.100.100 / fd7a::53) — a blanket `dport 53 accept` would let the run
/// reach those tailnet services and any LAN box on :53. Scoping to the forwarder
/// keeps name resolution working (it forwards upstream) while the tailnet/LAN
/// resolvers stay dropped. Note pasta mirrors the host's own routes into the
/// container, so this firewall is what actually contains it.
/// (Live-verified on native-Linux pasta: broker reachable, LAN + tailnet DNS
/// blocked, github.com still resolves through the forwarder.)
pub fn egress_nftables(host_ip: &str, resolver_ip: &str, ports: &[u16]) -> String {
    let mut lines = vec![
        "table inet ducktape {".to_string(),
        "  chain output {".to_string(),
        "    type filter hook output priority 0; policy accept;".to_string(),
        "    oifname \"lo\" accept".to_string(),
    ];
    if !ports.is_empty() {
        let allowed = ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("    ip daddr {host_ip} tcp dport {{ {allowed} }} accept"));
    }
    // DNS ONLY to the container's own forwarder — never a blanket :53 (see doc).
    lines.push(format!("    ip daddr {resolver_ip} udp dport 53 accept"));
    lines.push(format!("    ip daddr {resolver_ip} tcp dport 53 accept"));
    lines.push(
        "    ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 100.64.0.0/10, \
         169.254.0.0/16, 127.0.0.0/8 } drop"
            .to_string(),
    );
    // IPv6: drop ULA (incl. tailnet fd7a::/48) + link-local + loopback; public
    // v6 falls through to policy accept, same as v4.
    lines.push("    ip6 daddr { fc00::/7, fe80::/10, ::1/128 } drop".to_string());
    lines.push("  }".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// the OCI createRuntime hook (installs the egress firewall in the netns)
// ---------------------------------------------------------------------------

/// the OCI container state podman pipes to a hook on stdin. Only the fields the
/// egress hook needs are decoded; libpod sends many more.
#[derive(serde::Deserialize)]
struct OciState {
    pid: i32,
    #[serde(default)]
    annotations: BTreeMap<String, String>,
}

/// run the egress `createRuntime` hook: read the OCI state on stdin, and if this
/// run carries the `io.ducktape.egress=1` annotation, install the nft allowlist
/// inside the container's netns via `nsenter -n` + `nft`. Called by the node
/// binary's hidden `__egress-hook` subcommand.
///
/// FAILS CLOSED: any error returns `Err`, the subcommand exits non-zero, and
/// podman aborts the container — a run whose firewall could not be installed
/// never starts. (Verified live: a failing hook makes `containers/{id}/start`
/// return HTTP 500 and the container does not run.)
///
/// Namespace note (verified live on rootless podman 5.4.2): at `createRuntime`
/// the hook already runs INSIDE podman's rootless user namespace — the same one
/// that owns the container netns — so it enters the NET namespace ONLY
/// (`nsenter -n`, never `-U`, which fails with EINVAL here) and still has the
/// caps to load nft. The container itself runs with `NET_ADMIN`/`NET_RAW`
/// dropped, so it cannot undo the rules.
pub fn run_egress_hook() -> Result<(), String> {
    use std::io::Read as _;
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .map_err(|e| format!("egress hook: read OCI state from stdin: {e}"))?;
    let state: OciState =
        serde_json::from_str(&raw).map_err(|e| format!("egress hook: parse OCI state: {e}"))?;

    // no marker → this is not one of our runs; nothing to do.
    if state.annotations.get("io.ducktape.egress").map(String::as_str) != Some("1") {
        return Ok(());
    }
    let ports = state
        .annotations
        .get("io.ducktape.egress.ports")
        .map(|s| {
            s.split(',')
                .filter(|p| !p.is_empty())
                .map(|p| p.parse::<u16>().map_err(|e| format!("bad egress port {p:?}: {e}")))
                .collect::<Result<Vec<u16>, String>>()
        })
        .transpose()?
        .unwrap_or_default();

    // host + resolver are pasta's fixed link-local defaults, so the hook needs
    // no host-computed annotations — only the run's allowed ports.
    let ruleset = egress_nftables(PASTA_HOST, PASTA_DNS, &ports);
    install_nft_in_netns(state.pid, &ruleset)
}

/// pipe `ruleset` to `nft -f -` running inside pid's network namespace via
/// `nsenter -n`. The hook's own userns already owns that netns (see
/// [`run_egress_hook`]), so no `-U`.
fn install_nft_in_netns(pid: i32, ruleset: &str) -> Result<(), String> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let nsenter = find_system_tool("nsenter").ok_or("egress hook: nsenter not found")?;
    let nft = find_system_tool("nft").ok_or("egress hook: nft not found")?;
    let mut child = Command::new(nsenter)
        .args([
            "--preserve-credentials",
            "-n",
            "-t",
            &pid.to_string(),
            "--",
        ])
        .arg(&nft)
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("egress hook: spawn nsenter/nft: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("egress hook: nft stdin missing")?
        .write_all(ruleset.as_bytes())
        .map_err(|e| format!("egress hook: write ruleset to nft: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("egress hook: wait for nft: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "egress hook: nft rejected the ruleset ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// resolve a system tool by PATH, then the standard sbin/bin dirs a non-root
/// PATH usually omits — `nft` ships in `/usr/sbin`. Shared with the sandbox boot
/// probe ([`crate::SandboxBackend::probe`]).
pub(crate) fn find_system_tool(bin: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(bin);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    ["/usr/sbin", "/sbin", "/usr/bin", "/bin"]
        .into_iter()
        .map(|dir| Path::new(dir).join(bin))
        .find(|cand| cand.is_file())
}

// ---------------------------------------------------------------------------
// SpecGenerator (the libpod container-create body)
// ---------------------------------------------------------------------------

/// an OCI bind mount as libpod's create endpoint expects it.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct OciMount {
    pub destination: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub source: String,
    pub options: Vec<String>,
}

/// a libpod `Namespace` — `nsmode` is `"pasta"`: the modern rootless netns
/// backend, and the ONLY one podman 6 ships (slirp4netns was removed). pasta is
/// used explicitly, not via `"private"`, so the run's host + resolver addresses
/// are the fixed pasta defaults ([`PASTA_HOST`] / [`PASTA_DNS`]) the egress hook
/// keys on — deterministic, no host-side probing. pasta must be installed
/// ([`crate::SandboxBackend::probe`] enforces it); it ships with podman 6 and is
/// the `passt` package on older hosts.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Namespace {
    pub nsmode: String,
}

/// OCI CPU limits; `cpus` become a quota over the standard 100 000 µs period.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct CpuLimit {
    pub quota: i64,
    pub period: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct MemoryLimit {
    pub limit: i64,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub(crate) struct ResourceLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryLimit>,
}

/// the container-create body. Only the fields a provider run sets are present;
/// everything else takes podman's default. Field names are libpod's json tags.
#[derive(Debug, Serialize)]
pub(crate) struct SpecGenerator {
    pub image: String,
    pub command: Vec<String>,
    pub work_dir: String,
    pub env: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub mounts: Vec<OciMount>,
    pub netns: Namespace,
    pub cap_drop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,
    /// container-side pty (interactive TUI). headless runs leave it false.
    pub terminal: bool,
    /// keep stdin open so the prompt / keystrokes can be written on attach.
    pub stdin: bool,
    /// We own removal explicitly on teardown (after `wait` reads the exit code);
    /// auto-remove would race the wait/remove and 404 it, so it stays off.
    pub remove: bool,
    pub labels: BTreeMap<String, String>,
}

/// the parameters the sandbox hands the spec builder — the neutral guest paths
/// already resolved, env/argv already translated.
pub(crate) struct SpecInputs<'a> {
    pub image: &'a str,
    pub guest_bin: &'a Path,
    pub guest_workdir: &'a Path,
    pub args: &'a [String],
    /// (name, value) with values already translated to guest paths.
    pub env: &'a [(String, String)],
    pub mounts: &'a [Mount],
    /// numeric limits by dimension (`cores`, `mem_gb`); unknown ones ignored.
    pub limits: &'a BTreeMap<String, u64>,
    pub labels: &'a [String],
    pub terminal: bool,
}

impl SpecGenerator {
    /// assemble the create body. `NET_ADMIN`/`NET_RAW` are always dropped so the
    /// workload cannot touch the egress firewall or open raw sockets; the netns
    /// is always the private slirp4netns with host-loopback + IPv6 off.
    pub(crate) fn build(inputs: SpecInputs<'_>) -> Self {
        let mut command = vec![inputs.guest_bin.display().to_string()];
        command.extend(inputs.args.iter().cloned());

        let mounts = inputs
            .mounts
            .iter()
            .map(|m| OciMount {
                destination: m.guest.display().to_string(),
                kind: "bind".to_string(),
                source: m.host.display().to_string(),
                options: vec![
                    "rbind".to_string(),
                    if m.read_only { "ro" } else { "rw" }.to_string(),
                ],
            })
            .collect();

        let cpu = inputs.limits.get("cores").map(|cores| CpuLimit {
            period: 100_000,
            quota: (*cores as i64) * 100_000,
        });
        let memory = inputs.limits.get("mem_gb").map(|gb| MemoryLimit {
            limit: (*gb as i64) * 1024 * 1024 * 1024,
        });
        let resource_limits =
            (cpu.is_some() || memory.is_some()).then_some(ResourceLimits { cpu, memory });

        let labels = inputs
            .labels
            .iter()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect();

        SpecGenerator {
            image: inputs.image.to_string(),
            command,
            work_dir: inputs.guest_workdir.display().to_string(),
            env: inputs.env.iter().cloned().collect(),
            annotations: BTreeMap::new(),
            mounts,
            netns: Namespace {
                nsmode: "pasta".to_string(),
            },
            cap_drop: vec!["NET_ADMIN".to_string(), "NET_RAW".to_string()],
            resource_limits,
            terminal: inputs.terminal,
            stdin: true,
            remove: false,
            labels,
        }
    }

    /// attach the egress-hook annotations: the marker the `--hooks-dir` hook
    /// matches, plus the run's allowed ports (its broker + node RPC). The host
    /// IP and DNS resolver are NOT passed — under pasta they are the fixed
    /// link-local defaults [`PASTA_HOST`] / [`PASTA_DNS`], which the hook uses
    /// directly, so nothing about them is host-computed. Host-side only — none
    /// of this reaches the guest.
    pub(crate) fn set_egress(&mut self, ports: &[u16]) {
        let ports = ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        self.annotations
            .insert("io.ducktape.egress".to_string(), "1".to_string());
        self.annotations
            .insert("io.ducktape.egress.ports".to_string(), ports);
    }
}

/// pasta's fixed `host.containers.internal` address — the link-local pasta maps
/// the host's loopback to. The container reaches this run's broker + node RPC
/// there, so the egress rule that allows those ports targets exactly this.
/// (Verified live on native-Linux pasta: stable across runs at `169.254.1.2`.)
pub(crate) const PASTA_HOST: &str = "169.254.1.2";

/// pasta's fixed DNS forwarder — the link-local resolver pasta injects as the
/// container's primary nameserver, forwarding to the host's real DNS. The egress
/// rule scopes DNS to THIS ip (never a blanket `:53`), so the tailnet/LAN
/// resolvers that pasta also copies into `resolv.conf` stay blocked. (Verified
/// live: primary nameserver `169.254.1.1`, name resolution works, tailnet DNS
/// dropped.)
pub(crate) const PASTA_DNS: &str = "169.254.1.1";

// ---------------------------------------------------------------------------
// the socket client
// ---------------------------------------------------------------------------

/// which multiplexed stream an attach frame carries (headless, non-tty attach).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameStream {
    Stdout,
    Stderr,
}

/// a libpod client bound to one rootless podman socket. Each call opens a fresh
/// connection (runs are infrequent; no pool needed).
#[derive(Debug, Clone)]
pub(crate) struct Podman {
    socket: PathBuf,
}

impl Podman {
    pub(crate) fn new(socket: PathBuf) -> Self {
        Self { socket }
    }

    /// create a container from `spec`; returns its id.
    pub(crate) async fn create(&self, spec: &SpecGenerator) -> Result<String, String> {
        let body = serde_json::to_vec(spec).map_err(|e| format!("encode create spec: {e}"))?;
        let resp = self
            .request("POST", &format!("{API}/containers/create"), Some(&body))
            .await?;
        resp.ok()?;
        #[derive(serde::Deserialize)]
        struct Created {
            #[serde(rename = "Id")]
            id: String,
        }
        let created: Created =
            serde_json::from_slice(&resp.body).map_err(|e| format!("decode create reply: {e}"))?;
        Ok(created.id)
    }

    pub(crate) async fn start(&self, id: &str) -> Result<(), String> {
        self.request("POST", &format!("{API}/containers/{id}/start"), None)
            .await?
            .ok()
    }

    /// wait for the container to exit; returns its exit code. libpod returns the
    /// code as a bare integer in the response body.
    pub(crate) async fn wait(&self, id: &str) -> Result<i32, String> {
        let resp = self
            .request(
                "POST",
                &format!("{API}/containers/{id}/wait?condition=exited"),
                None,
            )
            .await?;
        resp.ok()?;
        let text = String::from_utf8_lossy(&resp.body);
        text.trim()
            .parse::<i32>()
            .map_err(|e| format!("decode wait exit code {text:?}: {e}"))
    }

    pub(crate) async fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        self.request(
            "POST",
            &format!("{API}/containers/{id}/resize?w={cols}&h={rows}"),
            None,
        )
        .await?
        .ok()
    }

    pub(crate) async fn kill(&self, id: &str, signal: &str) -> Result<(), String> {
        self.request(
            "POST",
            &format!("{API}/containers/{id}/kill?signal={signal}"),
            None,
        )
        .await?
        .ok()
    }

    pub(crate) async fn remove(&self, id: &str) -> Result<(), String> {
        // force + remove volumes: teardown must not leave the container behind.
        self.request("DELETE", &format!("{API}/containers/{id}?force=true&v=true"), None)
            .await?
            .ok()
    }

    /// container ids (running or not) carrying `label` — the boot reaper's query
    /// for this node's orphaned sandbox containers. `label` is a bare key or
    /// `key=value`; libpod wants it JSON-encoded in the `filters` query.
    pub(crate) async fn list_by_label(&self, label: &str) -> Result<Vec<String>, String> {
        let filters = format!("{{\"label\":[{:?}]}}", label);
        let query = crate::podman_api::urlencode(&filters);
        let resp = self
            .request("GET", &format!("{API}/containers/json?all=true&filters={query}"), None)
            .await?;
        resp.ok()?;
        #[derive(serde::Deserialize)]
        struct Listed {
            #[serde(rename = "Id")]
            id: String,
        }
        let listed: Vec<Listed> =
            serde_json::from_slice(&resp.body).map_err(|e| format!("decode container list: {e}"))?;
        Ok(listed.into_iter().map(|c| c.id).collect())
    }

    /// attach stdin+stdout+stderr to a running container. The HTTP connection is
    /// hijacked: after the response headers, the socket carries raw bytes (tty)
    /// or Docker-multiplexed frames (non-tty). Returns the split stream.
    pub(crate) async fn attach(&self, id: &str, tty: bool) -> Result<AttachStream, String> {
        let mut stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|e| format!("connect podman socket for attach: {e}"))?;
        let path =
            format!("{API}/containers/{id}/attach?stdin=true&stdout=true&stderr=true&stream=true");
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: d\r\nConnection: Upgrade\r\nUpgrade: tcp\r\nContent-Length: 0\r\n\r\n"
        );
        stream
            .write_all(req.as_bytes())
            .await
            .map_err(|e| format!("send attach request: {e}"))?;
        stream
            .flush()
            .await
            .map_err(|e| format!("flush attach request: {e}"))?;
        let (status, leftover) = read_response_head(&mut stream).await?;
        // 101 (upgrade) is the norm; 200 also means the stream is hijacked.
        if status != 101 && status != 200 {
            return Err(format!("attach refused: HTTP {status}"));
        }
        let (read, write) = stream.into_split();
        Ok(AttachStream {
            read,
            write,
            leftover,
            tty,
        })
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        let mut stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|e| format!("connect podman socket: {e}"))?;
        let mut head = format!("{method} {path} HTTP/1.1\r\nHost: d\r\nConnection: close\r\n");
        if let Some(b) = body {
            head.push_str("Content-Type: application/json\r\n");
            head.push_str(&format!("Content-Length: {}\r\n", b.len()));
        }
        head.push_str("\r\n");
        stream
            .write_all(head.as_bytes())
            .await
            .map_err(|e| format!("send request head: {e}"))?;
        if let Some(b) = body {
            stream
                .write_all(b)
                .await
                .map_err(|e| format!("send request body: {e}"))?;
        }
        stream
            .flush()
            .await
            .map_err(|e| format!("flush request: {e}"))?;
        // Connection: close → the whole response arrives before EOF.
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .await
            .map_err(|e| format!("read response: {e}"))?;
        parse_response(&raw)
    }
}

/// a hijacked attach stream, freshly connected. Both the headless loop and an
/// interactive session need to WRITE stdin while READING output concurrently,
/// so the only thing you do with one is [`AttachStream::into_split`] it into an
/// independently-owned write half (container stdin) and read half.
pub(crate) struct AttachStream {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    /// bytes already read past the response head — the start of the raw stream.
    leftover: Vec<u8>,
    tty: bool,
}

impl AttachStream {
    /// split into the container-stdin write half (an `OwnedWriteHalf`, already
    /// `AsyncWrite`) and the output read half. The two halves are moved into
    /// separate tasks (input feed vs output pump).
    pub(crate) fn into_split(self) -> (OwnedWriteHalf, AttachReader) {
        (
            self.write,
            AttachReader {
                read: self.read,
                leftover: self.leftover,
                tty: self.tty,
            },
        )
    }
}

/// the read half of an attach stream: raw for a tty session, Docker-multiplexed
/// frames for a headless run.
pub(crate) struct AttachReader {
    read: OwnedReadHalf,
    leftover: Vec<u8>,
    tty: bool,
}

impl AttachReader {
    /// read the next raw chunk (tty session). Drains any leftover first, then
    /// the socket. `Ok(0)` is EOF (container exited / stream closed).
    pub(crate) async fn read_raw(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.leftover.is_empty() {
            let n = buf.len().min(self.leftover.len());
            buf[..n].copy_from_slice(&self.leftover[..n]);
            self.leftover.drain(..n);
            return Ok(n);
        }
        self.read.read(buf).await
    }

    /// read one demuxed frame (headless, non-tty). `None` at EOF. The Docker
    /// mux header is 8 bytes: `[stream, 0,0,0, len_be_u32]`, `stream` 1=stdout
    /// 2=stderr, followed by `len` payload bytes.
    pub(crate) async fn read_frame(&mut self) -> std::io::Result<Option<(FrameStream, Vec<u8>)>> {
        debug_assert!(!self.tty, "read_frame is for non-tty attach only");
        let mut header = [0u8; 8];
        if !self.fill(&mut header).await? {
            return Ok(None);
        }
        let stream = match header[0] {
            2 => FrameStream::Stderr,
            _ => FrameStream::Stdout, // 0 (stdin echo) and 1 both surface as stdout
        };
        let len = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let mut payload = vec![0u8; len];
        if len > 0 && !self.fill(&mut payload).await? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "attach frame truncated",
            ));
        }
        Ok(Some((stream, payload)))
    }

    /// fill `buf` completely from leftover-then-socket. Returns `Ok(false)` on a
    /// clean EOF at a boundary (no byte of `buf` read yet); errors on EOF
    /// mid-buffer (a truncated frame).
    async fn fill(&mut self, buf: &mut [u8]) -> std::io::Result<bool> {
        let mut filled = 0;
        while filled < buf.len() {
            if !self.leftover.is_empty() {
                let n = (buf.len() - filled).min(self.leftover.len());
                buf[filled..filled + n].copy_from_slice(&self.leftover[..n]);
                self.leftover.drain(..n);
                filled += n;
                continue;
            }
            let n = self.read.read(&mut buf[filled..]).await?;
            if n == 0 {
                if filled == 0 {
                    return Ok(false); // clean EOF at a frame boundary
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "attach stream truncated mid-frame",
                ));
            }
            filled += n;
        }
        Ok(true)
    }
}

/// the headless run's stdio, adapted from an attach stream so the existing
/// output loop sees ordinary `AsyncRead`/`AsyncWrite` streams: `stdin` feeds
/// container stdin, `stdout`/`stderr` are the demuxed halves. A background pump
/// task reads attach frames and forwards each to the matching duplex; it ends
/// (closing both, so the readers see EOF) when the attach stream EOFs.
pub(crate) struct HeadlessIo {
    pub(crate) stdin: OwnedWriteHalf,
    pub(crate) stdout: tokio::io::DuplexStream,
    pub(crate) stderr: tokio::io::DuplexStream,
    pub(crate) pump: tokio::task::JoinHandle<()>,
}

/// adapt an attach stream into [`HeadlessIo`] (see its doc).
pub(crate) fn headless_io(attach: AttachStream) -> HeadlessIo {
    let (stdin, reader) = attach.into_split();
    // 64 KiB matches the invoke loop's read buffer granularity; the pump never
    // blocks long because the loop drains continuously.
    let (out_w, out_r) = tokio::io::duplex(64 * 1024);
    let (err_w, err_r) = tokio::io::duplex(64 * 1024);
    let pump = tokio::spawn(pump_frames(reader, out_w, err_w));
    HeadlessIo {
        stdin,
        stdout: out_r,
        stderr: err_r,
        pump,
    }
}

async fn pump_frames(
    mut reader: AttachReader,
    mut out: tokio::io::DuplexStream,
    mut err: tokio::io::DuplexStream,
) {
    loop {
        match reader.read_frame().await {
            Ok(Some((FrameStream::Stdout, payload))) => {
                if out.write_all(&payload).await.is_err() {
                    break;
                }
            }
            Ok(Some((FrameStream::Stderr, payload))) => {
                if err.write_all(&payload).await.is_err() {
                    break;
                }
            }
            // clean EOF or a stream error both end the run's output; dropping
            // `out`/`err` here closes the duplexes so the loop reads EOF.
            Ok(None) | Err(_) => break,
        }
    }
}

// ---------------------------------------------------------------------------
// hand-rolled HTTP/1.1 response parsing
// ---------------------------------------------------------------------------

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl HttpResponse {
    /// a 2xx or bust; the error carries the body podman returned (its JSON
    /// `{"message": ...}` is the useful part).
    fn ok(&self) -> Result<(), String> {
        if (200..300).contains(&self.status) {
            Ok(())
        } else {
            Err(format!(
                "podman API {} — {}",
                self.status,
                String::from_utf8_lossy(&self.body).trim()
            ))
        }
    }
}

/// parse a complete (Connection: close) HTTP/1.1 response: status line, headers,
/// and a body that is either `Content-Length`-delimited, chunked, or read to
/// EOF.
fn parse_response(raw: &[u8]) -> Result<HttpResponse, String> {
    let split = find_double_crlf(raw).ok_or("HTTP response has no header terminator")?;
    let head = &raw[..split];
    let body = &raw[split + 4..];
    let head = std::str::from_utf8(head).map_err(|_| "non-utf8 HTTP head".to_string())?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or("empty HTTP head")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or_else(|| format!("bad HTTP status line: {status_line:?}"))?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    Ok(HttpResponse { status, body })
}

/// read just the response head from a still-open (hijacked) stream, returning
/// the status and any bytes already read past the `\r\n\r\n` (the raw stream's
/// first bytes).
async fn read_response_head(stream: &mut UnixStream) -> Result<(u16, Vec<u8>), String> {
    let mut buf = Vec::with_capacity(256);
    let mut chunk = [0u8; 256];
    loop {
        if let Some(split) = find_double_crlf(&buf) {
            let head = std::str::from_utf8(&buf[..split])
                .map_err(|_| "non-utf8 attach response head".to_string())?;
            let status = head
                .split("\r\n")
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|c| c.parse::<u16>().ok())
                .ok_or("bad attach status line")?;
            let leftover = buf[split + 4..].to_vec();
            return Ok((status, leftover));
        }
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|e| format!("read attach response head: {e}"))?;
        if n == 0 {
            return Err("attach connection closed before response head".to_string());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// decode HTTP/1.1 chunked transfer-encoding.
fn dechunk(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let nl = body
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("chunk size line has no CRLF")?;
        let size_str = std::str::from_utf8(&body[..nl]).map_err(|_| "non-utf8 chunk size")?;
        // a chunk-size line may carry `;ext`; the size is the leading hex.
        let hex = size_str.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(hex, 16).map_err(|_| format!("bad chunk size {hex:?}"))?;
        body = &body[nl + 2..];
        if size == 0 {
            break;
        }
        if body.len() < size + 2 {
            return Err("chunk body truncated".to_string());
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..]; // skip the chunk's trailing CRLF
    }
    Ok(out)
}

/// percent-encode a query-parameter value (the libpod `filters` JSON). Encodes
/// everything that is not an unreserved character — enough for the JSON blobs
/// the list filter needs.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for b in value.bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// PodmanService — the node-private rootless podman the sandbox drives
// ---------------------------------------------------------------------------

/// the node's OWN rootless podman: a dedicated socket, storage root, and OCI
/// hooks dir, supervised as a child `podman system service`. This is what keeps
/// the sandbox from colliding with any other podman on the host — the operator's
/// own `podman`, another ducktape node, a system service. Nothing here is
/// shared:
/// - a private **socket** (`<data>/podman.sock`), never the default user socket;
/// - a private **storage root** (`--root <data>/storage`), so this node's
///   containers/images are a separate world an operator `podman system prune`
///   cannot touch, and two nodes never fight over one store;
/// - a private **hooks dir** carrying only the egress hook, passed on THIS
///   service's argv — so the firewall applies solely to containers created
///   through this socket and never alters the operator's other podman use.
///
/// Dropping the handle stops the service child (its containers die with it via
/// each run's own teardown; the boot reaper sweeps any that outlive a crash).
pub struct PodmanService {
    socket: PathBuf,
    child: tokio::process::Child,
}

impl PodmanService {
    /// the node-private socket path under `data_dir` — deterministic, so the
    /// config layer can name the [`SandboxBackend::Podman`] socket the boot
    /// layer will serve on, with no handshake between them.
    pub fn socket_path(data_dir: &Path) -> PathBuf {
        data_dir.join("podman").join("podman.sock")
    }

    /// start the service for a resolved [`crate::SandboxBackend`] — a no-op
    /// (returns `None`) for a non-Podman backend, so a boot path can call this
    /// unconditionally and hold the returned guard for the node's lifetime. The
    /// `data_dir` is derived from the backend's socket path (which
    /// [`Self::socket_path`] built), and `podman` + `self_exe` are resolved
    /// here. A start failure is fatal (fail-closed: no firewall, no runs).
    pub async fn start_for(
        backend: &crate::SandboxBackend,
        self_exe: &Path,
    ) -> Result<Option<Self>, String> {
        let crate::SandboxBackend::Podman { socket, .. } = backend else {
            return Ok(None);
        };
        // socket = <data_dir>/podman/podman.sock → data_dir is two parents up.
        let data_dir = socket
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| format!("podman socket {} has no node data dir", socket.display()))?;
        let podman = find_system_tool("podman")
            .ok_or_else(|| "podman is not on PATH; the sandbox cannot start its service".to_string())?;
        Self::start(data_dir, &podman, self_exe).await.map(Some)
    }

    /// start the node-private service under `data_dir`, writing the egress hook
    /// JSON that points back at `self_exe __egress-hook`. Idempotent per node:
    /// a stale socket file is removed first. `podman_bin` is the resolved
    /// runtime (from [`SandboxBackend::probe`]).
    pub async fn start(
        data_dir: &Path,
        podman_bin: &Path,
        self_exe: &Path,
    ) -> Result<Self, String> {
        let root = data_dir.join("podman");
        let storage = root.join("storage");
        let runroot = root.join("run");
        let hooks_dir = root.join("hooks");
        let socket = root.join("podman.sock");
        for dir in [&storage, &runroot, &hooks_dir] {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("podman service: create {}: {e}", dir.display()))?;
        }
        // the hook fires only for our containers (annotation match) and only on
        // THIS service (its own --hooks-dir), so it never touches other podman.
        let hook_json = format!(
            r#"{{"version":"1.0.0","hook":{{"path":{exe:?},"args":["ducktape","__egress-hook"]}},"when":{{"annotations":{{"io.ducktape.egress":"1"}}}},"stages":["createRuntime"]}}"#,
            exe = self_exe.display().to_string(),
        );
        std::fs::write(hooks_dir.join("ducktape-egress.json"), hook_json)
            .map_err(|e| format!("podman service: write egress hook: {e}"))?;
        // a leftover socket from a previous boot would make `service` fail to bind.
        let _ = std::fs::remove_file(&socket);

        let child = tokio::process::Command::new(podman_bin)
            .arg("--root")
            .arg(&storage)
            .arg("--runroot")
            .arg(&runroot)
            .arg("--hooks-dir")
            .arg(&hooks_dir)
            .arg("system")
            .arg("service")
            .arg("--time=0") // never idle-exit; the node owns its lifetime
            .arg(format!("unix://{}", socket.display()))
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("podman service: spawn `{} system service`: {e}", podman_bin.display()))?;

        let service = Self { socket, child };
        service.await_socket().await?;
        Ok(service)
    }

    /// the node-private socket path — what [`SandboxBackend::Podman`] carries.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// a client bound to this service's socket.
    pub(crate) fn client(&self) -> Podman {
        Podman::new(self.socket.clone())
    }

    /// remove any of this node's containers left over from a previous crash —
    /// they carry the managed label. Best-effort: a reap failure is logged by
    /// the caller, never fatal (a fresh node has none).
    pub async fn reap_orphans(&self, label: &str) -> Result<usize, String> {
        let client = self.client();
        let ids = client.list_by_label(label).await?;
        let mut removed = 0;
        for id in &ids {
            if client.remove(id).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// wait until the service is answering on its socket (bounded). The service
    /// child creates the socket asynchronously after spawn; poll a cheap `_ping`
    /// until it responds rather than sleeping a fixed time.
    async fn await_socket(&self) -> Result<(), String> {
        let client = self.client();
        for _ in 0..100 {
            if UnixStream::connect(&self.socket).await.is_ok()
                && client.request("GET", "/_ping", None).await.is_ok()
            {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Err(format!(
            "podman service did not answer on {} within 5s",
            self.socket.display()
        ))
    }

    /// stop the service child (best-effort; `kill_on_drop` is the backstop).
    pub async fn shutdown(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_hides_every_host_path_behind_ducktape() {
        let home = Path::new("/home/eddy");
        let plan = plan_mounts(
            Path::new("/home/eddy/.ducktape/provider-runs/7/workspace"),
            Path::new("/usr/bin/claude"),
            &[PathBuf::from("/opt/skills")],
            &[PathBuf::from("/home/eddy/.claude")],
            home,
        );
        assert_eq!(plan.guest_workdir, Path::new("/ducktape/workspace"));
        assert_eq!(plan.guest_bin, Path::new("/ducktape/bin/claude"));
        assert_eq!(plan.guest_home, Path::new("/ducktape/home"));
        // the auth dir lands under the neutral home at its relative path.
        let claude = plan
            .mounts
            .iter()
            .find(|m| m.host == Path::new("/home/eddy/.claude"))
            .unwrap();
        assert_eq!(claude.guest, Path::new("/ducktape/home/.claude"));
        assert!(!claude.read_only);
        // NO guest path leaks a host component.
        for m in &plan.mounts {
            let g = m.guest.to_string_lossy();
            assert!(g.starts_with("/ducktape/"), "guest not neutral: {g}");
            assert!(!g.contains("eddy") && !g.contains("provider-runs"), "leak: {g}");
        }
    }

    #[test]
    fn translate_rewrites_workdir_inside_a_codex_arg_and_sanitizes_home() {
        let home = Path::new("/home/eddy");
        let guest_home = Path::new("/ducktape/home");
        let mounts = vec![Mount {
            host: PathBuf::from("/home/eddy/.ducktape/provider-runs/7/workspace"),
            guest: PathBuf::from("/ducktape/workspace"),
            read_only: false,
        }];
        let arg = "projects.\"/home/eddy/.ducktape/provider-runs/7/workspace\".trust_level=\"untrusted\"";
        let out = translate(arg, &mounts, home, guest_home);
        assert_eq!(
            out,
            "projects.\"/ducktape/workspace\".trust_level=\"untrusted\""
        );
        // a bare $HOME-prefixed value with no explicit mount is still sanitized.
        assert_eq!(
            translate("/home/eddy/.config/x", &mounts, home, guest_home),
            "/ducktape/home/.config/x"
        );
    }

    #[test]
    fn egress_ruleset_orders_allow_before_private_drop() {
        // pasta's link-local host + resolver both sit inside 169.254.0.0/16,
        // which the ruleset drops — so their specific accepts MUST come first.
        let rs = egress_nftables(PASTA_HOST, PASTA_DNS, &[45001, 8080]);
        let allow = rs.find("dport { 45001, 8080 } accept").expect("allow line");
        let dns = rs
            .find(&format!("{PASTA_DNS} udp dport 53 accept"))
            .expect("scoped dns line");
        let drop_ll = rs.find("169.254.0.0/16").expect("link-local drop line");
        assert!(allow < drop_ll, "broker allow must precede the 169.254 drop:\n{rs}");
        assert!(dns < drop_ll, "scoped DNS must precede the 169.254 drop:\n{rs}");
        assert!(rs.contains(PASTA_HOST), "pasta host ip pinned");
        // DNS is scoped to the resolver, NOT a blanket dport 53 — a bare
        // `dport 53 accept` (no daddr) would reach LAN/tailnet resolvers.
        assert!(
            !rs.lines().any(|l| l.trim() == "udp dport 53 accept"),
            "DNS must be scoped to the resolver, never universal:\n{rs}"
        );
        assert!(rs.contains("100.64.0.0/10"), "tailnet v4 blocked");
        assert!(rs.contains("ip6 daddr { fc00::/7"), "tailnet/ULA v6 blocked");
        assert!(rs.contains("oifname \"lo\" accept"));
    }

    #[test]
    fn egress_ruleset_with_no_ports_still_valid() {
        let rs = egress_nftables(PASTA_HOST, PASTA_DNS, &[]);
        assert!(!rs.contains("dport {"), "no port allow-list when no ports:\n{rs}");
        assert!(
            rs.contains(&format!("{PASTA_DNS} udp dport 53 accept")),
            "dns still scoped:\n{rs}"
        );
        assert!(rs.contains("drop"));
    }

    #[test]
    fn spec_json_has_neutral_paths_private_netns_and_dropped_caps() {
        let home = Path::new("/home/eddy");
        let plan = plan_mounts(
            Path::new("/home/eddy/.ducktape/provider-runs/7/workspace"),
            Path::new("/usr/bin/claude"),
            &[],
            &[PathBuf::from("/home/eddy/.claude")],
            home,
        );
        let mut spec = SpecGenerator::build(SpecInputs {
            image: "img",
            guest_bin: &plan.guest_bin,
            guest_workdir: &plan.guest_workdir,
            args: &["--print".to_string()],
            env: &[("HOME".to_string(), "/ducktape/home".to_string())],
            mounts: &plan.mounts,
            limits: &BTreeMap::from([("cores".to_string(), 4u64), ("mem_gb".to_string(), 8u64)]),
            labels: &["io.ducktape.run=abc".to_string()],
            terminal: false,
        });
        spec.set_egress(&[45001, 8080]);
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"work_dir\":\"/ducktape/workspace\""), "{json}");
        assert!(json.contains("/ducktape/bin/claude"), "{json}");
        assert!(json.contains("\"nsmode\":\"pasta\""), "{json}");
        assert!(json.contains("NET_ADMIN") && json.contains("NET_RAW"), "{json}");
        assert!(json.contains("\"remove\":false"), "own removal, no auto-remove: {json}");
        assert!(json.contains("\"quota\":400000"), "cpu quota: {json}");
        assert!(json.contains("io.ducktape.egress"), "egress marker annotation: {json}");
        assert!(json.contains("io.ducktape.egress.ports"), "ports annotation: {json}");
        // host + resolver are pasta constants in the hook, NOT annotations.
        assert!(!json.contains("io.ducktape.egress.host"), "no host annotation: {json}");
        assert!(!json.contains("io.ducktape.egress.resolver"), "no resolver annotation: {json}");
        // the GUEST-visible fields (command, work_dir, env, mount destinations)
        // are neutral. bind-mount `source` fields legitimately carry the host
        // path — that is the host side of the mount, never seen inside the
        // container — so the guest-facing sides are what must not leak.
        let guest_view = format!(
            "{}|{}|{:?}|{:?}",
            spec.work_dir,
            spec.command.join(" "),
            spec.env,
            spec.mounts.iter().map(|m| &m.destination).collect::<Vec<_>>()
        );
        assert!(
            !guest_view.contains("eddy") && !guest_view.contains("provider-runs"),
            "guest-visible leak: {guest_view}"
        );
    }

    #[test]
    fn parse_response_content_length() {
        let raw =
            b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"Id\":\"abc\"}\n";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 201);
        assert_eq!(r.body, b"{\"Id\":\"abc\"}\n");
    }

    #[test]
    fn parse_response_chunked() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"hello world");
    }

    #[test]
    fn parse_response_error_surfaces_body() {
        let raw = b"HTTP/1.1 404 Not Found\r\nContent-Length: 26\r\n\r\n{\"message\":\"no such ctr\"}\n";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 404);
        let err = r.ok().unwrap_err();
        assert!(err.contains("404") && err.contains("no such ctr"), "{err}");
    }

    #[tokio::test]
    async fn attach_demuxes_stdout_and_stderr_frames() {
        // a real UnixStream pair: writer pushes Docker-mux frames, reader demuxes.
        let (c1, c2) = UnixStream::pair().unwrap();
        let (r, w) = c1.into_split();
        let mut att = AttachReader {
            read: r,
            leftover: Vec::new(),
            tty: false,
        };
        let _keep_write = w;
        // writer side pushes two frames then closes.
        let (_r2, mut w2) = c2.into_split();
        let mut wire = Vec::new();
        // stdout "hi"
        wire.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 2]);
        wire.extend_from_slice(b"hi");
        // stderr "!"
        wire.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 1]);
        wire.extend_from_slice(b"!");
        w2.write_all(&wire).await.unwrap();
        drop(w2);

        let f1 = att.read_frame().await.unwrap().unwrap();
        assert_eq!(f1, (FrameStream::Stdout, b"hi".to_vec()));
        let f2 = att.read_frame().await.unwrap().unwrap();
        assert_eq!(f2, (FrameStream::Stderr, b"!".to_vec()));
        assert!(att.read_frame().await.unwrap().is_none(), "EOF after frames");
    }
}
