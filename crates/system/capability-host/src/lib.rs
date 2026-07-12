//! host-side capability providers — the I/O half of the capability seam.
//!
//! the capability module (consensus) replicates *who provides what*; this
//! crate is the machine-local counterpart that actually provides it. a
//! [`Provider`] wraps one locally installed executor CLI, and [`discover`]
//! probes the host for the executors the operator brought — BYO by
//! construction: provider children never receive host credentials. Built-in
//! Codex runs authenticate through a run-scoped loopback broker owned by this
//! host process; other providers must add an equivalent broker before opting
//! into the clean process boundary.
//!
//! ## executors are data: the capability spec
//!
//! WHICH executors exist, how to detect them, the argv to run them, and how
//! to parse their output is all described by TOML capability specs (see
//! [`spec`] and `docs/records/specs/capability-spec.md`), not by Rust. the built-in
//! executor support ships as embedded spec files parsed by the same code
//! path as operator-provided specs under `$DUCKTAPE_CAPABILITY_DIR` (default
//! `~/.ducktape/capabilities`). adding an executor — or retuning a built-in's
//! flags, including which model it runs — is a config change on the
//! operator's machine, never a code change here. dispatch is by EXPLICIT
//! capability tag: [`ProviderSet::resolve`] takes the tag a job names,
//! nothing is inferred from model names.
//!
//! the CLIs are agentic, not plain inference endpoints, so a provider runs
//! them fenced: non-interactive mode, the sandbox flags encoded in the spec's
//! argv, and a working directory the spec's `[workspace]` policy picks — an
//! empty scratch dir by default, or a stable per-agent workspace under the
//! host's agent-workspaces root when the spec opts in (see [`workspace`]).
//! either way the child never sees the node's data directory itself. the
//! spec's `[session]` policy adds host-local thread continuity on top (see
//! [`session`]): capture the CLI's own session id, resume it for the next
//! run of the same thread.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// Reserved run-local state. Workspace committers remove this directory before
/// scanning outputs, so provider auth/cache/temp files can never become an
/// agent artifact.
pub const RUN_RUNTIME_DIR: &str = ".ducktape-run";

/// the hard ceiling on one child's lifetime, as a multiple of its idle
/// timeout: `spec.timeout_secs` bounds SILENCE (any output refreshes it —
/// long agentic runs that keep streaming are never killed mid-work), and
/// `idle × this` bounds even a continuously-chatty child, guarding the
/// host's own resources. the RUN's committed outcome is bounded by the
/// saga's consensus deadline regardless (ADR X3) — this factor only decides
/// how long this host keeps paying for one child.
const HARD_TIMEOUT_FACTOR: u32 = 36;

mod broker;
mod session;
mod spec;
mod variants;
mod workspace;
pub use session::{ResumeArgv, SessionCapture, SessionSpec};
pub use spec::{CapabilitySpec, OutputFormat, SpecSet};
pub use workspace::WorkspaceMode;

/// per-run, host-local context riding beside the prompt: which agent is
/// running and which conversation thread the run continues. populated by the
/// worker from the run envelope; legacy envelope-less runs pass
/// [`RunContext::default`] and behave exactly as before. NEVER consensus
/// data — providers only use it to pick a workspace dir and a session slot
/// on this machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunContext {
    pub agent_id: Option<String>,
    pub thread_key: Option<String>,
    /// the live-output registry key (the dispatch_id half of the saga id) —
    /// set by the oracle pool before provider.run so the output sink can key
    /// a per-run ring the app subscribes as run-output:<dispatch_id>.
    pub run_key: Option<String>,
    /// an already-materialized workspace this specific run must execute in.
    /// set only by the provisioning wrapper (`dispatch-oracle::bind_workspace`)
    /// after a successful per-run duckfs checkout — never a consensus-supplied
    /// path (D7). when set, the provider's cwd is the evidence-backed
    /// workspace; an unusable mount fails the run (W1), never falls back to
    /// the shared scratch dir.
    pub workdir_override: Option<PathBuf>,
    /// run-scoped environment variables for host-provided tool bindings.
    /// these are additive to the process environment and apply only to the
    /// spawned provider child.
    pub env: BTreeMap<String, String>,
    /// path entries prepended to `PATH` for run-scoped tool bindings.
    pub path_entries: Vec<PathBuf>,
    /// true for portable v3 runs: native CLI sessions are host-local
    /// optimizations and must not be resumed or captured for portable state.
    pub portable: bool,
}

/// which child stream produced one live output line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// one line observed from a provider child as it arrives. `line` does not
/// include the trailing newline, matching `BufReader::lines()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    pub stream: OutputStream,
    pub line: String,
}

/// provider-reported token counters. cached/reasoning values are subsets of
/// input/output, so totals add only the two top-level fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOutput {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

/// optional live-tail callback for provider output. The run context is passed
/// beside each line so embedders can key their own per-run registry with the
/// host-local identity available to capability-host.
pub type OutputSink = Arc<dyn Fn(&RunContext, OutputLine) + Send + Sync>;

/// a machine-local executor for one capability tag. implementations do real
/// I/O (spawn processes); nothing consensus-side may ever hold one. the
/// input is just the fully rendered prompt — everything else about the
/// invocation (binary, flags, model) is the spec's literal argv. rendering
/// (conversation -> text) is the CALLER's business — this crate is
/// deliberately ignorant of chat shapes and saga specs.
///
/// `Send + Sync` (and a Send `run` future) on purpose: provider execution is
/// long (a CLI call can run minutes) and hosts run it on SPAWNED background
/// tasks, never inline on their event loop — so the provider surface must be
/// shareable across tasks. this is a Rust seam bound only; the provider CLI
/// contract (spec argv, stdin prompt, stdout answer) is untouched.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// the capability tag this provider serves — matches the capability
    /// module's registry entries, so "what i can run" and "what i announce"
    /// cannot drift apart.
    fn capability(&self) -> &str;
    /// run one prompt to completion and return the assistant's final text.
    /// `ctx` is host-local run identity (see [`RunContext`]) — implementations
    /// that predate workspaces/sessions may simply ignore it.
    async fn run(&self, prompt: &str, ctx: &RunContext) -> Result<String, String>;
    /// the same run plus optional executor-reported usage. legacy/custom
    /// providers inherit the text-only default without changing their API.
    async fn run_with_usage(
        &self,
        prompt: &str,
        ctx: &RunContext,
    ) -> Result<ProviderOutput, String> {
        self.run(prompt, ctx)
            .await
            .map(|text| ProviderOutput { text, usage: None })
    }
}

/// the host's provider surface: every LOADED spec (for routing — an
/// uninstalled capability still routes, so errors can name it) plus the
/// DISCOVERED providers (for execution — exactly what this node announces).
pub struct ProviderSet {
    specs: SpecSet,
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderSet {
    pub fn assemble(specs: SpecSet, providers: Vec<Box<dyn Provider>>) -> Self {
        Self { specs, providers }
    }

    /// no specs, no providers — the "nothing installed, nothing loaded" test
    /// seam. every resolve() fails with a clean error.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            specs: SpecSet::from_specs(Vec::new()),
            providers: Vec::new(),
        }
    }

    fn find(&self, capability: &str) -> Option<&dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.capability() == capability)
            .map(Box::as_ref)
    }

    /// the sorted tag list of DISCOVERED providers — the truthful payload for
    /// a capability announcement.
    pub fn capabilities(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .providers
            .iter()
            .map(|p| p.capability().to_string())
            .collect();
        tags.sort();
        tags
    }

    /// capability-tag resolution, the one entry point callers should use.
    /// dispatch is explicit: the tag arrives with the job, nothing is
    /// inferred. each failure is a distinct, actionable error — a tag no
    /// loaded spec claims (likely an operator typo somewhere), or a
    /// capability this node knows of but does not provide.
    pub fn resolve(&self, capability: &str) -> Result<&dyn Provider, String> {
        if self.specs.get(capability).is_none() {
            let loaded: Vec<&str> = self.specs.iter().map(|s| s.tag.as_str()).collect();
            return Err(format!(
                "no capability spec is loaded for tag {capability:?}; loaded specs: {loaded:?}"
            ));
        }
        let Some(provider) = self.find(capability) else {
            return Err(format!(
                "capability {capability:?} is not provided by this node; \
                 this node provides {:?}",
                self.capabilities()
            ));
        };
        Ok(provider)
    }
}

/// the host-wired roots for per-agent state (see [`workspace`] and
/// [`session`]): where persistent agent workspaces and session files live.
/// binaries derive both from their data dir ([`AgentDirs::under`]);
/// `DUCKTAPE_AGENT_WORKSPACES` / `DUCKTAPE_AGENT_SESSIONS` override either
/// (the `DUCKTAPE_PROVIDER_TIMEOUT_SECS` precedent). an absent root simply
/// disables the feature it serves — embedders that never wire one keep the
/// v1 scratch-and-cold behavior.
#[derive(Debug, Clone, Default)]
pub struct AgentDirs {
    pub workspaces_root: Option<PathBuf>,
    pub sessions_root: Option<PathBuf>,
}

impl AgentDirs {
    /// the binaries' convention: both roots under one data dir.
    pub fn under(data_dir: &Path) -> Self {
        Self {
            workspaces_root: Some(data_dir.join("agent-workspaces")),
            sessions_root: Some(data_dir.join("agent-sessions")),
        }
    }

    /// apply the env overrides — injected like every other env access here,
    /// so tests never mutate process state.
    fn resolved(self, env: &dyn Fn(&str) -> Option<OsString>) -> Self {
        let over = |key: &str, wired: Option<PathBuf>| env(key).map(PathBuf::from).or(wired);
        Self {
            workspaces_root: over("DUCKTAPE_AGENT_WORKSPACES", self.workspaces_root),
            sessions_root: over("DUCKTAPE_AGENT_SESSIONS", self.sessions_root),
        }
    }
}

/// a [`Provider`] that interprets one [`CapabilitySpec`] against one resolved
/// binary: spawn `bin` with the spec's literal argv, feed the prompt on
/// stdin, parse stdout with the spec's named format.
pub(crate) struct CliProvider {
    spec: CapabilitySpec,
    bin: PathBuf,
    /// the child's DEFAULT working directory — an empty scratch dir, never
    /// the node's data directory, so an agentic CLI has nothing to wander
    /// into. a spec's `[workspace] mode = "persistent"` swaps this for a
    /// per-agent dir under `dirs.workspaces_root` when the run carries an
    /// agent id.
    workdir: PathBuf,
    /// the IDLE window, not a wall clock: any child output refreshes it, so
    /// a streaming agentic run outlives it freely; only silence this long
    /// kills the child. `idle × HARD_TIMEOUT_FACTOR` is the absolute cap.
    timeout: Duration,
    /// host-wired roots for persistent workspaces and session files; both
    /// default absent (scratch + cold runs).
    dirs: AgentDirs,
    /// optional live per-line output sink. `None` means no output lines are
    /// forwarded; stdout/stderr are still accumulated for the existing parse
    /// and error contracts.
    output_sink: Option<OutputSink>,
}

