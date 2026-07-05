//! host-side capability providers — the I/O half of the capability seam.
//!
//! the capability module (consensus) replicates *who provides what*; this
//! crate is the machine-local counterpart that actually provides it. a
//! [`Provider`] wraps one locally installed executor CLI, and [`discover`]
//! probes the host for the executors the operator brought — BYO by
//! construction: the node spawns `codex` / `claude` as child processes and
//! never reads, writes, or refreshes any credential file. auth, token
//! rotation, and endpoint choice are entirely the CLI's own business.
//!
//! the CLIs are agentic, not plain inference endpoints, so a provider runs
//! them fenced: non-interactive one-shot mode, most-restricted sandbox flags,
//! and an empty scratch working directory — the child never sees the node's
//! data directory. the fence is what turns "orchestrate a coding agent" into
//! "use it as a text oracle".

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::AsyncWriteExt as _;

/// the capability tag served by a locally installed codex CLI.
pub const CODEX: &str = "codex";
/// the capability tag served by a locally installed claude code CLI.
pub const CLAUDE: &str = "claude";

/// how long a provider may run one job before the child is killed. generous:
/// a saga effect is already async and retried, but a hung CLI must never
/// wedge the worker lane forever.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// one unit of provider work: the fully rendered prompt and the model to run
/// it on. rendering (conversation -> text) is the CALLER's business — this
/// crate is deliberately ignorant of chat shapes and saga specs.
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

/// the host's discovered provider set: what this node can actually serve, and
/// therefore exactly what it should announce to the capability module.
pub struct ProviderSet {
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderSet {
    pub fn new(providers: Vec<Box<dyn Provider>>) -> Self {
        Self { providers }
    }

    pub fn find(&self, capability: &str) -> Option<&dyn Provider> {
        self.providers
            .iter()
            .find(|p| p.capability() == capability)
            .map(Box::as_ref)
    }

    /// the sorted tag list — the truthful payload for a capability
    /// announcement.
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
}

/// which CLI dialect a [`CliProvider`] speaks: argv shape and output parsing
/// differ per executor, everything else (spawn, fence, timeout) is shared.
enum Flavor {
    Codex,
    Claude,
}

/// a [`Provider`] backed by one locally installed CLI binary.
pub struct CliProvider {
    bin: PathBuf,
    flavor: Flavor,
    /// the child's working directory — an empty scratch dir, never the node's
    /// data directory, so an agentic CLI has nothing to wander into.
    workdir: PathBuf,
    timeout: Duration,
}

impl CliProvider {
    pub fn codex(bin: PathBuf) -> Self {
        Self::new(bin, Flavor::Codex, CODEX)
    }

    pub fn claude(bin: PathBuf) -> Self {
        Self::new(bin, Flavor::Claude, CLAUDE)
    }

