//! host-side capability providers — the I/O half of the capability seam.
//!
//! the capability module (consensus) replicates *who provides what*; this
//! crate is the machine-local counterpart that actually provides it. a
//! [`Provider`] wraps one locally installed executor CLI, and [`discover`]
//! probes the host for the executors the operator brought — BYO by
//! construction: the operator logs their CLI in, the node just spawns it.
//!
//! ## where the credential goes
//!
//! BYO auth used to mean the node never touched a credential at all: the child
//! inherited the operator's environment and HOME, and the CLI found its own
//! dotfiles. that is still the floor, but a spec can now do better, and the two
//! options are mutually exclusive by construction (see [`spec`]):
//!
//!   * `[isolation]` — the STRONG path. the HOST reads the credential and holds
//!     it in this process; a per-run loopback [`broker`] serves the model API
//!     and the child gets only an opaque bearer plus a FRESH, empty config home
//!     (so the CLI cannot fall back to reading the operator's real one). the
//!     credential never enters the child's process tree at all. codex is here.
//!   * `[sandbox] rw_dirs` — the WEAK path, for a CLI with no broker: its auth
//!     dir crosses into the sandbox and the credential DOES enter the child.
//!     claude is here, until an Anthropic-side broker exists.
//!
//! orthogonally, [`SandboxBackend`] decides HOW the child is spawned (Direct,
//! or a resource-capped Podman/Tart jail). the two compose: codex under Podman
//! gets the broker AND the jail.
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

/// the hard ceiling on one child's lifetime, as a multiple of its idle
/// timeout: `spec.timeout_secs` bounds SILENCE (any output refreshes it —
/// long agentic runs that keep streaming are never killed mid-work), and
/// `idle × this` bounds even a continuously-chatty child, guarding the
/// host's own resources. the RUN's committed outcome is bounded by the
/// saga's consensus deadline regardless (ADR X3) — this factor only decides
/// how long this host keeps paying for one child.
const HARD_TIMEOUT_FACTOR: u32 = 36;

/// how long a cancelled child process group gets to handle SIGTERM before the
/// host escalates to SIGKILL. Podman gets the same budget for each targeted
/// stop/wait operation; every cleanup command is itself kill-on-drop.
const TERMINATION_GRACE: Duration = Duration::from_secs(2);
const PODMAN_CONTROL_TIMEOUT: Duration = Duration::from_secs(3);
const PODMAN_CID_POLL_INTERVAL: Duration = Duration::from_millis(25);
const PODMAN_RETRY_MIN: Duration = Duration::from_millis(250);
const PODMAN_RETRY_MAX: Duration = Duration::from_secs(1);
const PODMAN_EMPTY_MIN_OBSERVATIONS: usize = 3;
const PODMAN_REAP_MAX: usize = 64;
const PODMAN_MANAGED_LABEL: &str = "io.ducktape.managed=capability-host";
const PODMAN_NODE_LABEL: &str = "io.ducktape.node";
const PODMAN_OWNER_BOOT_LABEL: &str = "io.ducktape.owner.boot";
const PODMAN_OWNER_PID_LABEL: &str = "io.ducktape.owner.pid";
const PODMAN_OWNER_START_LABEL: &str = "io.ducktape.owner.starttime";
const PODMAN_INSTANCE_LABEL: &str = "io.ducktape.instance";
const TART_SETUP_TIMEOUT: Duration = Duration::from_secs(90);
const TART_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// reserved run-local state INSIDE the run's workdir: the fresh provider config
/// home lives here (see [`CliProvider::prepare_config_home`]). the provisioner's
/// commit bracket removes this directory before duckfs/forge scan the tree, so a
/// provider's own runtime files can never become an agent artifact — which is
/// exactly why the config home is allowed to sit inside the workdir at all.
pub const RUN_RUNTIME_DIR: &str = ".ducktape-run";

/// the env var the provisioner exports to point a run at its read-only W6 skills
/// tree (`bin/noded/src/agent_provision.rs`, consumed by `bin/mcp`). the sandbox
/// backends read it to know what to MOUNT — see [`CliProvider::sandbox_ro_paths`].
const SKILLS_ROOT_ENV: &str = "DUCKTAPE_RUN_SKILLS";

/// the opaque per-run bearer the broker hands the child. NOT a credential: it
/// authenticates the child to this host's loopback endpoint and dies with the
/// run. the spec's argv names it (`env_key` in the model-provider block).
const BROKER_TOKEN_ENV: &str = "DUCKTAPE_MODEL_BROKER_TOKEN";
/// Separate run-local capability used only by the MCP control tool. These are
/// reserved: inherited or RunContext-supplied lookalikes are always removed.
const PROVIDER_CONTROL_URL_ENV: &str = "DUCKTAPE_PROVIDER_CONTROL_URL";
const PROVIDER_CONTROL_TOKEN_ENV: &str = "DUCKTAPE_PROVIDER_CONTROL_TOKEN";

/// the upstream credential env vars a broker takes over. the HOST reads these
/// (see [`broker::UpstreamCredential::from_host`]); the child must not see them,
/// or it would dial the provider directly and walk straight past the broker.
const UPSTREAM_CREDENTIAL_ENV: [&str; 1] = ["OPENAI_API_KEY"];

mod broker;
mod sandbox;
mod session;
mod spec;
mod variants;
mod workspace;
pub use sandbox::{SandboxBackend, TART_MIN_CORES, wrap_podman};
pub use session::{ResumeArgv, SessionCapture, SessionSpec};
pub use spec::{BrokerKind, CapabilitySpec, ContextLocation, IsolationSpec, OutputFormat, SpecSet};
pub use workspace::WorkspaceMode;

