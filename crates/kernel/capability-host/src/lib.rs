//! host-side capability providers — the I/O half of the capability seam.
//!
//! the capability module (consensus) replicates *who provides what*; this
//! crate is the machine-local counterpart that actually provides it. a
//! [`Provider`] wraps one locally installed executor CLI, and [`discover`]
//! probes the host for the executors the operator brought — BYO by
//! construction: the node spawns child processes and never reads, writes, or
//! refreshes any credential file. auth, token rotation, and endpoint choice
//! are entirely the CLI's own business.
//!
//! ## executors are data: the capability spec
//!
//! WHICH executors exist, how to detect them, the argv to run them, how to
//! parse their output, and which model refs route to them is all described by
//! TOML capability specs (see [`spec`] and `docs/capability-spec.md`), not by
//! Rust. the built-in codex/claude support is two embedded spec files parsed
//! by the same code path as operator-provided specs under
//! `$DUCKTAPE_CAPABILITY_DIR` (default `~/.ducktape/capabilities`). adding an
//! executor — or retuning a built-in's flags — is a config change on the
//! operator's machine, never a code change here.
//!
//! the CLIs are agentic, not plain inference endpoints, so a provider runs
//! them fenced: non-interactive one-shot mode, most-restricted sandbox flags
//! (as encoded in the spec's argv), and an empty scratch working directory —
//! the child never sees the node's data directory. the fence is what turns
//! "orchestrate a coding agent" into "use it as a text oracle".

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::AsyncWriteExt as _;

mod spec;
pub use spec::{CapabilitySpec, OutputFormat, PromptMode, SpecSet, builtin_specs};

/// the tag of the embedded codex spec — a convenience for tests and wiring;
/// the authoritative source is `specs/codex.toml`.
pub const CODEX: &str = "codex";
/// the tag of the embedded claude spec (see `specs/claude.toml`).
pub const CLAUDE: &str = "claude";

/// one unit of provider work: the fully rendered prompt and the RESOLVED
/// model to run it on (resolution — pinned ref or spec default — happens in
/// [`ProviderSet::resolve`]). rendering (conversation -> text) is the
/// CALLER's business — this crate is deliberately ignorant of chat shapes
/// and saga specs.
pub struct ProviderJob {
    pub prompt: String,
    pub model_ref: String,
}

/// a machine-local executor for one capability tag. implementations do real
/// I/O (spawn processes); nothing consensus-side may ever hold one.
#[async_trait::async_trait(?Send)]
pub trait Provider {
    /// the capability tag this provider serves — matches the capability
    /// module's registry entries, so "what i can run" and "what i announce"
    /// cannot drift apart.
    fn capability(&self) -> &str;
    /// run one job to completion and return the assistant's final text.
    async fn run(&self, job: &ProviderJob) -> Result<String, String>;
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
    pub fn empty() -> Self {
        Self {
            specs: SpecSet::from_specs(Vec::new()),
            providers: Vec::new(),
        }
    }