impl CliProvider {
    /// the general constructor: any spec, any resolved binary. the timeout
    /// starts at the spec's `timeout_secs`.
    pub fn from_spec(spec: CapabilitySpec, bin: PathBuf) -> Self {
        let workdir = std::env::temp_dir().join(format!(
            "ducktape-provider-{}-{}",
            spec.tag,
            std::process::id()
        ));
        let timeout = Duration::from_secs(spec.timeout_secs);
        Self {
            spec,
            bin,
            workdir,
            timeout,
            dirs: AgentDirs::default(),
            output_sink: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_workdir(mut self, workdir: PathBuf) -> Self {
        self.workdir = workdir;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_agent_dirs(mut self, dirs: AgentDirs) -> Self {
        self.dirs = dirs;
        self
    }

    pub fn with_output_sink(mut self, output_sink: OutputSink) -> Self {
        self.output_sink = Some(output_sink);
        self
    }

    fn command(
        &self,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        isolation: Option<&RunIsolation>,
        broker: Option<&broker::BrokerEndpoint>,
    ) -> Result<tokio::process::Command, String> {
        let mut cmd = tokio::process::Command::new(&self.bin);
        // argv straight from the spec, fully literal (the resume path's
        // {session_id} slot is substituted host-side BEFORE this point, with
        // an id the executor itself minted — never job content). args are
        // passed verbatim to exec — never shell-interpreted, so nothing in a
        // job can inject flags or commands.
        if let (Some(selector), Some(broker)) = (args.first(), broker) {
            // A custom Responses provider points Codex at the host-owned
            // loopback broker. The random bearer scopes requests to this run;
            // it is not the operator's credential and cannot recover it.
            let project_key =
                toml::Value::String(workdir.to_string_lossy().into_owned()).to_string();
            cmd.arg(selector)
                .args([
                    "-c".to_string(),
                    format!(
                        "model_providers.ducktape={{ name=\"Ducktape run broker\", base_url=\"{}\", wire_api=\"responses\", env_key=\"DUCKTAPE_MODEL_BROKER_TOKEN\", request_max_retries=0, stream_max_retries=0 }}",
                        broker.base_url
                    ),
                    "-c".to_string(),
                    "model_provider=\"ducktape\"".to_string(),
                    "-c".to_string(),
                    format!("projects.{project_key}.trust_level=\"untrusted\""),
                ])
                .args(args.iter().skip(1));
        } else {
            cmd.args(args.iter());
        }
        cmd.current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // dropping the wait future (timeout) must kill the child — a hung
            // CLI never outlives its job.
            .kill_on_drop(true);
        if let Some(isolation) = isolation {
            // Agent runs are a clean process boundary. In particular this drops
            // operator HOME/CODEX_HOME, cloud/Git credentials, hooks, plugins,
            // memory settings and unrelated MCP discovery. The small inherited
            // set is non-secret process/tool plumbing. Real provider auth is
            // held only by the host-side broker, never copied into this tree.
            cmd.env_clear();
            for key in [
                "LANG",
                "LC_ALL",
                "TERM",
                "COLORTERM",
                "NO_COLOR",
                "SSL_CERT_FILE",
                "SSL_CERT_DIR",
                "NIX_SSL_CERT_FILE",
                "SYSTEMROOT",
                "COMSPEC",
                "PATHEXT",
            ] {
                if let Some(value) = std::env::var_os(key) {
                    cmd.env(key, value);
                }
            }
            for (key, value) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("LC_") {
                    cmd.env(key, value);
                }
            }
            // rustup's toolchain store contains binaries, not application
            // credentials. Keep it reachable while CARGO_HOME itself is fresh
            // and run-local, so registry tokens/global cargo config do not leak.
            if let Some(rustup_home) = host_rustup_home() {
                cmd.env("RUSTUP_HOME", rustup_home);
            }
            cmd.env("HOME", &isolation.home)
                .env("TMPDIR", &isolation.tmp)
                .env("TMP", &isolation.tmp)
                .env("TEMP", &isolation.tmp)
                .env("CARGO_HOME", &isolation.cargo_home)
                .env("CARGO_TARGET_DIR", &isolation.cargo_target);
            if let (Some(name), Some(config_home)) = (
                self.spec.isolation.config_home_env.as_deref(),
                isolation.config_home.as_ref(),
            ) {
                cmd.env(name, config_home);
            }
            if let Some(broker) = broker {
                cmd.env("DUCKTAPE_MODEL_BROKER_TOKEN", &broker.run_bearer);
            }
        }
        cmd.envs(ctx.env.iter());

        let mut path = ctx.path_entries.clone();
        if let Some(existing) = std::env::var_os("PATH") {
            path.extend(std::env::split_paths(&existing));
        }
        if !path.is_empty() {
            let joined = std::env::join_paths(path).map_err(|e| {
                format!(
                    "run-local PATH for {} contains an invalid path entry: {e}",
                    self.spec.tag
                )
            })?;
            cmd.env("PATH", joined);
        }
        Ok(cmd)
    }

    /// where this run's child executes: the per-agent persistent workspace
    /// when the spec opts in AND the run names an agent AND the host wired a
    /// root — the scratch default otherwise. the agent id is defensively
    /// checked as a path component even though registry caps bound it.
    fn workdir_for(&self, ctx: &RunContext) -> Result<PathBuf, String> {
        if let Some(workdir) = &ctx.workdir_override {
            return Ok(workdir.clone());
        }
        if self.spec.workspace == WorkspaceMode::Persistent
            && let (Some(root), Some(agent_id)) = (&self.dirs.workspaces_root, &ctx.agent_id)
        {
            workspace::safe_path_component(agent_id)?;
            return Ok(root.join(agent_id));
        }
        Ok(self.workdir.clone())
    }

    /// resolve the run's cwd to a per-run WRITABLE directory, creating it.
    ///
    /// a `workdir_override` is a workspace the provisioner ALREADY materialized
    /// (the only setter is `dispatch-oracle`'s `bind_workspace`, after a
    /// successful checkout — the envelope itself carries no host path, D7). if
    /// creating it fails, the run must FAIL so the saga can retry elsewhere:
    /// falling back would silently execute the run in `self.workdir` — a single
    /// dir shared by every run of this capability tag on the node, so
    /// concurrent runs would collide and the workspace commit would read the
    /// untouched real mount and report a clean tree (W1 violation, masked).
    ///
    /// the persistent per-agent choice keeps its scratch fallback: it may sit
    /// on a read-only volume, and those runs never promised a workspace.
    fn ensure_writable_workdir(&self, ctx: &RunContext) -> Result<PathBuf, String> {
        let preferred = self.workdir_for(ctx)?;
        match std::fs::create_dir_all(&preferred) {
            Ok(()) => Ok(preferred),
            Err(e) if ctx.workdir_override.is_some() => Err(format!(
                "provisioned workspace mount {} is unusable: {e}; refusing the \
                 shared scratch fallback for a portable run (W1)",
                preferred.display()
            )),
            Err(_) if preferred != self.workdir => {
                std::fs::create_dir_all(&self.workdir).map_err(|e| {
                    format!(
                        "provider workdir {} is unusable and the scratch fallback \
                         {} could not be created: {e}",
                        preferred.display(),
                        self.workdir.display()
                    )
                })?;
                Ok(self.workdir.clone())
            }
            Err(e) => Err(format!("provider workdir {}: {e}", preferred.display())),
        }
    }

    /// the session slot for this run — `Some` only when the spec opts into
    /// `[session]`, the run carries both continuity coordinates, and the
    /// host wired a sessions root. anything less runs cold, by design.
    fn session_store<'a>(
        &'a self,
        ctx: &'a RunContext,
    ) -> Result<Option<(&'a SessionSpec, session::SessionStore<'a>)>, String> {
        if ctx.portable {
            return Ok(None);
        }
        let Some(session) = &self.spec.session else {
            return Ok(None);
        };
        let (Some(root), Some(agent_id), Some(thread_key)) =
            (&self.dirs.sessions_root, &ctx.agent_id, &ctx.thread_key)
        else {
            return Ok(None);
        };
        workspace::safe_path_component(agent_id)?;
        Ok(Some((
            session,
            session::SessionStore::new(root, agent_id, thread_key),
        )))
    }
}

/// one finished invocation: the parsed answer plus the raw stdout the
/// session capture reads (the session id is a sibling of the answer in the
/// CLI's output, not part of it).
struct Invocation {
    text: String,
    usage: Option<TokenUsage>,
    stdout: String,
}

/// Fresh filesystem/process homes for one provider run. This lives below the
/// materialized workspace so it is disk-backed with the checkout; the
/// provisioner's commit bracket removes [`RUN_RUNTIME_DIR`] before scanning
/// agent outputs.
struct RunIsolation {
    home: PathBuf,
    config_home: Option<PathBuf>,
    tmp: PathBuf,
    cargo_home: PathBuf,
    cargo_target: PathBuf,
}

fn host_rustup_home() -> Option<PathBuf> {
    std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")))
        .filter(|path| path.is_dir())
}

fn runtime_slot(ctx: &RunContext, workdir: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(ctx.run_key.as_deref().unwrap_or("unkeyed").as_bytes());
    digest.update([0]);
    digest.update(ctx.agent_id.as_deref().unwrap_or("agent").as_bytes());
    digest.update([0]);
    digest.update(workdir.as_os_str().to_string_lossy().as_bytes());
    digest
        .finalize()
        .iter()
        .take(12)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn create_private_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("create isolated provider directory {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|e| {
            format!(
                "restrict isolated provider directory {} permissions: {e}",
                path.display()
            )
        })?;
    }
    Ok(())
}