/// canonical label-safe identity for the node executing a provider run.
pub fn execution_node_id(identity: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(identity.len() * 2);
    for byte in identity {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

/// a cloneable, run-local cancellation signal. cancelling one clone wakes all
/// current waiters, and future waiters observe the already-cancelled state.
#[derive(Debug, Clone)]
pub struct RunCancellation {
    tx: tokio::sync::watch::Sender<bool>,
}

impl RunCancellation {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::watch::channel(false);
        Self { tx }
    }

    /// idempotently mark the run cancelled and wake every waiter.
    pub fn cancel(&self) {
        self.tx.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.tx.borrow()
    }

    /// wait until this run is cancelled. safe for late subscribers: cancellation
    /// is state, not an edge-triggered notification that can be missed.
    pub async fn cancelled(&self) {
        let mut rx = self.tx.subscribe();
        loop {
            if *rx.borrow() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Default for RunCancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for RunCancellation {
    fn eq(&self, other: &Self) -> bool {
        self.tx.same_channel(&other.tx)
    }
}

impl Eq for RunCancellation {}

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
    /// host-local cancellation for this live run. `None` preserves the legacy
    /// run-to-completion behavior; cancelling the token terminates the provider
    /// process tree and any managed Podman container.
    pub cancellation: Option<RunCancellation>,
    /// canonical [`execution_node_id`] of the node running this attempt. Direct
    /// runs may omit it; Podman requires it so lifecycle cleanup can never
    /// cross into another Ducktape node sharing the same rootless user.
    pub executing_node: Option<String>,
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
    /// the run's numeric resource demands (`ExecJob.demands`), keyed by
    /// dimension (`cores`, `mem_gb`, ...). the pool fills this before
    /// `provider.run`; under a `Podman` backend the dimensions this backend
    /// knows how to enforce become container limit flags, the rest are inert
    /// (scheduling already matched them). Default empty — a Direct backend
    /// ignores it entirely.
    pub limits: BTreeMap<String, u64>,
    /// true for portable v3 runs: native CLI sessions are host-local
    /// optimizations and must not be resumed or captured for portable state.
    pub portable: bool,
    /// the run's assembled context document — the agent's curated skills, built
    /// into ONE markdown doc by the provisioner (the "soul"). ONE assembly, TWO
    /// doors, and the SPEC picks which: a spec declaring `[context]` gets it
    /// written to the file its CLI already auto-loads (see
    /// [`ContextLocation`]); one that declares none gets it prepended to the
    /// stdin prompt. `None` (a probe or legacy run) means neither door does
    /// anything.
    pub context_doc: Option<String>,
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
    /// that predate workspaces/sessions may ignore it only when
    /// `ctx.cancellation` is `None`. Once cancellation is present, resolving
    /// this future means every execution resource owned by this run is proven
    /// stopped and its wait/reap completed. If that proof cannot be obtained,
    /// the implementation must remain pending fail-closed rather than return.
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

/// a sandbox backend's env overlay (`(key, value)` pairs) plus the spec's
/// `~/`-relative rw mount dirs expanded to absolute host paths.
type SandboxEnvRw = (Vec<(String, String)>, Vec<PathBuf>);

/// everything one run hands its child so the CLI can authenticate WITHOUT the
/// operator's credential — both `None` for a plain BYO spec (no `[isolation]`),
/// which is the historical posture and still the default.
///
/// this is the whole of it: a directory and run-local broker capabilities. the
/// upstream credential is not here and never will be — it stays in this process,
/// behind the [`broker`].
#[derive(Default)]
struct RunAuth<'a> {
    /// this run's FRESH config home (materialized under [`RUN_RUNTIME_DIR`]),
    /// exported as the spec's `isolation.config_home_env`. auth-load-bearing:
    /// it is what stops the CLI reading the operator's real config home, and so
    /// what forces it through the broker.
    config_home: Option<&'a Path>,
    /// this run's live broker endpoint, when the spec declares one.
    broker: Option<&'a broker::BrokerEndpoint>,
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
    /// how the child is spawned: `Direct`, rootless `Podman`, or an ephemeral
    /// Tart VM. set once at discovery for the whole provider set.
    backend: SandboxBackend,
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
            backend: SandboxBackend::Direct,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_workdir(mut self, workdir: PathBuf) -> Self {
        self.workdir = workdir;
        self
    }

    pub fn with_backend(mut self, backend: SandboxBackend) -> Self {
        self.backend = backend;
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

    #[cfg(test)]
    fn command(
        &self,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<tokio::process::Command, String> {
        self.prepared_command(args, workdir, ctx, auth)
            .map(|prepared| prepared.command)
    }

    fn prepared_command(
        &self,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<PreparedCommand, String> {
        // a broker rewrites the argv BEFORE the backend sees it, so all three
        // backends aim the CLI at the loopback endpoint identically.
        let args = self.broker_argv(args, workdir, auth);
        // the backend picks HOW the child is spawned; the stdio/kill/cwd
        // handling below is identical either way. args are passed verbatim to
        // exec — never shell-interpreted (the resume path's {session_id} slot
        // was substituted host-side with an executor-minted id, never job
        // content) — so nothing in a job can inject flags or commands.
        let (mut command, podman) = match &self.backend {
            SandboxBackend::Direct => (self.direct_command(&args, ctx, auth)?, None),
            SandboxBackend::Podman { image } => {
                let (command, run) =
                    self.podman_command(image, &args, workdir, ctx, auth)?;
                (command, Some(run))
            }
            // Tart has a multi-process lifecycle (clone/set/boot, then SSH), so
            // [`Self::invoke`] constructs it before reaching this single-child
            // Direct/Podman command seam.
            SandboxBackend::Tart { .. } => {
                return Err("internal error: Tart command bypassed its VM lifecycle".into());
            }
        };
        command
            .current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // final panic/drop safety; cancellation and timeouts take the
            // explicit process-group + Podman cleanup path in invoke().
            .kill_on_drop(true);
        Ok(PreparedCommand { command, podman })
    }

    /// the plain host spawn: the spec's binary with the spec's argv and an
    /// ADDITIVE env overlay (the inherited environment plus this run's scoped
    /// `ctx.env` / PATH bindings, plus [`Self::apply_auth_env`]). providers run
    /// the claude/codex CLI HEADLESS, so a CLI with no broker reads its OWN
    /// credentials from the ambient env or a dotfile under HOME (~/.claude) —
    /// the child MUST inherit that environment or it cannot authenticate.
    ///
    /// D7 isolation floor — the env half: hiding the node's ambient secrets
    /// (HOME => ~/.ducktape/user.key, DUCKTAPE_*, the data dir) from the child
    /// WITHOUT also hiding the operator's CLI credentials (same HOME) cannot be
    /// done with `env_clear` alone — it needs an enforcement MECHANISM. The
    /// `Podman` backend IS that mechanism (a fresh mount namespace exposing
    /// only the spec's `[sandbox] rw_dirs` under HOME); under `Direct` the
    /// ACTIVE D7 measure remains the WORKSPACE RELOCATION (a portable run's cwd
    /// is the provisioner's per-run mount OUTSIDE <storage>, so a `..` from the
    /// cwd no longer reaches the key tree).
    fn direct_command(
        &self,
        args: &[String],
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<tokio::process::Command, String> {
        let mut cmd = tokio::process::Command::new(&self.bin);
        cmd.args(args.iter());
        cmd.envs(ctx.env.iter());
        cmd.env_remove(PROVIDER_CONTROL_URL_ENV);
        cmd.env_remove(PROVIDER_CONTROL_TOKEN_ENV);
        // the child INHERITS this process's environment here, so a broker-backed
        // run must actively remove the upstream credential vars — see
        // [`Self::apply_auth_env`], where that subtraction is the load-bearing
        // half. (a sandbox backend has no such problem: its env is an allowlist.)
        self.apply_auth_env(auth, |k, v| {
            cmd.env(k, v);
        });
        if auth.broker.is_some() {
            for key in UPSTREAM_CREDENTIAL_ENV {
                cmd.env_remove(key);
            }
        }
        if let Some(path) = self.run_path(ctx)? {
            cmd.env("PATH", path);
        }
        Ok(cmd)
    }

    /// wrap the same invocation in `podman run`: identical container paths, the
    /// run's numeric limits enforced, and ONLY the spec's `[sandbox] rw_dirs`
    /// (the CLI's auth/state) crossing under HOME. HOME is set (so the CLI
    /// finds its dotfiles at their identical mounted paths) but not itself
    /// mounted — the node's data dir and user key stay outside (D7).
    ///
    /// a broker composes with this backend: `--network=host` leaves the host's
    /// loopback reachable from inside the container, so the child can dial the
    /// broker's `127.0.0.1:<port>` at the very address the argv names. Tart uses
    /// its separate host-gateway mapping in [`Self::start_broker`].
    fn podman_command(
        &self,
        image: &str,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<(tokio::process::Command, PodmanRun), String> {
        let (envs, rw_dirs) = self.sandbox_env_and_rw(ctx, auth)?;
        let ro_paths = self.sandbox_ro_paths(ctx, workdir, auth)?;
        let workdir = canonical_mount_path(workdir, "Podman workdir")?;
        let bin_path = canonical_mount_path(&self.bin, "Podman executor")?;
        let run = PodmanRun::new(ctx, &workdir, &bin_path, &ro_paths, &rw_dirs)?;
        let (bin, argv) = sandbox::wrap_podman_managed(
            image,
            &bin_path,
            args,
            &workdir,
            &envs,
            &ro_paths,
            &rw_dirs,
            &ctx.limits,
            &run.cidfile,
            &run.labels,
        );
        let mut cmd = tokio::process::Command::new(bin);
        cmd.args(argv)
            .envs(envs.iter().map(|(key, value)| (key, value)));
        Ok((cmd, run))
    }

    /// Assemble the real Tart VM boot + guest execution plan. Writable auth
    /// directories are created before they become virtiofs sources, and the
    /// executor is canonicalized so a Homebrew symlink cannot point outside
    /// the read-only mount presented to the guest.
    fn tart_plan(
        &self,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<sandbox::TartPlan, String> {
        let (envs, rw_dirs) = self.sandbox_env_and_rw(ctx, auth)?;
        for dir in &rw_dirs {
            std::fs::create_dir_all(dir).map_err(|e| {
                format!(
                    "{}: create Tart auth mount {}: {e}",
                    self.spec.tag,
                    dir.display()
                )
            })?;
        }
        let bin = std::fs::canonicalize(&self.bin).map_err(|e| {
            format!(
                "{}: resolve Tart executor {}: {e}",
                self.spec.tag,
                self.bin.display()
            )
        })?;
        static NEXT_VM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let vm = format!(
            "ducktape-{}-{}",
            std::process::id(),
            NEXT_VM.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        sandbox::tart_plan(
            &vm,
            &bin,
            args,
            workdir,
            &envs,
            &self.sandbox_ro_paths(ctx, workdir, auth)?,
            &rw_dirs,
        )
    }

    /// the env carried into a sandbox + the spec's `~/` rw_dirs expanded against
    /// $HOME at their identical host paths — shared by the Podman and Tart
    /// backends. both need the CLI's auth dotfiles under a SET HOME (so the CLI
    /// finds them at their mounted paths) while deliberately NOT carrying the
    /// node's ambient secrets; $HOME itself is never mounted (D7). $HOME unset
    /// is a loud error, not a silent unsandboxed fallback.
    ///
    /// this env is an ALLOWLIST — a sandboxed child inherits nothing it is not
    /// handed here — so a broker's upstream credential vars are excluded by
    /// simply never being added, with no subtraction step to forget.
    fn sandbox_env_and_rw(
        &self,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<SandboxEnvRw, String> {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            format!(
                "{}: a sandbox backend needs $HOME set to mount the CLI's auth dirs",
                self.spec.tag
            )
        })?;
        let home = canonical_mount_path(&home, "sandbox HOME")?;
        let mut envs: Vec<(String, String)> = vec![("HOME".into(), home.display().to_string())];
        if let Some(path) = self.run_path(ctx)? {
            envs.push(("PATH".into(), path.to_string_lossy().into_owned()));
        }
        for (key, value) in &ctx.env {
            if key == "HOME"
                || key == PROVIDER_CONTROL_URL_ENV
                || key == PROVIDER_CONTROL_TOKEN_ENV
            {
                continue;
            }
            let value = if key == SKILLS_ROOT_ENV {
                canonical_mount_path(Path::new(value), "sandbox skills root")?
                    .display()
                    .to_string()
            } else {
                value.clone()
            };
            envs.push((key.clone(), value));
        }
        self.apply_auth_env(auth, |k, v| envs.push((k.to_string(), v)));
        // spec.rs already rejected absolute / `..` entries, so join is safe.
        let rw_dirs: Vec<PathBuf> = self
            .spec
            .rw_dirs
            .iter()
            .map(|d| {
                let path = home.join(d.strip_prefix("~/").unwrap_or(d));
                std::fs::create_dir_all(&path).map_err(|error| {
                    format!("create sandbox writable mount {}: {error}", path.display())
                })?;
                let resolved = canonical_mount_path(&path, "sandbox writable mount")?;
                if resolved != path {
                    return Err(format!(
                        "sandbox writable mount {} resolves to {}; symlinked auth mounts are refused",
                        path.display(),
                        resolved.display()
                    ));
                }
                Ok(resolved)
            })
            .collect::<Result<_, _>>()?;
        Ok((envs, rw_dirs))
    }

    /// the paths mounted READ-ONLY into a sandbox: the run's PATH entries (its
    /// tool bindings) plus the W6 skills tree, when the provisioner mounted one.
    ///
    /// the provisioner materializes the skill ro-mounts at a SIBLING of the rw
    /// checkout (`<slug>-ro/<name>` — deliberately OUTSIDE the workdir, so
    /// `commit` never scans them) and points the child at it with
    /// [`SKILLS_ROOT_ENV`]. under Direct that env is the whole mechanism: the
    /// path is right there on the host. inside a container/VM only what we mount
    /// exists — so without this the agent's own skills would be a DANGLING path:
    /// the env var set, the directory simply absent.
    ///
    /// the run's context doc joins them when it lands OUTSIDE the workdir (a
    /// `workspace-parent:` soul does; a `config-home:` one is under the workdir,
    /// which every backend already mounts, so it crosses for free). without this
    /// the file would exist on the host and simply not be there for the child —
    /// a silently unsouled agent, the one failure mode this feature must not have.
    fn sandbox_ro_paths(
        &self,
        ctx: &RunContext,
        workdir: &Path,
        auth: &RunAuth<'_>,
    ) -> Result<Vec<PathBuf>, String> {
        let mut paths = ctx.path_entries.clone();
        paths.extend(ctx.env.get(SKILLS_ROOT_ENV).map(PathBuf::from));
        if ctx.context_doc.is_some()
            && let Some(doc) = self.context_target(workdir, auth.config_home)?
            && !doc.starts_with(workdir)
        {
            paths.push(doc);
        }
        if matches!(self.backend, SandboxBackend::Direct) {
            Ok(paths)
        } else {
            paths
                .into_iter()
                .map(|path| canonical_mount_path(&path, "sandbox read-only mount"))
                .collect()
        }
    }

    /// where this run's assembled context doc lands — the spec's [`ContextLocation`]
    /// resolved against THIS run's directories. `None` when the spec names no
    /// `[context]` (that spec's door is the stdin prompt instead).
    fn context_target(
        &self,
        workdir: &Path,
        config_home: Option<&Path>,
    ) -> Result<Option<PathBuf>, String> {
        let dir = match &self.spec.context {
            None => return Ok(None),
            // parse-time guaranteed: `config-home:` requires isolation.config_home_env,
            // which is exactly what makes prepare_config_home materialize one. so this
            // is unreachable-by-construction, and loud rather than silently unsouled.
            Some(ContextLocation::ConfigHome(file)) => config_home
                .ok_or_else(|| {
                    format!(
                        "{}: context.path names the run's config home, but none was \
                         prepared for this run",
                        self.spec.tag
                    )
                })?
                .join(file),
            Some(ContextLocation::WorkspaceParent(file)) => workdir
                .parent()
                .ok_or_else(|| {
                    format!(
                        "{}: context.path names the parent of the run's workdir, but \
                         {} has none",
                        self.spec.tag,
                        workdir.display()
                    )
                })?
                .join(file),
        };
        Ok(Some(dir))
    }

    /// write the run's assembled context doc where the spec says the executor
    /// auto-loads it from, BEFORE the child starts. a write failure fails the run
    /// loudly — a silently unsouled agent is a different agent, and it would still
    /// answer, so the failure would never surface.
    ///
    /// the returned guard removes the file when it lives outside the workdir (see
    /// [`ContextGuard`]); a doc inside the workdir needs none.
    fn deliver_context(
        &self,
        workdir: &Path,
        config_home: Option<&Path>,
        ctx: &RunContext,
    ) -> Result<Option<ContextGuard>, String> {
        let (Some(doc), Some(path)) = (
            ctx.context_doc.as_ref(),
            self.context_target(workdir, config_home)?,
        ) else {
            return Ok(None);
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "{}: preparing {} for the run's context document failed: {e}",
                    self.spec.tag,
                    parent.display()
                )
            })?;
        }
        std::fs::write(&path, doc).map_err(|e| {
            format!(
                "{}: writing the run's context document to {} failed: {e} — refusing \
                 to run the agent without the context it was assembled with",
                self.spec.tag,
                path.display()
            )
        })?;
        // `then`, NOT `then_some`: the guard DELETES the file on drop, and
        // `then_some` builds its argument eagerly — so a doc inside the workdir
        // (the false arm) would construct a guard, drop it on the spot, and erase
        // the soul it had just written. it did, until this line.
        Ok((!path.starts_with(workdir)).then(|| ContextGuard(path)))
    }

    /// the prompt as the child receives it: for a spec with NO `[context]` (a raw
    /// provider — no ambient-instructions convention to use), the assembled doc
    /// is prepended to the stdin prompt. a spec WITH `[context]` already got it as
    /// a file, so prepending too would ship the soul twice.
    fn prompt_with_context(&self, prompt: &str, ctx: &RunContext) -> String {
        match (&self.spec.context, &ctx.context_doc) {
            (None, Some(doc)) => format!("{doc}\n\n{prompt}"),
            _ => prompt.to_string(),
        }
    }

    /// materialize this run's FRESH executor config home — `None` unless the
    /// spec names one. it lands under [`RUN_RUNTIME_DIR`] INSIDE the workdir
    /// (0700), which is deliberate on two counts: the workdir is the one path
    /// every backend already mounts rw, so the child reaches the dir at the same
    /// path in a container as on the host; and the provisioner's commit bracket
    /// deletes that reserved dir before scanning, so the CLI's own runtime files
    /// never land in a snapshot or a commit.
    ///
    /// the directory is EMPTY, and that is the auth-load-bearing part: pointed
    /// at an empty `CODEX_HOME`, codex cannot read the operator's `auth.json`
    /// and must use the model provider the broker argv names.
    fn prepare_config_home(
        &self,
        workdir: &Path,
        ctx: &RunContext,
    ) -> Result<Option<PathBuf>, String> {
        if self.spec.isolation.config_home_env.is_none() {
            return Ok(None);
        }
        let dir = workdir
            .join(RUN_RUNTIME_DIR)
            .join(runtime_slot(ctx, workdir))
            .join("provider-config");
        create_private_dir(&dir)?;
        Ok(Some(dir))
    }

    /// start this run's credential broker — `None` unless the spec declares one.
    /// the broker reads the operator's credential HERE, in the host process, and
    /// serves an endpoint the child dials with an opaque per-run bearer; dropping
    /// it (any exit path of [`Self::run_output`]) tears the endpoint down. Tart
    /// binds the host side of its private NAT; the guest plan maps that gateway
    /// to `ducktape-host`. Direct/Podman remain loopback-only.
    async fn start_broker(&self) -> Result<Option<broker::RunBroker>, String> {
        let Some(kind) = self.spec.isolation.broker else {
            return Ok(None);
        };
        match kind {
            BrokerKind::CodexResponses => {
                if matches!(self.backend, SandboxBackend::Tart { .. }) {
                    broker::RunBroker::start_for_tart().await.map(Some)
                } else {
                    broker::RunBroker::start().await.map(Some)
                }
            }
        }
    }

    /// the run's auth env, backend-independent: the fresh config home (so the
    /// CLI cannot read the operator's real one), the broker's model bearer, and
    /// its separately-scoped provider-control capability. `set` is how the
    /// caller applies one binding — a `Command` env for Direct, or the Podman
    /// process environment behind a value-free `-e K`.
    ///
    /// NOTE what is NOT here: the credential itself. that is the whole point —
    /// the host holds it and the broker spends it, so there is nothing to pass.
    fn apply_auth_env(&self, auth: &RunAuth<'_>, mut set: impl FnMut(&str, String)) {
        if let (Some(name), Some(dir)) = (
            self.spec.isolation.config_home_env.as_deref(),
            auth.config_home,
        ) {
            set(name, dir.display().to_string());
        }
        if let Some(broker) = auth.broker {
            set(BROKER_TOKEN_ENV, broker.run_bearer.clone());
            set(PROVIDER_CONTROL_URL_ENV, broker.control_url.clone());
            set(
                PROVIDER_CONTROL_TOKEN_ENV,
                broker.control_token.clone(),
            );
        }
    }

    /// point the executor at this run's broker, by splicing a custom model
    /// provider in after the subcommand selector (`args[0]`, e.g. `exec`) —
    /// where codex expects its `-c` overrides. a no-op without a broker.
    ///
    /// the child is given a base URL and [`BROKER_TOKEN_ENV`], and neither can
    /// recover the operator's credential: the bearer is 32 random bytes minted
    /// for this run, and the endpoint dies with it.
    fn broker_argv(&self, args: &[String], workdir: &Path, auth: &RunAuth<'_>) -> Vec<String> {
        let (Some(broker), Some(selector)) = (auth.broker, args.first()) else {
            return args.to_vec();
        };
        // the workdir is a path, and codex keys `projects.<key>` by TOML string —
        // so it must be QUOTED as one (a bare path breaks the `-c` parse).
        let project_key = toml::Value::String(workdir.to_string_lossy().into_owned()).to_string();
        let mut argv = vec![
            selector.clone(),
            "-c".into(),
            format!(
                "model_providers.ducktape={{ name=\"Ducktape run broker\", base_url=\"{}\", wire_api=\"responses\", env_key=\"{BROKER_TOKEN_ENV}\", request_max_retries=0, stream_max_retries=0 }}",
                broker.base_url
            ),
            "-c".into(),
            "model_provider=\"ducktape\"".into(),
            "-c".into(),
            format!("projects.{project_key}.trust_level=\"untrusted\""),
        ];
        argv.extend(args.iter().skip(1).cloned());
        argv
    }

    /// The complete Tart lifecycle up to a guest ready for work: concurrency
    /// permit, COW clone, `tart set`, headless boot with virtiofs mounts,
    /// `tart ip --wait`, and a real SSH readiness probe. Every failure after
    /// clone is guarded by stop/delete cleanup; there is no Direct fallback.
    async fn tart_setup(
        &self,
        plan: Option<&sandbox::TartPlan>,
        ctx: &RunContext,
    ) -> Result<Option<TartGuard>, String> {
        let SandboxBackend::Tart { image } = &self.backend else {
            return Ok(None);
        };
        let plan = plan.ok_or_else(|| "internal error: missing Tart execution plan".to_string())?;
        let vm = &plan.vm;
        let set_argv = sandbox::tart_set_argv(vm, &ctx.limits)
            .map_err(|error| format!("{}: {error}", self.spec.tag))?;
        // WAITS if 2 tart runs are already live — this is the cap, not an error.
        let permit = tokio::select! {
            permit = sandbox::tart_semaphore().acquire() => permit
                .map_err(|e| format!("{}: tart concurrency gate closed: {e}", self.spec.tag))?,
            _ = cancellation_requested(ctx.cancellation.as_ref()) => {
                return Err(format!("{}: Tart setup cancelled at concurrency gate", self.spec.tag));
            }
        };
        // Install the guard before clone starts: a cancelled/dropped clone may
        // have created a partial VM even when it never returns success.
        let mut guard = TartGuard {
            vm: vm.clone(),
            ip: String::new(),
            run: None,
            setup: None,
            vm_may_exist: false,
            _permit: permit,
        };
        let clone_args = vec!["clone".into(), image.clone(), vm.clone()];
        let output = guard
            .setup_command(
                "tart",
                &clone_args,
                ctx.cancellation.as_ref(),
                TART_SETUP_TIMEOUT,
                false,
            )
            .await
            .map_err(|error| format!("{}: {error}", self.spec.tag))?;
        if !output.status.success() {
            return Err(format!(
                "{}: `tart clone {image} {vm}` exited with {}",
                self.spec.tag, output.status
            ));
        }

        if let Some(set_argv) = set_argv {
            let output = guard
                .setup_command(
                    "tart",
                    &set_argv,
                    ctx.cancellation.as_ref(),
                    TART_SETUP_TIMEOUT,
                    false,
                )
                .await
                .map_err(|error| format!("{}: {error}", self.spec.tag))?;
            if !output.status.success() {
                return Err(format!(
                    "{}: `tart set {vm}` exited with {}",
                    self.spec.tag, output.status
                ));
            }
        }

        guard.run = Some(
            GroupChild::spawn("tart", &plan.run_argv, false)
                .map_err(|e| format!("{}: `tart run {vm}` failed to spawn: {e}", self.spec.tag))?,
        );

        let ip_args = vec!["ip".into(), vm.clone(), "--wait".into(), "60".into()];
        let output = guard
            .setup_command(
                "tart",
                &ip_args,
                ctx.cancellation.as_ref(),
                TART_SETUP_TIMEOUT,
                true,
            )
            .await
            .map_err(|error| format!("{}: {error}", self.spec.tag))?;
        if !output.status.success() {
            return Err(format!(
                "{}: `tart ip {vm} --wait 60` exited with {}: {}",
                self.spec.tag,
                output.status,
                excerpt(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        guard.ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if guard.ip.is_empty() {
            return Err(format!(
                "{}: `tart ip {vm}` returned no address",
                self.spec.tag
            ));
        }

        for attempt in 0..30 {
            let ssh_args = sandbox::tart_ssh_argv(&guard.ip, "true");
            let status = guard
                .setup_command(
                    "sshpass",
                    &ssh_args,
                    ctx.cancellation.as_ref(),
                    TART_PROBE_TIMEOUT,
                    false,
                )
                .await;
            match status {
                Ok(output) if output.status.success() => return Ok(Some(guard)),
                Err(error)
                    if error.contains("failed to spawn")
                        && (error.contains("not found")
                            || error.contains("No such file")
                            || error.contains("os error 2")) =>
                {
                    return Err(format!(
                        "{}: sshpass is required for Tart guest execution; install it with \
                         `brew install cirruslabs/cli/sshpass`: {error}",
                        self.spec.tag
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "{}: Tart SSH readiness probe failed: {error}",
                        self.spec.tag
                    ));
                }
                Ok(_) => {}
            }
            #[cfg(unix)]
            let run_exited = {
                let group = guard
                    .run
                    .as_ref()
                    .and_then(|run| run.process_group)
                    .expect("Tart run has an owned process group");
                leader_exited_unreaped(group)
                    .map_err(|e| format!("{}: inspect `tart run {vm}`: {e}", self.spec.tag))?
            };
            #[cfg(not(unix))]
            let run_exited = guard
                .run
                .as_mut()
                .expect("Tart run child is installed")
                .child
                .as_mut()
                .expect("Tart run process is installed")
                .try_wait()
                .map_err(|e| format!("{}: inspect `tart run {vm}`: {e}", self.spec.tag))?
                .is_some();
            if run_exited {
                return Err(format!(
                    "{}: `tart run {vm}` exited during boot",
                    self.spec.tag
                ));
            }
            if attempt < 29 {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                    _ = cancellation_requested(ctx.cancellation.as_ref()) => {
                        return Err(format!("{}: Tart SSH readiness cancelled", self.spec.tag));
                    }
                }
            }
        }
        Err(format!(
            "{}: Tart VM {vm} got IP {} but SSH did not become ready within 30s",
            self.spec.tag, guard.ip
        ))
    }

    /// the run-scoped PATH: `ctx.path_entries` prepended to the inherited PATH,
    /// or `None` when the run adds no entries. shared by both backends (Direct
    /// sets it as the child's PATH env; Podman exports it via `-e PATH`).
    fn run_path(&self, ctx: &RunContext) -> Result<Option<OsString>, String> {
        if ctx.path_entries.is_empty() {
            return Ok(None);
        }
        let mut path = if matches!(self.backend, SandboxBackend::Direct) {
            ctx.path_entries.clone()
        } else {
            ctx.path_entries
                .iter()
                .map(|path| canonical_mount_path(path, "sandbox PATH mount"))
                .collect::<Result<Vec<_>, _>>()?
        };
        if let Some(existing) = std::env::var_os("PATH") {
            path.extend(std::env::split_paths(&existing));
        }
        std::env::join_paths(path).map(Some).map_err(|e| {
            format!(
                "run-local PATH for {} contains an invalid path entry: {e}",
                self.spec.tag
            )
        })
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

/// this run's subdirectory under [`RUN_RUNTIME_DIR`]. distinct runs can share a
/// workdir (a persistent per-agent workspace serves every run of that agent, and
/// the scratch dir is shared per tag), so the slot keeps two concurrent runs from
/// stepping on each other's config home. deterministic, never random.
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

/// create a directory only this user can read. the provider's config home holds
/// whatever session/state the CLI writes for the run; 0700 keeps it off other
/// local accounts even on a shared box.
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

/// Resolve every sandbox mount before handing it to a container/VM. Relative
/// paths and symlink aliases make containment checks lie about which host tree
/// is exposed, so they fail before a sandbox command is built.
fn canonical_mount_path(path: &Path, purpose: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{purpose} must be absolute, got {}",
            path.display()
        ));
    }
    std::fs::canonicalize(path)
        .map_err(|error| format!("resolve {purpose} {}: {error}", path.display()))
}

fn ensure_podman_control_hidden<'a>(
    root: &Path,
    mounts: impl IntoIterator<Item = &'a Path>,
) -> Result<(), String> {
    for mount in mounts {
        let mount = canonical_mount_path(mount, "Podman host mount")?;
        if root.starts_with(&mount) {
            return Err(format!(
                "refusing Podman run: host-only cidfile root {} would be exposed by mount {}",
                root.display(),
                mount.display()
            ));
        }
    }
    Ok(())
}

/// removes a run's context document on drop — every exit path (success, error,
/// timeout, panic). only built for a doc OUTSIDE the workdir (`workspace-parent:`):
/// it sits beside the checkout, where nothing else would ever clean it up, and a
/// stale soul left there would silently join the NEXT run whose checkout lands in
/// the same parent. a `config-home:` doc is inside the reserved run-runtime dir
/// the provisioner already deletes, so it needs no guard.
struct ContextGuard(PathBuf);

impl Drop for ContextGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.0) {
            eprintln!(
                "[capability-host] removing the run's context document {} failed: {e}",
                self.0.display()
            );
        }
    }
}

/// A prepared provider command plus the host-only Podman identity needed to
/// stop the exact container if this invocation is cancelled.
struct PreparedCommand {
    command: tokio::process::Command,
    podman: Option<PodmanRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PodmanOwner {
    boot_id: String,
    pid: u32,
    starttime: u64,
}

impl PodmanOwner {
    #[cfg(target_os = "linux")]
    fn current() -> Result<Self, String> {
        let pid = std::process::id();
        Ok(Self {
            boot_id: linux_boot_id()?,
            pid,
            starttime: linux_process_starttime(pid)?.ok_or_else(|| {
                format!("current process {pid} disappeared while building Podman ownership")
            })?,
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn current() -> Result<Self, String> {
        // Non-Linux hosts still use exact cidfile/label cancellation. Startup
        // reaping is disabled there; these labels are diagnostic only.
        let pid = std::process::id();
        Ok(Self {
            boot_id: format!("{pid:08x}"),
            pid,
            starttime: 1,
        })
    }

    fn from_inspect(output: &str) -> Option<Self> {
        let mut fields = output.trim().split('\t');
        let boot_id = fields.next()?.to_string();
        let pid = fields.next()?.parse().ok()?;
        let starttime = fields.next()?.parse().ok()?;
        (fields.next().is_none()
            && valid_boot_id(&boot_id)
            && pid != 0
            && starttime != 0)
            .then_some(Self {
                boot_id,
                pid,
                starttime,
            })
    }
}

/// Host-owned Podman lifecycle state. The cidfile deliberately lives below
/// `$HOME/.ducktape/provider-runs`, which the Podman sandbox does not mount;
/// we also reject any spec/run mount that would expose it.
struct PodmanRun {
    cidfile: PathBuf,
    labels: Vec<String>,
}

impl PodmanRun {
    fn new(
        ctx: &RunContext,
        workdir: &Path,
        bin: &Path,
        ro_paths: &[PathBuf],
        rw_dirs: &[PathBuf],
    ) -> Result<Self, String> {
        static NEXT_RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            "Podman lifecycle isolation needs $HOME set for its host-only cidfile".to_string()
        })?;
        let home = canonical_mount_path(&home, "Podman HOME")?;
        let root = home
            .join(".ducktape")
            .join("provider-runs")
            .join("podman");
        create_private_dir(&root)?;
        let root = canonical_mount_path(&root, "Podman cidfile root")?;
        let owner = PodmanOwner::current()?;
        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("Podman run clock is before Unix epoch: {error}"))?
            .as_nanos();
        let instance = format!(
            "{}-{}-{}-{created}-{}",
            owner.boot_id,
            owner.pid,
            owner.starttime,
            NEXT_RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let cidfile = root.join(format!("{instance}.cid"));
        let executing_node = ctx.executing_node.as_deref().ok_or_else(|| {
            "Podman provider run is missing its canonical executing-node id".to_string()
        })?;
        if !valid_execution_node_id(executing_node) {
            return Err(format!(
                "Podman provider run has invalid executing-node id {executing_node:?}"
            ));
        }

        let exposed = std::iter::once(workdir)
            .chain(std::iter::once(bin))
            .chain(ro_paths.iter().map(PathBuf::as_path))
            .chain(rw_dirs.iter().map(PathBuf::as_path));
        ensure_podman_control_hidden(&root, exposed)?;

        Ok(Self {
            cidfile,
            labels: vec![
                PODMAN_MANAGED_LABEL.into(),
                format!("{PODMAN_NODE_LABEL}={executing_node}"),
                format!("{PODMAN_OWNER_BOOT_LABEL}={}", owner.boot_id),
                format!("{PODMAN_OWNER_PID_LABEL}={}", owner.pid),
                format!("{PODMAN_OWNER_START_LABEL}={}", owner.starttime),
                format!("io.ducktape.run={}", runtime_slot(ctx, workdir)),
                format!("{PODMAN_INSTANCE_LABEL}={instance}"),
            ],
        })
    }

    fn prepare_cidfile(&self) -> Result<(), String> {
        let root = self
            .cidfile
            .parent()
            .expect("a Podman cidfile always has its managed root");
        create_private_dir(root)?;
        self.remove_cidfile();
        Ok(())
    }

    fn container_id(&self) -> Option<String> {
        let id = std::fs::read_to_string(&self.cidfile).ok()?;
        let id = id.trim();
        valid_container_id(id).then(|| id.to_string())
    }

    async fn wait_container_id_until(&self, deadline: tokio::time::Instant) -> Option<String> {
        loop {
            if let Some(cid) = self.container_id() {
                return Some(cid);
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return None;
            }
            tokio::time::sleep_until((now + PODMAN_CID_POLL_INTERVAL).min(deadline)).await;
        }
    }

    fn stop_argv(cid: &str) -> Vec<String> {
        vec![
            "stop".into(),
            "--ignore".into(),
            "--time".into(),
            TERMINATION_GRACE.as_secs().to_string(),
            cid.into(),
        ]
    }

    fn wait_argv(cid: &str) -> Vec<String> {
        vec!["wait".into(), cid.into()]
    }

    fn remove_argv(cid: &str) -> Vec<String> {
        vec!["rm".into(), "--force".into(), "--ignore".into(), cid.into()]
    }

    async fn control(&self, args: &[String]) -> Result<String, String> {
        let args = args.to_vec();
        tokio::task::spawn_blocking(move || {
            run_command_bounded(Path::new("podman"), &args, PODMAN_CONTROL_TIMEOUT)
        })
        .await
        .map_err(|error| format!("join bounded Podman control command: {error}"))?
    }

    fn control_blocking(&self, args: &[String]) -> Result<String, String> {
        run_command_bounded(Path::new("podman"), args, PODMAN_CONTROL_TIMEOUT)
    }

    fn exact_query_argv(&self) -> Vec<String> {
        let mut argv = vec![
            "ps".into(),
            "--all".into(),
            "--quiet".into(),
            "--no-trunc".into(),
        ];
        for label in &self.labels {
            argv.extend(["--filter".into(), format!("label={label}")]);
        }
        argv
    }

    /// Prove this exact run's container is stopped or absent. `stop --ignore`
    /// success is the proof for a known CID. Before a CID exists, only the full
    /// unique label set may locate it; a stable sequence of successful exact
    /// empty queries after the `podman run` process is reaped proves absence.
    /// Any Podman error retries forever, deliberately retaining the caller's
    /// resource reservation rather than declaring an unproven stop.
    async fn stop_until_confirmed(&self, mut cid: Option<String>) -> Option<String> {
        let mut empty_since = None;
        let mut empty_observations = 0usize;
        let mut failures = 0u64;
        let mut retry_delay = PODMAN_RETRY_MIN;
        loop {
            if cid.is_none() {
                cid = self.container_id();
            }
            if let Some(id) = cid.as_deref() {
                match self.control(&Self::stop_argv(id)).await {
                    Ok(_) => return Some(id.to_string()),
                    Err(error) => {
                        failures += 1;
                        if failures == 1 || failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] exact Podman stop for {id} not yet proven \
                                 (attempt {failures}): {error}"
                            );
                        }
                    }
                }
            } else {
                match self
                    .control(&self.exact_query_argv())
                    .await
                    .and_then(|output| parse_container_ids(&output))
                {
                    Ok(ids) if ids.is_empty() => {
                        let since = empty_since.get_or_insert_with(tokio::time::Instant::now);
                        empty_observations += 1;
                        if empty_observations >= PODMAN_EMPTY_MIN_OBSERVATIONS
                            && since.elapsed() >= PODMAN_CONTROL_TIMEOUT
                        {
                            return None;
                        }
                    }
                    Ok(mut ids) if ids.len() == 1 => {
                        cid = ids.pop();
                        empty_since = None;
                        empty_observations = 0;
                    }
                    Ok(ids) => {
                        empty_since = None;
                        empty_observations = 0;
                        failures += 1;
                        if failures == 1 || failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] exact Podman identity matched {} containers; \
                                 refusing broad cleanup (attempt {failures})",
                                ids.len()
                            );
                        }
                    }
                    Err(error) => {
                        empty_since = None;
                        empty_observations = 0;
                        failures += 1;
                        if failures == 1 || failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] exact Podman absence query failed \
                                 (attempt {failures}): {error}"
                            );
                        }
                    }
                }
            }
            tokio::time::sleep(retry_delay).await;
            retry_delay = retry_delay.saturating_mul(2).min(PODMAN_RETRY_MAX);
        }
    }

    async fn cleanup_stopped(&self, cid: &str) {
        if let Err(error) = self.control(&Self::wait_argv(cid)).await {
            eprintln!("[capability-host] wait for stopped Podman container {cid}: {error}");
        }
        let mut failures = 0u64;
        let mut retry_delay = PODMAN_RETRY_MIN;
        loop {
            match self.control(&Self::remove_argv(cid)).await {
                Ok(_) => return,
                Err(error) => {
                    failures += 1;
                    if failures == 1 || failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] remove stopped Podman container {cid} \
                             failed (attempt {failures}): {error}"
                        );
                    }
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = retry_delay.saturating_mul(2).min(PODMAN_RETRY_MAX);
                }
            }
        }
    }

    /// Synchronous twin used only from a dropped invoke future. Drop cannot
    /// await, and returning would release the dispatch reservation, so an
    /// unavailable Podman daemon deliberately keeps retrying fail-closed.
    fn cleanup_blocking(&self) {
        let mut cid = self.container_id();
        let mut empty_since = None;
        let mut empty_observations = 0usize;
        let mut failures = 0u64;
        let mut retry_delay = PODMAN_RETRY_MIN;
        loop {
            if cid.is_none() {
                cid = self.container_id();
            }
            if let Some(id) = cid.as_deref() {
                match self.control_blocking(&Self::stop_argv(id)) {
                    Ok(_) => break,
                    Err(error) => {
                        failures += 1;
                        if failures == 1 || failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] dropped-run Podman stop for {id} \
                                 not yet proven (attempt {failures}): {error}"
                            );
                        }
                    }
                }
            } else {
                match self
                    .control_blocking(&self.exact_query_argv())
                    .and_then(|output| parse_container_ids(&output))
                {
                    Ok(ids) if ids.is_empty() => {
                        let since = empty_since.get_or_insert_with(std::time::Instant::now);
                        empty_observations += 1;
                        if empty_observations >= PODMAN_EMPTY_MIN_OBSERVATIONS
                            && since.elapsed() >= PODMAN_CONTROL_TIMEOUT
                        {
                            self.remove_cidfile();
                            return;
                        }
                    }
                    Ok(mut ids) if ids.len() == 1 => {
                        cid = ids.pop();
                        empty_since = None;
                        empty_observations = 0;
                    }
                    Ok(ids) => {
                        empty_since = None;
                        empty_observations = 0;
                        failures += 1;
                        if failures == 1 || failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] dropped-run identity matched {} containers; \
                                 refusing broad cleanup (attempt {failures})",
                                ids.len()
                            );
                        }
                    }
                    Err(error) => {
                        empty_since = None;
                        empty_observations = 0;
                        failures += 1;
                        if failures == 1 || failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] dropped-run Podman absence query failed \
                                 (attempt {failures}): {error}"
                            );
                        }
                    }
                }
            }
            std::thread::sleep(retry_delay);
            retry_delay = retry_delay.saturating_mul(2).min(PODMAN_RETRY_MAX);
        }

        let cid = cid.expect("a successful exact stop has a container id");
        let _ = self.control_blocking(&Self::wait_argv(&cid));
        let mut retry_delay = PODMAN_RETRY_MIN;
        loop {
            match self.control_blocking(&Self::remove_argv(&cid)) {
                Ok(_) => break,
                Err(error) => {
                    failures += 1;
                    if failures == 1 || failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] dropped-run Podman remove for {cid} failed \
                             (attempt {failures}): {error}"
                        );
                    }
                    std::thread::sleep(retry_delay);
                    retry_delay = retry_delay.saturating_mul(2).min(PODMAN_RETRY_MAX);
                }
            }
        }
        self.remove_cidfile();
    }

    fn remove_cidfile(&self) {
        if let Err(error) = std::fs::remove_file(&self.cidfile)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "[capability-host] removing Podman cidfile {} failed: {error}",
                self.cidfile.display()
            );
        }
    }
}