    pub fn find(&self, capability: &str) -> Option<&dyn Provider> {
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

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn specs(&self) -> &SpecSet {
        &self.specs
    }

    /// full model-ref resolution, the one entry point callers should use:
    /// route the ref to a spec, resolve the effective model (pinned ref, else
    /// the spec's default), and look up the local provider. every failure is
    /// a distinct, actionable error naming what is missing — a mis-typed
    /// model, a spec without a default, or a capability this node simply does
    /// not provide.
    pub fn resolve(&self, model_ref: &str) -> Result<(&dyn Provider, String), String> {
        let Some(spec) = self.specs.route(model_ref) else {
            let loaded: Vec<&str> = self.specs.iter().map(|s| s.tag.as_str()).collect();
            return Err(format!(
                "no capability spec matches model {model_ref:?}; loaded specs: {loaded:?}"
            ));
        };
        let model = match model_ref.trim() {
            "" => spec.default_model.clone().ok_or_else(|| {
                format!(
                    "capability '{}' has no default model; pin a model_ref",
                    spec.tag
                )
            })?,
            pinned => pinned.to_string(),
        };
        let Some(provider) = self.find(&spec.tag) else {
            return Err(format!(
                "capability '{}' (model {model:?}) is not provided by this node; \
                 this node provides {:?}",
                spec.tag,
                self.capabilities()
            ));
        };
        Ok((provider, model))
    }
}

/// a [`Provider`] that interprets one [`CapabilitySpec`] against one resolved
/// binary: spawn `bin` with the spec's argv (`{model}` substituted), feed the
/// prompt on stdin, parse stdout with the spec's named format.
pub struct CliProvider {
    spec: CapabilitySpec,
    bin: PathBuf,
    /// the child's working directory — an empty scratch dir, never the node's
    /// data directory, so an agentic CLI has nothing to wander into.
    workdir: PathBuf,
    timeout: Duration,
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
        }
    }

    /// a provider from the embedded codex spec — test/wiring convenience.
    pub fn codex(bin: PathBuf) -> Self {
        Self::from_builtin(CODEX, bin)
    }

    /// a provider from the embedded claude spec.
    pub fn claude(bin: PathBuf) -> Self {
        Self::from_builtin(CLAUDE, bin)
    }

    fn from_builtin(tag: &str, bin: PathBuf) -> Self {
        let spec = builtin_specs()
            .into_iter()
            .find(|s| s.tag == tag)
            .expect("builtin spec tags are CI-validated");
        Self::from_spec(spec, bin)
    }

    pub fn with_workdir(mut self, workdir: PathBuf) -> Self {
        self.workdir = workdir;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn command(&self, job: &ProviderJob) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(&self.bin);
        // argv straight from the spec, `{model}` substituted. args are passed
        // verbatim to exec — never shell-interpreted, so a prompt or model
        // ref cannot inject flags or commands.
        cmd.args(
            self.spec
                .args
                .iter()
                .map(|a| a.replace("{model}", &job.model_ref)),
        );
        cmd.current_dir(&self.workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // dropping the wait future (timeout) must kill the child — a hung
            // CLI never outlives its job.
            .kill_on_drop(true);
        cmd
    }
}

#[async_trait::async_trait(?Send)]
impl Provider for CliProvider {
    fn capability(&self) -> &str {
        &self.spec.tag
    }

    async fn run(&self, job: &ProviderJob) -> Result<String, String> {
        std::fs::create_dir_all(&self.workdir)
            .map_err(|e| format!("provider scratch dir {}: {e}", self.workdir.display()))?;
        let mut child = self
            .command(job)
            .spawn()
            .map_err(|e| format!("spawn {} failed: {e}", self.bin.display()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "child stdin was not piped".to_string())?;

        // feed the prompt CONCURRENTLY with collecting output: a prompt larger
        // than the pipe buffer would deadlock a sequential write-then-wait if
        // the CLI streams output before draining stdin. (PromptMode::Stdin is
        // the only v1 mode; the irrefutable match fails loud in review when a
        // second mode lands and this site needs a real branch.)
        let PromptMode::Stdin = self.spec.prompt;
        let feed = async {
            stdin.write_all(job.prompt.as_bytes()).await?;
            stdin.shutdown().await?;
            drop(stdin); // EOF: the prompt is complete
            Ok::<(), std::io::Error>(())
        };
        let (fed, out) = tokio::time::timeout(self.timeout, async {
            tokio::join!(feed, child.wait_with_output())
        })
        .await
        .map_err(|_| {
            format!(
                "{} timed out after {:?} (child killed)",
                self.bin.display(),
                self.timeout
            )
        })?;
        let out = out.map_err(|e| format!("waiting on {} failed: {e}", self.bin.display()))?;

        if !out.status.success() {
            // a failed exit is the primary diagnostic — it subsumes any
            // stdin write error (an early-exiting child EPIPEs the feed).
            return Err(format!(
                "{} exited with {}: {}",
                self.bin.display(),
                out.status,
                excerpt(&String::from_utf8_lossy(&out.stderr))
            ));
        }
        if let Err(e) = fed {
            return Err(format!(
                "writing the prompt to {} failed: {e}",
                self.bin.display()
            ));
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        match self.spec.output {
            OutputFormat::CodexJsonl => parse_codex_output(&stdout),
            OutputFormat::ClaudeJson => parse_claude_output(&stdout),
            OutputFormat::Text => parse_text_output(&stdout),
        }
    }
}

/// the LAST agent message in a `codex exec --json` event stream. tolerant of
/// the two shapes the CLI has shipped (item events with `type` or `item_type`,
/// and the older `msg` envelope) and of non-json noise lines; anything else is
/// an explicit error carrying an output excerpt, never a silent empty answer.
fn parse_codex_output(stdout: &str) -> Result<String, String> {
    let mut last: Option<String> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(item) = v.get("item") {
            let kind = item
                .get("type")
                .or_else(|| item.get("item_type"))
                .and_then(Value::as_str);
            if kind == Some("agent_message") {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    last = Some(text.to_string());
                }
            }
        }
        if let Some(msg) = v.get("msg") {
            if msg.get("type").and_then(Value::as_str) == Some("agent_message") {
                if let Some(text) = msg.get("message").and_then(Value::as_str) {
                    last = Some(text.to_string());
                }
            }
        }
    }
    last.ok_or_else(|| format!("codex output carried no agent message: {}", excerpt(stdout)))
}

