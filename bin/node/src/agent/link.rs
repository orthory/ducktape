//! the daemon's live ws attachment to its node: pty commands in, session events
//! out, on ONE connection.
//!
//! Unlike compute's link this one is genuinely bidirectional, because an
//! interactive session is. Compute PULLS its work (it re-reads committed state
//! on a hint), which is right for placement-driven work that the chain already
//! records. A keystroke is not on the chain and never will be, so the node
//! PUSHES it — down the connection the daemon dialed, which is what keeps the
//! node from ever needing to dial a service.
//!
//! The daemon claims the link with one `service_attach` frame before anything
//! else. Until the node accepts it, this connection is an ordinary ws client
//! with no interactive plane behind it; if the node refuses (an unreadable or
//! stale link token, another daemon already attached), it says so and this
//! connection ends.
//!
//! A dropped socket is ordinary — the node restarts, the operator upgrades — so
//! the task reconnects forever, logging attempt 1 and every Nth with an
//! `attempts` field. An unconditional warn here would be a log bomb on a node
//! that stays down, and the counter IS the diagnosis. A REFUSAL IS COUNTED AND
//! PACED THE SAME WAY: the dial succeeded, so it is not a reconnect failure,
//! but it does not self-heal on a fresh socket either — an unpaced redial spins
//! on the node at full speed and writes the same line forever (118 282 of them
//! in one afternoon, from a second daemon nobody noticed was up).

use std::sync::Arc;

use agent_service::{Sessions, wire};
use futures::{SinkExt as _, StreamExt as _};
use tokio::sync::mpsc;

/// how many session events may queue before a pty pump waits. Deep enough that
/// a TUI redraw burst never stalls the pty; bounded so a wedged socket applies
/// back-pressure instead of growing without limit.
pub(crate) const EVENT_LANE: usize = 1024;
/// how many reconnect failures pass between log lines after the first.
const LOG_EVERY: u64 = 30;
/// whether this attempt earns a line: the first one, then every Nth. The
/// counter IS the diagnosis, so the line carries `attempts` and the ones
/// between it are silence, not loss.
fn worth_logging(attempts: u64) -> bool {
    attempts == 1 || attempts.is_multiple_of(LOG_EVERY)
}
/// how long to wait before redialing a node that is not answering.
const REDIAL: std::time::Duration = std::time::Duration::from_secs(2);

/// Run the daemon's ws attachment until the process stops.
pub(crate) async fn attach(
    ws_url: String,
    workspace: std::path::PathBuf,
    sessions: Arc<Sessions>,
    mut events: mpsc::Receiver<wire::Event>,
) {
    // two causes, two counters: a node that will not answer, and a node that
    // answers and refuses. Each resets on the outcome that disproves it, so
    // neither hides behind the other's silence.
    let mut failures: u64 = 0;
    let mut refusals: u64 = 0;
    loop {
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((socket, _)) => {
                if failures > 0 {
                    tracing::info!(
                        target: "ducktape::service",
                        attempts = failures,
                        "agent daemon reattached to its node"
                    );
                }
                failures = 0;
                let end = pump(socket, &workspace, &sessions, &mut events).await;
                // the connection is gone, and with it every session: the node
                // forgot them the moment this link dropped, so a surviving pty
                // would be a container nobody can reach, feed or close.
                sessions.close_all().await;
                match end {
                    // a link that lived and dropped is the ordinary case, and
                    // it clears the refusal streak: whatever the node objected
                    // to, it does not object now.
                    LinkEnd::Closed => refusals = 0,
                    LinkEnd::Refused(detail) => {
                        refusals += 1;
                        if worth_logging(refusals) {
                            tracing::error!(
                                target: "ducktape::service",
                                attempts = refusals,
                                reason = "link_refused",
                                %detail,
                                "the node refused this agent daemon's link"
                            );
                        }
                        tokio::time::sleep(REDIAL).await;
                    }
                }
            }
            Err(error) => {
                failures += 1;
                if worth_logging(failures) {
                    tracing::warn!(
                        target: "ducktape::service",
                        attempts = failures,
                        reason = "ws_attach_failed",
                        "agent daemon cannot reach its node's stream surface: {error}"
                    );
                }
                tokio::time::sleep(REDIAL).await;
            }
        }
    }
}

