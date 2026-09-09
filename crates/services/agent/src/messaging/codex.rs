//! the Codex adapters: an existing session through the installed CLI, and a
//! service-managed session through the local App Server.
//!
//! ## `codex queue` — an existing session
//!
//! `codex queue --thread <id> --message <text>` places a message on a live
//! session's queue. Invoked as an ARGUMENT ARRAY, never as interpolated shell
//! text: a message body is arbitrary peer-supplied UTF-8, and the one thing it
//! must never become is a word in a command line.
//!
//! Its acceptance semantics, measured against the installed build rather than
//! assumed: a thread it cannot read exits NON-ZERO and names the failure on
//! stderr (`failed to queue session message: … no rollout found for thread id
//! …`). So exit 0 is the CLI's own statement that the thread store took the
//! message — which is [`crate::wire::State::AdapterAccepted`] exactly as the
//! spec defines it: the input interface accepted it, NOT that a model read it,
//! understood it, or acted on it. Nothing here upgrades that to a claim about
//! the model, and nothing reads the process's OUTPUT to decide: the exit status
//! is the signal, because a pipeline's exit status is the last command's and a
//! grep for "error" is a guess.
//!
//! ## App Server — a service-managed session
//!
//! Local JSON-RPC over the child's stdio: `initialize`, then `thread/resume`,
//! then `turn/start` when the thread is idle. When a turn IS running, ordinary
//! input waits for the next safe boundary; only urgent input steers, through
//! `turn/steer` with its `expectedTurnId` precondition. The server rejects a
//! steer whose expected turn is not the active one, and the answer to that is
//! to LOOK AGAIN — never to steer whatever turn happens to be active now,
//! which is how an instruction about one piece of work lands in the middle of
//! another.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

use super::{Offer, Outcome};
use crate::wire::Capabilities;

// ---- the existing-session adapter ---------------------------------------

/// place messages on an existing local Codex session's queue.
pub struct CodexQueue {
    thread_id: String,
    /// the executable to run. Named so a test can point it at a stub without
    /// this adapter growing a "test mode" branch.
    program: PathBuf,
}

impl CodexQueue {
    /// `program` is the Codex executable to run — resolved by the daemon
    /// rather than hard-coded, so an operator can name an installed path and a
    /// test can drive a real child process it owns. There is no "test mode"
    /// branch inside this adapter; it runs whatever it was given.
    pub fn new(thread_id: String, program: PathBuf) -> Self {
        Self { thread_id, program }
    }

    /// what queueing into an existing session can do.
    ///
    /// `steers_active_turn` is false: the CLI queues, and queueing is not
    /// steering. `reports_acceptance` is true because a non-zero exit is a
    /// real refusal from the provider's own input interface.
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            accepts_while_busy: true,
            wakes_idle: true,
            reports_acceptance: true,
            steers_active_turn: false,
        }
    }

    /// the argv this adapter runs. One element per argument, so nothing in a
    /// message body is ever parsed as shell syntax — it is one `argv` slot,
    /// whatever it contains.
    fn argv(&self, text: &str) -> Vec<String> {
        vec![
            "queue".to_string(),
            "--thread".to_string(),
            self.thread_id.clone(),
            "--message".to_string(),
            text.to_string(),
        ]
    }

    pub async fn offer(&self, offer: &Offer<'_>) -> Outcome {
        let output = tokio::process::Command::new(&self.program)
            .args(self.argv(offer.text))
            // the child inherits nothing it does not need, and writes nowhere
            // we then parse for a verdict.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await;
        let Ok(output) = output else {
            // the CLI is not installed or could not start: nothing was offered,
            // so the message stays queued rather than settling on a guess.
            return Outcome::Deferred {
                reason: "queue_cli_unavailable".to_string(),
            };
        };
        classify_exit(output.status.success(), &output.stderr)
    }
}

/// map the CLI's exit onto an outcome.
///
/// Pure, and split out because THE mistake this adapter can make is deciding
/// from anything other than the exit status. `stderr` is read only to pick a
/// stable token for the log — never to overturn the status.
fn classify_exit(success: bool, stderr: &[u8]) -> Outcome {
    if success {
        return Outcome::Accepted {
            reason: Some("queued_by_cli".to_string()),
        };
    }
    let text = String::from_utf8_lossy(stderr);
    let names_a_missing_thread =
        text.contains("no rollout found") || text.contains("failed to read thread");
    let reason = if names_a_missing_thread {
        "unknown_thread"
    } else {
        "queue_refused"
    };
    Outcome::Refused {
        reason: reason.to_string(),
    }
}