fn valid_container_id(id: &str) -> bool {
    id.len() >= 12 && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_execution_node_id(id: &str) -> bool {
    !id.is_empty()
        && id.len().is_multiple_of(2)
        && id.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_boot_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
}

#[cfg(target_os = "linux")]
fn linux_boot_id() -> Result<String, String> {
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|error| format!("read Linux boot id for Podman ownership: {error}"))?;
    let boot_id = boot_id.trim().to_ascii_lowercase();
    valid_boot_id(&boot_id)
        .then_some(boot_id)
        .ok_or_else(|| "Linux returned an invalid boot id for Podman ownership".into())
}

#[cfg(target_os = "linux")]
fn linux_process_starttime(pid: u32) -> Result<Option<u64>, String> {
    let path = format!("/proc/{pid}/stat");
    let stat = match std::fs::read_to_string(&path) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read {path} for Podman ownership: {error}")),
    };
    // `comm` is parenthesized and may itself contain spaces or `)`, so fields
    // after it start at the final `)`. starttime is proc stat field 22: index
    // 19 in the field-3-and-later suffix.
    let suffix = stat
        .rsplit_once(')')
        .map(|(_, suffix)| suffix)
        .ok_or_else(|| format!("{path} has no parenthesized command field"))?;
    let starttime = suffix
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| format!("{path} has no starttime field"))?
        .parse::<u64>()
        .map_err(|error| format!("parse {path} starttime: {error}"))?;
    Ok(Some(starttime))
}