/// how one connection ended. The caller redials either way, but only one of
/// these is ordinary — so `pump` reports which it was and the redial loop, which
/// owns the counters and the pace, decides what to say about it.
enum LinkEnd {
    /// the socket closed: the node restarted, the operator upgraded, the link
    /// simply dropped.
    Closed,
    /// the node refused this daemon's claim, carrying its reason.
    Refused(String),
}

/// One connection's lifetime: claim the link, then commands in and events out
/// until it closes. Returns so the caller redials.
async fn pump<S>(
    socket: S,
    workspace: &std::path::Path,
    sessions: &Arc<Sessions>,
    events: &mut mpsc::Receiver<wire::Event>,
) -> LinkEnd
where
    S: futures::Sink<
            tokio_tungstenite::tungstenite::Message,
            Error = tokio_tungstenite::tungstenite::Error,
        > + futures::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    use tokio_tungstenite::tungstenite::Message;
    let (mut tx, mut rx) = socket.split();
    // claim the link FIRST. The claim carries no build stamp: the node does not
    // gate on one, so sending it would only be a field nobody reads.
    //
    // re-read per attach, never latched: a node restart mints a fresh token, and
    // a daemon holding a stale one would be refused forever.
    let token = match noded::services::read_link_token(workspace) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!(
                target: "ducktape::service",
                reason = "link_token_unreadable",
                "the agent daemon cannot present its node's service-link token: {error}"
            );
            return LinkEnd::Closed;
        }
    };
    let claim = serde_json::json!({
        "op": "service_attach",
        "kind": noded::services::AGENT_KIND,
        "token": token,
    })
    .to_string();
    if tx.send(Message::Text(claim)).await.is_err() {
        return LinkEnd::Closed;
    }
    loop {
        tokio::select! {
            frame = rx.next() => {
                let Some(Ok(Message::Text(text))) = frame else {
                    // a close, an error, or a frame shape we do not read:
                    // anything but a live text frame ends this connection.
                    let closed = !matches!(frame, Some(Ok(_)));
                    if closed { return LinkEnd::Closed; }
                    continue;
                };
                match classify(&text) {
                    Incoming::Ignore => {}
                    Incoming::Command(command) => execute(sessions, command).await,
                    // the only errors this connection can earn are refusals of
                    // its claim, and none self-heals on this socket: a stale
                    // token needs a re-read of the node's freshly minted one,
                    // another daemon holding the link needs that daemon to go.
                    // Redialing is the honest retry — PACED, by the caller.
                    Incoming::Refused(detail) => return LinkEnd::Refused(detail),
                }
            }
            event = events.recv() => {
                let Some(event) = event else { return LinkEnd::Closed };
                let frame = serde_json::json!({ "op": "agent_event", "event": event }).to_string();
                if tx.send(Message::Text(frame)).await.is_err() {
                    return LinkEnd::Closed;
                }
            }
        }
    }
}

/// what a server frame means to this link. Exactly two things matter: a command
/// to perform, and the error that says our claim was refused. Everything else on
/// the hub is for subscribers, and this connection subscribes to nothing.
enum Incoming {
    Ignore,
    Command(wire::Command),
    Refused(String),
}