// ---- the managed-session adapter ----------------------------------------

/// what to do with one offer, given what the thread is doing right now.
///
/// A decide-fn: it returns a directive and performs nothing, so every rule
/// below is testable without a running App Server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// the thread is idle — begin a turn with this input.
    Start,
    /// a turn is running and this input may steer it, under the precondition
    /// that the turn is still that one.
    Steer { expected_turn: String },
    /// do not offer it now. The message stays queued, visibly.
    Defer { reason: &'static str },
}

/// what the thread is doing, as the App Server last reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadState {
    Idle,
    Running {
        turn_id: String,
    },
    /// the thread is not loaded, so there is nothing to start or steer.
    NotLoaded,
}

/// the whole steering rule, in one place.
///
/// Ordinary input NEVER interrupts a running turn: it waits for the next safe
/// boundary, which is what "normal delivery is queued until the provider's
/// supported safe boundary" means. Urgent input may steer, and only then.
pub fn plan(state: &ThreadState, urgent: bool) -> Plan {
    match state {
        ThreadState::NotLoaded => Plan::Defer {
            reason: "thread_not_loaded",
        },
        ThreadState::Idle => Plan::Start,
        ThreadState::Running { turn_id } => {
            if !urgent {
                return Plan::Defer {
                    reason: "turn_busy",
                };
            }
            Plan::Steer {
                expected_turn: turn_id.clone(),
            }
        }
    }
}

/// a local App Server child, and the thread this daemon manages on it.
pub struct CodexAppServer {
    thread_id: String,
    rpc: Arc<Rpc>,
}

impl CodexAppServer {
    /// bind this daemon's managed thread on an already-started App Server.
    pub fn new(thread_id: String, rpc: Arc<Rpc>) -> Self {
        Self { thread_id, rpc }
    }

    /// what a managed session can do. The only binding that steers.
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            accepts_while_busy: true,
            wakes_idle: true,
            reports_acceptance: true,
            steers_active_turn: true,
        }
    }

    pub fn thread_id(&self) -> &str {
        &self.thread_id
    }

    /// offer one message, at the boundary the thread's state allows.
    ///
    /// A steer whose expected turn is no longer active is the race this is
    /// built around: the server refuses it, and the answer is to RE-READ the
    /// state and decide again — at most once, so a thread churning through
    /// turns cannot spin here. What it must never do is drop the precondition
    /// and steer whatever is running now.
    pub async fn offer(&self, offer: &Offer<'_>) -> Outcome {
        match self.attempt(offer).await {
            Attempt::Settled(outcome) => outcome,
            Attempt::TurnMoved => match self.attempt(offer).await {
                Attempt::Settled(outcome) => outcome,
                // still moving. Leave it queued rather than steer blind.
                Attempt::TurnMoved => Outcome::Deferred {
                    reason: "turn_moved".to_string(),
                },
            },
        }
    }

    /// one pass: read the state, plan, perform.
    async fn attempt(&self, offer: &Offer<'_>) -> Attempt {
        let state = match self.rpc.thread_state(&self.thread_id).await {
            Ok(state) => state,
            Err(reason) => return Attempt::Settled(Outcome::Deferred { reason }),
        };
        match plan(&state, offer.urgent) {
            Plan::Defer { reason } => Attempt::Settled(Outcome::Deferred {
                reason: reason.to_string(),
            }),
            Plan::Start => Attempt::Settled(self.rpc.turn_start(&self.thread_id, offer).await),
            Plan::Steer { expected_turn } => {
                self.rpc
                    .turn_steer(&self.thread_id, &expected_turn, offer)
                    .await
            }
        }
    }
}

