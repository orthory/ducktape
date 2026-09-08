//! An agent run anchored to a chat message, shown live under its anchor
//! while it runs. The chain carries only the anchor and the committed reply;
//! the progress rides the node's `run-output:<dispatch>` topic, folded here
//! into a bounded row of status, activity titles and an answer preview. The
//! row lives exactly as long as the run is pending in `runs`: the entry
//! prunes in the block that posts the reply, so the committed message takes
//! the row's place.

use super::*;
use iced::futures::SinkExt as _;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::tungstenite::Message;

/// Activity titles kept per row; older ones fall off the front.
pub(crate) const MAX_LIVE_ACTIVITY: usize = 12;
/// The answer preview a row carries across the wire, in bytes.
pub(crate) const MAX_LIVE_PREVIEW_BYTES: usize = 2048;
const ACTIVITY_DETAIL_CHARS: usize = 60;
const PENDING_POLL: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize)]
pub struct LiveActivity {
    pub label: String,
    pub done: bool,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize)]
pub struct LiveAgentRow {
    pub anchor_seq: i64,
    /// The anchor's thread root when the anchor is itself a reply, else 0.
    pub thread_root: i64,
    pub run_id: String,
    pub dispatch: String,
    pub agent: String,
    pub status: String,
    pub activity: Vec<LiveActivity>,
    pub answer_preview: String,
    pub elapsed_ms: i64,
}

#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct LiveAgentNotice {
    pub rows: Vec<LiveAgentRow>,
}

/// Fold one parsed output event into the row. Status lines replace the status;
/// activities upsert by label and mark done; previews and answers replace the
/// preview; errors become the status.
pub(crate) fn live_row_apply(mut row: LiveAgentRow, event: &AgentChatEvent) -> LiveAgentRow {
    match event.kind.as_str() {
        "status" => row.status = event.title.clone(),
        "activity" => {
            let label = if event.detail.is_empty() {
                event.title.clone()
            } else {
                format!(
                    "{}: {}",
                    event.title,
                    clip_text(&event.detail, ACTIVITY_DETAIL_CHARS)
                )
            };
            let done = event.status == "done";
            match row.activity.iter_mut().find(|act| act.label == label) {
                Some(act) => act.done |= done,
                None => row.activity.push(LiveActivity { label, done }),
            }
            let overflow = row.activity.len().saturating_sub(MAX_LIVE_ACTIVITY);
            row.activity.drain(..overflow);
            row.status = event.title.clone();
        }
        "preview" | "answer" => {
            row.answer_preview = clip_bytes(&event.answer, MAX_LIVE_PREVIEW_BYTES);
            row.status = if event.kind == "answer" {
                "Done".into()
            } else {
                "Answering".into()
            };
        }
        "error" => {
            row.status = if event.answer.is_empty() {
                event.title.clone()
            } else {
                clip_bytes(&event.answer, 200)
            };
        }
        _ => {}
    }
    row
}

fn clip_bytes(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

type Rows = Arc<Mutex<BTreeMap<String, (LiveAgentRow, std::time::Instant)>>>;

fn snapshot(rows: &Rows) -> LiveAgentNotice {
    let rows = rows.lock().unwrap_or_else(|e| e.into_inner());
    LiveAgentNotice {
        rows: rows
            .values()
            .map(|(row, started)| LiveAgentRow {
                elapsed_ms: started.elapsed().as_millis() as i64,
                ..row.clone()
            })
            .collect(),
    }
}

/// The runs anchored in `channel_id`, live. Polls `runs` for the pending set
/// and keeps one output watcher per run; a run leaving the pending set takes
/// its row with it.
pub fn chat_live_agents(
    rpc: String,
    channel_id: String,
) -> iced::futures::stream::BoxStream<'static, LiveAgentNotice> {
    use iced::futures::StreamExt as _;
    let (sender, receiver) = tokio::sync::mpsc::channel::<LiveAgentNotice>(64);
    tokio::spawn(async move {
        let Ok(client) = rpc_client(&rpc) else {
            return;
        };
        let rows: Rows = Arc::default();
        let mut watchers: BTreeMap<String, tokio::task::JoinHandle<()>> = BTreeMap::new();
        let ask = serde_json::json!("pending_runs");
        while !sender.is_closed() {
            let Ok(pending) = client.query::<_, serde_json::Value>("runs", &ask).await else {
                tokio::time::sleep(PENDING_POLL).await;
                continue;
            };
            let mut seen = Vec::new();
            for record in pending["pending_runs"].as_array().into_iter().flatten() {
                if record["channel_id"].as_str() != Some(channel_id.as_str()) {
                    continue;
                }
                let dispatch = record["dispatch_id"].as_str().unwrap_or_default().to_string();
                seen.push(dispatch.clone());
                if watchers.contains_key(&dispatch) {
                    continue;
                }
                let agent_id = record["agent_id"].as_str().unwrap_or_default();
                let row = LiveAgentRow {
                    anchor_seq: record["anchor_seq"].as_i64().unwrap_or(0),
                    thread_root: record["thread_root"].as_i64().unwrap_or(0),
                    run_id: record["run_id"].as_str().unwrap_or_default().to_string(),
                    dispatch: dispatch.clone(),
                    agent: names().member_label(agent_id),
                    status: "Starting".into(),
                    ..LiveAgentRow::default()
                };
                rows.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(dispatch.clone(), (row, std::time::Instant::now()));
                watchers.insert(
                    dispatch.clone(),
                    tokio::spawn(watch_live_output(
                        rpc.clone(),
                        dispatch,
                        rows.clone(),
                        sender.clone(),
                    )),
                );
            }
            let gone: Vec<String> = watchers
                .keys()
                .filter(|dispatch| !seen.contains(dispatch))
                .cloned()
                .collect();
            for dispatch in gone {
                if let Some(handle) = watchers.remove(&dispatch) {
                    handle.abort();
                }
                rows.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&dispatch);
            }
            if sender.send(snapshot(&rows)).await.is_err() {
                break;
            }
            tokio::time::sleep(PENDING_POLL).await;
        }
        for handle in watchers.into_values() {
            handle.abort();
        }
    });
    iced::futures::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| (event, receiver))
    })
    .boxed()
}