impl CliProvider {
    fn prepare_isolation(
        &self,
        workdir: &Path,
        ctx: &RunContext,
    ) -> Result<Option<RunIsolation>, String> {
        // Context-less legacy probes/helpers are not agent jobs. Every real
        // Ducktape agent envelope carries agent_id and gets the clean boundary,
        // portable or resident/chat alike.
        if ctx.agent_id.is_none() {
            return Ok(None);
        }
        let root = workdir
            .join(RUN_RUNTIME_DIR)
            .join(runtime_slot(ctx, workdir));
        let home = root.join("home");
        let tmp = root.join("tmp");
        let cargo_home = root.join("cargo-home");
        let cargo_target = root.join("cargo-target");
        for dir in [&home, &tmp, &cargo_home, &cargo_target] {
            create_private_dir(dir)?;
        }
        let config_home = self
            .spec
            .isolation
            .config_home_env
            .as_ref()
            .map(|_| root.join("provider-config"));
        if let Some(config_home) = &config_home {
            create_private_dir(config_home)?;
        }

        Ok(Some(RunIsolation {
            home,
            config_home,
            tmp,
            cargo_home,
            cargo_target,
        }))
    }
}

/// append one raw chunk to `pending` and forward every newline-completed
/// line to the sink (strips `\n`/`\r\n`, lossy on invalid utf-8 — matching
/// the final-output accumulation). a no-op without a sink.
fn forward_lines(
    pending: &mut Vec<u8>,
    chunk: &[u8],
    stream: OutputStream,
    sink: &Option<OutputSink>,
    ctx: &RunContext,
) {
    let Some(sink) = sink else { return };
    pending.extend_from_slice(chunk);
    while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
        let mut line: Vec<u8> = pending.drain(..=pos).collect();
        line.pop(); // the newline
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        sink(
            ctx,
            OutputLine {
                stream,
                line: String::from_utf8_lossy(&line).into_owned(),
            },
        );
    }
}

/// the stream closed: a trailing newline-less line still reaches the sink.
fn flush_pending_line(
    pending: &mut Vec<u8>,
    stream: OutputStream,
    sink: &Option<OutputSink>,
    ctx: &RunContext,
) {
    let Some(sink) = sink else {
        pending.clear();
        return;
    };
    if pending.is_empty() {
        return;
    }
    let line = std::mem::take(pending);
    sink(
        ctx,
        OutputLine {
            stream,
            line: String::from_utf8_lossy(&line).into_owned(),
        },
    );
}

impl CliProvider {
    /// one child process, start to parsed answer, with an explicit argv and
    /// working directory — the shared engine under the cold and resume paths.
    async fn invoke(
        &self,
        prompt: &str,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        isolation: Option<&RunIsolation>,
        broker: Option<&broker::BrokerEndpoint>,
    ) -> Result<Invocation, String> {
        let mut child = self
            .command(args, workdir, ctx, isolation, broker)?
            .spawn()
            .map_err(|e| format!("spawn {} failed: {e}", self.bin.display()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "child stdin was not piped".to_string())?;
        let mut stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| "child stdout was not piped".to_string())?;
        let mut stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| "child stderr was not piped".to_string())?;

        // feed the prompt CONCURRENTLY with collecting output: a prompt larger
        // than the pipe buffer would deadlock a sequential write-then-wait if
        // the CLI streams output before draining stdin.
        let feed = async {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
            drop(stdin); // EOF: the prompt is complete
            Ok::<(), std::io::Error>(())
        };
        // the live tail rides the SAME chunk reads the refreshable timeout
        // watches: complete lines forward to the sink as they arrive, a
        // trailing partial line flushes when its stream closes.
        let output_sink = self.output_sink.clone();
        let mut out_pending: Vec<u8> = Vec::new();
        let mut err_pending: Vec<u8> = Vec::new();

        // REFRESHABLE timeout: `self.timeout` is an IDLE window, not a wall
        // clock — any child output (either stream) refreshes it, so a
        // long-running agentic loop that keeps streaming events (codex --json)
        // or emitting tool output is never killed mid-work; only a SILENT
        // child dies at the window. a CLI that is quiet by design (claude -p
        // prints one result object at the end) keeps exactly the old
        // semantics: its silence budget is the spec's timeout. the hard
        // ceiling ([`HARD_TIMEOUT_FACTOR`] × idle) guards this host's
        // resources against a chatty-forever child; the RUN's outcome is
        // bounded by the saga's consensus deadline regardless (ADR X3).
        let idle = self.timeout;
        let hard = tokio::time::Instant::now() + idle.saturating_mul(HARD_TIMEOUT_FACTOR);
        let mut feed = std::pin::pin!(feed);
        let mut fed: Option<Result<(), std::io::Error>> = None;
        let mut out_bytes: Vec<u8> = Vec::new();
        let mut err_bytes: Vec<u8> = Vec::new();
        let (mut out_open, mut err_open) = (true, true);
        let mut obuf = [0u8; 8192];
        let mut ebuf = [0u8; 8192];
        let mut last_activity = tokio::time::Instant::now();
        while out_open || err_open {
            let deadline = (last_activity + idle).min(hard);
            tokio::select! {
                r = &mut feed, if fed.is_none() => fed = Some(r),
                r = stdout_pipe.read(&mut obuf), if out_open => match r {
                    Ok(0) => {
                        out_open = false;
                        flush_pending_line(&mut out_pending, OutputStream::Stdout, &output_sink, ctx);
                    }
                    Ok(n) => {
                        out_bytes.extend_from_slice(&obuf[..n]);
                        forward_lines(&mut out_pending, &obuf[..n], OutputStream::Stdout, &output_sink, ctx);
                        last_activity = tokio::time::Instant::now();
                    }
                    Err(e) => {
                        return Err(format!(
                            "reading {} stdout failed: {e}",
                            self.bin.display()
                        ));
                    }
                },
                r = stderr_pipe.read(&mut ebuf), if err_open => match r {
                    Ok(0) => {
                        err_open = false;
                        flush_pending_line(&mut err_pending, OutputStream::Stderr, &output_sink, ctx);
                    }
                    Ok(n) => {
                        err_bytes.extend_from_slice(&ebuf[..n]);
                        forward_lines(&mut err_pending, &ebuf[..n], OutputStream::Stderr, &output_sink, ctx);
                        last_activity = tokio::time::Instant::now();
                    }
                    Err(e) => {
                        return Err(format!(
                            "reading {} stderr failed: {e}",
                            self.bin.display()
                        ));
                    }
                },
                // returning drops `child` (kill_on_drop): a stalled or
                // runaway CLI never outlives its job.
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(if deadline == hard {
                        format!(
                            "{} timed out: still running at the hard cap of {:?} \
                             ({HARD_TIMEOUT_FACTOR}x the idle window; child killed)",
                            self.bin.display(),
                            idle.saturating_mul(HARD_TIMEOUT_FACTOR)
                        )
                    } else {
                        format!(
                            "{} timed out after {:?} with no output (child killed); \
                             an actively-streaming run refreshes this window",
                            self.bin.display(),
                            idle
                        )
                    });
                }
            }
        }
        // both streams closed: the child is done (or moments from it) — a
        // bounded wait, never indefinite.
        let status = tokio::time::timeout(idle, child.wait())
            .await
            .map_err(|_| {
                format!(
                    "{} closed its output but did not exit within {idle:?}",
                    self.bin.display()
                )
            })?
            .map_err(|e| format!("waiting on {} failed: {e}", self.bin.display()))?;
        // an unfinished feed at this point means the child exited without
        // draining stdin — the exit status below is the primary diagnostic.
        let fed = fed.unwrap_or(Ok(()));

        if !status.success() {
            // a failed exit is the primary diagnostic — it subsumes any
            // stdin write error (an early-exiting child EPIPEs the feed).
            let stderr = String::from_utf8_lossy(&err_bytes);
            let stdout = String::from_utf8_lossy(&out_bytes);
            let detail = if stderr.trim().is_empty() {
                excerpt(&stdout)
            } else {
                excerpt(&stderr)
            };
            return Err(format!(
                "{} exited with {}: {}",
                self.bin.display(),
                status,
                detail
            ));
        }
        if let Err(e) = fed {
            return Err(format!(
                "writing the prompt to {} failed: {e}",
                self.bin.display()
            ));
        }
        let stdout = String::from_utf8_lossy(&out_bytes).into_owned();
        let (text, usage) = match self.spec.output {
            OutputFormat::JsonlEvents => {
                (parse_jsonl_events(&stdout)?, parse_token_usage(&stdout))
            }
            OutputFormat::JsonResult => {
                (parse_json_result(&stdout)?, parse_token_usage(&stdout))
            }
            // Plain stdout is model-authored answer text, not a provider
            // telemetry envelope. Never infer usage from answer content.
            OutputFormat::Text => (parse_text_output(&stdout)?, None),
        };
        Ok(Invocation {
            text,
            usage,
            stdout,
        })
    }

    async fn run_output(&self, prompt: &str, ctx: &RunContext) -> Result<ProviderOutput, String> {
        let workdir = self.ensure_writable_workdir(ctx)?;
        let isolation = self.prepare_isolation(&workdir, ctx)?;
        let broker = if isolation.is_some() {
            match self.spec.isolation.broker {
                Some(spec::BrokerKind::CodexResponses) => Some(broker::RunBroker::start().await?),
                None => None,
            }
        } else {
            None
        };
        let endpoint = broker.as_ref().map(|broker| &broker.endpoint);

        let Some((session, store)) = self.session_store(ctx)? else {
            // no session plumbing for this run: one cold invocation.
            let run = self
                .invoke(
                    prompt,
                    &self.spec.args,
                    &workdir,
                    ctx,
                    isolation.as_ref(),
                    endpoint,
                )
                .await?;
            return Ok(ProviderOutput {
                text: run.text,
                usage: run.usage,
            });
        };

        if let Some(session_id) = store.load() {
            let argv = session::resume_argv(&self.spec.args, &session.resume, &session_id);
            match self
                .invoke(
                    prompt,
                    &argv,
                    &workdir,
                    ctx,
                    isolation.as_ref(),
                    endpoint,
                )
                .await
            {
                Ok(run) => {
                    // re-capture on success: a CLI that rotates ids on
                    // resume stays resumable next time.
                    store.store_captured(&session.capture, &run.stdout);
                    return Ok(ProviderOutput {
                        text: run.text,
                        usage: run.usage,
                    });
                }
                Err(e) => {
                    // a stale/expired session must degrade to a cold start,
                    // never break the agent: forget it and retry ONCE below.
                    // (if the failure was not the session's fault, the cold
                    // retry fails the same way and that error is reported.)
                    eprintln!(
                        "[capability-host] {}: resumed session {session_id} failed ({e}); \
                         retrying cold",
                        self.spec.tag
                    );
                    store.forget();
                }
            }
        }
        let run = self
            .invoke(
                prompt,
                &self.spec.args,
                &workdir,
                ctx,
                isolation.as_ref(),
                endpoint,
            )
            .await?;
        store.store_captured(&session.capture, &run.stdout);
        Ok(ProviderOutput {
            text: run.text,
            usage: run.usage,
        })
    }
}