// ThreadResumeResponse carries status and turns inside `thread`, not a
// top-level turn. Only a confirmed idle status permits a new turn.
fn resumed_thread_state(resumed: &serde_json::Value) -> Result<ThreadState, String> {
    let thread = &resumed["thread"];
    match thread["status"]["type"].as_str() {
        Some("idle") => Ok(ThreadState::Idle),
        Some("notLoaded") => Ok(ThreadState::NotLoaded),
        Some("active") => resumed_active_turn(thread),
        _ => Err("thread_status_unavailable".to_string()),
    }
}

fn resumed_active_turn(thread: &serde_json::Value) -> Result<ThreadState, String> {
    let turns = thread["turns"]
        .as_array()
        .ok_or("active_turn_unavailable")?;
    let mut active = turns.iter().filter(|turn| turn["status"] == "inProgress");
    let turn = active.next().ok_or("active_turn_unavailable")?;
    let turn_id = turn["id"].as_str().ok_or("active_turn_unavailable")?;
    let ambiguous = active.next().is_some() || turn_id.is_empty();
    if ambiguous {
        return Err("active_turn_unavailable".to_string());
    }
    Ok(ThreadState::Running {
        turn_id: turn_id.to_string(),
    })
}

/// the result of one pass at offering.
enum Attempt {
    Settled(Outcome),
    /// the expected turn was not the active one. Look again.
    TurnMoved,
}

/// a JSON-RPC peer over a child process's stdio.
pub struct Rpc {
    next_id: AtomicI64,
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
    pending: Arc<
        std::sync::Mutex<
            std::collections::HashMap<i64, tokio::sync::oneshot::Sender<serde_json::Value>>,
        >,
    >,
    /// the last thread status each notification told us about.
    threads: Arc<std::sync::Mutex<std::collections::HashMap<String, ThreadState>>>,
    /// kept so the child is killed with this struct rather than outliving it.
    _child: tokio::sync::Mutex<tokio::process::Child>,
}

impl Rpc {
    /// spawn `codex app-server` and complete the handshake.
    pub async fn start(program: &std::path::Path) -> Result<Arc<Self>, String> {
        let mut child = tokio::process::Command::new(program)
            .arg("app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("spawn app-server: {error}"))?;
        let stdin = child.stdin.take().ok_or("app-server has no stdin")?;
        let stdout = child.stdout.take().ok_or("app-server has no stdout")?;
        let rpc = Arc::new(Self {
            next_id: AtomicI64::new(1),
            stdin: tokio::sync::Mutex::new(stdin),
            pending: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            threads: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            _child: tokio::sync::Mutex::new(child),
        });
        let pending = rpc.pending.clone();
        let threads = rpc.threads.clone();
        tokio::spawn(async move { read_loop(stdout, pending, threads).await });
        rpc.request(
            "initialize",
            serde_json::json!({
                "clientInfo": { "name": "ducktape-agent", "version": env!("CARGO_PKG_VERSION") }
            }),
        )
        .await
        .map(|_| ())
        .map_err(|error| format!("initialize: {error}"))?;
        Ok(rpc)
    }

    /// resume the managed thread and read back what it is doing.
    pub async fn thread_state(&self, thread_id: &str) -> Result<ThreadState, String> {
        if let Some(state) = self
            .threads
            .lock()
            .expect("thread state lock poisoned")
            .get(thread_id)
        {
            return Ok(state.clone());
        }
        let resumed = self
            .request(
                "thread/resume",
                serde_json::json!({ "threadId": thread_id }),
            )
            .await
            .map_err(|_| "thread_resume_failed".to_string())?;
        resumed_thread_state(&resumed)
    }

    async fn turn_start(&self, thread_id: &str, offer: &Offer<'_>) -> Outcome {
        let started = self
            .request(
                "turn/start",
                serde_json::json!({
                    "threadId": thread_id,
                    "input": [{ "type": "text", "text": offer.text }],
                }),
            )
            .await;
        match started {
            Ok(_) => Outcome::Accepted {
                reason: Some("turn_started".to_string()),
            },
            Err(error) => Outcome::Deferred {
                reason: rpc_reason(&error, "turn_start_failed"),
            },
        }
    }

    async fn turn_steer(&self, thread_id: &str, expected: &str, offer: &Offer<'_>) -> Attempt {
        let steered = self
            .request(
                "turn/steer",
                serde_json::json!({
                    "threadId": thread_id,
                    "expectedTurnId": expected,
                    "input": [{ "type": "text", "text": offer.text }],
                }),
            )
            .await;
        match steered {
            Ok(_) => Attempt::Settled(Outcome::Accepted {
                reason: Some("turn_steered".to_string()),
            }),
            Err(error) if names_a_turn_mismatch(&error) => {
                // the turn we were told about is not the one running now. The
                // instruction was about THAT turn, so it does not get injected
                // into this one.
                self.threads
                    .lock()
                    .expect("thread state lock poisoned")
                    .remove(thread_id);
                Attempt::TurnMoved
            }
            Err(error) => Attempt::Settled(Outcome::Deferred {
                reason: rpc_reason(&error, "turn_steer_failed"),
            }),
        }
    }

    /// send one request and await its response.
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending
            .lock()
            .expect("rpc pending lock poisoned")
            .insert(id, tx);
        let frame = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params,
        });
        let line = format!("{frame}\n");
        let written = {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(line.as_bytes()).await.is_ok() && stdin.flush().await.is_ok()
        };
        if !written {
            self.pending
                .lock()
                .expect("rpc pending lock poisoned")
                .remove(&id);
            return Err(serde_json::json!({ "message": "app_server_write_failed" }));
        }
        let Ok(response) = rx.await else {
            return Err(serde_json::json!({ "message": "app_server_closed" }));
        };
        match response.get("error") {
            Some(error) => Err(error.clone()),
            None => Ok(response["result"].clone()),
        }
    }
}