/// the result text of a `claude -p --output-format json` run: one result
/// object, whole-output first, then per-line for robustness against banner
/// noise. an `is_error` result is surfaced as the error it is.
fn parse_claude_output(stdout: &str) -> Result<String, String> {
    let candidates = std::iter::once(stdout.trim()).chain(stdout.lines().rev().map(str::trim));
    for candidate in candidates {
        let Ok(v) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("result") {
            continue;
        }
        if v.get("is_error").and_then(Value::as_bool) == Some(true) {
            return Err(format!(
                "claude reported an error result: {}",
                excerpt(candidate)
            ));
        }
        if let Some(text) = v.get("result").and_then(Value::as_str) {
            return Ok(text.to_string());
        }
    }
    Err(format!("claude output carried no result: {}", excerpt(stdout)))
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
/// `DUCKTAPE_PROVIDER_TIMEOUT_SECS` overrides every spec's timeout at once.
/// what discovery finds is exactly what the node announces.
pub fn discover() -> Result<ProviderSet, String> {
    let specs = SpecSet::load(operator_spec_dir().as_deref())?;
    let timeout = std::env::var("DUCKTAPE_PROVIDER_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs);
    Ok(discover_with(
        specs,
        std::env::var_os("PATH"),
        &|k| std::env::var_os(k),
        timeout,
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
fn discover_with(
    specs: SpecSet,
    path: Option<OsString>,
    env: &dyn Fn(&str) -> Option<OsString>,
    global_timeout: Option<Duration>,
) -> ProviderSet {
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    for spec in specs.iter() {
        let Some(bin) = resolve_bin(spec, path.as_deref(), env) else {
            continue;
        };
        let mut provider = CliProvider::from_spec(spec.clone(), bin);
        if let Some(t) = global_timeout {
            provider = provider.with_timeout(t);
        }
        providers.push(Box::new(provider));
    }
    ProviderSet::assemble(specs, providers)
}

/// resolve one spec's binary: the spec's env override wins (and a BROKEN
/// override is a loud warning + absent capability, never a silent fallback to
/// PATH — the operator said "use this", and this does not exist), else the
/// first executable `detect.bin` on `path`.
fn resolve_bin(
    spec: &CapabilitySpec,
    path: Option<&OsStr>,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    if let Some(explicit) = spec.env.as_deref().and_then(env) {
        let p = PathBuf::from(&explicit);
        if is_executable(&p) {
            return Some(p);
        }
        eprintln!(
            "[capability-host] override for '{}' ({}) is not an executable file; \
             the capability will NOT be announced",
            spec.tag,
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

    /// write an executable /bin/sh script standing in for a CLI.
    fn fake_cli(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create fake cli");
        writeln!(f, "#!/bin/sh\n{body}").expect("write fake cli");
        f.set_permissions(std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake cli");
        path
    }

    fn job(prompt: &str) -> ProviderJob {
        ProviderJob {
            prompt: prompt.into(),
            model_ref: "test-model".into(),
        }
    }

    fn no_env(_: &str) -> Option<OsString> {
        None
    }

    fn builtins() -> SpecSet {
        SpecSet::load(None).unwrap()
    }

    // ---- discovery ----------------------------------------------------------

    #[test]
    fn discovery_finds_executables_on_path() {
        let dir = scratch("discovery-path");
        fake_cli(&dir, "codex", "exit 0");
        let set = discover_with(builtins(), Some(dir.clone().into_os_string()), &no_env, None);
        assert_eq!(set.capabilities(), vec![CODEX], "codex found, claude absent");
        assert!(set.find(CODEX).is_some());
        assert!(set.find(CLAUDE).is_none());
    }

    #[test]
    fn discovery_finds_both_and_sorts_tags() {
        let dir = scratch("discovery-both");
        fake_cli(&dir, "codex", "exit 0");
        fake_cli(&dir, "claude", "exit 0");
        let set = discover_with(builtins(), Some(dir.into_os_string()), &no_env, None);
        assert_eq!(set.capabilities(), vec![CLAUDE, CODEX], "sorted tag list");
    }

    #[test]
    fn explicit_override_wins_and_a_broken_override_announces_nothing() {
        let dir = scratch("discovery-override");
        let real = fake_cli(&dir, "my-codex", "exit 0");
        // the embedded codex spec names DUCKTAPE_CODEX_BIN as its override.
        let real_os = real.into_os_string();
        let env = move |k: &str| (k == "DUCKTAPE_CODEX_BIN").then(|| real_os.clone());
        let set = discover_with(builtins(), None, &env, None);
        assert_eq!(set.capabilities(), vec![CODEX]);

        // ... and a dangling override is absent, not a silent PATH fallback.
        let missing = dir.join("nope").into_os_string();
        let env = move |k: &str| (k == "DUCKTAPE_CODEX_BIN").then(|| missing.clone());
        fake_cli(&dir, "codex", "exit 0");
        let set = discover_with(builtins(), Some(dir.into_os_string()), &env, None);
        assert!(
            set.find(CODEX).is_none(),
            "broken override must not fall back to PATH"
        );
    }

    #[test]
    fn non_executable_files_are_not_discovered() {
        let dir = scratch("discovery-noexec");
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("codex");
        std::fs::write(&path, "not a program").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let set = discover_with(builtins(), Some(dir.into_os_string()), &no_env, None);
        assert!(set.find(CODEX).is_none(), "mode 644 is not executable");
    }

    #[test]
    fn an_operator_spec_discovers_a_custom_executor() {
        // the whole point of the spec format: a third executor with ZERO code.
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
args = ["{model}"]
prompt = "stdin"
[output]
format = "text"
[models]
patterns = ["my-*"]
"#,
            "test",
        )
        .unwrap();
        let specs = SpecSet::from_specs(vec![custom]);
        let set = discover_with(specs, Some(dir.into_os_string()), &no_env, None);
        assert_eq!(set.capabilities(), vec!["myllm"]);
    }

    // ---- resolve() ----------------------------------------------------------

    #[test]
    fn resolve_routes_resolves_defaults_and_names_every_failure() {
        let dir = scratch("resolve");
        fake_cli(&dir, "codex", "exit 0");
        let set = discover_with(builtins(), Some(dir.into_os_string()), &no_env, None);

        // pinned ref routes and keeps its model.
        let (p, model) = set.resolve("gpt-5.5-codex").unwrap();
        assert_eq!(p.capability(), CODEX);
        assert_eq!(model, "gpt-5.5-codex");

        // unpinned routes to the catch-all and takes ITS default model.
        let (p, model) = set.resolve("").unwrap();
        assert_eq!(p.capability(), CODEX);
        assert_eq!(model, "gpt-5.3-codex-spark");

        // routed-but-not-installed names the capability and what IS provided.
        let err = set.resolve("claude-sonnet-5").err().expect("claude is not installed");
        assert!(err.contains("capability 'claude'"), "got: {err}");
        assert!(err.contains("codex"), "names what the node provides: {err}");

        // an empty set fails with "no spec", not a panic.
        let err = ProviderSet::empty().resolve("anything").err().expect("empty set");
        assert!(err.contains("no capability spec matches"), "got: {err}");
    }

    #[test]
    fn resolve_unpinned_without_a_default_model_is_a_clear_error() {
        let dir = scratch("resolve-nodefault");
        fake_cli(&dir, "x-cli", "exit 0");
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "x"
[detect]
bin = "x-cli"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
[models]
patterns = ["*"]
"#,
            "test",
        )
        .unwrap();
        let set = discover_with(
            SpecSet::from_specs(vec![spec]),
            Some(dir.into_os_string()),
            &no_env,
            None,
        );
        let err = set.resolve("").err().expect("no default model");
        assert!(err.contains("no default model"), "got: {err}");
    }

    // ---- providers end-to-end ------------------------------------------------

    #[tokio::test]
    async fn codex_provider_round_trips_the_prompt() {
        let dir = scratch("codex-run");
        // echo the stdin prompt back inside an agent_message event, plus noise
        // lines the parser must skip.
        let bin = fake_cli(
            &dir,
            "codex",
            r#"prompt=$(cat)
echo "not json"
printf '{"type":"item.completed","item":{"type":"agent_message","text":"echo: %s"}}\n' "$prompt""#,
        );
        let p = CliProvider::codex(bin).with_workdir(scratch("codex-run-wd"));
        let text = p.run(&job("ping")).await.unwrap();
        assert_eq!(text, "echo: ping", "prompt fed on stdin, text parsed back");
    }

    #[tokio::test]
    async fn codex_parser_takes_the_last_agent_message() {
        let dir = scratch("codex-last");
        let bin = fake_cli(
            &dir,
            "codex",
            r#"cat > /dev/null
printf '{"type":"item.completed","item":{"type":"agent_message","text":"first"}}\n'
printf '{"type":"item.completed","item":{"item_type":"agent_message","text":"second"}}\n'"#,
        );
        let p = CliProvider::codex(bin).with_workdir(scratch("codex-last-wd"));
        assert_eq!(p.run(&job("x")).await.unwrap(), "second");
    }

    #[tokio::test]
    async fn claude_provider_parses_the_result_object() {
        let dir = scratch("claude-run");
        let bin = fake_cli(
            &dir,
            "claude",
            r#"cat > /dev/null
printf '{"type":"result","subtype":"success","is_error":false,"result":"pong"}\n'"#,
        );
        let p = CliProvider::claude(bin).with_workdir(scratch("claude-run-wd"));
        assert_eq!(p.run(&job("ping")).await.unwrap(), "pong");
    }

    #[tokio::test]
    async fn claude_error_results_surface_as_errors() {
        let dir = scratch("claude-err");
        let bin = fake_cli(
            &dir,
            "claude",
            r#"cat > /dev/null
printf '{"type":"result","subtype":"error_max_turns","is_error":true,"result":"boom"}\n'"#,
        );
        let p = CliProvider::claude(bin).with_workdir(scratch("claude-err-wd"));
        let err = p.run(&job("ping")).await.unwrap_err();
        assert!(err.contains("error result"), "got: {err}");
    }

    #[tokio::test]
    async fn text_format_returns_trimmed_stdout_and_rejects_empty() {
        let dir = scratch("text-run");
        let spec = CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "plain"
[detect]
bin = "plain"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
[models]
patterns = ["*"]
"#,
            "test",
        )
        .unwrap();
        let bin = fake_cli(&dir, "plain", "cat > /dev/null\necho '  the answer  '");
        let p = CliProvider::from_spec(spec.clone(), bin).with_workdir(scratch("text-run-wd"));
        assert_eq!(p.run(&job("q")).await.unwrap(), "the answer");

        // "ran fine, said nothing" is a broken executor, not an answer.
        let silent = fake_cli(&dir, "silent", "cat > /dev/null");
        let p = CliProvider::from_spec(spec, silent).with_workdir(scratch("text-silent-wd"));
        let err = p.run(&job("q")).await.unwrap_err();
        assert!(err.contains("no output"), "got: {err}");
    }

    #[tokio::test]
    async fn the_model_placeholder_is_substituted_into_argv() {
        let dir = scratch("model-subst");
        // the fake prints its FIRST ARG — which the spec routes {model} into.
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
[models]
patterns = ["*"]
"#,
            "test",
        )
        .unwrap();
        let bin = fake_cli(
            &dir,
            "argecho",
            r#"cat > /dev/null
echo "model=$1""#,
        );
        let p = CliProvider::from_spec(spec, bin).with_workdir(scratch("model-subst-wd"));
        let out = p
            .run(&ProviderJob {
                prompt: "q".into(),
                model_ref: "my-model-9".into(),
            })
            .await
            .unwrap();
        assert_eq!(out, "model=my-model-9");
    }

    #[tokio::test]
    async fn a_failing_cli_surfaces_status_and_stderr() {
        let dir = scratch("cli-fail");
        let bin = fake_cli(&dir, "codex", "cat > /dev/null\necho 'auth missing' >&2\nexit 3");
        let p = CliProvider::codex(bin).with_workdir(scratch("cli-fail-wd"));
        let err = p.run(&job("x")).await.unwrap_err();
        assert!(err.contains("auth missing"), "stderr in error: {err}");
        assert!(err.contains("exited with"), "status in error: {err}");
    }

    #[tokio::test]
    async fn output_without_an_agent_message_is_an_error_not_empty() {
        let dir = scratch("no-message");
        let bin = fake_cli(
            &dir,
            "codex",
            r#"cat > /dev/null
printf '{"type":"turn.completed"}\n'"#,
        );
        let p = CliProvider::codex(bin).with_workdir(scratch("no-message-wd"));
        let err = p.run(&job("x")).await.unwrap_err();
        assert!(err.contains("no agent message"), "got: {err}");
    }

    #[tokio::test]
    async fn a_hung_cli_is_killed_at_the_timeout() {
        let dir = scratch("hang");
        let bin = fake_cli(&dir, "codex", "sleep 30");
        let p = CliProvider::codex(bin)
            .with_workdir(scratch("hang-wd"))
            .with_timeout(Duration::from_millis(200));
        let err = p.run(&job("x")).await.unwrap_err();
        assert!(err.contains("timed out"), "got: {err}");
    }

    #[tokio::test]
    async fn a_prompt_larger_than_the_pipe_buffer_does_not_deadlock() {
        let dir = scratch("big-prompt");
        // the fake streams output BEFORE draining stdin — the deadlock shape a
        // sequential write-then-wait would hit with a >64KiB prompt.
        let bin = fake_cli(
            &dir,
            "codex",
            r#"printf '{"type":"item.completed","item":{"type":"agent_message","text":"ok"}}\n'
cat > /dev/null"#,
        );
        let p = CliProvider::codex(bin)
            .with_workdir(scratch("big-prompt-wd"))
            .with_timeout(Duration::from_secs(10));
        let big = "x".repeat(256 * 1024);
        assert_eq!(p.run(&job(&big)).await.unwrap(), "ok");
    }

    #[test]
    fn spec_timeout_seeds_the_provider_and_global_override_wins() {
        let spec = builtin_specs().into_iter().find(|s| s.tag == CODEX).unwrap();
        let p = CliProvider::from_spec(spec.clone(), PathBuf::from("/x"));
        assert_eq!(p.timeout, Duration::from_secs(spec.timeout_secs));

        let dir = scratch("global-timeout");
        fake_cli(&dir, "codex", "exit 0");
        let set = discover_with(
            builtins(),
            Some(dir.into_os_string()),
            &no_env,
            Some(Duration::from_secs(7)),
        );
        assert_eq!(set.capabilities(), vec![CODEX], "override plumbed without error");
    }
}
