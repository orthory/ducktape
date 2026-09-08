//! An agent run anchored to a chat message, shown live under its anchor
//! while it runs. The chain carries only the anchor and the committed reply;
//! the progress rides the node's `run-output:<dispatch>` topic, folded here
//! into a bounded row of status, activity titles and an answer preview. A row
//! lives exactly as long as its run is pending in `runs`: the entry prunes in
//! the block that posts the reply, so the committed message takes the row's
//! place.
//!
//! ONE READING PER NODE, NOT PER ROOM. Every pending run the node knows is
//! folded here and each row NAMES ITS ROOM, so which rows reach the screen is
//! a filter on `channel_id` (`module_view::encode_chat_props`) rather than a
//! stream that has to be torn down and relaunched on every room switch — the
//! eight handlers that move `active_channel` would each have had to remember
//! to, and the one that forgot would have drawn another room's runs. The
//! reading also names the NODE it was taken from, because two networks can
//! both have a room called `general`: a reading from the node she just left is
//! dropped whole (`live_agents_of`).

use super::*;
use iced::futures::SinkExt as _;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::tungstenite::Message;

/// Activity titles kept per row; older ones fall off the front.
pub(crate) const MAX_LIVE_ACTIVITY: usize = 12;
/// The answer preview a row carries across the wire, in bytes. Small on
/// purpose: the committed message replaces the row within a block or two, and
/// every byte here is taken out of the timeline's frame budget
/// (`module_view::LIVE_AGENT_TEXT_BUDGET`).
pub(crate) const MAX_LIVE_PREVIEW_BYTES: usize = 512;
const ACTIVITY_DETAIL_CHARS: usize = 60;
const PENDING_POLL: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize)]
pub struct LiveActivity {
    pub label: String,
    pub done: bool,
}

/// One agent run in flight, as the row drawn under its anchor.
///
/// Every field here is DRAWN or decides WHERE the card is drawn. Nothing that
/// changes on a clock belongs in it: the guest memoizes the timeline on this
/// value's hash, so an elapsed-millis field would have rebuilt the whole
/// message window on every poll tick for as long as any agent was working.
#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize)]
pub struct LiveAgentRow {
    /// The room the anchor is in — the one fact that decides whether this row
    /// reaches the screen at all.
    pub channel_id: String,
    pub anchor_seq: i64,
    /// The anchor's thread root when the anchor is itself a reply, else 0: the
    /// reply posts into that thread, so the rail draws the card there.
    pub thread_root: i64,
    pub run_id: String,
    pub agent: String,
    pub status: String,
    pub activity: Vec<LiveActivity>,
    pub answer_preview: String,
}

/// One reading of the node's pending runs, and the node it was read from.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct LiveAgentNotice {
    pub rpc: String,
    pub rows: Vec<LiveAgentRow>,
}

/// The rows of a reading taken from the node on screen. A reading that crossed
/// with a reconnect names the node she LEFT, and its rooms are not hers: it is
/// dropped whole rather than filtered, because room ids are not unique across
/// networks.
pub fn live_agents_of(notice: &LiveAgentNotice, rpc: &str) -> Vec<LiveAgentRow> {
    if notice.rpc != rpc {
        return Vec::new();
    }
    notice.rows.clone()
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
            row.answer_preview = clip_text(&event.answer, MAX_LIVE_PREVIEW_BYTES);
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
                clip_text(&event.answer, 200)
            };
        }
        _ => {}
    }
    row
}

/// The live rows, keyed by the dispatch whose output feeds them. The dispatch
/// id never reaches the screen — it is the watcher's handle, nothing the
/// reader can act on.
type Rows = Arc<Mutex<BTreeMap<String, LiveAgentRow>>>;

fn snapshot(rpc: &str, rows: &Rows) -> LiveAgentNotice {
    let rows = rows.lock().unwrap_or_else(|e| e.into_inner());
    LiveAgentNotice {
        rpc: rpc.to_string(),
        rows: rows.values().cloned().collect(),
    }
}