/// whether an error names the steer precondition failing, rather than anything
/// else. Only this one may be retried by looking again.
fn names_a_turn_mismatch(error: &serde_json::Value) -> bool {
    let message = error["message"].as_str().unwrap_or_default().to_lowercase();
    message.contains("expectedturnid")
        || message.contains("expected turn")
        || (message.contains("turn") && message.contains("not") && message.contains("active"))
}

/// a stable snake_case token for an rpc failure. The server's own message is
/// prose from another program and never becomes our `reason`.
fn rpc_reason(error: &serde_json::Value, fallback: &str) -> String {
    let message = error["message"].as_str().unwrap_or_default();
    match message {
        "app_server_closed" => "app_server_closed".to_string(),
        "app_server_write_failed" => "app_server_write_failed".to_string(),
        _ => fallback.to_string(),
    }
}

/// read responses and notifications off the child's stdout forever.
async fn read_loop(
    stdout: tokio::process::ChildStdout,
    pending: Arc<
        std::sync::Mutex<
            std::collections::HashMap<i64, tokio::sync::oneshot::Sender<serde_json::Value>>,
        >,
    >,
    threads: Arc<std::sync::Mutex<std::collections::HashMap<String, ThreadState>>>,
) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(frame) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(id) = frame["id"].as_i64() {
            let waiter = pending
                .lock()
                .expect("rpc pending lock poisoned")
                .remove(&id);
            if let Some(waiter) = waiter {
                let _ = waiter.send(frame);
            }
            continue;
        }
        if let Some((thread_id, state)) = thread_state_of(&frame) {
            threads
                .lock()
                .expect("thread state lock poisoned")
                .insert(thread_id, state);
        }
    }
}

