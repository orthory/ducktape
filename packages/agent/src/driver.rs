//! claude-code subprocess driver.
//!
//! shells out to the `claude` cli in `-p` (print) mode with json output and
//! parses the result envelope. the binary path is a *config field* (not looked
//! up on PATH) so tests can point it at a mock script — see the test at the
//! bottom of this file.
//!
//! the cli we drive looks roughly like:
//!
//! ```sh
//! claude -p "<prompt>" --output-format json \
//!     --permission-mode acceptEdits \
//!     [--model ..] [--append-system-prompt ..] [--allowed-tools ..]
//! ```
//!
//! and emits a single json object on stdout: `{ result, session_id,
//! total_cost_usd, is_error, .. }`. extra fields are ignored.

use std::path::PathBuf;

use serde::Deserialize;
use tokio::process::Command;

/// configuration for a `claude` invocation.
///
/// `claude_bin` is deliberately a field rather than a PATH lookup: tests inject
/// a mock script here, and a real deployment can pin an absolute path.
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    /// path to the `claude` binary. defaults to the bare name `"claude"`.
    pub claude_bin: PathBuf,
    /// `--model` override, if any.
    pub model: Option<String>,
    /// `--permission-mode`. defaults to `"acceptEdits"`.
    pub permission_mode: String,
    /// `--append-system-prompt`, if any.
    pub append_system_prompt: Option<String>,
    /// `--allowed-tools`, if any.
    pub allowed_tools: Option<String>,
    /// working directory the subprocess runs in.
    pub cwd: PathBuf,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            claude_bin: PathBuf::from("claude"),
            model: None,
            permission_mode: "acceptEdits".to_string(),
            append_system_prompt: None,
            allowed_tools: None,
            cwd: PathBuf::from("."),
        }
    }
}

/// the parsed result of one `claude` run.
///
/// deserialized directly from the cli's json envelope; unknown fields (type,
/// subtype, duration_ms, ..) are ignored. only `result` is required.
#[derive(Debug, Clone, Deserialize)]
pub struct TaskRun {
    /// session id, for resuming with [`resume`].
    #[serde(default)]
    pub session_id: Option<String>,
    /// the agent's final textual result.
    pub result: String,
    /// total cost of the run in usd, if the cli reported it.
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    /// whether the run ended in an error state.
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn claude: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("claude exited with status {0}: {1}")]
    NonZeroExit(i32, String),
    #[error("failed to parse claude json envelope: {0}")]
    Parse(#[from] serde_json::Error),
}

/// build the configured `claude` command. when `session` is `Some`, adds
/// `--resume {session}`. the prompt is passed as a single argv entry, so no
/// shell interpolation happens.
fn build_command(cfg: &ClaudeConfig, prompt: &str, session: Option<&str>) -> Command {
    let mut cmd = Command::new(&cfg.claude_bin);
    cmd.current_dir(&cfg.cwd)
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("json")
        .arg("--permission-mode")
        .arg(&cfg.permission_mode);

    if let Some(model) = &cfg.model {
        cmd.arg("--model").arg(model);
    }
    if let Some(asp) = &cfg.append_system_prompt {
        cmd.arg("--append-system-prompt").arg(asp);
    }
    if let Some(tools) = &cfg.allowed_tools {
        cmd.arg("--allowed-tools").arg(tools);
    }
    if let Some(session) = session {
        cmd.arg("--resume").arg(session);
    }
    cmd
}

/// spawn `claude`, capture stdout, and parse the json envelope.
async fn invoke(cfg: &ClaudeConfig, prompt: &str, session: Option<&str>) -> Result<TaskRun, Error> {
    let output = build_command(cfg, prompt, session).output().await?;
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(Error::NonZeroExit(code, stderr));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

/// run a fresh `claude` task with the given prompt.
pub async fn run(cfg: &ClaudeConfig, prompt: &str) -> Result<TaskRun, Error> {
    invoke(cfg, prompt, None).await
}

/// resume an existing session by id with a follow-up prompt.
pub async fn resume(cfg: &ClaudeConfig, session_id: &str, prompt: &str) -> Result<TaskRun, Error> {
    invoke(cfg, prompt, Some(session_id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// writes a mock `claude` script that echoes a fixed json envelope, chmods
    /// it +x, and returns (script_path, cwd). the cwd is a unique tempdir so
    /// parallel test runs don't collide.
    fn mock_claude(envelope: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "agent-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        let script = dir.join("claude");
        std::fs::write(&script, format!("#!/bin/sh\ncat <<'EOF'\n{envelope}\nEOF\n"))
            .expect("write mock");
        let mut perms = std::fs::metadata(&script).expect("stat mock").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod mock");
        (script, dir)
    }

    #[tokio::test]
    async fn run_parses_envelope() {
        let (script, dir) = mock_claude(
            r#"{"result":"done","session_id":"abc","total_cost_usd":0.01,"is_error":false}"#,
        );
        let cfg = ClaudeConfig {
            claude_bin: script,
            cwd: dir,
            ..Default::default()
        };

        let run = run(&cfg, "do the thing").await.expect("run succeeds");
        assert_eq!(run.result, "done");
        assert_eq!(run.session_id.as_deref(), Some("abc"));
        assert_eq!(run.total_cost_usd, Some(0.01));
        assert!(!run.is_error);
    }

    #[tokio::test]
    async fn resume_passes_session() {
        // the mock ignores args, so this just verifies resume's happy path
        // parses the same envelope shape.
        let (script, dir) = mock_claude(r#"{"result":"resumed","is_error":false}"#);
        let cfg = ClaudeConfig {
            claude_bin: script,
            cwd: dir,
            ..Default::default()
        };

        let run = resume(&cfg, "abc", "keep going").await.expect("resume succeeds");
        assert_eq!(run.result, "resumed");
        assert_eq!(run.session_id, None);
        assert_eq!(run.total_cost_usd, None);
    }
}