/// Every agent run this node holds in flight, live. Polls `runs` for the
/// pending set and keeps one output watcher per run; a run leaving the pending
/// set takes its row with it, which is how a completed, failed or cancelled
/// run reconciles — the committed reply (or nothing) stands alone afterwards.
pub fn chat_live_agents(rpc: String) -> iced::futures::stream::BoxStream<'static, LiveAgentNotice> {
    use iced::futures::StreamExt as _;
    let (sender, receiver) = tokio::sync::mpsc::channel::<LiveAgentNotice>(64);
    tokio::spawn(async move {
        let Ok(client) = rpc_client(&rpc) else {
            return;
        };
        let rows: Rows = Arc::default();
        let mut watchers: BTreeMap<String, tokio::task::JoinHandle<()>> = BTreeMap::new();
        let mut labels: BTreeMap<String, String> = BTreeMap::new();
        let ask = serde_json::json!("pending_runs");
        while !sender.is_closed() {
            let Ok(pending) = client.query::<_, serde_json::Value>("runs", &ask).await else {
                tokio::time::sleep(PENDING_POLL).await;
                continue;
            };
            let anchored: Vec<&serde_json::Value> = pending["pending_runs"]
                .as_array()
                .into_iter()
                .flatten()
                // A job-backed run has no anchor in any room (`PendingRun`
                // leaves `channel_id` empty for one), so there is nowhere in
                // chat to draw it.
                .filter(|record| {
                    !record["channel_id"]
                        .as_str()
                        .unwrap_or_default()
                        .is_empty()
                })
                .collect();
            // ONE ROSTER READ PER NEW AGENT, not one per poll: a run's agent
            // cannot be renamed mid-run, and a quiet node must not pay a query
            // every two seconds to learn nothing.
            let unnamed = anchored
                .iter()
                .any(|record| !labels.contains_key(record["agent_id"].as_str().unwrap_or_default()));
            if unnamed {
                labels.extend(agent_labels(&client).await);
                // AND ASKED ONLY ONCE. A run whose agent the roster does not
                // name (it was deleted, or the query failed) falls back to the
                // agent id — without seating that fallback, `unnamed` would
                // stand for as long as the run does and cost a second query
                // every poll for a name that is not coming.
                for record in &anchored {
                    let id = record["agent_id"].as_str().unwrap_or_default();
                    labels.entry(id.to_string()).or_insert_with(|| id.to_string());
                }
            }
            let mut seen = Vec::new();
            for record in anchored {
                let dispatch = record["dispatch_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                seen.push(dispatch.clone());
                if watchers.contains_key(&dispatch) {
                    continue;
                }
                let agent_id = record["agent_id"].as_str().unwrap_or_default();
                let row = LiveAgentRow {
                    channel_id: record["channel_id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    anchor_seq: record["anchor_seq"].as_i64().unwrap_or(0),
                    thread_root: record["thread_root"].as_i64().unwrap_or(0),
                    run_id: record["run_id"].as_str().unwrap_or_default().to_string(),
                    agent: labels
                        .get(agent_id)
                        .cloned()
                        .unwrap_or_else(|| agent_id.to_string()),
                    status: "Starting".into(),
                    ..LiveAgentRow::default()
                };
                rows.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(dispatch.clone(), row);
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
            if sender.send(snapshot(&rpc, &rows)).await.is_err() {
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

/// Every registered agent's display name by id — the same record the Agents
/// tab and the roster label an agent from, so one agent reads the same in all
/// three. A node that cannot answer names nobody, and the row falls back to
/// the agent id.
async fn agent_labels(client: &RpcClient) -> BTreeMap<String, String> {
    let ask = runs::RunsQuery::Model {
        query: runs::ModelQuery::Agents,
    };
    let Ok(runs::RunsReply::Model(runs::ModelReply::Agents(records))) =
        client.query::<_, runs::RunsReply>("runs", &ask).await
    else {
        return BTreeMap::new();
    };
    records
        .into_iter()
        .map(|record| (record.agent_id, record.display_name))
        .collect()
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
        if let Some(row) = rows.get_mut(&dispatch) {
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
        let subscribe = serde_json::json!({"op": "subscribe", "topics": [topic], "token": token});
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
            // A PENDING RUN DOES NOT NAME ITS PROVIDER (`PendingRun` carries
            // the agent and the anchor, not the worker), and the parse only
            // branches on the provider for Claude's `{"type":"result"}` line —
            // every other shape, Codex's included, is read the same way. So
            // this reads one more line than a Codex run would emit and loses
            // nothing; it is not a claim about which worker took the run.
            if let Some(event) = provider_output_event("claude", line, id) {
                id += 1;
                fold(&event);
                if sender.send(snapshot(&rpc, &rows)).await.is_err() {
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
        let _ = sender.send(snapshot(&rpc, &rows)).await;
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

    /// THE NETWORK GUARD. A reading in flight when the reader reconnects
    /// elsewhere names the node she left, and room ids are not unique across
    /// networks — so it is dropped whole rather than filtered by room.
    #[test]
    fn a_reading_from_another_node_is_dropped_whole() {
        let notice = LiveAgentNotice {
            rpc: "http://127.0.0.1:8844".into(),
            rows: vec![LiveAgentRow {
                channel_id: "general".into(),
                anchor_seq: 2,
                agent: "ferris".into(),
                ..LiveAgentRow::default()
            }],
        };
        assert_eq!(
            live_agents_of(&notice, "http://127.0.0.1:8844").len(),
            1,
            "her own node's reading stands"
        );
        assert!(
            live_agents_of(&notice, "http://127.0.0.1:9844").is_empty(),
            "another node's rooms are not hers, however they are named"
        );
    }
}
