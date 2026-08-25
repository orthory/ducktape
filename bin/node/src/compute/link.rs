//! the daemon's live ws attachment to its node: work-intake hints in, run
//! output out, on ONE connection.
//!
//! Two seams share it, because both are "this daemon and its node, continuously":
//!
//! - **hints.** The node sends a `heartbeat` frame on every block wake AND on a
//!   timer, so an unsubscribed connection is already the changed-hint feed —
//!   no topic, no cursor, no catch-up. Each hint wakes one intake pass; the
//!   daemon's own slower timer is the backstop that makes a dropped socket a
//!   DELAY rather than a stall.
//! - **run output.** The in-process `OutputSink` that used to feed the node's
//!   run-output ring cannot reach it across a process boundary; a `run_output`
//!   client frame is that sink, on the connection already open.
//!
//! A dropped socket is ordinary (the node restarts, the operator upgrades): the
//! task reconnects forever, logging attempt 1 and every Nth with an `attempts`
//! field. An unconditional warn here would be a log bomb on a node that stays
//! down — and the counter IS the diagnosis.

use futures::{SinkExt as _, StreamExt as _};
use tokio::sync::{Notify, mpsc};

/// one line of a run's live output, on its way to the node's ring.
pub(crate) struct OutputLine {
    pub(crate) run_key: String,
    pub(crate) stderr: bool,
    pub(crate) line: String,
}

/// how many reconnect failures pass between log lines after the first.
const LOG_EVERY: u64 = 30;
/// how long to wait before redialing a node that is not answering.
const REDIAL: std::time::Duration = std::time::Duration::from_secs(2);
/// the output lane's depth. Lines are display-only, so a burst that outruns the
/// socket is dropped at the sink rather than back-pressuring a provider's
/// stdout — a chatty run must never be able to stall its own execution.
pub(crate) const OUTPUT_LANE: usize = 1024;

/// Run the daemon's ws attachment until the process stops.
///
/// `hint` is notified once per node heartbeat; `lines` carries run output the
/// other way. Both are best-effort by design: the chain is the source of truth
/// for work, and the output ring is a display buffer.
pub(crate) async fn attach(
    ws_url: String,
    hint: std::sync::Arc<Notify>,
    mut lines: mpsc::Receiver<OutputLine>,
) {
    let mut failures: u64 = 0;
    loop {
        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((socket, _)) => {
                if failures > 0 {
                    tracing::info!(
                        target: "ducktape::service",
                        "compute daemon reattached to its node"
                    );
                }
                failures = 0;
                pump(socket, &hint, &mut lines).await;
            }
            Err(error) => {
                failures += 1;
                if failures == 1 || failures.is_multiple_of(LOG_EVERY) {
                    tracing::warn!(
                        target: "ducktape::service",
                        attempts = failures,
                        reason = "ws_attach_failed",
                        "compute daemon cannot reach its node's stream surface: {error}"
                    );
                }
                tokio::time::sleep(REDIAL).await;
            }
        }
    }
}

/// One connection's lifetime: hints out of it, output lines into it. Returns
/// when the socket closes, so the caller redials.
async fn pump<S>(socket: S, hint: &Notify, lines: &mut mpsc::Receiver<OutputLine>)
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
    loop {
        tokio::select! {
            frame = rx.next() => {
                let Some(Ok(Message::Text(text))) = frame else {
                    // a close, an error, or a frame shape we do not read:
                    // anything but a live text frame ends this connection.
                    let closed = !matches!(frame, Some(Ok(_)));
                    if closed { return; }
                    continue;
                };
                // the ONLY frame this daemon reads. Everything else on the hub
                // is for subscribers, and this connection subscribes to nothing.
                let is_hint = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|value| value["type"].as_str().map(|kind| kind == "heartbeat"))
                    .unwrap_or(false);
                if is_hint {
                    hint.notify_one();
                }
            }
            line = lines.recv() => {
                let Some(line) = line else { return };
                let stream = if line.stderr { "stderr" } else { "stdout" };
                let frame = serde_json::json!({
                    "op": "run_output",
                    "id": line.run_key,
                    "stream": stream,
                    "line": line.line,
                })
                .to_string();
                if tx.send(Message::Text(frame)).await.is_err() {
                    return;
                }
            }
        }
    }
}