#[async_trait::async_trait]
impl Provider for CliProvider {
    fn capability(&self) -> &str {
        &self.spec.tag
    }

    async fn run(&self, prompt: &str, ctx: &RunContext) -> Result<String, String> {
        self.run_output(prompt, ctx).await.map(|output| output.text)
    }

    async fn run_with_usage(
        &self,
        prompt: &str,
        ctx: &RunContext,
    ) -> Result<ProviderOutput, String> {
        self.run_output(prompt, ctx).await
    }
}

/// the JSON objects of a JSONL-ish stream: trimmed lines that parse; non-json
/// noise skipped. shared by the output parser and the session capture.
pub(crate) fn json_lines(stdout: &str) -> impl Iterator<Item = Value> + '_ {
    stdout.lines().filter_map(|line| {
        let line = line.trim();
        if !line.starts_with('{') {
            return None;
        }
        serde_json::from_str::<Value>(line).ok()
    })
}

/// the candidate `{"type":"result",..}` objects of a single-JSON-result print
/// mode: the whole trimmed output first, then per-line for robustness against
/// banner noise. shared by the output parser and the session capture.
pub(crate) fn result_objects(stdout: &str) -> impl Iterator<Item = Value> + '_ {
    std::iter::once(stdout.trim())
        .chain(stdout.lines().rev().map(str::trim))
        .filter_map(|candidate| serde_json::from_str::<Value>(candidate).ok())
        .filter(|v| v.get("type").and_then(Value::as_str) == Some("result"))
}