/// read a thread's state out of one server notification.
///
/// Only the lifecycle notifications are read. Tool output, deltas and plan
/// updates are declared results of a turn, not statements about whether input
/// may be offered — collecting them here is how an adapter starts treating
/// arbitrary model output as a state transition.
fn thread_state_of(frame: &serde_json::Value) -> Option<(String, ThreadState)> {
    let method = frame["method"].as_str()?;
    let params = &frame["params"];
    let thread_id = params["threadId"].as_str()?.to_string();
    match method {
        "turn/started" => {
            let turn_id = params["turn"]["id"].as_str()?.to_string();
            Some((thread_id, ThreadState::Running { turn_id }))
        }
        "turn/completed" => Some((thread_id, ThreadState::Idle)),
        "thread/closed" => Some((thread_id, ThreadState::NotLoaded)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::MessageId;

    fn an_offer(text: &str, urgent: bool) -> Offer<'_> {
        Offer {
            text,
            message_id: MessageId {
                generation: 2,
                sequence: 1,
            },
            urgent,
        }
    }

    /// A body is arbitrary peer-supplied text. It must reach the CLI as ONE
    /// argv element, so nothing in it can ever be a word in a command line.
    #[test]
    fn a_message_body_is_one_argument_not_shell_text() {
        let queue = CodexQueue::new("thread-abc".to_string(), PathBuf::from("codex"));
        let hostile = "hi; rm -rf / && echo $(whoami) `id` | tee /tmp/x #";
        let argv = queue.argv(hostile);
        assert_eq!(
            argv,
            vec![
                "queue".to_string(),
                "--thread".to_string(),
                "thread-abc".to_string(),
                "--message".to_string(),
                hostile.to_string(),
            ]
        );
        // the body occupies exactly one slot, verbatim — not split, not quoted,
        // not escaped, because it is never parsed.
        assert_eq!(argv.iter().filter(|arg| arg.contains("rm -rf")).count(), 1);
    }

    /// The measured contract. A thread the CLI cannot read exits non-zero, so
    /// a refusal is a real provider signal — and success is the CLI's own
    /// statement that its queue took the message.
    #[test]
    fn the_exit_status_decides_and_the_output_only_names_it() {
        let accepted = classify_exit(true, b"");
        let Outcome::Accepted { reason } = accepted else {
            panic!("exit 0 is the CLI's acceptance, got {accepted:?}");
        };
        assert_eq!(reason.as_deref(), Some("queued_by_cli"));

        let missing = classify_exit(
            false,
            b"Error: failed to queue session message: thread/queue/add failed: \
              failed to read thread: no rollout found for thread id abc (code -32603)",
        );
        let Outcome::Refused { reason } = missing else {
            panic!("a non-zero exit is a refusal, got {missing:?}");
        };
        assert_eq!(reason, "unknown_thread");

        assert!(matches!(
            classify_exit(false, b"something else entirely"),
            Outcome::Refused { .. }
        ));
    }

    /// The trap this test exists for: a process that PRINTS an error while
    /// exiting 0 must not be read as a failure, and — the direction that
    /// actually bites — a process that prints nothing while exiting non-zero
    /// must not be read as a success. The status is the signal.
    #[test]
    fn output_never_overturns_the_exit_status() {
        assert!(matches!(
            classify_exit(true, b"Error: something scary on stderr"),
            Outcome::Accepted { .. }
        ));
        assert!(matches!(classify_exit(false, b""), Outcome::Refused { .. }));
    }

    #[test]
    fn an_idle_thread_starts_a_turn() {
        assert_eq!(plan(&ThreadState::Idle, false), Plan::Start);
        assert_eq!(plan(&ThreadState::Idle, true), Plan::Start);
    }

    /// Ordinary input waits for the next safe boundary. It does not interrupt
    /// a running tool, and it does not fall back to anything else — it stays
    /// visibly queued.
    #[test]
    fn ordinary_input_waits_out_a_busy_turn() {
        let busy = ThreadState::Running {
            turn_id: "turn-1".to_string(),
        };
        assert_eq!(
            plan(&busy, false),
            Plan::Defer {
                reason: "turn_busy"
            }
        );
    }

    /// Urgent input may steer — but only the turn it was told about.
    #[test]
    fn urgent_input_steers_the_turn_it_was_told_about() {
        let busy = ThreadState::Running {
            turn_id: "turn-1".to_string(),
        };
        assert_eq!(
            plan(&busy, true),
            Plan::Steer {
                expected_turn: "turn-1".to_string()
            }
        );
    }

    #[test]
    fn an_unloaded_thread_is_neither_started_nor_steered() {
        for urgent in [false, true] {
            assert_eq!(
                plan(&ThreadState::NotLoaded, urgent),
                Plan::Defer {
                    reason: "thread_not_loaded"
                }
            );
        }
    }

    /// The precondition failure the App Server reports, told apart from every
    /// other error — because only this one is retried by looking again, and
    /// treating an unrelated failure as a mismatch would re-drive the thread.
    #[test]
    fn only_the_expected_turn_precondition_is_a_mismatch() {
        for mismatch in [
            serde_json::json!({"message": "expectedTurnId does not match the active turn"}),
            serde_json::json!({"message": "Expected turn turn-1 is no longer active"}),
            serde_json::json!({"message": "turn turn-1 is not the active turn"}),
        ] {
            assert!(names_a_turn_mismatch(&mismatch), "{mismatch}");
        }
        for other in [
            serde_json::json!({"message": "thread not found"}),
            serde_json::json!({"message": "rate limited"}),
            serde_json::json!({"message": "app_server_closed"}),
        ] {
            assert!(!names_a_turn_mismatch(&other), "{other}");
        }
    }

    /// A server's prose never becomes our reason token.
    #[test]
    fn an_rpc_failure_is_named_by_a_stable_token() {
        assert_eq!(
            rpc_reason(
                &serde_json::json!({"message": "some sentence the server wrote"}),
                "turn_start_failed"
            ),
            "turn_start_failed"
        );
        assert_eq!(
            rpc_reason(&serde_json::json!({"message": "app_server_closed"}), "x"),
            "app_server_closed"
        );
    }

    /// Only lifecycle notifications move the state. Model output does not.
    #[test]
    fn lifecycle_notifications_move_the_state_and_output_does_not() {
        let started = serde_json::json!({
            "jsonrpc": "2.0", "method": "turn/started",
            "params": { "threadId": "t-1", "turn": { "id": "turn-9" } },
        });
        assert_eq!(
            thread_state_of(&started),
            Some((
                "t-1".to_string(),
                ThreadState::Running {
                    turn_id: "turn-9".to_string()
                }
            ))
        );

        let completed = serde_json::json!({
            "jsonrpc": "2.0", "method": "turn/completed",
            "params": { "threadId": "t-1", "turn": { "id": "turn-9" } },
        });
        assert_eq!(
            thread_state_of(&completed),
            Some(("t-1".to_string(), ThreadState::Idle))
        );

        for noise in [
            serde_json::json!({"method": "turn/plan/updated", "params": {"threadId": "t-1"}}),
            serde_json::json!({"method": "turn/diff/updated", "params": {"threadId": "t-1"}}),
            serde_json::json!({"method": "thread/tokenUsage/updated", "params": {"threadId": "t-1"}}),
            serde_json::json!({"id": 3, "result": {}}),
        ] {
            assert_eq!(
                thread_state_of(&noise),
                None,
                "model output is not a state transition: {noise}"
            );
        }
    }

    #[test]
    fn a_resumed_active_thread_uses_the_nested_in_progress_turn() {
        let response = serde_json::json!({"thread": {
            "status": {"type": "active", "activeFlags": []},
            "turns": [
                {"id": "old", "status": "completed"},
                {"id": "live", "status": "inProgress"}
            ]
        }});
        let state = resumed_thread_state(&response).unwrap();
        assert_eq!(
            state,
            ThreadState::Running {
                turn_id: "live".into()
            }
        );
        assert_eq!(
            plan(&state, false),
            Plan::Defer {
                reason: "turn_busy"
            }
        );
        assert_eq!(
            plan(&state, true),
            Plan::Steer {
                expected_turn: "live".into()
            }
        );
    }

    #[test]
    fn a_resumed_thread_needs_an_explicit_idle_status_to_start() {
        for thread in [
            serde_json::json!({"status": {"type": "active"}, "turns": []}),
            serde_json::json!({"status": {"type": "systemError"}}),
            serde_json::json!({}),
        ] {
            assert!(resumed_thread_state(&serde_json::json!({"thread": thread})).is_err());
        }
        assert_eq!(
            resumed_thread_state(&serde_json::json!({"thread": {
                "status": {"type": "idle"}, "turns": [{"id":"old", "status":"completed"}]
            }})),
            Ok(ThreadState::Idle)
        );
    }

    /// A CLI that is not installed leaves the message queued rather than
    /// settling it — nothing was offered, so nothing is known.
    #[tokio::test]
    async fn a_missing_cli_defers_rather_than_refusing() {
        let queue = CodexQueue {
            thread_id: "t".to_string(),
            program: PathBuf::from("/nonexistent/ducktape-not-a-codex"),
        };
        let outcome = queue.offer(&an_offer("hi", false)).await;
        let Outcome::Deferred { reason } = outcome else {
            panic!("a missing CLI must not settle the delivery, got {outcome:?}");
        };
        assert_eq!(reason, "queue_cli_unavailable");
    }
}
