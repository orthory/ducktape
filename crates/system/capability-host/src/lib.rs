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
pub use sandbox::{SandboxBackend, wrap_podman, wrap_tart};
pub use session::{ResumeArgv, SessionCapture, SessionSpec};
pub use spec::{BrokerKind, CapabilitySpec, ContextLocation, IsolationSpec, OutputFormat, SpecSet};
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

/// a sandbox backend's env overlay (`(key, value)` pairs) plus the spec's
/// `~/`-relative rw mount dirs expanded to absolute host paths.
type SandboxEnvRw = (Vec<(String, String)>, Vec<PathBuf>);

/// everything one run hands its child so the CLI can authenticate WITHOUT the
/// operator's credential — both `None` for a plain BYO spec (no `[isolation]`),
/// which is the historical posture and still the default.
///
/// this is the whole of it: a directory and a bearer. the credential is not here
/// and never will be — it stays in this process, behind the [`broker`].
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
    /// how the child is spawned: `Direct` (the plain host spawn) or `Podman`
    /// (every run wrapped in a rootless container that enforces the run's
    /// numeric limits — see [`sandbox`]). set once at discovery for the whole
    /// provider set.
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

    fn command(
        &self,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<tokio::process::Command, String> {
        // a broker rewrites the argv BEFORE the backend sees it, so all three
        // backends aim the CLI at the loopback endpoint identically.
        let args = self.broker_argv(args, workdir, auth);
        // the backend picks HOW the child is spawned; the stdio/kill/cwd
        // handling below is identical either way. args are passed verbatim to
        // exec — never shell-interpreted (the resume path's {session_id} slot
        // was substituted host-side with an executor-minted id, never job
        // content) — so nothing in a job can inject flags or commands.
        let mut cmd = match &self.backend {
            SandboxBackend::Direct => self.direct_command(&args, ctx, auth)?,
            SandboxBackend::Podman { image } => {
                self.podman_command(image, &args, workdir, ctx, auth)?
            }
            // the base image is used at CLONE time (tart_setup), not in the run
            // argv — the run targets the already-cloned per-run VM.
            SandboxBackend::Tart { .. } => self.tart_command(&args, workdir, ctx, auth)?,
        };
        cmd.current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // dropping the wait future (timeout) must kill the child — a hung
            // CLI never outlives its job. under Podman the killed process is
            // `podman run`, which tears down the container (--rm) with it.
            .kill_on_drop(true);
        Ok(cmd)
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
    /// broker's `127.0.0.1:<port>` at the very address the argv names. (this is
    /// exactly what a VM guest CANNOT do — see [`Self::start_broker`].)
    fn podman_command(
        &self,
        image: &str,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<tokio::process::Command, String> {
        let (envs, rw_dirs) = self.sandbox_env_and_rw(ctx, auth)?;
        let (bin, argv) = sandbox::wrap_podman(
            image,
            &self.bin,
            args,
            workdir,
            &envs,
            &self.sandbox_ro_paths(ctx, workdir, auth)?,
            &rw_dirs,
            &ctx.limits,
        );
        let mut cmd = tokio::process::Command::new(bin);
        cmd.args(argv);
        Ok(cmd)
    }

    /// wrap the same invocation in `tart run <per-run-vm>`: the VM was already
    /// COW-cloned by [`Self::tart_setup`]; this only assembles the run argv, at
    /// identical host mount paths, with the same HOME/PATH/env + rw_dirs the
    /// Podman backend crosses (see [`sandbox::wrap_tart`] for tart's guest-exec
    /// and mount-point simplifications — real-Mac QA deferred).
    fn tart_command(
        &self,
        args: &[String],
        workdir: &Path,
        ctx: &RunContext,
        auth: &RunAuth<'_>,
    ) -> Result<tokio::process::Command, String> {
        let (envs, rw_dirs) = self.sandbox_env_and_rw(ctx, auth)?;
        let vm = sandbox::tart_vm_name(workdir);
        let (bin, argv) = sandbox::wrap_tart(
            &vm,
            &self.bin,
            args,
            workdir,
            &envs,
            &self.sandbox_ro_paths(ctx, workdir, auth)?,
            &rw_dirs,
            &ctx.limits,
        );
        let mut cmd = tokio::process::Command::new(bin);
        cmd.args(argv);
        Ok(cmd)
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
        let mut envs: Vec<(String, String)> = vec![("HOME".into(), home.display().to_string())];
        if let Some(path) = self.run_path(ctx)? {
            envs.push(("PATH".into(), path.to_string_lossy().into_owned()));
        }
        envs.extend(ctx.env.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.apply_auth_env(auth, |k, v| envs.push((k.to_string(), v)));
        // spec.rs already rejected absolute / `..` entries, so join is safe.
        let rw_dirs: Vec<PathBuf> = self
            .spec
            .rw_dirs
            .iter()
            .map(|d| home.join(d.strip_prefix("~/").unwrap_or(d)))
            .collect();
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
        Ok(paths)
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
    /// serves a loopback endpoint the child dials with an opaque per-run bearer;
    /// dropping it (any exit path of [`Self::run_output`]) tears the endpoint down.
    ///
    /// TART + BROKER IS UNSUPPORTED, loudly. a VM guest has its own network
    /// stack, so the host's `127.0.0.1:<port>` — the address the broker binds and
    /// the argv names — resolves inside the guest to the GUEST's own loopback,
    /// where nothing is listening. the run would fail at the first model call
    /// with a connection error that looks like a broken login. Podman is fine:
    /// `--network=host` shares the host's loopback (see [`Self::podman_command`]).
    async fn start_broker(&self) -> Result<Option<broker::RunBroker>, String> {
        let Some(kind) = self.spec.isolation.broker else {
            return Ok(None);
        };
        if let SandboxBackend::Tart { .. } = &self.backend {
            return Err(format!(
                "{}: the Tart backend cannot host a credential broker — a VM guest \
                 cannot reach the host's 127.0.0.1, so the broker endpoint would be \
                 unreachable and every model call would fail as a login error. run \
                 this spec under the Direct or Podman backend (Podman's --network=host \
                 shares the host loopback); giving the guest a host-gateway address is \
                 the upgrade path.",
                self.spec.tag
            ));
        }
        match kind {
            BrokerKind::CodexResponses => broker::RunBroker::start().await.map(Some),
        }
    }

    /// the run's auth env, backend-independent: the fresh config home (so the
    /// CLI cannot read the operator's real one) and the broker's opaque per-run
    /// bearer. `set` is how the caller applies one binding — a `Command` env for
    /// Direct, a `-e K=V` entry for a sandbox.
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

    /// the Tart backend's impure per-run lifecycle: acquire the process-wide
    /// concurrency permit (WAITS past 2, never errors — Apple's 2-VM limit),
    /// then APFS-COW-clone the base image into this run's VM. returns a guard
    /// whose Drop deletes the clone on EVERY exit path; `Ok(None)` for the
    /// Direct/Podman backends (no-op).
    ///
    /// REAL-MAC QA DEFERRED: there is no tart or macOS on the build box, so the
    /// clone/delete spawns are unit-untested — [`sandbox::wrap_tart`]'s pure
    /// argv is the tested surface, and it documents the guest-exec/mount model
    /// this lifecycle would need to grow (`tart set`, boot + `ssh`, guest path
    /// remap) for a live run. clone failure is a LOUD error — no silent
    /// fallback to unsandboxed execution.
    async fn tart_setup(&self, workdir: &Path) -> Result<Option<TartGuard>, String> {
        let SandboxBackend::Tart { image } = &self.backend else {
            return Ok(None);
        };
        let vm = sandbox::tart_vm_name(workdir);
        // WAITS if 2 tart runs are already live — this is the cap, not an error.
        let permit = sandbox::tart_semaphore()
            .acquire()
            .await
            .map_err(|e| format!("{}: tart concurrency gate closed: {e}", self.spec.tag))?;
        let status = tokio::process::Command::new("tart")
            .args(["clone", image, &vm])
            .status()
            .await
            .map_err(|e| {
                format!("{}: `tart clone {image} {vm}` failed to spawn: {e}", self.spec.tag)
            })?;
        if !status.success() {
            return Err(format!(
                "{}: `tart clone {image} {vm}` exited with {status}",
                self.spec.tag
            ));
        }
        Ok(Some(TartGuard { vm, _permit: permit }))
    }

    /// the run-scoped PATH: `ctx.path_entries` prepended to the inherited PATH,
    /// or `None` when the run adds no entries. shared by both backends (Direct
    /// sets it as the child's PATH env; Podman exports it via `-e PATH=`).
    fn run_path(&self, ctx: &RunContext) -> Result<Option<OsString>, String> {
        if ctx.path_entries.is_empty() {
            return Ok(None);
        }
        let mut path = ctx.path_entries.clone();
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

/// holds a Tart run's process-wide concurrency permit and deletes its per-run
/// clone on drop — the run's LAST teardown step, on every exit path (success,
/// error, timeout, panic). declared before the `tart run` child so the child
/// drops first (kill_on_drop stops the VM), then this removes the clone and
/// releases the permit. best-effort: a delete failure is logged, never
/// surfaced (the run's own result already left).
/// ponytail: a still-running VM may need `tart stop` before delete; the
/// real-Mac pass confirms whether killing `tart run` frees the clone.
struct TartGuard {
    vm: String,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

impl Drop for TartGuard {
    fn drop(&mut self) {
        // synchronous best-effort cleanup: Drop can't await, and a brief block
        // to reclaim a VM clone is acceptable teardown.
        match std::process::Command::new("tart")
            .args(["delete", &self.vm])
            .status()
        {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("[capability-host] `tart delete {}` exited with {s}", self.vm),
            Err(e) => eprintln!("[capability-host] `tart delete {}` failed to spawn: {e}", self.vm),
        }
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
        auth: &RunAuth<'_>,
    ) -> Result<Invocation, String> {
        // Tart backend: acquire the concurrency permit + COW-clone the base
        // image BEFORE spawning; the guard (declared first, so it drops LAST —
        // after the `tart run` child) deletes the clone on every exit path. a
        // no-op guard for Direct/Podman. clone failure aborts the run loudly.
        let _tart_guard = self.tart_setup(workdir).await?;
        let mut child = self
            .command(args, workdir, ctx, auth)?
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
        let workdir = self.ensure_writable_workdir(ctx)?;
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
        let auth = RunAuth {
            config_home: config_home.as_deref(),
            broker: broker.as_ref().map(|b| &b.endpoint),
        };

        let Some((session, store)) = self.session_store(ctx)? else {
            // no session plumbing for this run: one cold invocation.
            let run = self
                .invoke(prompt, &self.spec.args, &workdir, ctx, &auth)
                .await?;
            return Ok(ProviderOutput {
                text: run.text,
                usage: run.usage,
            });
        };

        if let Some(session_id) = store.load() {
            let argv = session::resume_argv(&self.spec.args, &session.resume, &session_id);
            match self.invoke(prompt, &argv, &workdir, ctx, &auth).await {
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
            .invoke(prompt, &self.spec.args, &workdir, ctx, &auth)
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
    backend: SandboxBackend,
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

    // ---- podman backend glue ------------------------------------------------

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
        let provider = CliProvider::from_spec(spec, PathBuf::from("/usr/bin/pod"))
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let ctx = RunContext {
            limits: BTreeMap::from([("cores".to_string(), 2u64)]),
            ..RunContext::default()
        };
        let cmd = provider
            .command(
                &["--go".into()],
                Path::new("/tmp/wd"),
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
        let home = std::env::var("HOME").expect("HOME set in test env");
        // the spec's `~/.claude` expands against the real HOME and mounts rw at
        // its IDENTICAL container path; the bin mounts ro; limits become flags.
        assert!(
            joined.contains(&format!("-v {home}/.claude:{home}/.claude")),
            "rw mount at identical path: {joined}"
        );
        assert!(joined.contains("-v /usr/bin/pod:/usr/bin/pod:ro"), "{joined}");
        assert!(joined.contains(&format!("-e HOME={home}")), "{joined}");
        assert!(joined.contains("--cpus 2"), "{joined}");
        assert!(joined.ends_with("img /usr/bin/pod --go"), "{joined}");
    }

    #[test]
    fn tart_backend_wraps_command_with_identical_mount_paths_and_rw_dirs() {
        // the command() glue for Tart: HOME assembled, the spec's `~/` rw_dirs
        // expanded against it, identical host mount paths handed to wrap_tart.
        // wrap_tart's own translation is covered in sandbox.rs; the clone/delete
        // lifecycle is real-Mac-QA-deferred (no tart on this box).
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
[sandbox]
rw_dirs = ["~/.claude"]
"#,
            "test",
        )
        .unwrap();
        let provider = CliProvider::from_spec(spec, PathBuf::from("/usr/bin/vm"))
            .with_backend(SandboxBackend::Tart {
                image: "ghcr.io/example/macos-base:latest".into(),
            });
        let ctx = RunContext {
            limits: BTreeMap::from([("mem_gb".to_string(), 4u64)]),
            ..RunContext::default()
        };
        // workdir's final component becomes the (deterministic) per-run VM name.
        let cmd = provider
            .command(
                &["--go".into()],
                Path::new("/tmp/ducktape-run-7"),
                &ctx,
                &RunAuth::default(),
            )
            .expect("tart command builds");
        let std = cmd.as_std();
        assert_eq!(std.get_program(), std::ffi::OsStr::new("tart"));
        let joined: Vec<String> = std
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let joined = joined.join(" ");
        let home = std::env::var("HOME").expect("HOME set in test env");
        // the spec's `~/.claude` expands against the real HOME and mounts rw at
        // its identical host path (source); memory is MB; HOME rides the env
        // prefix; the run targets the per-run VM named for the workdir.
        assert!(
            joined.contains(&format!(":{home}/.claude ")),
            "rw mount source at identical path: {joined}"
        );
        assert!(joined.contains("--memory 4096"), "{joined}");
        assert!(joined.contains("ducktape-run-7 env "), "per-run vm name: {joined}");
        assert!(joined.contains(&format!("HOME={home}")), "{joined}");
        assert!(joined.ends_with("/usr/bin/vm --go"), "{joined}");
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

    #[test]
    fn a_sandboxed_run_mounts_its_skills_tree_read_only_when_it_has_one() {
        // W6 skills live at a SIBLING of the rw checkout (outside the workdir, so
        // `commit` never scans them), and the provisioner tells the child where
        // via DUCKTAPE_RUN_SKILLS. under Direct the env alone works — the path is
        // on the host. under a sandbox, only what we mount exists, so without the
        // mount the agent would find its own skills dir simply MISSING.
        let provider = CliProvider::from_spec(sandbox_spec("pod"), PathBuf::from("/usr/bin/pod"))
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let ctx = RunContext {
            env: BTreeMap::from([(
                SKILLS_ROOT_ENV.to_string(),
                "/var/run/ducktape/agent-7-ro".to_string(),
            )]),
            ..RunContext::default()
        };
        let cmd = provider
            .command(&[], Path::new("/tmp/wd"), &ctx, &RunAuth::default())
            .expect("podman command builds");
        let joined = argv_of(&cmd);
        // READ-ONLY, at the identical path the env names: the agent may read its
        // skills, never rewrite them.
        assert!(
            joined.contains(
                "-v /var/run/ducktape/agent-7-ro:/var/run/ducktape/agent-7-ro:ro"
            ),
            "the skills root mounts ro at its identical path: {joined}"
        );
        assert!(
            joined.contains(&format!("-e {SKILLS_ROOT_ENV}=/var/run/ducktape/agent-7-ro")),
            "and the env still points at it: {joined}"
        );
    }

    #[test]
    fn a_run_with_no_skills_mounts_none() {
        // no skills on the run = no mount. (the provisioner omits the env entirely
        // when the agent has no skill records — see agent_provision's plane tests.)
        let provider = CliProvider::from_spec(sandbox_spec("pod"), PathBuf::from("/usr/bin/pod"))
            .with_backend(SandboxBackend::Podman {
                image: "img".into(),
            });
        let cmd = provider
            .command(
                &[],
                Path::new("/tmp/wd"),
                &RunContext::default(),
                &RunAuth::default(),
            )
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
        let ctx = RunContext {
            context_doc: Some(SOUL.to_string()),
            ..RunContext::default()
        };

        // OUTSIDE the workdir mount: without this bind the file would exist on the
        // host and simply not be there for the child — a silently unsouled agent.
        let provider = CliProvider::from_spec(
            spec_with("pod", "[context]\npath = \"workspace-parent:CLAUDE.md\"\n"),
            PathBuf::from("/usr/bin/pod"),
        )
        .with_backend(SandboxBackend::Podman {
            image: "img".into(),
        });
        let cmd = provider
            .command(&[], Path::new("/tmp/wd"), &ctx, &RunAuth::default())
            .expect("podman command builds");
        let joined = argv_of(&cmd);
        assert!(
            joined.contains("-v /tmp/CLAUDE.md:/tmp/CLAUDE.md:ro"),
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
            PathBuf::from("/usr/bin/pod"),
        )
        .with_backend(SandboxBackend::Podman {
            image: "img".into(),
        });
        let config_home = PathBuf::from("/tmp/wd/.ducktape-run/slot/provider-config");
        let auth = RunAuth {
            config_home: Some(&config_home),
            broker: None,
        };
        let cmd = provider
            .command(&[], Path::new("/tmp/wd"), &ctx, &auth)
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
    async fn tart_plus_a_broker_fails_loudly_rather_than_mysteriously() {
        // a VM guest has its own network stack, so the broker's host-loopback
        // endpoint is simply unreachable from inside it — the run would die at
        // the first model call with what LOOKS like a broken login. so the
        // combination is refused up front, by name, with the upgrade path.
        let provider = CliProvider::from_spec(broker_spec("vm"), PathBuf::from("/usr/bin/vm"))
            .with_backend(SandboxBackend::Tart {
                image: "ghcr.io/example/macos-base:latest".into(),
            });
        // (no `unwrap_err`: RunBroker holds a live credential and deliberately
        // has no Debug — a panic message must never be able to print one.)
        let Err(err) = provider.start_broker().await else {
            panic!("Tart + broker must be refused, not started");
        };
        assert!(err.contains("Tart"), "names the backend: {err:?}");
        assert!(err.contains("127.0.0.1"), "names the limitation: {err:?}");
        assert!(err.contains("host-gateway"), "names the upgrade path: {err:?}");

        // Podman is fine — --network=host shares the host's loopback — and so is
        // Direct. (neither actually starts here: with no host credential the
        // broker refuses, which is itself the proof we got PAST the backend gate.)
        for backend in [
            SandboxBackend::Direct,
            SandboxBackend::Podman {
                image: "img".into(),
            },
        ] {
            let provider =
                CliProvider::from_spec(broker_spec("c"), PathBuf::from("/usr/bin/c"))
                    .with_backend(backend.clone());
            if let Err(e) = provider.start_broker().await {
                assert!(
                    !e.contains("cannot host a credential broker"),
                    "{backend:?} must not be refused for Tart's reason: {e:?}"
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
    fn without_a_broker_the_argv_and_env_are_untouched() {
        // the BYO posture is the default and stays byte-for-byte what it was: no
        // model-provider splice, no bearer, and nothing removed from the child's
        // inherited environment.
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
        let envs: Vec<String> = cmd
            .as_std()
            .get_envs()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert!(envs.is_empty(), "no auth env overlay at all: {envs:?}");
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
        let expected: BTreeMap<String, Option<String>> = env
            .iter()
            .map(|(k, v)| (k.clone(), Some(v.clone())))
            .collect();

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
                "portable={portable}: env is the additive overlay (no env_clear, no HOME override)"
            );
        }
    }

    /// a portable run INHERITS the ambient `$HOME` — same as a non-portable run
    /// — so the headless claude/codex CLI finds its BYO credentials. (The env
    /// half of the D7 isolation floor needs the ADR's deferred sandbox
    /// mechanism; the ACTIVE D7 measure is the workspace relocation, i.e. the
    /// cwd being the per-run mount outside <storage>, not an env rewrite.)
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