    fn new(bin: PathBuf, flavor: Flavor, tag: &str) -> Self {
        let workdir = std::env::temp_dir().join(format!(
            "ducktape-provider-{tag}-{}",
            std::process::id()
        ));
        Self {
            bin,
            flavor,
            workdir,
            timeout: DEFAULT_TIMEOUT,
        }
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
        match self.flavor {
            // one-shot non-interactive exec, most-restricted sandbox, prompt
            // on stdin (`-`), machine-readable event stream on stdout.
            Flavor::Codex => {
                cmd.args([
                    "exec",
                    "--json",
                    "--sandbox",
                    "read-only",
                    "--skip-git-repo-check",
                    "--model",
                    &job.model_ref,
                    "-",
                ]);
            }
            // print mode with a single json result object; one turn so the
            // agent answers instead of embarking on tool-use loops.
            Flavor::Claude => {
                cmd.args([
                    "-p",
                    "--output-format",
                    "json",
                    "--max-turns",
                    "1",
                    "--model",
                    &job.model_ref,
                ]);
            }
        }
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
        match self.flavor {
            Flavor::Codex => CODEX,
            Flavor::Claude => CLAUDE,
        }
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
        // the CLI streams output before draining stdin.
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
            return Err(format!("writing the prompt to {} failed: {e}", self.bin.display()));
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        match self.flavor {
            Flavor::Codex => parse_codex_output(&stdout),
            Flavor::Claude => parse_claude_output(&stdout),
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
            return Err(format!("claude reported an error result: {}", excerpt(candidate)));
        }
        if let Some(text) = v.get("result").and_then(Value::as_str) {
            return Ok(text.to_string());
        }
    }
    Err(format!("claude output carried no result: {}", excerpt(stdout)))
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

/// probe this host for installed executor CLIs and build the provider set.
/// `DUCKTAPE_CODEX_BIN` / `DUCKTAPE_CLAUDE_BIN` override the `PATH` probe with
/// an explicit binary; `DUCKTAPE_PROVIDER_TIMEOUT_SECS` tunes the per-job
/// timeout. what discovery finds is exactly what the node announces — nothing
/// configured is silently invented, nothing installed is silently dropped.
pub fn discover() -> ProviderSet {
    discover_with(
        std::env::var_os("PATH"),
        std::env::var_os("DUCKTAPE_CODEX_BIN"),
        std::env::var_os("DUCKTAPE_CLAUDE_BIN"),
        std::env::var("DUCKTAPE_PROVIDER_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs),
    )
}

/// the parameterized core of [`discover`], env-free so tests can drive it
/// without mutating process state.
fn discover_with(
    path: Option<OsString>,
    codex_override: Option<OsString>,
    claude_override: Option<OsString>,
    timeout: Option<Duration>,
) -> ProviderSet {
    let tune = |p: CliProvider| match timeout {
        Some(t) => p.with_timeout(t),
        None => p,
    };
    let mut providers: Vec<Box<dyn Provider>> = Vec::new();
    if let Some(bin) = resolve_bin(CODEX, codex_override, path.as_deref()) {
        providers.push(Box::new(tune(CliProvider::codex(bin))));
    }
    if let Some(bin) = resolve_bin(CLAUDE, claude_override, path.as_deref()) {
        providers.push(Box::new(tune(CliProvider::claude(bin))));
    }
    ProviderSet::new(providers)
}

/// resolve one executor binary: an explicit override wins (and a BROKEN
/// override is a loud warning + absent capability, never a silent fallback to
/// PATH — the operator said "use this", and this doesn't exist), else the
/// first executable `name` on `path`.
fn resolve_bin(
    name: &str,
    explicit: Option<OsString>,
    path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    if let Some(explicit) = explicit {
        let p = PathBuf::from(&explicit);
        if is_executable(&p) {
            return Some(p);
        }
        eprintln!(
            "[capability-host] override for '{name}' ({}) is not an executable file; \
             the capability will NOT be announced",
            p.display()
        );
        return None;
    }
    let path = path?;
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
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

    #[test]
    fn discovery_finds_executables_on_path() {
        let dir = scratch("discovery-path");
        fake_cli(&dir, "codex", "exit 0");
        let set = discover_with(Some(dir.clone().into_os_string()), None, None, None);
        assert_eq!(set.capabilities(), vec![CODEX], "codex found, claude absent");
        assert!(set.find(CODEX).is_some());
        assert!(set.find(CLAUDE).is_none());
    }

    #[test]
    fn discovery_finds_both_and_sorts_tags() {
        let dir = scratch("discovery-both");
        fake_cli(&dir, "codex", "exit 0");
        fake_cli(&dir, "claude", "exit 0");
        let set = discover_with(Some(dir.into_os_string()), None, None, None);
        assert_eq!(set.capabilities(), vec![CLAUDE, CODEX], "sorted tag list");
    }

    #[test]
    fn explicit_override_wins_and_a_broken_override_announces_nothing() {
        let dir = scratch("discovery-override");
        let real = fake_cli(&dir, "my-codex", "exit 0");
        // override beats an empty PATH ...
        let set = discover_with(None, Some(real.into_os_string()), None, None);
        assert_eq!(set.capabilities(), vec![CODEX]);
        // ... and a dangling override is absent, not a silent PATH fallback.
        let missing = dir.join("nope");
        let set = discover_with(
            Some(dir.clone().into_os_string()),
            Some(missing.into_os_string()),
            None,
            None,
        );
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
        let set = discover_with(Some(dir.into_os_string()), None, None, None);
        assert!(set.find(CODEX).is_none(), "mode 644 is not executable");
    }

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
}