fn token_usage_from(value: &Value) -> Option<TokenUsage> {
    let usage = value.get("usage")?.as_object()?;
    let read = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
    let has_tokens = [
        "input_tokens",
        "cached_input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "output_tokens",
        "reasoning_output_tokens",
    ]
    .iter()
    .any(|key| usage.contains_key(*key));
    if !has_tokens {
        return None;
    }

    let raw_input = read("input_tokens");
    let cached_input = usage
        .get("cached_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| read("cache_read_input_tokens"));
    let cache_write = read("cache_creation_input_tokens");
    // Codex reports cached_input_tokens as a subset of input_tokens. Claude's
    // cache read/write counters are additional to its input_tokens field.
    let input_tokens = if usage.contains_key("cached_input_tokens") {
        raw_input
    } else {
        raw_input
            .saturating_add(cached_input)
            .saturating_add(cache_write)
    };
    Some(TokenUsage {
        input_tokens,
        cached_input_tokens: cached_input,
        cache_write_input_tokens: cache_write,
        output_tokens: read("output_tokens"),
        reasoning_output_tokens: read("reasoning_output_tokens"),
    })
}

fn parse_token_usage(stdout: &str) -> Option<TokenUsage> {
    json_lines(stdout)
        .filter_map(|value| token_usage_from(&value))
        .last()
        .or_else(|| {
            result_objects(stdout)
                .filter_map(|value| token_usage_from(&value))
                .next()
        })
}

/// the LAST agent message in a JSONL event stream. tolerant of
/// the two shapes the CLI has shipped (item events with `type` or `item_type`,
/// and the older `msg` envelope) and of non-json noise lines; anything else is
/// an explicit error carrying an output excerpt, never a silent empty answer.
fn parse_jsonl_events(stdout: &str) -> Result<String, String> {
    let mut last: Option<String> = None;
    for v in json_lines(stdout) {
        if let Some(item) = v.get("item") {
            let kind = item
                .get("type")
                .or_else(|| item.get("item_type"))
                .and_then(Value::as_str);
            if kind == Some("agent_message")
                && let Some(text) = item.get("text").and_then(Value::as_str)
            {
                last = Some(text.to_string());
            }
        }
        if let Some(msg) = v.get("msg")
            && msg.get("type").and_then(Value::as_str) == Some("agent_message")
            && let Some(text) = msg.get("message").and_then(Value::as_str)
        {
            last = Some(text.to_string());
        }
    }
    last.ok_or_else(|| {
        format!(
            "executor event stream carried no agent message: {}",
            excerpt(stdout)
        )
    })
}

/// the result text of a single-JSON-result print mode: one result object,
/// whole-output first, then per-line for robustness against banner noise. an
/// `is_error` result is surfaced as the error it is.
fn parse_json_result(stdout: &str) -> Result<String, String> {
    for v in result_objects(stdout) {
        if v.get("is_error").and_then(Value::as_bool) == Some(true) {
            return Err(format!(
                "executor reported an error result: {}",
                excerpt(&v.to_string())
            ));
        }
        if let Some(text) = v.get("result").and_then(Value::as_str) {
            return Ok(text.to_string());
        }
    }
    Err(format!(
        "executor output carried no result object: {}",
        excerpt(stdout)
    ))
}

/// `format = "text"`: the CLI's trimmed stdout IS the answer — the generic
/// escape hatch for wiring any plain-printing CLI with zero code. empty
/// output on a zero exit is still an error: "ran fine, said nothing" is a
/// broken executor, not an answer.
fn parse_text_output(stdout: &str) -> Result<String, String> {
    let text = stdout.trim();
    if text.is_empty() {
        return Err("executor exited successfully but produced no output".into());
    }
    Ok(text.to_string())
}

/// a bounded, char-boundary-safe slice of diagnostic output for error strings.
fn excerpt(s: &str) -> String {
    const MAX: usize = 400;
    let s = s.trim();
    if s.len() <= MAX {
        return s.to_string();
    }
    let mut end = MAX;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

// ---- discovery --------------------------------------------------------------

/// load this host's capability specs and probe for their binaries.
///
/// spec sources: the embedded built-ins, then `$DUCKTAPE_CAPABILITY_DIR`
/// (explicitly set and missing = hard error — the operator asked for a dir
/// that is not there) or `~/.ducktape/capabilities` when it exists. a broken
/// spec is a hard `Err`: an operator config error fails the boot loudly, it
/// does not silently drop an executor.
///
/// per spec: the `detect.env` override wins (broken override = loud warning +
/// absent capability), else the first executable `detect.bin` on `PATH`.
/// `DUCKTAPE_PROVIDER_TIMEOUT_SECS` overrides every spec's IDLE timeout at
/// once (refreshed by child output; see [`HARD_TIMEOUT_FACTOR`]).
/// what discovery finds is exactly what the node announces.
///
/// per-agent roots: node binaries pass `AgentDirs::under(<data dir>)`;
/// embedders with no data dir pass `AgentDirs::default()` (workspaces and
/// sessions stay off beyond env overrides). `DUCKTAPE_AGENT_WORKSPACES` /
/// `DUCKTAPE_AGENT_SESSIONS` override the wired roots. `output_sink`
/// installs a live tail on every discovered CLI provider.
pub fn discover(
    dirs: AgentDirs,
    output_sink: Option<OutputSink>,
) -> Result<ProviderSet, String> {
    let specs = SpecSet::load(operator_spec_dir().as_deref())?;
    let timeout = std::env::var("DUCKTAPE_PROVIDER_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs);
    Ok(discover_with_sink(
        specs,
        std::env::var_os("PATH"),
        &|k| std::env::var_os(k),
        timeout,
        dirs.resolved(&|k| std::env::var_os(k)),
        output_sink,
    ))
}

/// the operator spec dir: an explicit `$DUCKTAPE_CAPABILITY_DIR` is returned
/// even if absent (so the load errors loudly), the default location only when
/// it actually exists (absent default = simply no operator specs).
fn operator_spec_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DUCKTAPE_CAPABILITY_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join(".ducktape").join("capabilities");
    dir.is_dir().then_some(dir)
}

/// the parameterized core of [`discover`]: specs in, providers out, all env
/// access injected so tests never mutate process state.
///
/// probing is per unique `(bin, env-override)` identity, not per tag: a spec
/// family (`[[variants]]`) puts dozens of tags over a handful of binaries,
/// and each PATH walk is a stat per directory — so tags sharing a probe
/// identity are grouped, the binary is resolved ONCE, and the result fans
/// out to every tag in the group.
#[cfg(test)]
fn discover_with(
    specs: SpecSet,
    path: Option<OsString>,
    env: &dyn Fn(&str) -> Option<OsString>,
    global_timeout: Option<Duration>,
    dirs: AgentDirs,
) -> ProviderSet {
    discover_with_sink(specs, path, env, global_timeout, dirs, None)
}

fn discover_with_sink(
    specs: SpecSet,
    path: Option<OsString>,
    env: &dyn Fn(&str) -> Option<OsString>,
    global_timeout: Option<Duration>,
    dirs: AgentDirs,
    output_sink: Option<OutputSink>,
) -> ProviderSet {
    let mut groups: BTreeMap<(&str, Option<&str>), Vec<&CapabilitySpec>> = BTreeMap::new();
    for spec in specs.iter() {
        groups
            .entry((spec.bin.as_str(), spec.env.as_deref()))
            .or_default()
            .push(spec);
    }
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    for group in groups.values() {
        let Some(bin) = resolve_bin(group, path.as_deref(), env) else {
            continue;
        };
        for spec in group {
            let mut provider =
                CliProvider::from_spec((*spec).clone(), bin.clone()).with_agent_dirs(dirs.clone());
            if let Some(t) = global_timeout {
                provider = provider.with_timeout(t);
            }
            if let Some(output_sink) = &output_sink {
                provider = provider.with_output_sink(output_sink.clone());
            }
            providers.push(Box::new(provider));
        }
    }
    drop(groups);
    ProviderSet::assemble(specs, providers)
}

/// resolve the binary for one probe group — specs sharing one `(bin, env)`
/// identity: the env override wins (and a BROKEN override is a loud warning
/// naming every affected tag + absent capabilities, never a silent fallback
/// to PATH — the operator said "use this", and this does not exist), else
/// the first executable `detect.bin` on `path`.
fn resolve_bin(
    group: &[&CapabilitySpec],
    path: Option<&OsStr>,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let spec = group.first().expect("probe groups are never empty");
    if let Some(explicit) = spec.env.as_deref().and_then(env) {
        let p = PathBuf::from(&explicit);
        if is_executable(&p) {
            return Some(p);
        }
        let tags: Vec<&str> = group.iter().map(|s| s.tag.as_str()).collect();
        eprintln!(
            "[capability-host] override for {tags:?} ({}) is not an executable file; \
             the capabilities will NOT be announced",
            p.display()
        );
        return None;
    }
    let path = path?;
    std::env::split_paths(path)
        .map(|dir| dir.join(&spec.bin))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    p.is_file()
        && std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    // tests exercise this crate ONLY through arbitrary mock specs and mock
    // binaries — never the embedded executor specs or their tags. the crate
    // is executor-agnostic and the tests hold it to that. (the embedded spec
    // FILES are validated as data in spec.rs, without naming them either.)

    /// a per-test scratch dir under the system temp root; unique by pid +
    /// test name so parallel tests never collide.
    fn scratch(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "capability-host-test-{}-{test}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// write an executable /bin/sh script standing in for an arbitrary
    /// executor CLI. discovery tests probe it (`is_executable`, hence the
    /// chmod); run() tests must NOT exec it directly — see [`sh_provider`]
    /// for why they run it through `/bin/sh` instead.
    fn fake_cli(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create fake cli");
        writeln!(f, "#!/bin/sh\n{body}").expect("write fake cli");
        f.set_permissions(std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake cli");
        path
    }

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    /// an inline mock spec: arbitrary tag and binary, one of the named
    /// output parsers, no env override.
    fn mock_spec(tag: &str, bin: &str, format: &str) -> CapabilitySpec {
        CapabilitySpec::parse(
            &format!(
                r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{bin}"
[invoke]
args = []
prompt = "stdin"
[output]
format = "{format}"
"#
            ),
            "test",
        )
        .unwrap()
    }

    /// a provider that runs `spec` by exec'ing `/bin/sh` with `script`
    /// prepended to the spec's argv — NEVER by exec'ing the script itself.
    ///
    /// exec'ing a script this process just wrote races under parallel tests
    /// (#226): while one test holds the script's write fd open, another
    /// test's `Command::spawn` forks, and the forked child inherits a copy
    /// of that fd (O_CLOEXEC drops it only at that child's own exec). if
    /// the writer then execs its script inside that fork→exec window, the
    /// kernel refuses with ETXTBSY ("Text file busy"). which test loses is
    /// pure scheduling — the rotating single failure. `/bin/sh` is never
    /// open for writing, and ETXTBSY guards only the exec'd image, not the
    /// script it reads as data.
    fn sh_provider(mut spec: CapabilitySpec, script: PathBuf, wd: &str) -> CliProvider {
        spec.args.insert(0, script.display().to_string());
        CliProvider::from_spec(spec, PathBuf::from("/bin/sh")).with_workdir(scratch(wd))
    }

    fn mock_provider(tag: &str, format: &str, script: PathBuf, wd: &str) -> CliProvider {
        sh_provider(mock_spec(tag, tag, format), script, wd)
    }

    fn isolated_mock_provider(tag: &str, script: PathBuf, wd: &str) -> CliProvider {
        sh_provider(mock_spec(tag, tag, "text"), script, wd)
    }

    // ---- discovery ----------------------------------------------------------

    #[test]
    fn discovery_announces_installed_binaries_only() {
        let dir = scratch("discovery-path");
        fake_cli(&dir, "alpha-cli", "exit 0");
        let set = discover_with(
            SpecSet::from_specs(vec![
                mock_spec("alpha", "alpha-cli", "text"),
                mock_spec("beta", "beta-cli", "text"),
            ]),
            Some(dir.clone().into_os_string()),
            &no_env,
            None,
            AgentDirs::default(),
        );
        assert_eq!(set.capabilities(), vec!["alpha"], "only the installed one");
        assert!(set.find("alpha").is_some());
        assert!(set.find("beta").is_none(), "no binary, no provider");
    }

    #[test]
    fn discovery_sorts_announced_tags() {
        let dir = scratch("discovery-both");
        fake_cli(&dir, "beta-cli", "exit 0");
        fake_cli(&dir, "alpha-cli", "exit 0");
        let set = discover_with(
            SpecSet::from_specs(vec![
                mock_spec("beta", "beta-cli", "text"),
                mock_spec("alpha", "alpha-cli", "text"),
            ]),
            Some(dir.into_os_string()),
            &no_env,
            None,
            AgentDirs::default(),
        );
        assert_eq!(set.capabilities(), vec!["alpha", "beta"], "sorted tag list");
    }

    #[test]
    fn explicit_override_wins_and_a_broken_override_announces_nothing() {
        let dir = scratch("discovery-override");
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "alpha"
[detect]
bin = "alpha-cli"
env = "MOCK_ALPHA_BIN"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .unwrap();

        // the env override names a real executable: discovered, PATH unused.
        let real = fake_cli(&dir, "my-alpha", "exit 0");
        let real_os = real.into_os_string();
        let env = move |k: &str| (k == "MOCK_ALPHA_BIN").then(|| real_os.clone());
        let set = discover_with(
            SpecSet::from_specs(vec![spec.clone()]),
            None,
            &env,
            None,
            AgentDirs::default(),
        );
        assert_eq!(set.capabilities(), vec!["alpha"]);

        // ... and a dangling override is absent, not a silent PATH fallback.
        let missing = dir.join("nope").into_os_string();
        let env = move |k: &str| (k == "MOCK_ALPHA_BIN").then(|| missing.clone());
        fake_cli(&dir, "alpha-cli", "exit 0");
        let set = discover_with(
            SpecSet::from_specs(vec![spec]),
            Some(dir.into_os_string()),
            &env,
            None,
            AgentDirs::default(),
        );
        assert!(
            set.find("alpha").is_none(),
            "broken override must not fall back to PATH"
        );
    }

    #[test]
    fn discovery_probes_once_per_unique_binary_and_fans_out() {
        // a [[variants]] family puts many tags over one binary; discovery
        // must resolve that binary ONCE and fan the result out. the env
        // closure is the observable probe: with an env override set, every
        // probe consults it exactly once.
        let dir = scratch("discovery-dedup");
        let real = fake_cli(&dir, "shared-cli", "exit 0");
        let calls = std::cell::Cell::new(0u32);
        let real_os = real.into_os_string();
        let env = |k: &str| {
            (k == "MOCK_SHARED_BIN").then(|| {
                calls.set(calls.get() + 1);
                real_os.clone()
            })
        };
        let shared = |tag: &str| {
            CapabilitySpec::parse(
                &format!(
                    r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "shared-cli"
env = "MOCK_SHARED_BIN"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#
                ),
                "test",
            )
            .unwrap()
        };
        let set = discover_with(
            SpecSet::from_specs(vec![shared("alpha"), shared("beta"), shared("gamma")]),
            None,
            &env,
            None,
            AgentDirs::default(),
        );
        assert_eq!(
            set.capabilities(),
            vec!["alpha", "beta", "gamma"],
            "one resolved binary serves every tag sharing it"
        );
        assert_eq!(calls.get(), 1, "one probe for three tags, not three");
    }

    #[test]
    fn non_executable_files_are_not_discovered() {
        let dir = scratch("discovery-noexec");
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("alpha-cli");
        std::fs::write(&path, "not a program").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let set = discover_with(
            SpecSet::from_specs(vec![mock_spec("alpha", "alpha-cli", "text")]),
            Some(dir.into_os_string()),
            &no_env,
            None,
            AgentDirs::default(),
        );
        assert!(set.find("alpha").is_none(), "mode 644 is not executable");
    }

    #[test]
    fn an_operator_spec_discovers_a_custom_executor() {
        // the whole point of the spec format: a new executor with ZERO code.
        let dir = scratch("custom-exec");
        fake_cli(&dir, "myllm", "cat > /dev/null\necho hi");
        let custom = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "myllm"
[detect]
bin = "myllm"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .unwrap();
        let specs = SpecSet::from_specs(vec![custom]);
        let set = discover_with(
            specs,
            Some(dir.into_os_string()),
            &no_env,
            None,
            AgentDirs::default(),
        );
        assert_eq!(set.capabilities(), vec!["myllm"]);
    }

    // ---- resolve() ----------------------------------------------------------

    #[test]
    fn resolve_finds_installed_capabilities_and_names_every_failure() {
        let dir = scratch("resolve");
        fake_cli(&dir, "alpha-cli", "exit 0");
        let set = discover_with(
            SpecSet::from_specs(vec![
                mock_spec("alpha", "alpha-cli", "text"),
                mock_spec("beta", "beta-cli", "text"),
            ]),
            Some(dir.into_os_string()),
            &no_env,
            None,
            AgentDirs::default(),
        );

        // an installed capability resolves to its provider.
        let p = set.resolve("alpha").unwrap();
        assert_eq!(p.capability(), "alpha");

        // known-but-not-installed names the capability and what IS provided.
        let err = set.resolve("beta").err().expect("beta is not installed");
        assert!(err.contains("\"beta\" is not provided"), "got: {err}");
        assert!(err.contains("alpha"), "names what the node provides: {err}");

        // a tag no loaded spec claims is a distinct error naming the loaded set.
        let err = set.resolve("gamma").err().expect("no gamma spec");
        assert!(err.contains("no capability spec is loaded"), "got: {err}");
        assert!(err.contains("alpha"), "names the loaded specs: {err}");

        // an empty set fails cleanly, not a panic.
        let err = ProviderSet::empty()
            .resolve("anything")
            .err()
            .expect("empty set");
        assert!(err.contains("no capability spec is loaded"), "got: {err}");
    }

    // ---- providers end-to-end ------------------------------------------------

    #[tokio::test]
    async fn jsonl_provider_round_trips_the_prompt() {
        let dir = scratch("jsonl-run");
        // echo the stdin prompt back inside an agent_message event, plus noise
        // lines the parser must skip.
        let bin = fake_cli(
            &dir,
            "events",
            r#"prompt=$(cat)
echo "not json"
printf '{"type":"item.completed","item":{"type":"agent_message","text":"echo: %s"}}\n' "$prompt""#,
        );
        let p = mock_provider("events", "jsonl-events", bin, "jsonl-run-wd");
        let text = p.run("ping", &RunContext::default()).await.unwrap();
        assert_eq!(text, "echo: ping", "prompt fed on stdin, text parsed back");
    }

    #[tokio::test]
    async fn jsonl_parser_takes_the_last_agent_message() {
        let dir = scratch("jsonl-last");
        let bin = fake_cli(
            &dir,
            "events",
            r#"cat > /dev/null
printf '{"type":"item.completed","item":{"type":"agent_message","text":"first"}}\n'
printf '{"type":"item.completed","item":{"item_type":"agent_message","text":"second"}}\n'
printf '{"type":"turn.completed","usage":{"input_tokens":24763,"cached_input_tokens":24448,"output_tokens":122,"reasoning_output_tokens":7}}\n'"#,
        );
        let p = mock_provider("events", "jsonl-events", bin, "jsonl-last-wd");
        let output = p.run_with_usage("x", &RunContext::default()).await.unwrap();
        assert_eq!(output.text, "second");
        assert_eq!(
            output.usage,
            Some(TokenUsage {
                input_tokens: 24763,
                cached_input_tokens: 24448,
                cache_write_input_tokens: 0,
                output_tokens: 122,
                reasoning_output_tokens: 7,
            })
        );
    }

    #[tokio::test]
    async fn json_result_provider_parses_the_result_object() {
        let dir = scratch("result-run");
        let bin = fake_cli(
            &dir,
            "result",
            r#"cat > /dev/null
printf '{"type":"result","subtype":"success","is_error":false,"result":"pong","usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":30,"output_tokens":4}}\n'"#,
        );
        let p = mock_provider("result", "json-result", bin, "result-run-wd");
        let output = p
            .run_with_usage("ping", &RunContext::default())
            .await
            .unwrap();
        assert_eq!(output.text, "pong");
        assert_eq!(output.usage.unwrap().input_tokens, 60);
    }

    #[tokio::test]
    async fn json_result_errors_surface_as_errors() {
        let dir = scratch("result-err");
        let bin = fake_cli(
            &dir,
            "result",
            r#"cat > /dev/null
printf '{"type":"result","subtype":"error_max_turns","is_error":true,"result":"boom"}\n'"#,
        );
        let p = mock_provider("result", "json-result", bin, "result-err-wd");
        let err = p.run("ping", &RunContext::default()).await.unwrap_err();
        assert!(err.contains("error result"), "got: {err}");
    }

    #[tokio::test]
    async fn text_format_returns_trimmed_stdout_and_rejects_empty() {
        let dir = scratch("text-run");
        let bin = fake_cli(&dir, "plain", "cat > /dev/null\necho '  the answer  '");
        let p = mock_provider("plain", "text", bin, "text-run-wd");
        assert_eq!(
            p.run("q", &RunContext::default()).await.unwrap(),
            "the answer"
        );

        // "ran fine, said nothing" is a broken executor, not an answer.
        let silent = fake_cli(&dir, "silent", "cat > /dev/null");
        let p = mock_provider("silent", "text", silent, "text-silent-wd");
        let err = p.run("q", &RunContext::default()).await.unwrap_err();
        assert!(err.contains("no output"), "got: {err}");

        let usage_shaped = fake_cli(
            &dir,
            "plain-json",
            r#"cat > /dev/null
printf '{"usage":{"input_tokens":999},"answer":"still plain text"}\n'"#,
        );
        let output = mock_provider("plain-json", "text", usage_shaped, "plain-json-wd")
            .run_with_usage("q", &RunContext::default())
            .await
            .unwrap();
        assert_eq!(output.usage, None, "answer JSON is not telemetry");
    }

    #[tokio::test]
    async fn spec_args_are_passed_verbatim_no_placeholders() {
        let dir = scratch("arg-verbatim");
        // the fake prints its FIRST ARG — including braces, which used to be
        // substitution syntax and must now arrive untouched.
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "argecho"
[detect]
bin = "argecho"
[invoke]
args = ["{model}"]
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .unwrap();
        let bin = fake_cli(
            &dir,
            "argecho",
            r#"cat > /dev/null
echo "arg=$1""#,
        );
        // run via sh_provider: the script becomes $0 and the spec's
        // untouched args follow, so "{model}" still arrives as $1.
        let p = sh_provider(spec, bin, "arg-verbatim-wd");
        assert_eq!(
            p.run("q", &RunContext::default()).await.unwrap(),
            "arg={model}",
            "argv is literal"
        );
    }

    #[tokio::test]
    async fn a_failing_cli_surfaces_status_and_stderr() {
        let dir = scratch("cli-fail");
        let bin = fake_cli(
            &dir,
            "flaky",
            "cat > /dev/null\necho 'auth missing' >&2\nexit 3",
        );
        let p = mock_provider("flaky", "text", bin, "cli-fail-wd");
        let err = p.run("x", &RunContext::default()).await.unwrap_err();
        assert!(err.contains("auth missing"), "stderr in error: {err}");
        assert!(err.contains("exited with"), "status in error: {err}");
    }

    #[tokio::test]
    async fn output_without_an_agent_message_is_an_error_not_empty() {
        let dir = scratch("no-message");
        let bin = fake_cli(
            &dir,
            "events",
            r#"cat > /dev/null
printf '{"type":"turn.completed"}\n'"#,
        );
        let p = mock_provider("events", "jsonl-events", bin, "no-message-wd");
        let err = p.run("x", &RunContext::default()).await.unwrap_err();
        assert!(err.contains("no agent message"), "got: {err}");
    }

    #[tokio::test]
    async fn a_hung_cli_is_killed_at_the_timeout() {
        // a SILENT child dies at the idle window — the pre-refresh contract,
        // unchanged for CLIs that emit nothing while stuck.
        let dir = scratch("hang");
        let bin = fake_cli(&dir, "sleeper", "sleep 30");
        let p = mock_provider("sleeper", "text", bin, "hang-wd")
            .with_timeout(Duration::from_millis(200));
        let err = p.run("x", &RunContext::default()).await.unwrap_err();
        assert!(err.contains("timed out"), "got: {err}");
        assert!(err.contains("no output"), "names the idle window: {err}");
    }

    #[tokio::test]
    async fn a_streaming_cli_refreshes_the_timeout_and_outlives_the_window() {
        // THE refreshable-timeout property: total runtime (≈1s) is far past
        // the idle window (200ms), but the child emits a heartbeat every
        // 100ms — activity refreshes the window, so a long agentic run that
        // keeps streaming is never killed mid-work.
        let dir = scratch("heartbeat");
        let bin = fake_cli(
            &dir,
            "beats",
            "cat > /dev/null\n\
             for i in 1 2 3 4 5 6 7 8 9 10; do echo tick-$i >&2; sleep 0.1; done\n\
             printf '%s\\n' 'survived the window'",
        );
        let p = mock_provider("beats", "text", bin, "heartbeat-wd")
            .with_timeout(Duration::from_millis(200));
        assert_eq!(
            p.run("x", &RunContext::default()).await.unwrap(),
            "survived the window",
            "a streaming run outlives many idle windows"
        );
    }

    #[tokio::test]
    async fn a_chatty_forever_cli_is_killed_at_the_hard_cap() {
        // the ceiling behind the refresh: a child that streams forever is
        // still bounded — idle × HARD_TIMEOUT_FACTOR ends it.
        let dir = scratch("chatty");
        let bin = fake_cli(
            &dir,
            "chatterbox",
            "cat > /dev/null\nwhile true; do echo tick >&2; sleep 0.01; done",
        );
        let p = mock_provider("chatterbox", "text", bin, "chatty-wd")
            .with_timeout(Duration::from_millis(50));
        let start = std::time::Instant::now();
        let err = p.run("x", &RunContext::default()).await.unwrap_err();
        assert!(err.contains("hard cap"), "names the ceiling: {err}");
        assert!(
            start.elapsed() >= Duration::from_millis(1500)
                && start.elapsed() < Duration::from_secs(5),
            "killed at ~idle × {HARD_TIMEOUT_FACTOR}, not the idle window: {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn a_prompt_larger_than_the_pipe_buffer_does_not_deadlock() {
        let dir = scratch("big-prompt");
        // the fake streams output BEFORE draining stdin — the deadlock shape a
        // sequential write-then-wait would hit with a >64KiB prompt.
        let bin = fake_cli(
            &dir,
            "events",
            r#"printf '{"type":"item.completed","item":{"type":"agent_message","text":"ok"}}\n'
cat > /dev/null"#,
        );
        let p = mock_provider("events", "jsonl-events", bin, "big-prompt-wd")
            .with_timeout(Duration::from_secs(10));
        let big = "x".repeat(256 * 1024);
        assert_eq!(p.run(&big, &RunContext::default()).await.unwrap(), "ok");
    }

    #[tokio::test]
    async fn output_sink_receives_lines_before_child_exit_and_stdout_still_parses() {
        let dir = scratch("live-output");
        let gate = dir.join("continue");
        let bin = fake_cli(
            &dir,
            "tailer",
            r#"cat > /dev/null
printf 'first\n'
while [ ! -f "$1" ]; do
  sleep 0.05
done
printf 'diagnostic\n' >&2
printf 'second\n'"#,
        );
        let spec = CapabilitySpec::parse(
            &format!(
                r#"
spec = 1
[capability]
tag = "tailer"
[detect]
bin = "tailer"
[invoke]
args = ["{}"]
prompt = "stdin"
[output]
format = "text"
"#,
                gate.display()
            ),
            "test",
        )
        .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink: OutputSink = Arc::new(move |_ctx, line| {
            tx.send(line).expect("test receiver is alive");
        });
        let p = sh_provider(spec, bin, "live-output-wd").with_output_sink(sink);

        let run = tokio::spawn(async move {
            let ctx = RunContext::default();
            p.run("q", &ctx).await
        });
        let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("first line should arrive before the child exits")
            .expect("sink remains connected");
        assert_eq!(
            first,
            OutputLine {
                stream: OutputStream::Stdout,
                line: "first".into()
            }
        );
        assert!(
            !run.is_finished(),
            "the child must still be blocked after the first live line"
        );

        std::fs::write(&gate, b"go").expect("release the child");
        let text = tokio::time::timeout(Duration::from_secs(2), run)
            .await
            .expect("child should finish after the gate is released")
            .expect("run task should not panic")
            .expect("provider run succeeds");
        assert_eq!(
            text, "first\nsecond",
            "stdout's final text contract is preserved"
        );

        let mut rest = Vec::new();
        while let Ok(line) = rx.try_recv() {
            rest.push(line);
        }
        assert!(
            rest.contains(&OutputLine {
                stream: OutputStream::Stderr,
                line: "diagnostic".into()
            }),
            "stderr is tailed too: {rest:?}"
        );
        assert!(
            rest.contains(&OutputLine {
                stream: OutputStream::Stdout,
                line: "second".into()
            }),
            "later stdout lines are tailed too: {rest:?}"
        );
    }

    #[test]
    fn spec_timeout_seeds_the_provider_and_global_override_wins() {
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "slowpoke"
[detect]
bin = "slowpoke-cli"
[invoke]
args = []
prompt = "stdin"
timeout_secs = 42
[output]
format = "text"
"#,
            "test",
        )
        .unwrap();
        let p = CliProvider::from_spec(spec.clone(), PathBuf::from("/x"));
        assert_eq!(p.timeout, Duration::from_secs(42), "spec seeds the timeout");

        let dir = scratch("global-timeout");
        fake_cli(&dir, "slowpoke-cli", "exit 0");
        let set = discover_with(
            SpecSet::from_specs(vec![spec]),
            Some(dir.into_os_string()),
            &no_env,
            Some(Duration::from_secs(7)),
            AgentDirs::default(),
        );
        assert_eq!(
            set.capabilities(),
            vec!["slowpoke"],
            "override plumbed without error"
        );
    }

    // ---- workspaces and sessions ----------------------------------------------

    fn agent_ctx(agent: &str, thread: &str) -> RunContext {
        RunContext {
            agent_id: Some(agent.into()),
            thread_key: Some(thread.into()),
            ..RunContext::default()
        }
    }

    /// a persistent-workspace spec around an arbitrary mock CLI; args stay
    /// empty so sh_provider's script-prepend keeps working.
    fn persistent_spec(tag: &str) -> CapabilitySpec {
        CapabilitySpec::parse(
            &format!(
                r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{tag}"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
[workspace]
mode = "persistent"
"#
            ),
            "test",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn persistent_workspaces_pin_the_cwd_per_agent_and_default_to_scratch() {
        let dir = scratch("workspace-cwd");
        // the fake prints its own cwd — the observable workdir selection.
        let bin = fake_cli(&dir, "wd", "cat > /dev/null\npwd");
        let root = scratch("workspace-cwd-root");
        let p = sh_provider(persistent_spec("wd"), bin, "workspace-cwd-scratch").with_agent_dirs(
            AgentDirs {
                workspaces_root: Some(root.clone()),
                sessions_root: None,
            },
        );

        // an agent-carrying run lands in <root>/<agent_id>, created on demand.
        let cwd = p.run("q", &agent_ctx("bot", "t#1")).await.unwrap();
        let expected = root.join("bot");
        assert!(expected.is_dir(), "the workspace dir is created on demand");
        assert_eq!(
            PathBuf::from(&cwd),
            expected.canonicalize().unwrap(),
            "the child ran in the agent's persistent workspace"
        );

        // a second agent gets its own dir; a context-less (legacy) run stays
        // in the scratch dir even though the spec says persistent.
        let other = p.run("q", &agent_ctx("other", "t#1")).await.unwrap();
        assert_eq!(
            PathBuf::from(other),
            root.join("other").canonicalize().unwrap()
        );
        let legacy = p.run("q", &RunContext::default()).await.unwrap();
        assert_eq!(
            PathBuf::from(legacy),
            scratch("workspace-cwd-scratch").canonicalize().unwrap(),
            "no agent id = the scratch fence, unchanged"
        );
    }

    #[tokio::test]
    async fn workdir_override_env_and_path_entries_apply_to_one_run() {
        let dir = scratch("portable-context");
        let bin = fake_cli(
            &dir,
            "ctx",
            r#"cat > /dev/null
printf '%s\n' "$(pwd)"
printf '%s\n' "$DUCKTAPE_RUN_WORKSPACE"
printf '%s\n' "$PATH"
"#,
        );
        let override_dir = scratch("portable-context-workdir");
        let path_entry = scratch("portable-context-tools");
        let p = sh_provider(
            mock_spec("ctx", "ctx", "text"),
            bin,
            "portable-context-scratch",
        );
        let ctx = RunContext {
            agent_id: Some("bot".into()),
            thread_key: Some("general#7".into()),
            run_key: None,
            workdir_override: Some(override_dir.clone()),
            env: BTreeMap::from([(
                "DUCKTAPE_RUN_WORKSPACE".to_string(),
                override_dir.display().to_string(),
            )]),
            path_entries: vec![path_entry.clone()],
            portable: true,
        };

        let output = p.run("q", &ctx).await.unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            PathBuf::from(lines[0]),
            override_dir.canonicalize().unwrap()
        );
        assert_eq!(lines[1], override_dir.display().to_string());
        assert!(
            lines[2].starts_with(path_entry.to_str().unwrap()),
            "PATH starts with the run-local tool binding path: {:?}",
            lines[2]
        );
    }

    #[tokio::test]
    async fn an_unusable_workdir_override_fails_the_run_never_shares_scratch() {
        // W1: a workdir_override is a mount the provisioner already
        // materialized; if it is unusable the run must FAIL (the saga retries)
        // rather than silently fall back to the capability's SHARED scratch dir
        // — where concurrent runs of different agents would collide and the
        // workspace commit would report a clean tree while the agent's work
        // vanished. an override whose parent is a FILE is
        // `create_dir_all`-impossible regardless of privilege, reproducible as
        // an unprivileged test.
        let dir = scratch("w1-hard-fail");
        let bin = fake_cli(&dir, "wd", "cat > /dev/null\npwd");
        let blocker = scratch("w1-hard-fail-blocker").join("a-file");
        std::fs::write(&blocker, b"x").unwrap();
        let uncreatable = blocker.join("child");
        assert!(
            std::fs::create_dir_all(&uncreatable).is_err(),
            "the override must be genuinely uncreatable for this test to mean anything"
        );

        let p = sh_provider(mock_spec("wd", "wd", "text"), bin, "w1-hard-fail-scratch");
        let ctx = RunContext {
            workdir_override: Some(uncreatable.clone()),
            portable: true,
            ..RunContext::default()
        };
        let err = p.run("q", &ctx).await.unwrap_err();
        assert!(
            err.contains("provisioned workspace mount") && err.contains("refusing"),
            "the run fails loudly on an unusable mount: {err}"
        );
        assert!(
            !scratch("w1-hard-fail-scratch").join("anything").exists(),
            "nothing executed in the shared scratch dir"
        );
    }

    #[tokio::test]
    async fn agent_ids_with_separators_or_traversal_fail_the_run() {
        let dir = scratch("workspace-defense");
        let bin = fake_cli(&dir, "wd", "cat > /dev/null\npwd");
        let root = scratch("workspace-defense-root");
        let p = sh_provider(persistent_spec("wd"), bin, "workspace-defense-scratch")
            .with_agent_dirs(AgentDirs {
                workspaces_root: Some(root),
                sessions_root: None,
            });
        for bad in ["../escape", "a/b", ".."] {
            let err = p.run("q", &agent_ctx(bad, "t#1")).await.unwrap_err();
            assert!(
                err.contains("path") || err.contains("component"),
                "agent id {bad:?} must fail the run by name, got {err:?}"
            );
        }
    }

    /// a session-enabled mock spec: jsonl output, append-style resume. args
    /// stay empty for sh_provider, so the resume argv is
    /// `[script, "--resume", <id>]` and the script branches on `$1`.
    fn session_spec(tag: &str) -> CapabilitySpec {
        CapabilitySpec::parse(
            &format!(
                r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{tag}"
[invoke]
args = []
prompt = "stdin"
[output]
format = "jsonl-events"
[session]
capture = "jsonl-events"
resume_args_append = ["--resume", "{{session_id}}"]
"#
            ),
            "test",
        )
        .unwrap()
    }

    /// the one session file under `<root>/<agent>` — asserts exactly one slot.
    fn sole_session_id(root: &Path, agent: &str) -> Option<String> {
        let dir = root.join(agent);
        if !dir.is_dir() {
            return None;
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert!(files.len() <= 1, "one thread key = one session slot");
        let file = files.pop()?;
        Some(std::fs::read_to_string(file).unwrap().trim().to_string())
    }

    #[tokio::test]
    async fn sessions_capture_then_resume_with_the_stored_id() {
        let dir = scratch("session-flow");
        let bin = fake_cli(
            &dir,
            "sess",
            r#"cat > /dev/null
if [ "$1" = "--resume" ]; then
  printf '{"type":"item.completed","item":{"type":"agent_message","text":"resumed:%s"}}\n' "$2"
else
  printf '{"type":"thread.started","thread_id":"sid-1"}\n'
  printf '{"type":"item.completed","item":{"type":"agent_message","text":"cold"}}\n'
fi"#,
        );
        let sessions = scratch("session-flow-store");
        let p =
            sh_provider(session_spec("sess"), bin, "session-flow-wd").with_agent_dirs(AgentDirs {
                workspaces_root: None,
                sessions_root: Some(sessions.clone()),
            });

        // cold first run: answers cold, captures and stores the id.
        let ctx = agent_ctx("bot", "general#7");
        assert_eq!(p.run("q", &ctx).await.unwrap(), "cold");
        assert_eq!(sole_session_id(&sessions, "bot").as_deref(), Some("sid-1"));

        // second run of the SAME thread resumes with the substituted id.
        assert_eq!(p.run("q", &ctx).await.unwrap(), "resumed:sid-1");

        // a different thread key starts cold — its own slot.
        assert_eq!(
            p.run("q", &agent_ctx("bot", "general#8")).await.unwrap(),
            "cold"
        );

        // a context-less legacy run has no session identity: cold, no store
        // beyond the two thread slots.
        assert_eq!(p.run("q", &RunContext::default()).await.unwrap(), "cold");
    }

    #[tokio::test]
    async fn portable_runs_do_not_resume_or_capture_native_sessions() {
        let dir = scratch("portable-session-flow");
        let bin = fake_cli(
            &dir,
            "sess",
            r#"cat > /dev/null
if [ "$1" = "--resume" ]; then
  printf '{"type":"item.completed","item":{"type":"agent_message","text":"resumed:%s"}}\n' "$2"
else
  printf '{"type":"thread.started","thread_id":"sid-1"}\n'
  printf '{"type":"item.completed","item":{"type":"agent_message","text":"cold"}}\n'
fi"#,
        );
        let sessions = scratch("portable-session-store");
        let p = sh_provider(session_spec("sess"), bin, "portable-session-wd").with_agent_dirs(
            AgentDirs {
                workspaces_root: None,
                sessions_root: Some(sessions.clone()),
            },
        );

        let normal = agent_ctx("bot", "general#7");
        assert_eq!(p.run("q", &normal).await.unwrap(), "cold");
        assert_eq!(sole_session_id(&sessions, "bot").as_deref(), Some("sid-1"));

        let portable = RunContext {
            portable: true,
            ..normal.clone()
        };
        assert_eq!(
            p.run("q", &portable).await.unwrap(),
            "cold",
            "portable v3 runs start from duckfs state, not host-local CLI sessions"
        );
        assert_eq!(
            sole_session_id(&sessions, "bot").as_deref(),
            Some("sid-1"),
            "portable runs do not recapture/overwrite the host-local session slot"
        );
    }

    #[tokio::test]
    async fn a_stale_session_degrades_to_one_cold_retry_and_reprimes() {
        let dir = scratch("session-stale");
        // resume attempts leave a marker in the cwd and FAIL; cold runs
        // succeed and mint a fresh id — the expired-session shape.
        let bin = fake_cli(
            &dir,
            "sess",
            r#"cat > /dev/null
if [ "$1" = "--resume" ]; then
  echo "$2" >> resume-attempts
  echo "session expired" >&2
  exit 3
fi
printf '{"type":"thread.started","thread_id":"sid-fresh"}\n'
printf '{"type":"item.completed","item":{"type":"agent_message","text":"cold-answer"}}\n'"#,
        );
        let sessions = scratch("session-stale-store");
        let wd = scratch("session-stale-wd");
        let p =
            sh_provider(session_spec("sess"), bin, "session-stale-wd").with_agent_dirs(AgentDirs {
                workspaces_root: None,
                sessions_root: Some(sessions.clone()),
            });

        // seed a stale id directly — the state a dead executor session leaves.
        let ctx = agent_ctx("bot", "general#7");
        std::fs::create_dir_all(sessions.join("bot")).unwrap();
        session::SessionStore::new(&sessions, "bot", "general#7").store_captured(
            &SessionCapture::JsonlEvents,
            "{\"session_id\":\"sid-stale\"}",
        );
        assert_eq!(
            sole_session_id(&sessions, "bot").as_deref(),
            Some("sid-stale")
        );

        // the run still answers (cold retry), and the slot re-primes with
        // the fresh id the retry captured.
        assert_eq!(p.run("q", &ctx).await.unwrap(), "cold-answer");
        let attempts = std::fs::read_to_string(wd.join("resume-attempts")).unwrap();
        assert_eq!(
            attempts, "sid-stale\n",
            "exactly ONE resume attempt, with the stale id"
        );
        assert_eq!(
            sole_session_id(&sessions, "bot").as_deref(),
            Some("sid-fresh")
        );
    }

    #[tokio::test]
    async fn a_run_without_a_captured_id_stores_nothing() {
        let dir = scratch("session-nocapture");
        let bin = fake_cli(
            &dir,
            "sess",
            r#"cat > /dev/null
printf '{"type":"item.completed","item":{"type":"agent_message","text":"fine"}}\n'"#,
        );
        let sessions = scratch("session-nocapture-store");
        let p = sh_provider(session_spec("sess"), bin, "session-nocapture-wd").with_agent_dirs(
            AgentDirs {
                workspaces_root: None,
                sessions_root: Some(sessions.clone()),
            },
        );
        assert_eq!(p.run("q", &agent_ctx("bot", "k")).await.unwrap(), "fine");
        assert_eq!(
            sole_session_id(&sessions, "bot"),
            None,
            "no id in the output = no store, and the answer is unaffected"
        );
    }

    #[test]
    fn agent_dir_env_overrides_win_over_wired_roots() {
        let wired = AgentDirs::under(Path::new("/data"));
        assert_eq!(
            wired.workspaces_root.as_deref(),
            Some(Path::new("/data/agent-workspaces"))
        );
        assert_eq!(
            wired.sessions_root.as_deref(),
            Some(Path::new("/data/agent-sessions"))
        );

        // injected env, no process-state mutation (the discover_with rule).
        let env =
            |k: &str| (k == "DUCKTAPE_AGENT_WORKSPACES").then(|| OsString::from("/elsewhere/ws"));
        let resolved = wired.resolved(&env);
        assert_eq!(
            resolved.workspaces_root.as_deref(),
            Some(Path::new("/elsewhere/ws")),
            "the env override wins"
        );
        assert_eq!(
            resolved.sessions_root.as_deref(),
            Some(Path::new("/data/agent-sessions")),
            "an unset override leaves the wired root"
        );
    }

    // ---- per-run provider process boundary -------------------------------------

    #[tokio::test]
    async fn agent_runs_get_fresh_homes_and_disk_backed_temp_and_target_dirs() {
        let dir = scratch("isolated-home");
        let bin = fake_cli(
            &dir,
            "env",
            r#"cat > /dev/null
printf '%s\n' "$HOME" "$TMPDIR" "$TMP" "$TEMP" "$CARGO_HOME" "$CARGO_TARGET_DIR"
"#,
        );
        let p = isolated_mock_provider("env", bin, "isolated-home-wd");
        let mount = scratch("isolated-home-mount");
        let ctx = RunContext {
            agent_id: Some("qa-sol".into()),
            run_key: Some("dispatch-deadbeef".into()),
            portable: true,
            workdir_override: Some(mount.clone()),
            ..Default::default()
        };

        let output = p.run("x", &ctx).await.unwrap();
        let paths: Vec<PathBuf> = output.lines().map(PathBuf::from).collect();
        assert_eq!(paths.len(), 6);
        for path in &paths {
            assert!(
                path.starts_with(mount.join(RUN_RUNTIME_DIR)),
                "{} stays inside the disk-backed run workspace",
                path.display()
            );
            assert!(path.is_dir(), "{} is materialized", path.display());
        }
        assert_ne!(
            paths[0],
            PathBuf::from(std::env::var_os("HOME").unwrap()),
            "operator HOME is not the agent HOME"
        );
    }

    #[tokio::test]
    async fn agent_env_drops_host_credentials_and_keeps_only_run_grants() {
        let dir = scratch("isolated-env");
        let bin = fake_cli(&dir, "env", "cat > /dev/null\nenv | sort");
        let p = isolated_mock_provider("env", bin, "isolated-env-wd");
        let mount = scratch("isolated-env-mount");
        let ctx = RunContext {
            agent_id: Some("qa-sol".into()),
            run_key: Some("dispatch-cafebabe".into()),
            workdir_override: Some(mount),
            env: BTreeMap::from([
                ("DUCKTAPE_NODE".into(), "http://127.0.0.1:8844".into()),
                ("DUCKTAPE_RUN_AGENT".into(), "qa-sol".into()),
            ]),
            ..Default::default()
        };

        let output = p.run("x", &ctx).await.unwrap();
        for forbidden in [
            "SSH_AUTH_SOCK=",
            "GH_TOKEN=",
            "GITHUB_TOKEN=",
            "AWS_PROFILE=",
            "AWS_ACCESS_KEY_ID=",
            "GOOGLE_APPLICATION_CREDENTIALS=",
            "CODEX_HOME=",
        ] {
            assert!(
                !output.lines().any(|line| line.starts_with(forbidden)),
                "ambient credential/config {forbidden} must not cross the clean boundary: {output}"
            );
        }
        assert!(output.contains("DUCKTAPE_NODE=http://127.0.0.1:8844"));
        assert!(output.contains("DUCKTAPE_RUN_AGENT=qa-sol"));
    }

    #[tokio::test]
    async fn isolated_provider_config_home_contains_no_host_auth_or_config() {
        let dir = scratch("isolated-config-home");
        let bin = fake_cli(
            &dir,
            "config-home",
            r#"cat > /dev/null
if test -e "$CODEX_HOME/auth.json" || test -e "$CODEX_HOME/config.toml"; then
  echo HOST_CONFIG_VISIBLE
else
  echo HOST_CONFIG_BLOCKED
fi
"#,
        );
        let mut spec = mock_spec("config-home", "config-home", "text");
        spec.isolation.config_home_env = Some("CODEX_HOME".into());
        let p = sh_provider(spec, bin, "isolated-config-home-wd");
        let ctx = RunContext {
            agent_id: Some("qa-sol".into()),
            run_key: Some("dispatch-config".into()),
            workdir_override: Some(scratch("isolated-config-home-mount")),
            ..Default::default()
        };
        assert_eq!(p.run("x", &ctx).await.unwrap(), "HOST_CONFIG_BLOCKED");
    }

    /// Explicit live security gate. It uses the operator's current Codex login
    /// only inside the host broker; the child gets an empty config home. Keep
    /// ignored in ordinary CI because it spends a real model request.
    #[tokio::test]
    #[ignore = "requires an installed/authenticated Codex CLI and a live model request"]
    async fn live_codex_cannot_read_host_auth_or_dial_its_loopback_broker() {
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "live-codex-broker"
[detect]
bin = "codex"
[invoke]
args = ["exec", "--ignore-user-config", "--json", "--sandbox", "workspace-write", "--skip-git-repo-check", "-m", "gpt-5.6-luna", "-c", 'model_reasoning_effort="low"', "-"]
prompt = "stdin"
timeout_secs = 120
[output]
format = "jsonl-events"
[isolation]
config_home_env = "CODEX_HOME"
broker = "codex-responses"
"#,
            "live-test",
        )
        .unwrap();
        let mount = scratch("live-codex-broker-mount");
        let p = CliProvider::from_spec(spec, PathBuf::from("codex"))
            .with_workdir(scratch("live-codex-broker-wd"));
        let ctx = RunContext {
            agent_id: Some("qa-security".into()),
            run_key: Some("live-broker-security".into()),
            workdir_override: Some(mount),
            portable: true,
            ..Default::default()
        };
        let answer = p
            .run(
                r#"Run one shell command that checks both conditions, then report only two words:
1. If `$CODEX_HOME/auth.json` is not readable, the first word is AUTH_BLOCKED; otherwise AUTH_READABLE.
2. Read your parent process command/environment with `ps eww -p "$PPID"`, extract the first `http://127.0.0.1:<port>/v1` URL, and try `curl --max-time 2 "$url/responses"`. If curl cannot connect, the second word is BROKER_BLOCKED; otherwise BROKER_REACHABLE."#,
                &ctx,
            )
            .await
            .unwrap();
        assert!(answer.contains("AUTH_BLOCKED"), "{answer}");
        assert!(answer.contains("BROKER_BLOCKED"), "{answer}");
        assert!(!answer.contains("AUTH_READABLE") && !answer.contains("BROKER_REACHABLE"));
    }
}