async fn watch_live_output(
    rpc: String,
    dispatch: String,
    rows: Rows,
    sender: tokio::sync::mpsc::Sender<LiveAgentNotice>,
) {
    use iced::futures::StreamExt as _;
    let fold = |event: &AgentChatEvent| {
        let mut rows = rows.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((row, _)) = rows.get_mut(&dispatch) {
            *row = live_row_apply(std::mem::take(row), event);
        }
    };
    let watch = async {
        let (_, workspace) =
            workspace_at(&rpc).ok_or_else(|| "no local workspace for this node".to_string())?;
        let token = read_link_token(&workspace)?;
        let (mut socket, _) = tokio_tungstenite::connect_async(agent_ws_url(&rpc))
            .await
            .map_err(|error| format!("could not open the node event stream: {error}"))?;
        let topic = format!("run-output:{dispatch}");
        let subscribe =
            serde_json::json!({"op": "subscribe", "topics": [topic], "token": token});
        socket
            .send(Message::Text(subscribe.to_string()))
            .await
            .map_err(|error| format!("could not subscribe to the agent run: {error}"))?;
        let mut id = 1i64;
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(text.as_ref()) else {
                continue;
            };
            if let Some(detail) = subscription_refusal(&value) {
                return Err(detail);
            }
            if value["topic"].as_str() != Some(topic.as_str()) {
                continue;
            }
            let Some(line) = value["item"]["line"].as_str() else {
                continue;
            };
            if let Some(event) = provider_output_event("claude", line, id) {
                id += 1;
                fold(&event);
                if sender.send(snapshot(&rows)).await.is_err() {
                    return Ok(());
                }
            }
        }
        Ok::<(), String>(())
    };
    if let Err(message) = watch.await {
        fold(&AgentChatEvent {
            id: 0,
            kind: "error".into(),
            title: message,
            detail: String::new(),
            status: String::new(),
            answer: String::new(),
            saga_id: String::new(),
        });
        let _ = sender.send(snapshot(&rows)).await;
    }
}

/// Stop an anchored run: the runs module gates the cancel to the requester
/// or the agent's owner, so the signature is the reader's own.
pub async fn cancel_agent_run(
    rpc: String,
    password: String,
    run_id: String,
) -> Result<bool, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "runs",
            runs::encode_msg(&runs::RunsMsg::CancelRun { run_id }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, title: &str, status: &str) -> AgentChatEvent {
        AgentChatEvent {
            id: 1,
            kind: kind.into(),
            title: title.into(),
            detail: String::new(),
            status: status.into(),
            answer: String::new(),
            saga_id: String::new(),
        }
    }

    #[test]
    fn output_events_fold_into_a_status_line_and_a_checklist() {
        let row = live_row_apply(LiveAgentRow::default(), &event("status", "Thinking", ""));
        assert_eq!(row.status, "Thinking");
        let row = live_row_apply(row, &event("activity", "Command", "running"));
        assert_eq!(row.activity.len(), 1);
        assert!(!row.activity[0].done);
        let row = live_row_apply(row, &event("activity", "Command", "done"));
        assert_eq!(row.activity.len(), 1, "the same title upserts");
        assert!(row.activity[0].done);
        let row = live_row_apply(
            row,
            &AgentChatEvent {
                answer: "the reply".into(),
                ..event("answer", "", "")
            },
        );
        assert_eq!(row.answer_preview, "the reply");
        assert_eq!(row.status, "Done");
    }

    #[test]
    fn a_row_stays_bounded_however_long_the_run_talks() {
        let mut row = LiveAgentRow::default();
        for i in 0..40 {
            row = live_row_apply(row, &event("activity", &format!("Step {i}"), "done"));
        }
        assert_eq!(row.activity.len(), MAX_LIVE_ACTIVITY);
        assert_eq!(row.activity[0].label, "Step 28", "the oldest fall off");
        let long = "x".repeat(10_000);
        let row = live_row_apply(
            row,
            &AgentChatEvent {
                answer: long,
                ..event("preview", "", "")
            },
        );
        assert!(row.answer_preview.len() <= MAX_LIVE_PREVIEW_BYTES + '…'.len_utf8());
    }
}