/// Read one frame. Pure — it decides, and `pump` performs.
fn classify(text: &str) -> Incoming {
    let Ok(frame) = serde_json::from_str::<serde_json::Value>(text) else {
        return Incoming::Ignore;
    };
    match frame["type"].as_str() {
        Some("service_command") => {
            match serde_json::from_value::<wire::Command>(frame["command"].clone()) {
                Ok(command) => Incoming::Command(command),
                // KNOWN GAP, and the one direction that does not refuse
                // cleanly. Daemon→node skew is a named refusal the sender sees:
                // an undecodable frame earns a `BadFrame` carrying `unknown
                // field ...` and the socket stays open. This direction only
                // drops. A `TermCreate` this daemon cannot decode is warned
                // about HERE, where nobody is waiting, while the node's
                // `TerminalSessions::start` awaits a reply that will never come
                // — and it awaits with no timeout on purpose (a cold image pull
                // takes minutes), so the operator's `agent pty` hangs.
                //
                // Left as-is deliberately: reaching it needs a node and a
                // daemon built from DIFFERENT trees, nothing here owes that
                // support, and what it replaced was worse — before
                // `deny_unknown_fields` the extra field was dropped and the
                // session RAN without the restriction the node named. Hanging
                // is a worse failure than a fast refusal and a better one than
                // a silently weakened session.
                //
                // The fix, when it is worth doing, is session-scoped: recover
                // the `session` id out of the undecodable frame and answer
                // `TermRefused` on it, so the create fails fast with a
                // nameable reason instead of waiting.
                Err(_) => {
                    tracing::warn!(
                        target: "ducktape::service",
                        reason = "malformed_command",
                        "agent daemon dropped a frame it could not decode"
                    );
                    Incoming::Ignore
                }
            }
        }
        Some("error") => {
            Incoming::Refused(frame["detail"].as_str().unwrap_or("unknown").to_string())
        }
        _ => Incoming::Ignore,
    }
}

/// Perform one command, on this task or its own.
///
/// The link must never stop reading, so nothing slow may run on it:
///
/// - **create** starts a container (a cold image pull is minutes) and **close**
///   tears one down, so both get their own task. Neither needs ordering against
///   anything: the node does not release a session id to anyone until the create
///   is answered, and a close is the escape hatch that must not queue behind a
///   blocked pty.
/// - **input and resize** only ENQUEUE onto the target session's own ordered
///   lane, which is a map lookup and a channel send. That is what keeps
///   keystrokes in arrival order without making the link the queue — the pty
///   write itself happens on the session's driver task.
async fn execute(sessions: &Arc<Sessions>, command: wire::Command) {
    let touches_a_container = matches!(
        command,
        wire::Command::TermCreate(_) | wire::Command::TermClose { .. }
    );
    if touches_a_container {
        let sessions = sessions.clone();
        tokio::spawn(async move { sessions.dispatch(command).await });
        return;
    }
    sessions.dispatch(command).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_service_command_frame_decodes_to_its_command() {
        let frame = serde_json::json!({
            "type": "service_command",
            "command": { "op": "term_close", "session": "abc" },
        })
        .to_string();
        let Incoming::Command(wire::Command::TermClose { session }) = classify(&frame) else {
            panic!("a service_command frame must decode to its command");
        };
        assert_eq!(session, "abc");
    }

    #[test]
    fn an_error_frame_ends_the_connection_with_its_detail() {
        // a refusal the node can actually emit: `take_service_link` names the
        // token and the single-holder rule, and nothing else. There is no build
        // mismatch to report — this node compares no stamp.
        let frame = serde_json::json!({
            "type": "error",
            "detail": "refused: present this node's service-link token, and only one agent service may attach",
        })
        .to_string();
        let Incoming::Refused(detail) = classify(&frame) else {
            panic!("an error frame is a refusal");
        };
        assert!(detail.contains("service-link token"), "{detail}");
    }

    #[test]
    fn every_other_frame_on_the_hub_is_ignored() {
        // the node heartbeats and publishes to subscribers on the same socket;
        // none of it is this link's business.
        for frame in [
            r#"{"type":"heartbeat","height":12}"#,
            r#"{"type":"event","topic":"chat","cursor":"1"}"#,
            "not json at all",
            r#"{"type":"service_command","command":{"op":"nonsense"}}"#,
        ] {
            assert!(
                matches!(classify(frame), Incoming::Ignore),
                "must ignore: {frame}"
            );
        }
    }

    #[test]
    fn a_repeating_attempt_earns_the_first_line_and_every_nth() {
        // the refusal that cost 118 282 identical ERROR lines took this path
        // unconditionally, at redial speed, with no `attempts` on it.
        assert!(worth_logging(1));
        for attempts in 2..LOG_EVERY {
            assert!(!worth_logging(attempts), "attempt {attempts} must be quiet");
        }
        assert!(worth_logging(LOG_EVERY));
        assert!(worth_logging(LOG_EVERY * 2));
    }
}