fn parse_container_ids(output: &str) -> Result<Vec<String>, String> {
    output
        .lines()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| {
            valid_container_id(id)
                .then(|| id.to_string())
                .ok_or_else(|| format!("Podman returned an invalid container id: {id:?}"))
        })
        .collect()
}

fn podman_reap_query_argv(executing_node: &str) -> Vec<String> {
    vec![
        "ps".into(),
        "--all".into(),
        "--quiet".into(),
        "--no-trunc".into(),
        "--filter".into(),
        format!("label={PODMAN_MANAGED_LABEL}"),
        "--filter".into(),
        format!("label={PODMAN_NODE_LABEL}={executing_node}"),
    ]
}

fn podman_owner_inspect_argv(cid: &str) -> Vec<String> {
    vec![
        "inspect".into(),
        "--type".into(),
        "container".into(),
        "--format".into(),
        format!(
            "{{{{ index .Config.Labels \"{PODMAN_OWNER_BOOT_LABEL}\" }}}}\t\
             {{{{ index .Config.Labels \"{PODMAN_OWNER_PID_LABEL}\" }}}}\t\
             {{{{ index .Config.Labels \"{PODMAN_OWNER_START_LABEL}\" }}}}"
        ),
        cid.into(),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerLiveness {
    Alive,
    Dead,
    Unknown,
}

fn owner_liveness_with(
    owner: &PodmanOwner,
    current_boot_id: &str,
    process_starttime: &mut impl FnMut(u32) -> Result<Option<u64>, String>,
) -> OwnerLiveness {
    if owner.boot_id != current_boot_id {
        return OwnerLiveness::Dead;
    }
    match process_starttime(owner.pid) {
        Ok(Some(starttime)) if starttime == owner.starttime => OwnerLiveness::Alive,
        Ok(Some(_)) | Ok(None) => OwnerLiveness::Dead,
        Err(_) => OwnerLiveness::Unknown,
    }
}

/// Reap only a bounded set of containers whose Linux owner identity is proven
/// dead. A same-node live owner, a missing label, failed inspect, or unreadable
/// `/proc` record is preserved: node identity alone is never kill authority.
fn reap_podman_orphans_with(
    executing_node: &str,
    current_boot_id: &str,
    mut process_starttime: impl FnMut(u32) -> Result<Option<u64>, String>,
    mut run: impl FnMut(&[String]) -> Result<String, String>,
) -> Result<(), String> {
    if !valid_execution_node_id(executing_node) {
        return Err(format!(
            "refusing Podman reaper with invalid executing-node id {executing_node:?}"
        ));
    }
    if !valid_boot_id(current_boot_id) {
        return Err("refusing Podman reaper without a valid current boot id".into());
    }
    let query = podman_reap_query_argv(executing_node);
    let listed = run(&query)?;
    let ids = parse_container_ids(&listed)?;
    if ids.len() > PODMAN_REAP_MAX {
        return Err(format!(
            "refusing to inspect {} Podman containers; bounded reaper limit is {PODMAN_REAP_MAX}",
            ids.len()
        ));
    }
    for cid in ids {
        let owner = match run(&podman_owner_inspect_argv(&cid))
            .ok()
            .and_then(|output| PodmanOwner::from_inspect(&output))
        {
            Some(owner) => owner,
            None => continue,
        };
        if owner_liveness_with(&owner, current_boot_id, &mut process_starttime)
            != OwnerLiveness::Dead
        {
            continue;
        }
        // `stop --ignore` success proves this exact CID stopped or absent.
        // The managed run uses --rm, so it may auto-remove before `wait`; that
        // cleanup observation is deliberately non-fatal. `rm --ignore` then
        // proves no stopped metadata remains.
        run(&PodmanRun::stop_argv(&cid))?;
        let _ = run(&PodmanRun::wait_argv(&cid));
        run(&PodmanRun::remove_argv(&cid))?;
    }
    Ok(())
}

fn run_podman_bounded(args: &[String]) -> Result<String, String> {
    run_command_bounded(Path::new("podman"), args, PODMAN_CONTROL_TIMEOUT)
}

fn kill_std_child_fail_closed(
    child: &mut std::process::Child,
    group: u32,
    label: &str,
) -> std::process::ExitStatus {
    #[cfg(unix)]
    let _ = signal_process_group(group, libc::SIGKILL);
    let _ = child.kill();
    let mut failures = 0u64;
    loop {
        match child.wait() {
            Ok(status) => {
                #[cfg(unix)]
                wait_process_group_gone_blocking(group, label);
                return status;
            }
            Err(error) => {
                failures += 1;
                if failures == 1 || failures.is_multiple_of(16) {
                    eprintln!(
                        "[capability-host] wait/reap {label} failed \
                         (attempt {failures}): {error}"
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn run_command_bounded(
    program: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<String, String> {
    use std::io::Read as _;

    let display = format!("{} {}", program.display(), args.join(" "));
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("`{display}` failed to spawn: {error}"))?;
    let process_group = child.id();
    // Drain both pipes immediately. Waiting first deadlocks once either stream
    // fills its kernel pipe buffer (for example a large `podman ps` result).
    let stdout = child.stdout.take().expect("Podman stdout was piped");
    let stderr = child.stderr.take().expect("Podman stderr was piped");
    let stdout = std::thread::spawn(move || {
        let mut pipe = stdout;
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr = std::thread::spawn(move || {
        let mut pipe = stderr;
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = std::time::Instant::now() + timeout;
    let mut inspect_failures = 0u64;
    let status = loop {
        #[cfg(unix)]
        match leader_exited_unreaped(process_group) {
            Ok(true) => {
                if process_group_alive(process_group) {
                    let _ = signal_process_group(process_group, libc::SIGKILL);
                }
                let status = loop {
                    match child.wait() {
                        Ok(status) => break status,
                        Err(error) => {
                            inspect_failures += 1;
                            if inspect_failures == 1 || inspect_failures.is_multiple_of(16) {
                                eprintln!(
                                    "[capability-host] reap completed `{display}` \
                                     (attempt {inspect_failures}): {error}"
                                );
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                };
                wait_process_group_gone_blocking(process_group, &display);
                break Ok(status);
            }
            Ok(false) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(false) => {
                kill_std_child_fail_closed(&mut child, process_group, &display);
                break Err(format!("`{display}` exceeded {timeout:?}"));
            }
            Err(error) => {
                // Ownership could not be verified without consuming the
                // leader. Do not signal a numeric PGID on this path.
                inspect_failures += 1;
                if inspect_failures == 1 || inspect_failures.is_multiple_of(16) {
                    eprintln!(
                        "[capability-host] observe unreaped `{display}` \
                         (attempt {inspect_failures}): {error}"
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        #[cfg(not(unix))]
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                kill_std_child_fail_closed(&mut child, process_group, &display);
                break Err(format!("`{display}` exceeded {timeout:?}"));
            }
            Err(error) => {
                kill_std_child_fail_closed(&mut child, process_group, &display);
                break Err(format!("inspect `{display}`: {error}"));
            }
        }
    };
    let stdout = stdout
        .join()
        .map_err(|_| "Podman stdout reader panicked".to_string())?
        .map_err(|error| format!("read Podman stdout: {error}"))?;
    let stderr = stderr
        .join()
        .map_err(|_| "Podman stderr reader panicked".to_string())?
        .map_err(|error| format!("read Podman stderr: {error}"))?;
    let status = status?;
    if !status.success() {
        return Err(format!(
            "`{display}` exited with {status}: {}",
            excerpt(&String::from_utf8_lossy(&stderr))
        ));
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

#[cfg(target_os = "linux")]
fn reap_podman_orphans_once(executing_node: &str) -> Result<(), String> {
    struct Reap {
        executing_node: String,
        result: Result<(), String>,
    }
    static REAP: std::sync::OnceLock<Reap> = std::sync::OnceLock::new();
    let reap = REAP.get_or_init(|| Reap {
        executing_node: executing_node.to_string(),
        result: PodmanOwner::current().and_then(|owner| {
            reap_podman_orphans_with(
                executing_node,
                &owner.boot_id,
                linux_process_starttime,
                run_podman_bounded,
            )
        }),
    });
    if reap.executing_node != executing_node {
        return Err(format!(
            "Podman reaper already ran for node {}, refusing a second node {executing_node} in the same process",
            reap.executing_node
        ));
    }
    reap.result.clone()
}

#[cfg(not(target_os = "linux"))]
fn reap_podman_orphans_once(_executing_node: &str) -> Result<(), String> {
    // There is no /proc boot-id + starttime identity with which to distinguish
    // a live owner from PID reuse. Exact per-run cancellation still works, but
    // startup reaping is disabled fail-safe rather than guessing from PID.
    Ok(())
}

#[cfg(unix)]
fn configure_process_group(command: &mut tokio::process::Command) {
    use std::os::unix::process::CommandExt as _;
    command.as_std_mut().process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut tokio::process::Command) {}

#[cfg(unix)]
fn signal_process_group(group: u32, signal: libc::c_int) -> std::io::Result<()> {
    // SAFETY: kill is called with a valid signal and the negative child pid,
    // which targets only the process group created for this provider child.
    if unsafe { libc::kill(-(group as libc::pid_t), signal) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(unix)]
fn process_group_alive(group: u32) -> bool {
    // Signal 0 performs existence/permission checking without delivering a
    // signal. EPERM still means the group exists; ESRCH means it is gone.
    if unsafe { libc::kill(-(group as libc::pid_t), 0) } != 0 {
        return std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH);
    }

    // Linux keeps an orphaned zombie visible to kill(-pgid, 0) until PID 1
    // reaps it. A containerized test runner may itself be PID 1 and never reap
    // grandchildren, but zombies own no compute and cannot spawn descendants.
    // Treat a group containing only dead members as absent; any unreadable
    // /proc state stays fail-closed. Other Unix platforms retain kill(0).
    #[cfg(target_os = "linux")]
    return linux_process_group_has_live_member(group).unwrap_or(true);
    #[cfg(not(target_os = "linux"))]
    true
}

#[cfg(target_os = "linux")]
fn linux_process_group_has_live_member(group: u32) -> Option<bool> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        if entry.file_name().to_str()?.parse::<u32>().is_err() {
            continue;
        }
        let stat = match std::fs::read_to_string(entry.path().join("stat")) {
            Ok(stat) => stat,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        };
        let (state, process_group) = linux_process_state_and_group(&stat)?;
        if process_group == group && !matches!(state, 'Z' | 'X' | 'x') {
            return Some(true);
        }
    }
    Some(false)
}

#[cfg(target_os = "linux")]
fn linux_process_state_and_group(stat: &str) -> Option<(char, u32)> {
    // /proc/<pid>/stat starts `pid (comm) state ppid pgrp`; `comm` may contain
    // spaces and parentheses, so split after its final closing delimiter.
    let (_, fields) = stat.rsplit_once(") ")?;
    let mut fields = fields.split_whitespace();
    let state = fields.next()?.chars().next()?;
    fields.next()?; // ppid
    let process_group = fields.next()?.parse().ok()?;
    Some((state, process_group))
}

#[cfg(unix)]
fn leader_exited_unreaped(pid: u32) -> std::io::Result<bool> {
    // WNOWAIT observes exit without consuming it. Keeping the leader as a
    // zombie pins its PID/PGID while we decide whether its exact group still
    // needs a signal; only the final Child::wait opens a reuse window.
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    Ok(unsafe { info.si_pid() } != 0)
}

#[cfg(unix)]
async fn wait_leader_exit_unreaped(pid: u32, label: &str) {
    let mut failures = 0u64;
    loop {
        match leader_exited_unreaped(pid) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                failures += 1;
                if failures == 1 || failures.is_multiple_of(16) {
                    eprintln!(
                        "[capability-host] observe unreaped {label} exit \
                         (attempt {failures}): {error}"
                    );
                }
            }
        }
        tokio::time::sleep(PODMAN_CID_POLL_INTERVAL).await;
    }
}

#[cfg(unix)]
async fn wait_process_group_gone(group: u32, label: &str) {
    let mut observations = 0u64;
    while process_group_alive(group) {
        observations += 1;
        if observations == 1 || observations.is_multiple_of(160) {
            eprintln!(
                "[capability-host] waiting for {label} process group {group} to disappear"
            );
        }
        tokio::time::sleep(PODMAN_CID_POLL_INTERVAL).await;
    }
}

#[cfg(unix)]
fn wait_process_group_gone_blocking(group: u32, label: &str) {
    let mut observations = 0u64;
    while process_group_alive(group) {
        observations += 1;
        if observations == 1 || observations.is_multiple_of(400) {
            eprintln!(
                "[capability-host] waiting for {label} process group {group} to disappear"
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

async fn wait_tokio_child_fail_closed(
    child: &mut tokio::process::Child,
    label: &str,
) -> std::process::ExitStatus {
    let mut failures = 0u64;
    loop {
        match child.wait().await {
            Ok(status) => return status,
            Err(error) => {
                failures += 1;
                if failures == 1 || failures.is_multiple_of(16) {
                    eprintln!(
                        "[capability-host] wait/reap {label} failed \
                         (attempt {failures}): {error}"
                    );
                }
                tokio::time::sleep(PODMAN_CID_POLL_INTERVAL).await;
            }
        }
    }
}

#[cfg(unix)]
async fn reap_observed_child_group(
    child: &mut tokio::process::Child,
    group: u32,
    label: &str,
    leader_reaped: &mut bool,
) -> std::process::ExitStatus {
    // The leader was observed with WNOWAIT and still pins the group identity.
    // SIGKILL is a no-op for the zombie itself and removes any helper keeping
    // pipes or compute alive after the nominal command exited.
    if process_group_alive(group) {
        let _ = signal_process_group(group, libc::SIGKILL);
    }
    let status = wait_tokio_child_fail_closed(child, label).await;
    *leader_reaped = true;
    wait_process_group_gone(group, label).await;
    status
}

async fn wait_owned_child_complete(
    child: &mut tokio::process::Child,
    process_group: Option<u32>,
    label: &str,
    leader_reaped: &mut bool,
) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(unix)]
    {
        let group = process_group.ok_or_else(|| {
            std::io::Error::other(format!("{label} has no owned Unix process group"))
        })?;
        wait_leader_exit_unreaped(group, label).await;
        return Ok(reap_observed_child_group(child, group, label, leader_reaped).await);
    }
    #[cfg(not(unix))]
    {
        let _ = process_group;
        let _ = label;
        let status = child.wait().await?;
        *leader_reaped = true;
        Ok(status)
    }
}

/// Gracefully terminate the child tree, then forcibly reap anything that
/// ignored TERM. A SIGKILLed leader is always waited before return: a zombie is
/// therefore reaped, while an uninterruptible D-state leader intentionally
/// keeps this future (and its resource reservation) pending fail-closed.
/// Podman cleanup targets the exact CID/full unique run labels, never a node or
/// process-name match, and likewise does not return before stopped/absent proof.
async fn terminate_child(
    child: &mut tokio::process::Child,
    process_group: Option<u32>,
    podman: Option<&PodmanRun>,
    leader_reaped: &mut bool,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + TERMINATION_GRACE;
    #[cfg(unix)]
    if let Some(group) = process_group
        && let Err(error) = signal_process_group(group, libc::SIGTERM)
    {
        eprintln!("[capability-host] SIGTERM provider process group {group}: {error}");
    }

    // `podman run` may have created the container but not written --cidfile at
    // the instant cancellation wins. Poll within the same TERM grace window;
    // killing the CLI after one read can otherwise strand that exact race.
    let cid = match podman {
        Some(podman) => podman.wait_container_id_until(deadline).await,
        None => None,
    };
    #[cfg(unix)]
    {
        if let Some(group) = process_group {
            loop {
                match leader_exited_unreaped(group) {
                    Ok(true) => {
                        // The zombie leader still pins this PGID. Kill any
                        // helper that outlived it before performing the reap.
                        if process_group_alive(group) {
                            let _ = signal_process_group(group, libc::SIGKILL);
                        }
                        break;
                    }
                    Ok(false) if tokio::time::Instant::now() < deadline => {}
                    Ok(false) => {
                        if let Err(error) = signal_process_group(group, libc::SIGKILL) {
                            eprintln!(
                                "[capability-host] SIGKILL provider process group {group}: {error}"
                            );
                        }
                        let _ = child.start_kill();
                        break;
                    }
                    Err(error) => {
                        // ECHILD or an unreadable wait state makes ownership
                        // unverifiable. Retain the reservation fail-closed.
                        eprintln!(
                            "[capability-host] observe cancelled provider leader {group}: {error}"
                        );
                    }
                }
                tokio::time::sleep(PODMAN_CID_POLL_INTERVAL).await;
            }
            let _ = wait_tokio_child_fail_closed(child, "killed provider child").await;
            *leader_reaped = true;
            // Never signal after reap: only observe until the exact old group
            // is gone, so PID reuse cannot turn cleanup into an unrelated kill.
            wait_process_group_gone(group, "provider").await;
        } else {
            let _ = child.start_kill();
            let _ = wait_tokio_child_fail_closed(child, "killed provider child").await;
            *leader_reaped = true;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = child.start_kill();
        let _ = wait_tokio_child_fail_closed(child, "killed provider child").await;
        *leader_reaped = true;
    }

    cid
}

async fn cancellation_requested(cancellation: Option<&RunCancellation>) {
    match cancellation {
        Some(cancellation) => cancellation.cancelled().await,
        None => std::future::pending().await,
    }
}

struct GroupChild {
    child: Option<tokio::process::Child>,
    process_group: Option<u32>,
    leader_reaped: bool,
    cleaned: bool,
}

impl GroupChild {
    fn spawn(
        program: &str,
        args: &[String],
        capture: bool,
    ) -> Result<Self, std::io::Error> {
        let mut command = tokio::process::Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(if capture { Stdio::piped() } else { Stdio::null() })
            .stderr(if capture { Stdio::piped() } else { Stdio::null() })
            .kill_on_drop(true);
        configure_process_group(&mut command);
        let child = command.spawn()?;
        let process_group = child.id();
        Ok(Self {
            child: Some(child),
            process_group,
            leader_reaped: false,
            cleaned: false,
        })
    }

    fn kill_and_wait_blocking(&mut self) {
        if self.cleaned {
            return;
        }
        let Some(child) = self.child.as_mut() else {
            return;
        };
        #[cfg(unix)]
        if self.leader_reaped {
            if let Some(group) = self.process_group {
                // The PGID may now be reused. Never signal it; retain the
                // reservation until absence is observed fail-closed.
                wait_process_group_gone_blocking(group, "reaped setup");
            }
            self.cleaned = true;
            return;
        }
        #[cfg(unix)]
        if let Some(group) = self.process_group {
            let mut inspect_failures = 0u64;
            loop {
                match leader_exited_unreaped(group) {
                    Ok(_) => {
                        // Both a live and a zombie leader pin this exact PGID.
                        let _ = signal_process_group(group, libc::SIGKILL);
                        let _ = child.start_kill();
                        break;
                    }
                    Err(error) => {
                        inspect_failures += 1;
                        if inspect_failures == 1 || inspect_failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] inspect setup child before kill \
                                 (attempt {inspect_failures}): {error}"
                            );
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            let mut wait_failures = 0u64;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    Err(error) => {
                        wait_failures += 1;
                        if wait_failures == 1 || wait_failures.is_multiple_of(16) {
                            eprintln!(
                                "[capability-host] reap killed setup child \
                                 (attempt {wait_failures}): {error}"
                            );
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            self.leader_reaped = true;
            wait_process_group_gone_blocking(group, "setup");
            self.cleaned = true;
            return;
        }

        #[cfg(not(unix))]
        {
            let _ = child.start_kill();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) | Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
            self.cleaned = true;
            return;
        }

        #[cfg(unix)]
        let mut inspect_failures = 0u64;
        #[cfg(unix)]
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.leader_reaped = true;
                    self.cleaned = true;
                    return;
                }
                Ok(None) => break,
                Err(error) => {
                    inspect_failures += 1;
                    if inspect_failures == 1 || inspect_failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] inspect setup child before kill \
                             (attempt {inspect_failures}): {error}"
                        );
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        #[cfg(unix)]
        let _ = child.start_kill();
        #[cfg(unix)]
        let mut wait_failures = 0u64;
        #[cfg(unix)]
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.leader_reaped = true;
                    self.cleaned = true;
                    return;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    wait_failures += 1;
                    if wait_failures == 1 || wait_failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] reap killed setup child \
                             (attempt {wait_failures}): {error}"
                        );
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

impl Drop for GroupChild {
    fn drop(&mut self) {
        if !self.cleaned {
            self.kill_and_wait_blocking();
        }
    }
}

/// Drop owner for one live provider invocation. This is intentionally
/// synchronous on unexpected future destruction: returning from Drop would
/// release the outer dispatch reservation while descendants or a container
/// could still consume it.
struct LiveChild {
    process: GroupChild,
    podman: Option<PodmanRun>,
    cleaned: bool,
}

impl LiveChild {
    fn new(child: tokio::process::Child, podman: Option<PodmanRun>) -> Self {
        let process_group = child.id();
        Self {
            process: GroupChild {
                child: Some(child),
                process_group,
                leader_reaped: false,
                cleaned: false,
            },
            podman,
            cleaned: false,
        }
    }

    fn child_mut(&mut self) -> &mut tokio::process::Child {
        self.process
            .child
            .as_mut()
            .expect("a live invocation owns its child")
    }

    async fn terminate(&mut self) {
        let process_group = self.process.process_group;
        let cid = if self.process.cleaned {
            self.podman.as_ref().and_then(PodmanRun::container_id)
        } else if self.process.leader_reaped {
            #[cfg(unix)]
            if let Some(group) = process_group {
                wait_process_group_gone(group, "provider").await;
            }
            self.process.cleaned = true;
            self.podman.as_ref().and_then(PodmanRun::container_id)
        } else {
            let podman = self.podman.as_ref();
            let child = self
                .process
                .child
                .as_mut()
                .expect("a live invocation owns its child");
            terminate_child(
                child,
                process_group,
                podman,
                &mut self.process.leader_reaped,
            )
            .await
        };
        // terminate_child has reaped the leader and proved the group gone.
        // Mark that boundary before awaiting Podman, so aborting cleanup falls
        // through to this owner's exact synchronous container cleanup only.
        self.process.cleaned = true;
        if let Some(podman) = &self.podman {
            if let Some(cid) = podman.stop_until_confirmed(cid).await {
                podman.cleanup_stopped(&cid).await;
            }
            podman.remove_cidfile();
        }
        self.cleaned = true;
    }

    async fn wait_complete(&mut self, label: &str) -> std::io::Result<std::process::ExitStatus> {
        let process_group = self.process.process_group;
        let child = self
            .process
            .child
            .as_mut()
            .expect("a live invocation owns its child");
        let status = wait_owned_child_complete(
            child,
            process_group,
            label,
            &mut self.process.leader_reaped,
        )
        .await?;
        self.process.cleaned = true;
        // A successful Podman CLI exit is not by itself cleanup proof. Target
        // its exact CID/full label identity even on normal provider completion.
        if let Some(podman) = &self.podman {
            let cid = podman.stop_until_confirmed(podman.container_id()).await;
            if let Some(cid) = cid {
                podman.cleanup_stopped(&cid).await;
            }
            podman.remove_cidfile();
        }
        self.cleaned = true;
        Ok(status)
    }
}

impl Drop for LiveChild {
    fn drop(&mut self) {
        if self.cleaned {
            return;
        }
        self.process.kill_and_wait_blocking();
        if let Some(podman) = &self.podman {
            podman.cleanup_blocking();
        }
        self.cleaned = true;
    }
}

struct SetupOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Owns the boot process and concurrency permit for one ephemeral Tart VM.
/// `tart run` is the foreground VM compute owner (Tart has no detached daemon
/// in this lifecycle): its isolated process group is killed and its leader
/// reaped before Drop performs exact stop/delete/absence cleanup. The permit is
/// released only after `tart list --source local --quiet` proves this VM name
/// absent, so neither compute nor a partial clone escapes the concurrency gate.
struct TartGuard {
    vm: String,
    ip: String,
    run: Option<GroupChild>,
    setup: Option<GroupChild>,
    vm_may_exist: bool,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

impl TartGuard {
    async fn setup_command(
        &mut self,
        program: &str,
        args: &[String],
        cancellation: Option<&RunCancellation>,
        timeout: Duration,
        capture: bool,
    ) -> Result<SetupOutput, String> {
        if cancellation.is_some_and(RunCancellation::is_cancelled) {
            return Err(format!("`{program} {}` cancelled before spawn", args.join(" ")));
        }
        self.setup = Some(GroupChild::spawn(program, args, capture).map_err(|error| {
            format!(
                "`{program} {}` failed to spawn: {error}",
                args.join(" ")
            )
        })?);
        if program == "tart" && args.first().map(String::as_str) == Some("clone") {
            // From this point clone may have created metadata even if its
            // command is cancelled, dropped, or exits unsuccessfully.
            self.vm_may_exist = true;
        }
        let process = self
            .setup
            .as_mut()
            .expect("a Tart setup child was just installed");
        let process_group = process.process_group;
        let child = process
            .child
            .as_mut()
            .expect("a Tart setup child was just installed");
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = stdout {
                pipe.read_to_end(&mut bytes).await?;
            }
            Ok::<_, std::io::Error>(bytes)
        });
        let stderr = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = stderr {
                pipe.read_to_end(&mut bytes).await?;
            }
            Ok::<_, std::io::Error>(bytes)
        });

        enum Outcome {
            Exited(std::io::Result<std::process::ExitStatus>),
            Cancelled,
            TimedOut,
        }
        let outcome = tokio::select! {
            status = wait_owned_child_complete(
                child,
                process_group,
                program,
                &mut process.leader_reaped,
            ) => {
                Outcome::Exited(status)
            },
            _ = cancellation_requested(cancellation) => Outcome::Cancelled,
            _ = tokio::time::sleep(timeout) => Outcome::TimedOut,
        };
        if matches!(&outcome, Outcome::Cancelled | Outcome::TimedOut)
            && let Some(process) = self.setup.as_mut()
        {
            if process.leader_reaped {
                #[cfg(unix)]
                if let Some(group) = process.process_group {
                    wait_process_group_gone(group, program).await;
                }
            } else if let Some(child) = process.child.as_mut() {
                let _ = terminate_child(
                    child,
                    process.process_group,
                    None,
                    &mut process.leader_reaped,
                )
                .await;
            }
            process.cleaned = true;
        }
        if matches!(&outcome, Outcome::Exited(Ok(_)))
            && let Some(process) = self.setup.as_mut()
        {
            // wait_owned_child_complete reaped the leader and proved its exact
            // process group gone; Drop must not inspect that stale PGID.
            process.cleaned = true;
        }
        // Drop the process owner before joining pipe readers so every
        // cancellation/timeout path has completed its kill + wait first.
        drop(self.setup.take());
        let stdout = stdout
            .await
            .map_err(|error| format!("join `{program}` stdout reader: {error}"))?
            .map_err(|error| format!("read `{program}` stdout: {error}"))?;
        let stderr = stderr
            .await
            .map_err(|error| format!("join `{program}` stderr reader: {error}"))?
            .map_err(|error| format!("read `{program}` stderr: {error}"))?;
        let status = match outcome {
            Outcome::Exited(status) => status.map_err(|error| {
                format!("wait for `{program} {}`: {error}", args.join(" "))
            })?,
            Outcome::Cancelled => {
                return Err(format!("`{program} {}` cancelled", args.join(" ")))
            }
            Outcome::TimedOut => {
                return Err(format!(
                    "`{program} {}` exceeded {timeout:?}",
                    args.join(" ")
                ))
            }
        };
        Ok(SetupOutput {
            status,
            stdout,
            stderr,
        })
    }
}

impl Drop for TartGuard {
    fn drop(&mut self) {
        // If the setup future itself is dropped, kill and reap clone/set/ip/SSH
        // before touching the partially-created VM. Drop cannot await, so this
        // synchronous wait is deliberately fail-closed if the kernel cannot
        // reap a killed child.
        if let Some(mut setup) = self.setup.take() {
            setup.kill_and_wait_blocking();
        }
        // Compute boundary: the foreground `tart run` leader plus every helper
        // it spawned share this process group. SIGKILL + leader wait completes
        // before the semaphore field can be dropped.
        if let Some(mut run) = self.run.take() {
            run.kill_and_wait_blocking();
        }
        if !self.vm_may_exist {
            return;
        }
        let mut failures = 0u64;
        let mut retry_delay = PODMAN_RETRY_MIN;
        loop {
            match tart_vm_absent(&self.vm) {
                Ok(true) => break,
                Ok(false) => {
                    let _ = run_tart_cleanup_bounded("stop", &self.vm);
                    let _ = run_tart_cleanup_bounded("delete", &self.vm);
                }
                Err(error) => {
                    failures += 1;
                    if failures == 1 || failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] verify Tart VM {} absence \
                             (attempt {failures}): {error}",
                            self.vm
                        );
                    }
                }
            }
            match tart_vm_absent(&self.vm) {
                Ok(true) => break,
                Ok(false) => {
                    failures += 1;
                    if failures == 1 || failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] Tart VM {} still present after exact cleanup \
                             (attempt {failures})",
                            self.vm
                        );
                    }
                }
                Err(error) => {
                    failures += 1;
                    if failures == 1 || failures.is_multiple_of(16) {
                        eprintln!(
                            "[capability-host] Tart cleanup for {} remains unproven \
                             (attempt {failures}): {error}",
                            self.vm
                        );
                    }
                }
            }
            std::thread::sleep(retry_delay);
            retry_delay = retry_delay.saturating_mul(2).min(PODMAN_RETRY_MAX);
        }
    }
}

fn tart_vm_absent(vm: &str) -> Result<bool, String> {
    let output = run_command_bounded(
        Path::new("tart"),
        &[
            "list".into(),
            "--source".into(),
            "local".into(),
            "--quiet".into(),
        ],
        PODMAN_CONTROL_TIMEOUT,
    )?;
    Ok(!output.lines().any(|name| name.trim() == vm))
}

fn run_tart_cleanup_bounded(action: &str, vm: &str) -> Result<(), String> {
    run_command_bounded(
        Path::new("tart"),
        &[action.to_string(), vm.to_string()],
        PODMAN_CONTROL_TIMEOUT,
    )
    .map(|_| ())
}

/// one finished invocation: the parsed answer plus the raw stdout the
/// session capture reads (the session id is a sibling of the answer in the
/// CLI's output, not part of it).
struct Invocation {
    text: String,
    usage: Option<TokenUsage>,
    stdout: String,
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
        config_home: Option<&Path>,
        broker: Option<&broker::RunBroker>,
    ) -> Result<Invocation, String> {
        if ctx
            .cancellation
            .as_ref()
            .is_some_and(RunCancellation::is_cancelled)
        {
            return Err(format!("{} cancelled before spawn", self.bin.display()));
        }
        let broker_invocation = broker.map(broker::RunBroker::begin_invocation);
        let auth = RunAuth {
            config_home,
            broker: broker_invocation
                .as_ref()
                .map(|invocation| &invocation.endpoint),
        };
        let tart_plan = matches!(self.backend, SandboxBackend::Tart { .. })
            .then(|| {
                let args = self.broker_argv(args, workdir, &auth);
                self.tart_plan(&args, workdir, ctx, &auth)
            })
            .transpose()?;
        // Declared before the SSH child so VM stop/delete runs after the child
        // is dropped on success, error, or timeout.
        let tart_guard = self.tart_setup(tart_plan.as_ref(), ctx).await?;
        let (mut command, podman) = if let (Some(plan), Some(guard)) = (&tart_plan, &tart_guard) {
            let mut command = tokio::process::Command::new("sshpass");
            command.args(sandbox::tart_ssh_argv(&guard.ip, &plan.guest_script));
            (command, None)
        } else {
            let PreparedCommand { command, podman } =
                self.prepared_command(args, workdir, ctx, &auth)?;
            (command, podman)
        };
        if let Some(podman) = &podman {
            podman.prepare_cidfile()?;
        }
        command
            .current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        configure_process_group(&mut command);
        let idle = self.timeout;
        let hard = tokio::time::Instant::now() + idle.saturating_mul(HARD_TIMEOUT_FACTOR);
        if let Some(invocation) = &broker_invocation {
            invocation.arm(hard);
        }
        let child = command
            .spawn()
            .map_err(|e| format!("spawn {} failed: {e}", self.bin.display()))?;
        let mut live = LiveChild::new(child, podman);
        let mut stdin = live
            .child_mut()
            .stdin
            .take()
            .ok_or_else(|| "child stdin was not piped".to_string())?;
        let mut stdout_pipe = live
            .child_mut()
            .stdout
            .take()
            .ok_or_else(|| "child stdout was not piped".to_string())?;
        let mut stderr_pipe = live
            .child_mut()
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
        let mut explicit_deadline = broker_invocation
            .as_ref()
            .map(|invocation| invocation.idle_deadline.clone());
        let mut feed = std::pin::pin!(feed);
        let mut fed: Option<Result<(), std::io::Error>> = None;
        let mut out_bytes: Vec<u8> = Vec::new();
        let mut err_bytes: Vec<u8> = Vec::new();
        let (mut out_open, mut err_open) = (true, true);
        let mut obuf = [0u8; 8192];
        let mut ebuf = [0u8; 8192];
        let mut last_activity = tokio::time::Instant::now();
        while out_open || err_open {
            let explicit = explicit_deadline
                .as_ref()
                .and_then(|deadline| *deadline.borrow())
                .unwrap_or(last_activity + idle);
            let deadline = (last_activity + idle).max(explicit).min(hard);
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
                        if let Some(invocation) = &broker_invocation {
                            invocation.revoke();
                        }
                        live.terminate().await;
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
                        if let Some(invocation) = &broker_invocation {
                            invocation.revoke();
                        }
                        live.terminate().await;
                        return Err(format!(
                            "reading {} stderr failed: {e}",
                            self.bin.display()
                        ));
                    }
                },
                _ = cancellation_requested(ctx.cancellation.as_ref()) => {
                    if let Some(invocation) = &broker_invocation {
                        invocation.revoke();
                    }
                    live.terminate().await;
                    return Err(format!(
                        "{} cancelled (child terminated)",
                        self.bin.display()
                    ));
                },
                changed = async {
                    match explicit_deadline.as_mut() {
                        Some(deadline) => deadline.changed().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if changed.is_err() {
                        explicit_deadline = None;
                    }
                },
                _ = tokio::time::sleep_until(deadline) => {
                    if let Some(invocation) = &broker_invocation {
                        invocation.revoke();
                    }
                    live.terminate().await;
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
        if let Some(invocation) = &broker_invocation {
            invocation.revoke();
        }
        enum WaitOutcome {
            Exited(std::io::Result<std::process::ExitStatus>),
            Cancelled,
            TimedOut,
        }
        let label = self.bin.display().to_string();
        let status = match tokio::select! {
            status = live.wait_complete(&label) => {
                WaitOutcome::Exited(status)
            },
            _ = cancellation_requested(ctx.cancellation.as_ref()) => WaitOutcome::Cancelled,
            _ = tokio::time::sleep(idle) => WaitOutcome::TimedOut,
        } {
            WaitOutcome::Exited(Ok(status)) => status,
            WaitOutcome::Exited(Err(error)) => {
                if let Some(invocation) = &broker_invocation {
                    invocation.revoke();
                }
                live.terminate().await;
                return Err(format!(
                    "waiting on {} failed: {error}",
                    self.bin.display()
                ));
            }
            WaitOutcome::Cancelled => {
                if let Some(invocation) = &broker_invocation {
                    invocation.revoke();
                }
                live.terminate().await;
                return Err(format!(
                    "{} cancelled (child terminated)",
                    self.bin.display()
                ));
            }
            WaitOutcome::TimedOut => {
                if let Some(invocation) = &broker_invocation {
                    invocation.revoke();
                }
                live.terminate().await;
                return Err(format!(
                    "{} closed its output but did not exit within {idle:?} (child killed)",
                    self.bin.display()
                ));
            }
        };
        // an unfinished feed at this point means the child exited without
        // draining stdin — the exit status below is the primary diagnostic.
        let fed = fed.unwrap_or(Ok(()));

        if !status.success() {
            // a failed exit is the primary diagnostic — it subsumes any
            // stdin write error (an early-exiting child EPIPEs the feed).
            return Err(format!(
                "{} exited with {}: {}",
                self.bin.display(),
                status,
                excerpt(&String::from_utf8_lossy(&err_bytes))
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
        if ctx
            .cancellation
            .as_ref()
            .is_some_and(RunCancellation::is_cancelled)
        {
            return Err(format!("{} cancelled before start", self.bin.display()));
        }
        let workdir = self.ensure_writable_workdir(ctx)?;
        let workdir = if matches!(self.backend, SandboxBackend::Direct) {
            workdir
        } else {
            canonical_mount_path(&workdir, "sandbox workdir")?
        };
        // the run's auth materials, prepared once and shared by every invocation
        // below (a resume and its cold retry are the SAME run — one config home,
        // one broker). `broker` is held here so the endpoint outlives the child
        // and is torn down when this call returns, however it returns.
        let config_home = self.prepare_config_home(&workdir, ctx)?;
        // the assembled soul, delivered by whichever door the SPEC names: a file
        // the CLI auto-loads, or the stdin prompt. the guard (held for the whole
        // call) removes a doc that lives outside the workdir, on every exit path.
        let _context = self.deliver_context(&workdir, config_home.as_deref(), ctx)?;
        let prompt_buf = self.prompt_with_context(prompt, ctx);
        let prompt = prompt_buf.as_str();
        let broker = self.start_broker().await?;

        let Some((session, store)) = self.session_store(ctx)? else {
            // no session plumbing for this run: one cold invocation.
            let run = self
                .invoke(
                    prompt,
                    &self.spec.args,
                    &workdir,
                    ctx,
                    config_home.as_deref(),
                    broker.as_ref(),
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
                    config_home.as_deref(),
                    broker.as_ref(),
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
                    if ctx
                        .cancellation
                        .as_ref()
                        .is_some_and(RunCancellation::is_cancelled)
                    {
                        return Err(e);
                    }
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
                config_home.as_deref(),
                broker.as_ref(),
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
/// `node_identity` is the verified local signer/origin bytes; Podman uses its
/// canonical [`execution_node_id`] to scope crash reaping to this node only.
pub fn discover(
    node_identity: &[u8],
    dirs: AgentDirs,
    output_sink: Option<OutputSink>,
    backend: SandboxBackend,
) -> Result<ProviderSet, String> {
    let specs = SpecSet::load(operator_spec_dir().as_deref())?;
    let executing_node = execution_node_id(node_identity);
    if matches!(&backend, SandboxBackend::Podman { .. }) {
        reap_podman_orphans_once(&executing_node)
            .map_err(|error| format!("Podman crash-orphan cleanup failed: {error}"))?;
    }
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
        backend,
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
    discover_with_sink(
        specs,
        path,
        env,
        global_timeout,
        dirs,
        None,
        SandboxBackend::Direct,
    )
}

fn discover_with_sink(
    specs: SpecSet,
    path: Option<OsString>,
    env: &dyn Fn(&str) -> Option<OsString>,
    global_timeout: Option<Duration>,
    dirs: AgentDirs,
    output_sink: Option<OutputSink>,
    backend: SandboxBackend,
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
            let mut provider = CliProvider::from_spec((*spec).clone(), bin.clone())
                .with_agent_dirs(dirs.clone())
                .with_backend(backend.clone());
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

    fn podman_ctx() -> RunContext {
        RunContext {
            executing_node: Some(execution_node_id(b"test-node")),
            ..RunContext::default()
        }
    }

    #[test]
    fn execution_node_identity_is_canonical_lower_hex() {
        assert_eq!(execution_node_id(&[0, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[tokio::test]
    async fn cancellation_is_clone_visible_and_sticky() {
        let cancellation = RunCancellation::new();
        let waiter = cancellation.clone();
        let waiting = tokio::spawn(async move { waiter.cancelled().await });
        assert!(!cancellation.is_cancelled());

        cancellation.cancel();
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("an active waiter is woken")
            .expect("waiter task does not panic");
        assert!(cancellation.is_cancelled());
        tokio::time::timeout(Duration::from_millis(50), cancellation.cancelled())
            .await
            .expect("a late waiter observes the cancelled state");
    }

    // ---- podman backend glue ------------------------------------------------

    #[test]
    fn podman_cleanup_commands_target_one_container_id() {
        let cid = "0123456789abcdef";
        assert_eq!(
            PodmanRun::stop_argv(cid),
            vec!["stop", "--ignore", "--time", "2", cid]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            PodmanRun::wait_argv(cid),
            vec!["wait".to_string(), cid.to_string()]
        );
        assert_eq!(
            PodmanRun::remove_argv(cid),
            vec!["rm", "--force", "--ignore", cid]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn podman_late_cid_query_uses_the_full_unique_run_identity() {
        let labels = vec![
            PODMAN_MANAGED_LABEL.into(),
            format!("{PODMAN_NODE_LABEL}=node"),
            format!("{PODMAN_OWNER_BOOT_LABEL}=boot"),
            format!("{PODMAN_OWNER_PID_LABEL}=7"),
            format!("{PODMAN_OWNER_START_LABEL}=11"),
            format!("{PODMAN_INSTANCE_LABEL}=7-1"),
        ];
        let run = PodmanRun {
            cidfile: scratch("podman-exact-query").join("run.cid"),
            labels: labels.clone(),
        };
        let query = run.exact_query_argv();
        assert_eq!(query[..4].join(" "), "ps --all --quiet --no-trunc");
        assert_eq!(
            query.iter().filter(|arg| *arg == "--filter").count(),
            labels.len()
        );
        for label in labels {
            assert!(query.contains(&format!("label={label}")), "{query:?}");
        }
        assert!(query.iter().all(|arg| !arg.contains("name=") && !arg.contains("pkill")));
    }

    #[test]
    fn podman_process_reaper_preserves_live_owner_and_reaps_only_dead_owner() {
        let live_cid = "0123456789abcdef";
        let dead_cid = "fedcba9876543210";
        let executing_node = execution_node_id(b"node-a");
        let query = podman_reap_query_argv(&executing_node);
        let boot_id = "11111111-2222-3333-4444-555555555555";
        let mut calls = Vec::new();
        reap_podman_orphans_with(
            &executing_node,
            boot_id,
            |pid| match pid {
                101 => Ok(Some(1001)),
                // PID 202 exists but starttime differs: it was reused and is
                // not the process that created this container.
                202 => Ok(Some(9999)),
                _ => Err("unexpected pid".into()),
            },
            |args| {
                calls.push(args.to_vec());
                if args == query.as_slice() {
                    return Ok(format!("{live_cid}\n{dead_cid}\n"));
                }
                if args == podman_owner_inspect_argv(live_cid).as_slice() {
                    return Ok(format!("{boot_id}\t101\t1001\n"));
                }
                if args == podman_owner_inspect_argv(dead_cid).as_slice() {
                    return Ok(format!("{boot_id}\t202\t2002\n"));
                }
                if args == PodmanRun::wait_argv(dead_cid).as_slice() {
                    return Err("container auto-removed after stop".into());
                }
                Ok(String::new())
            },
        )
        .expect("fake managed container is reaped");

        assert_eq!(
            calls,
            vec![
                query.clone(),
                podman_owner_inspect_argv(live_cid),
                podman_owner_inspect_argv(dead_cid),
                PodmanRun::stop_argv(dead_cid),
                PodmanRun::wait_argv(dead_cid),
                PodmanRun::remove_argv(dead_cid),
            ]
        );
        assert!(calls.iter().all(|args| {
            !args.iter().any(|arg| arg.contains("pkill"))
                && (args.first().map(String::as_str) != Some("ps")
                    || (args
                        .iter()
                        .any(|arg| arg == "label=io.ducktape.managed=capability-host")
                        && args.iter().any(|arg| {
                            arg == &format!("label=io.ducktape.node={executing_node}")
                        })))
        }));
        assert!(parse_container_ids("not-a-container-id\n").is_err());
    }

    #[test]
    fn podman_process_reaper_preserves_missing_or_unverifiable_owner() {
        let missing = "0123456789abcdef";
        let unreadable = "fedcba9876543210";
        let executing_node = execution_node_id(b"node-a");
        let query = podman_reap_query_argv(&executing_node);
        let boot_id = "11111111-2222-3333-4444-555555555555";
        let mut calls = Vec::new();
        reap_podman_orphans_with(
            &executing_node,
            boot_id,
            |_| Err("/proc denied".into()),
            |args| {
                calls.push(args.to_vec());
                if args == query.as_slice() {
                    return Ok(format!("{missing}\n{unreadable}\n"));
                }
                if args == podman_owner_inspect_argv(missing).as_slice() {
                    return Ok("<no value>\t<no value>\t<no value>\n".into());
                }
                if args == podman_owner_inspect_argv(unreadable).as_slice() {
                    return Ok(format!("{boot_id}\t303\t3003\n"));
                }
                panic!("preserved containers must not receive cleanup: {args:?}");
            },
        )
        .expect("unknown owners are preserved safely");
        assert_eq!(calls.len(), 3, "one query and two exact inspections");
    }

    #[test]
    fn podman_process_reaper_has_a_hard_inspection_bound() {
        let executing_node = execution_node_id(b"node-a");
        let query = podman_reap_query_argv(&executing_node);
        let listed = (1..=PODMAN_REAP_MAX + 1)
            .map(|id| format!("{id:016x}"))
            .collect::<Vec<_>>()
            .join("\n");
        let error = reap_podman_orphans_with(
            &executing_node,
            "11111111-2222-3333-4444-555555555555",
            |_| panic!("limit is checked before /proc"),
            |args| {
                assert_eq!(args, query.as_slice());
                Ok(listed.clone())
            },
        )
        .unwrap_err();
        assert!(error.contains("bounded reaper limit"), "got: {error}");
    }

    #[test]
    fn bounded_command_drains_large_stdout_and_stderr_while_waiting() {
        let dir = scratch("podman-large-control-output");
        let script = fake_cli(
            &dir,
            "large-output",
            "i=0\n\
             while [ \"$i\" -lt 4096 ]; do printf '%064d\\n' 0; i=$((i + 1)); done\n\
             i=0\n\
             while [ \"$i\" -lt 4096 ]; do printf '%064d\\n' 1 >&2; i=$((i + 1)); done",
        );
        let stdout = run_command_bounded(
            Path::new("/bin/sh"),
            &[script.display().to_string()],
            Duration::from_secs(10),
        )
        .expect("both streams are drained before their pipe buffers fill");
        assert!(stdout.len() > 64 * 1024, "stdout crossed a pipe buffer");
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_removes_a_helper_that_keeps_its_pipes_open() {
        let dir = scratch("bounded-command-residual-helper");
        let pidfile = dir.join("helper.pid");
        let script = fake_cli(
            &dir,
            "residual-helper",
            &format!("sleep 30 &\necho $! > {}\nexit 0", pidfile.display()),
        );
        let started = std::time::Instant::now();
        run_command_bounded(
            Path::new("/bin/sh"),
            &[script.display().to_string()],
            Duration::from_secs(2),
        )
        .expect("the exited leader's residual group is removed before pipe join");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a background helper must not hold pipe readers until timeout"
        );
        let helper = std::fs::read_to_string(pidfile)
            .unwrap()
            .trim()
            .parse::<libc::pid_t>()
            .unwrap();
        #[cfg(target_os = "linux")]
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{helper}/stat")) {
            let (state, _) = linux_process_state_and_group(&stat).expect("helper proc stat");
            assert!(matches!(state, 'Z' | 'X' | 'x'), "helper still computes: {stat}");
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert_eq!(unsafe { libc::kill(helper, 0) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
        }
    }

    #[tokio::test]
    async fn podman_cancellation_collects_a_delayed_cid_before_exact_cleanup() {
        let dir = scratch("podman-delayed-cid");
        let cidfile = dir.join("run.cid");
        let run = PodmanRun {
            cidfile: cidfile.clone(),
            labels: Vec::new(),
        };
        let cid = "0123456789abcdef";
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            std::fs::write(cidfile, format!("{cid}\n")).unwrap();
        });
        let collected = run
            .wait_container_id_until(tokio::time::Instant::now() + Duration::from_secs(1))
            .await
            .expect("the cidfile creation race is collected");
        writer.await.unwrap();
        assert_eq!(collected, cid);
        assert_eq!(
            PodmanRun::stop_argv(&collected).last().map(String::as_str),
            Some(cid)
        );
        assert_eq!(
            PodmanRun::wait_argv(&collected).last().map(String::as_str),
            Some(cid)
        );
        assert_eq!(
            PodmanRun::remove_argv(&collected).last().map(String::as_str),
            Some(cid)
        );
    }

    #[cfg(unix)]
    #[test]
    fn podman_mount_checks_resolve_symlinks_and_reject_relative_paths() {
        use std::os::unix::fs::symlink;

        let dir = scratch("podman-canonical-mounts");
        let home = dir.join("real-home");
        let control = home.join(".ducktape/provider-runs/podman");
        std::fs::create_dir_all(&control).unwrap();
        let home_alias = dir.join("home-alias");
        symlink(&home, &home_alias).unwrap();
        assert_eq!(
            canonical_mount_path(&home_alias, "test HOME").unwrap(),
            std::fs::canonicalize(&home).unwrap()
        );

        let exposed_alias = dir.join("exposed-alias");
        symlink(&control, &exposed_alias).unwrap();
        let error = ensure_podman_control_hidden(
            &std::fs::canonicalize(&control).unwrap(),
            [exposed_alias.as_path()],
        )
        .unwrap_err();
        assert!(error.contains("would be exposed"), "got: {error}");
        assert!(canonical_mount_path(Path::new("relative/home"), "test HOME").is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn setup_kill_does_not_return_before_the_child_is_reaped() {
        let args = vec!["-c".into(), "sleep 30".into()];
        let mut process =
            GroupChild::spawn("/bin/sh", &args, false).expect("spawn setup process");
        process.kill_and_wait_blocking();
        assert!(matches!(
            process.child.as_mut().unwrap().try_wait(),
            Ok(Some(_))
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_stat_parser_handles_parentheses_and_distinguishes_zombies() {
        assert_eq!(
            linux_process_state_and_group("42 (agent (worker)) S 7 19 19 0"),
            Some(('S', 19))
        );
        assert_eq!(
            linux_process_state_and_group("43 (sleep) Z 1 19 19 0"),
            Some(('Z', 19))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn dropping_a_setup_process_reaps_its_whole_process_group() {
        let dir = scratch("tart-setup-drop");
        let pidfile = dir.join("setup.pid");
        let args = vec![
            "-c".into(),
            format!(
                "trap '' TERM; echo $$ > {}; sleep 30 & wait",
                pidfile.display()
            ),
        ];
        let process = GroupChild::spawn("/bin/sh", &args, false).expect("spawn setup process");
        let group = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(pid) = std::fs::read_to_string(&pidfile)
                    && let Ok(pid) = pid.trim().parse::<u32>()
                {
                    break pid;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("setup process reports its group");
        drop(process);
        tokio::time::timeout(Duration::from_secs(1), async {
            while process_group_alive(group) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("dropping setup leaves no descendant behind");
    }

    #[test]
    fn a_finished_podman_run_removes_its_host_cidfile() {
        let dir = scratch("podman-cidfile-cleanup");
        let cidfile = dir.join("run.cid");
        std::fs::write(&cidfile, b"0123456789abcdef\n").unwrap();
        let run = PodmanRun {
            cidfile: cidfile.clone(),
            labels: Vec::new(),
        };
        run.remove_cidfile();
        assert!(!cidfile.exists());
    }

    #[test]
    fn podman_refuses_a_run_without_its_executing_node_scope() {
        let root = scratch("podman-missing-node-scope");
        let bin = root.join("pod");
        let workdir = root.join("wd");
        std::fs::write(&bin, b"pod").unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        let provider = CliProvider::from_spec(sandbox_spec("pod"), bin)
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let error = match provider.command(
            &[],
            &workdir,
            &RunContext::default(),
            &RunAuth::default(),
        ) {
            Ok(_) => panic!("an unscoped Podman run could be reaped by the wrong node"),
            Err(error) => error,
        };
        assert!(error.contains("executing-node"), "got: {error}");
    }

    #[test]
    fn podman_backend_wraps_command_with_identical_paths_and_rw_mounts() {
        // the command() glue that assembles HOME, expands the spec's `~/`
        // rw_dirs against it, and hands identical container paths to
        // wrap_podman — asserted through the real command() the same way a run
        // would build it. wrap_podman's own translation is covered in sandbox.rs.
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "pod"
[detect]
bin = "pod"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
[sandbox]
rw_dirs = ["~/.claude"]
"#,
            "test",
        )
        .unwrap();
        let root = scratch("podman-command-paths");
        let bin = root.join("pod");
        let workdir = root.join("wd");
        std::fs::write(&bin, b"pod").unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        let provider = CliProvider::from_spec(spec, bin.clone())
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let ctx = RunContext {
            env: BTreeMap::from([("RUN_SECRET".to_string(), "not-in-argv".to_string())]),
            limits: BTreeMap::from([("cores".to_string(), 2u64)]),
            ..podman_ctx()
        };
        let cmd = provider
            .command(
                &["--go".into()],
                &workdir,
                &ctx,
                &RunAuth::default(),
            )
            .expect("podman command builds");
        let std = cmd.as_std();
        assert_eq!(std.get_program(), std::ffi::OsStr::new("podman"));
        let argv: Vec<String> = std
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let joined = argv.join(" ");
        let home = canonical_mount_path(
            &PathBuf::from(std::env::var_os("HOME").expect("HOME set in test env")),
            "test HOME",
        )
        .unwrap();
        // the spec's `~/.claude` expands against the real HOME and mounts rw at
        // its IDENTICAL container path; the bin mounts ro; limits become flags.
        assert!(
            joined.contains(&format!("-v {home}/.claude:{home}/.claude", home = home.display())),
            "rw mount at identical path: {joined}"
        );
        assert!(
            joined.contains(&format!("-v {bin}:{bin}:ro", bin = bin.display())),
            "{joined}"
        );
        assert!(joined.contains("-e HOME"), "{joined}");
        assert!(joined.contains("-e RUN_SECRET"), "{joined}");
        assert!(
            !joined.contains("not-in-argv"),
            "secret value leaked in argv: {joined}"
        );
        assert_eq!(
            std.get_envs()
                .find(|(key, _)| *key == std::ffi::OsStr::new("RUN_SECRET"))
                .and_then(|(_, value)| value),
            Some(std::ffi::OsStr::new("not-in-argv"))
        );
        assert_eq!(
            std.get_envs()
                .find(|(key, _)| *key == std::ffi::OsStr::new("HOME"))
                .and_then(|(_, value)| value),
            Some(std::ffi::OsStr::new(&home))
        );
        assert!(joined.contains("--cpus 2"), "{joined}");
        assert!(
            joined.contains("--cidfile")
                && joined.contains("--label io.ducktape.managed=capability-host")
                && joined.contains(&format!(
                    "--label io.ducktape.node={}",
                    execution_node_id(b"test-node")
                ))
                && joined.contains("--label io.ducktape.owner.boot=")
                && joined.contains("--label io.ducktape.owner.pid=")
                && joined.contains("--label io.ducktape.owner.starttime=")
                && joined.contains("--label io.ducktape.run=")
                && joined.contains("--label io.ducktape.instance="),
            "managed lifecycle identity: {joined}"
        );
        let cidfile = joined
            .split_whitespace()
            .skip_while(|arg| *arg != "--cidfile")
            .nth(1)
            .expect("cidfile value");
        assert!(
            !joined.contains(&format!("-v {cidfile}:"))
                && !joined.contains(&format!("-e {cidfile}")),
            "host cidfile is not visible inside the container: {joined}"
        );
        assert!(joined.ends_with(&format!("img {} --go", bin.display())), "{joined}");
    }

    #[test]
    fn tart_backend_builds_a_boot_then_ssh_plan() {
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "vm"
[detect]
bin = "vm"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .unwrap();
        let root = scratch("tart-plan");
        let bin = root.join("vm");
        std::fs::write(&bin, b"vm").unwrap();
        let workdir = root.join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        let provider = CliProvider::from_spec(spec, bin).with_backend(SandboxBackend::Tart {
            image: "ghcr.io/example/macos-base:latest".into(),
        });
        let ctx = RunContext {
            limits: BTreeMap::from([("mem_gb".to_string(), 4u64)]),
            ..RunContext::default()
        };
        let plan = provider
            .tart_plan(&["--go".into()], &workdir, &ctx, &RunAuth::default())
            .expect("Tart plan builds");
        assert!(plan.vm.starts_with("ducktape-"), "{}", plan.vm);
        assert_eq!(plan.run_argv.first().map(String::as_str), Some("run"));
        assert_eq!(plan.run_argv.last(), Some(&plan.vm));
        assert!(!plan.guest_script.contains("ssh"));
        assert!(plan.guest_script.contains("--go"));
        assert!(plan.guest_script.contains("rsync -aO --delete"));
    }

    /// a sandbox spec with no auth section — the shape both skills tests want.
    fn sandbox_spec(tag: &str) -> CapabilitySpec {
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
"#
            ),
            "test",
        )
        .unwrap()
    }

    async fn hardware_sandbox_smoke(name: &str, backend: SandboxBackend) {
        let root = scratch(name);
        let bin_dir = root.join("bin");
        let workdir = root.join("workspace");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        let bin = fake_cli(
            &bin_dir,
            "sandbox-smoke",
            r#"prompt=$(cat)
printf '%s' "$prompt" > sandbox-marker.txt
printf 'sandbox-ok:%s' "$prompt""#,
        );
        let provider =
            CliProvider::from_spec(sandbox_spec("hardware-smoke"), bin).with_backend(backend);
        let ctx = RunContext {
            workdir_override: Some(workdir.clone()),
            limits: BTreeMap::from([("cores".into(), 2), ("mem_gb".into(), 4)]),
            executing_node: Some(execution_node_id(b"hardware-smoke")),
            ..RunContext::default()
        };

        let answer = provider
            .run("hardware-prompt", &ctx)
            .await
            .expect("real sandbox provider cycle");
        assert_eq!(answer, "sandbox-ok:hardware-prompt");
        assert_eq!(
            std::fs::read_to_string(workdir.join("sandbox-marker.txt")).unwrap(),
            "hardware-prompt",
            "the sandbox must sync its writable workspace back to the host"
        );
        std::fs::remove_dir_all(root).ok();
    }

    /// Real Podman gate for macOS hardware. Kept ignored because it requires a
    /// running Podman machine and a pulled image; ops/smoke-macos-sandbox.sh
    /// owns those prerequisites and invokes this exact test.
    #[tokio::test]
    #[ignore = "requires a live Podman machine"]
    async fn macos_podman_hardware_smoke() {
        let image = std::env::var("DUCKTAPE_MACOS_PODMAN_IMAGE")
            .unwrap_or_else(|_| "docker.io/library/node:22-slim".into());
        hardware_sandbox_smoke("macos-podman-hardware", SandboxBackend::Podman { image }).await;
    }

    /// Real Tart gate for Apple Silicon hardware. The provider owns the full
    /// clone → configure → boot → SSH → rsync → stop → delete lifecycle.
    #[tokio::test]
    #[ignore = "requires Apple Silicon, Tart, sshpass, and a pulled macOS image"]
    async fn macos_tart_hardware_smoke() {
        if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            eprintln!("skipping Tart hardware smoke off Apple Silicon macOS");
            return;
        }
        let image = std::env::var("DUCKTAPE_MACOS_TART_IMAGE")
            .unwrap_or_else(|_| "ghcr.io/cirruslabs/macos-sonoma-base:latest".into());
        hardware_sandbox_smoke("macos-tart-hardware", SandboxBackend::Tart { image }).await;
    }

    #[test]
    fn a_sandboxed_run_mounts_its_skills_tree_read_only_when_it_has_one() {
        // W6 skills live at a SIBLING of the rw checkout (outside the workdir, so
        // `commit` never scans them), and the provisioner tells the child where
        // via DUCKTAPE_RUN_SKILLS. under Direct the env alone works — the path is
        // on the host. under a sandbox, only what we mount exists, so without the
        // mount the agent would find its own skills dir simply MISSING.
        let root = scratch("podman-skills-mount");
        let bin = root.join("pod");
        let workdir = root.join("wd");
        let skills = root.join("agent-7-ro");
        std::fs::write(&bin, b"pod").unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        std::fs::create_dir_all(&skills).unwrap();
        let provider = CliProvider::from_spec(sandbox_spec("pod"), bin)
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let ctx = RunContext {
            env: BTreeMap::from([(
                SKILLS_ROOT_ENV.to_string(),
                skills.display().to_string(),
            )]),
            ..podman_ctx()
        };
        let cmd = provider
            .command(&[], &workdir, &ctx, &RunAuth::default())
            .expect("podman command builds");
        let joined = argv_of(&cmd);
        // READ-ONLY, at the identical path the env names: the agent may read its
        // skills, never rewrite them.
        assert!(
            joined.contains(&format!("-v {s}:{s}:ro", s = skills.display())),
            "the skills root mounts ro at its identical path: {joined}"
        );
        assert!(
            joined.contains(&format!("-e {SKILLS_ROOT_ENV}")),
            "and the env still points at it: {joined}"
        );
    }

    #[test]
    fn a_run_with_no_skills_mounts_none() {
        // no skills on the run = no mount. (the provisioner omits the env entirely
        // when the agent has no skill records — see agent_provision's plane tests.)
        let root = scratch("podman-no-skills-mount");
        let bin = root.join("pod");
        let workdir = root.join("wd");
        std::fs::write(&bin, b"pod").unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        let provider = CliProvider::from_spec(sandbox_spec("pod"), bin)
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let cmd = provider
            .command(&[], &workdir, &podman_ctx(), &RunAuth::default())
            .expect("podman command builds");
        let joined = argv_of(&cmd);
        assert!(!joined.contains(SKILLS_ROOT_ENV), "{joined}");
        assert!(!joined.contains("-ro:"), "no ro sibling mount: {joined}");
    }

    // ---- the assembled context document (the agent's "soul") ----------------

    /// a mock spec plus whatever extra sections the test needs — the `[context]`
    /// / `[isolation]` shapes below, without a fixture per shape.
    fn spec_with(tag: &str, extra: &str) -> CapabilitySpec {
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
{extra}
"#
            ),
            "test",
        )
        .unwrap()
    }

    const SOUL: &str = "# soul\nbe kind\n";

    #[tokio::test]
    async fn a_workspace_parent_context_spec_writes_the_soul_beside_the_checkout_and_cleans_it_up()
    {
        // the doc lands at the parent of the run's checkout — where the CLI's own
        // convention finds it, OUTSIDE the tree `commit` scans, and layered UNDER
        // a repository's own instructions file rather than overwriting it.
        let root = scratch("soul-parent");
        // the mock CLI prints the delivered file, a separator, then its stdin.
        let script = fake_cli(&root, "soul.sh", "cat ../SOUL.md; echo ---; cat");
        let provider = sh_provider(
            spec_with("soul", "[context]\npath = \"workspace-parent:SOUL.md\"\n"),
            script,
            "soul-parent-scratch",
        );
        let ctx = RunContext {
            workdir_override: Some(root.join("checkout")),
            context_doc: Some(SOUL.to_string()),
            ..RunContext::default()
        };

        let out = provider
            .run("PROMPT", &ctx)
            .await
            .expect("the run succeeds");
        // the soul reached the FILE, and the stdin half is the bare prompt: a
        // spec with a native context door must not ALSO inflate the prompt.
        assert_eq!(out, "# soul\nbe kind\n---\nPROMPT");
        // and the file is gone: it lives outside the workdir, where nothing else
        // would ever clean it up and a stale soul would join the next run.
        assert!(
            !root.join("SOUL.md").exists(),
            "the context doc is removed when the run ends"
        );
    }

    #[tokio::test]
    async fn a_config_home_context_spec_writes_the_soul_into_the_runs_fresh_config_home() {
        // resolved against the per-run config home [isolation] already materializes
        // — inside the workdir's reserved runtime dir, which the provisioner deletes
        // before scanning, so no guard and no artifact.
        let root = scratch("soul-config-home");
        let script = fake_cli(
            &root,
            "soul.sh",
            "cat \"$TEST_HOME/AGENTS.md\"; echo ---; cat",
        );
        let provider = sh_provider(
            spec_with(
                "soul",
                "[isolation]\nconfig_home_env = \"TEST_HOME\"\n\n\
                 [context]\npath = \"config-home:AGENTS.md\"\n",
            ),
            script,
            "soul-config-home-scratch",
        );
        let workdir = root.join("checkout");
        let ctx = RunContext {
            workdir_override: Some(workdir.clone()),
            context_doc: Some(SOUL.to_string()),
            ..RunContext::default()
        };

        let out = provider
            .run("PROMPT", &ctx)
            .await
            .expect("the run succeeds");
        // the soul reached the CLI's own config home, and the stdin half is the
        // bare prompt — a native context door must not ALSO inflate the prompt.
        assert_eq!(out, "# soul\nbe kind\n---\nPROMPT");
        // no guard for this door: the doc is under the run-runtime dir the
        // provisioner's commit bracket removes.
        assert!(workdir.join(RUN_RUNTIME_DIR).is_dir());
    }

    #[tokio::test]
    async fn a_spec_with_no_context_section_prepends_the_soul_to_the_prompt() {
        // the raw-provider door: a CLI with no ambient-instructions convention
        // (ollama, any plain `text` executor) still gets the soul — on stdin.
        let root = scratch("soul-prompt");
        let script = fake_cli(&root, "soul.sh", "cat");
        let provider = sh_provider(spec_with("raw", ""), script, "soul-prompt-scratch");
        let ctx = RunContext {
            context_doc: Some(SOUL.to_string()),
            ..RunContext::default()
        };

        let out = provider
            .run("PROMPT", &ctx)
            .await
            .expect("the run succeeds");
        assert_eq!(out, format!("{SOUL}\n\nPROMPT").trim());
        // ... and a run with no doc at all is byte-for-byte the old behavior.
        let out = provider
            .run("PROMPT", &RunContext::default())
            .await
            .expect("the run succeeds");
        assert_eq!(out, "PROMPT", "no doc, no prepend");
    }

    #[test]
    fn a_sandbox_binds_a_workspace_parent_soul_read_only_and_needs_no_bind_for_a_config_home_one() {
        let root = scratch("podman-context-mount");
        let workdir = root.join("wd");
        let bin = root.join("pod");
        let soul = root.join("CLAUDE.md");
        std::fs::create_dir_all(&workdir).unwrap();
        std::fs::write(&bin, b"pod").unwrap();
        std::fs::write(&soul, SOUL).unwrap();
        let ctx = RunContext {
            context_doc: Some(SOUL.to_string()),
            ..podman_ctx()
        };

        // OUTSIDE the workdir mount: without this bind the file would exist on the
        // host and simply not be there for the child — a silently unsouled agent.
        let provider = CliProvider::from_spec(
            spec_with("pod", "[context]\npath = \"workspace-parent:CLAUDE.md\"\n"),
            bin.clone(),
        )
        .with_backend(SandboxBackend::Podman {
            image: "img".into(),
        });
        let cmd = provider
            .command(&[], &workdir, &ctx, &RunAuth::default())
            .expect("podman command builds");
        let joined = argv_of(&cmd);
        assert!(
            joined.contains(&format!("-v {s}:{s}:ro", s = soul.display())),
            "the soul binds ro at its identical path: {joined}"
        );

        // INSIDE it: a config-home doc lives under the workdir, which every backend
        // already mounts — no second bind, and none wanted.
        let provider = CliProvider::from_spec(
            spec_with(
                "pod",
                "[isolation]\nconfig_home_env = \"H\"\n\n\
                 [context]\npath = \"config-home:AGENTS.md\"\n",
            ),
            bin,
        )
        .with_backend(SandboxBackend::Podman {
            image: "img".into(),
        });
        let config_home = workdir.join(".ducktape-run/slot/provider-config");
        std::fs::create_dir_all(&config_home).unwrap();
        let auth = RunAuth {
            config_home: Some(&config_home),
            broker: None,
        };
        let cmd = provider
            .command(&[], &workdir, &ctx, &auth)
            .expect("podman command builds");
        let joined = argv_of(&cmd);
        assert!(
            !joined.contains("AGENTS.md"),
            "a doc under the workdir crosses for free: {joined}"
        );
    }

    // ---- the credential broker ----------------------------------------------

    /// a broker-backed spec: the strong auth path (and so, by the parse-time
    /// invariant, NO `[sandbox] rw_dirs`).
    fn broker_spec(tag: &str) -> CapabilitySpec {
        CapabilitySpec::parse(
            &format!(
                r#"
spec = 1
[capability]
tag = "{tag}"
[detect]
bin = "{tag}"
[invoke]
args = ["exec", "--json", "-"]
prompt = "stdin"
[output]
format = "text"
[isolation]
config_home_env = "CODEX_HOME"
broker = "codex-responses"
"#
            ),
            "test",
        )
        .unwrap()
    }

    fn argv_of(cmd: &tokio::process::Command) -> String {
        cmd.as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[tokio::test]
    async fn every_backend_accepts_the_credential_broker_shape() {
        // A missing host credential may still fail startup, but no backend is
        // structurally rejected: Tart now exposes the broker through its NAT
        // gateway while Direct/Podman retain loopback.
        for backend in [
            SandboxBackend::Direct,
            SandboxBackend::Podman {
                image: "img".into(),
            },
            SandboxBackend::Tart {
                image: "ghcr.io/example/macos-base:latest".into(),
            },
        ] {
            let provider = CliProvider::from_spec(broker_spec("c"), PathBuf::from("/usr/bin/c"))
                .with_backend(backend.clone());
            if let Err(e) = provider.start_broker().await {
                assert!(
                    !e.contains("cannot host a credential broker"),
                    "{backend:?} reached credential loading, not a backend veto: {e:?}"
                );
            }
        }
    }

    #[test]
    fn a_broker_aims_the_child_at_loopback_and_hands_it_no_credential() {
        // what the child gets: a base URL, an opaque bearer, and a fresh config
        // home. what it does NOT get: the credential — which is why the argv is
        // rewritten to point at the broker at all.
        let provider = CliProvider::from_spec(broker_spec("c"), PathBuf::from("/usr/bin/c"));
        let endpoint = broker::BrokerEndpoint {
            base_url: "http://127.0.0.1:54321/v1".into(),
            run_bearer: "opaque-run-bearer".into(),
            control_url: "http://127.0.0.1:54321/v1/control/provider-idle".into(),
            control_token: "opaque-control-token".into(),
        };
        let config_home = PathBuf::from("/tmp/wd/.ducktape-run/slot/provider-config");
        let auth = RunAuth {
            config_home: Some(&config_home),
            broker: Some(&endpoint),
        };
        let cmd = provider
            .command(
                &["exec".into(), "--json".into(), "-".into()],
                Path::new("/tmp/wd"),
                &RunContext::default(),
                &auth,
            )
            .expect("command builds");

        // the model provider is spliced in after args[0] (`exec`), and the
        // trailing "-" (prompt on stdin) is still last.
        let joined = argv_of(&cmd);
        assert!(joined.starts_with("exec -c model_providers.ducktape="), "{joined}");
        assert!(joined.contains("base_url=\"http://127.0.0.1:54321/v1\""), "{joined}");
        assert!(joined.contains("model_provider=\"ducktape\""), "{joined}");
        assert!(joined.ends_with("--json -"), "the stdin marker stays last: {joined}");

        let envs: BTreeMap<String, Option<String>> = cmd
            .as_std()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        // the fresh config home is what stops the CLI reading ~/.codex/auth.json.
        assert_eq!(
            envs.get("CODEX_HOME").cloned().flatten().as_deref(),
            Some(config_home.to_str().unwrap())
        );
        assert_eq!(
            envs.get(BROKER_TOKEN_ENV).cloned().flatten().as_deref(),
            Some("opaque-run-bearer")
        );
        assert_eq!(
            envs.get(PROVIDER_CONTROL_URL_ENV)
                .cloned()
                .flatten()
                .as_deref(),
            Some("http://127.0.0.1:54321/v1/control/provider-idle")
        );
        assert_eq!(
            envs.get(PROVIDER_CONTROL_TOKEN_ENV)
                .cloned()
                .flatten()
                .as_deref(),
            Some("opaque-control-token")
        );
        assert!(
            !joined.contains("opaque-control-token"),
            "the control credential stays out of argv: {joined}"
        );
        // and the upstream credential is REMOVED, not merely unset: a Direct child
        // inherits this process's env, and one that still saw OPENAI_API_KEY would
        // dial OpenAI directly, straight past the broker holding it.
        assert_eq!(
            envs.get("OPENAI_API_KEY"),
            Some(&None),
            "the inherited upstream credential is explicitly removed: {envs:?}"
        );
    }

    #[test]
    fn without_a_broker_has_no_control_capability() {
        // BYO providers get no model-provider splice or bearer. Reserved
        // control lookalikes are actively removed from the inherited env so a
        // stale operator value cannot turn into authority for another run.
        let provider = CliProvider::from_spec(sandbox_spec("plain"), PathBuf::from("/usr/bin/x"));
        let cmd = provider
            .command(
                &["run".into()],
                Path::new("/tmp/wd"),
                &RunContext::default(),
                &RunAuth::default(),
            )
            .expect("command builds");
        assert_eq!(argv_of(&cmd), "run");
        let envs: BTreeMap<String, Option<String>> = cmd
            .as_std()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(
            envs,
            BTreeMap::from([
                (PROVIDER_CONTROL_TOKEN_ENV.to_string(), None),
                (PROVIDER_CONTROL_URL_ENV.to_string(), None),
            ])
        );
    }

    #[test]
    fn sandbox_context_cannot_supply_reserved_control_env() {
        let provider = CliProvider::from_spec(sandbox_spec("plain"), PathBuf::from("/usr/bin/x"));
        let mut ctx = RunContext::default();
        ctx.env
            .insert(PROVIDER_CONTROL_URL_ENV.into(), "http://foreign".into());
        ctx.env
            .insert(PROVIDER_CONTROL_TOKEN_ENV.into(), "foreign-token".into());
        let (envs, _) = provider
            .sandbox_env_and_rw(&ctx, &RunAuth::default())
            .unwrap();
        assert!(envs.iter().all(|(key, _)| {
            key != PROVIDER_CONTROL_URL_ENV && key != PROVIDER_CONTROL_TOKEN_ENV
        }));
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
    async fn a_silent_cli_can_extend_only_its_live_broker_invocation() {
        let dir = scratch("idle-control");
        let endpoint_file = dir.join("control-endpoint");
        let bin = fake_cli(
            &dir,
            "controlled-sleeper",
            &format!(
                "cat > /dev/null\n\
                 printf '%s\\n%s\\n' \"$DUCKTAPE_PROVIDER_CONTROL_URL\" \
                   \"$DUCKTAPE_PROVIDER_CONTROL_TOKEN\" > {}\n\
                 sleep 0.45\n\
                 echo survived",
                endpoint_file.display()
            ),
        );
        let provider = sh_provider(
            broker_spec("controlled-sleeper"),
            bin,
            "idle-control-wd",
        )
        .with_timeout(Duration::from_millis(200));
        let broker = broker::RunBroker::start_for_test().await;
        let args = provider.spec.args.clone();
        let workdir = provider.workdir.clone();
        let running = tokio::spawn(async move {
            provider
                .invoke(
                    "x",
                    &args,
                    &workdir,
                    &RunContext::default(),
                    None,
                    Some(&broker),
                )
                .await
        });

        let (url, token) = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(contents) = std::fs::read_to_string(&endpoint_file) {
                    let values: Vec<_> = contents.lines().collect();
                    if values.len() == 2 && values.iter().all(|value| !value.is_empty()) {
                        break (values[0].to_string(), values[1].to_string());
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the provider receives its ambient control endpoint");
        let reply: serde_json::Value = reqwest::Client::new()
            .post(url)
            .header("x-ducktape-provider-control", token)
            .json(&serde_json::json!({
                "request_id":"silent-phase",
                "requested_secs":1,
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(reply["status"], "granted");
        let invocation = tokio::time::timeout(Duration::from_secs(2), running)
            .await
            .expect("the extension is bounded")
            .expect("provider task does not panic")
            .unwrap();
        assert_eq!(invocation.text, "survived");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_the_whole_provider_process_group() {
        let dir = scratch("cancel-process-group");
        let pidfile = dir.join("group.pid");
        let bin = fake_cli(
            &dir,
            "term-ignorer",
            &format!(
                "cat > /dev/null\ntrap '' TERM\necho $$ > {}\nsleep 30 &\nwait",
                pidfile.display()
            ),
        );
        let provider = mock_provider("term-ignorer", "text", bin, "cancel-process-group-wd")
            .with_timeout(Duration::from_secs(30));
        let cancellation = RunCancellation::new();
        let ctx = RunContext {
            cancellation: Some(cancellation.clone()),
            ..RunContext::default()
        };
        let running = tokio::spawn(async move { provider.run("x", &ctx).await });

        let group = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(pid) = std::fs::read_to_string(&pidfile)
                    && let Ok(pid) = pid.trim().parse::<u32>()
                {
                    break pid;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("fake provider reports its process group");

        cancellation.cancel();
        let error = tokio::time::timeout(Duration::from_secs(6), running)
            .await
            .expect("TERM grace is bounded and escalates to KILL")
            .expect("provider task does not panic")
            .unwrap_err();
        assert!(error.contains("cancelled"), "got: {error}");

        tokio::time::timeout(Duration::from_secs(1), async {
            while process_group_alive(group) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("no TERM-ignoring descendant survives the run");
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
            cancellation: None,
            executing_node: None,
            workdir_override: Some(override_dir.clone()),
            env: BTreeMap::from([(
                "DUCKTAPE_RUN_WORKSPACE".to_string(),
                override_dir.display().to_string(),
            )]),
            path_entries: vec![path_entry.clone()],
            limits: BTreeMap::new(),
            portable: true,
            context_doc: None,
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

    // ---- D7 isolation floor (portable env) --------------------------------------

    /// portable and non-portable runs use the SAME additive env overlay: no
    /// `env_clear`, no HOME override. get_envs (the explicitly-set overlay) is
    /// exactly ctx.env for both — so a headless CLI's BYO-auth (ambient
    /// ANTHROPIC_API_KEY, ~/.claude, &c) survives into a portable child.
    #[test]
    fn portable_and_nonportable_use_the_same_additive_env_overlay() {
        let spec = mock_spec("iso", "iso-cli", "text");
        let p = CliProvider::from_spec(spec, PathBuf::from("/bin/true"));
        let workdir = scratch("iso-portable-wd");

        let mut env = BTreeMap::new();
        env.insert("AGENT_TOKEN".to_string(), "abc".to_string());
        let mut expected: BTreeMap<String, Option<String>> = env
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();
        expected.insert(PROVIDER_CONTROL_URL_ENV.into(), None);
        expected.insert(PROVIDER_CONTROL_TOKEN_ENV.into(), None);

        for portable in [true, false] {
            let ctx = RunContext {
                portable,
                workdir_override: Some(workdir.clone()),
                env: env.clone(),
                ..Default::default()
            };
            let cmd = p
                .command(&[], &workdir, &ctx, &RunAuth::default())
                .expect("command");
            let envs: BTreeMap<String, Option<String>> = cmd
                .as_std()
                .get_envs()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.map(|v| v.to_string_lossy().into_owned()),
                    )
                })
                .collect();
            assert_eq!(
                envs, expected,
                "portable={portable}: env is additive except reserved control capabilities"
            );
        }
    }

    /// a portable run INHERITS the ambient `$HOME` — same as a non-portable run
    /// — so the headless claude/codex CLI finds its BYO credentials. This test
    /// covers the direct backend; Podman and Tart cross the D7 filesystem
    /// boundary only through their explicit auth and workspace mounts.
    #[tokio::test]
    async fn portable_runs_inherit_the_ambient_home_for_byo_auth() {
        let dir = scratch("portable-home");
        let bin = fake_cli(&dir, "home", "cat > /dev/null\nprintf '%s' \"$HOME\"");
        let p = mock_provider("home", "text", bin, "portable-home-wd");

        let real_home = std::env::var("HOME").expect("the test env has HOME set");
        let inherited = p.run("x", &RunContext::default()).await.unwrap();
        assert_eq!(inherited, real_home, "a non-portable run inherits HOME");

        let mount = scratch("portable-home-mount");
        let ctx = RunContext {
            portable: true,
            workdir_override: Some(mount.clone()),
            ..Default::default()
        };
        let portable_home = p.run("x", &ctx).await.unwrap();
        assert_eq!(
            portable_home, real_home,
            "a portable run ALSO inherits the ambient HOME so BYO-auth works"
        );
    }
}
