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
//! to, and the one that forgot would have drawn another room's runs.
//!
//! A reading is stamped with the CONNECTION it was taken over — endpoint, chain
//! id and connect attempt — because room ids are not unique across networks and
//! the endpoint is not unique across chains. A reading that crossed with a
//! reconnect is DROPPED by the handler (`live_agents_stale`), never assigned:
//! its emptiness describes a connection nobody is on, and writing it would
//! blank the cards the current one just installed.

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

/// One reading of the node's pending runs, stamped with the connection it was
/// taken over: the endpoint, the chain that endpoint was serving, and the
/// connect attempt.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct LiveAgentNotice {
    pub rpc: String,
    pub chain_id: String,
    pub generation: i64,
    pub rows: Vec<LiveAgentRow>,
}

/// Whether this reading describes a connection that is no longer the one on
/// screen. A caller that gets `true` must DROP the reading and leave the rows
/// it already has alone — a stale reading is not evidence that nothing is
/// running, and assigning its (empty) rows would blank the cards the CURRENT
/// connection just installed until the next poll.
///
/// THE ENDPOINT IS NOT AN IDENTITY. A workspace switch brings the node back on
/// the same loopback port, so the url alone would call a new chain's reading
/// current — the same trap `live_resynced` names `chain_left_behind`, one plane
/// over. `chain_id` is the node's own pushed status and `generation` is the
/// connect attempt, so the three together name THIS connection to THIS chain.
pub fn live_agents_stale(
    notice: &LiveAgentNotice,
    rpc: &str,
    chain_id: &str,
    generation: i64,
) -> bool {
    notice.rpc != rpc || notice.chain_id != chain_id || notice.generation != generation
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

/// The connection a reading is stamped with, carried verbatim from the
/// subscription's own arguments — this task never learns it for itself, so a
/// reading cannot claim a connection the app was not on when it was asked for.
#[derive(Clone)]
struct Taken {
    rpc: String,
    chain_id: String,
    generation: i64,
}

/// How many times a dropped output stream is re-dialed before the row keeps the
/// failure it last reported and stops trying. The pending poll is the clock, so
/// the attempts are one poll apart — a re-dial is never a second concurrent
/// watcher for the same run.
const MAX_OUTPUT_DIALS: u32 = 5;

/// What the screen says about a run whose progress this device is not entitled
/// to read. The run IS working — `runs` says so — and the card still carries the
/// agent, the anchor and a Stop; it is the stdout that is out of reach.
const OUTPUT_UNAVAILABLE: &str = "Working · progress unavailable from this device";

/// One run's output watcher and how many times it has been dialed.
struct Watcher {
    handle: tokio::task::JoinHandle<()>,
    dials: u32,
}

/// What the poll must do about one pending run's output watcher. ONE tagged
/// value, because "is there an entry in the map" was the whole question before
/// and it was the wrong one: a watcher whose socket dropped leaves a FINISHED
/// handle in the map, and a `contains_key` reads that as "being watched" — so a
/// single transient websocket failure left the run with no watcher for the rest
/// of its life.
#[derive(Debug, PartialEq)]
enum Dial {
    /// This device may not read a run's output at all. Never dialed, and never
    /// re-dialed: it is an entitlement, not a transient failure.
    Unreadable,
    /// No watcher yet.
    First,
    /// A live watcher is on it — never a second one for one run.
    Watching,
    /// Its watcher finished early; dial again, this many times tried so far.
    Again(u32),
    /// It has been dialed [`MAX_OUTPUT_DIALS`] times. The row keeps the failure
    /// its last attempt folded in, which is what the reader needs to see.
    GaveUp,
}

/// Decide from the entitlement and the watcher's own LIVENESS — never from its
/// presence in the map.
fn dial_for(output_readable: bool, watcher: Option<(bool, u32)>) -> Dial {
    if !output_readable {
        return Dial::Unreadable;
    }
    let Some((finished, dials)) = watcher else {
        return Dial::First;
    };
    if !finished {
        return Dial::Watching;
    }
    if dials < MAX_OUTPUT_DIALS {
        return Dial::Again(dials);
    }
    Dial::GaveUp
}

fn snapshot(taken: &Taken, rows: &Rows) -> LiveAgentNotice {
    let rows = rows.lock().unwrap_or_else(|e| e.into_inner());
    LiveAgentNotice {
        rpc: taken.rpc.clone(),
        chain_id: taken.chain_id.clone(),
        generation: taken.generation,
        rows: rows.values().cloned().collect(),
    }
}

/// Every agent run this node holds in flight, live. Polls `runs` for the
/// pending set and keeps one output watcher per run; a run leaving the pending
/// set takes its row with it, which is how a completed, failed or cancelled
/// run reconciles — the committed reply (or nothing) stands alone afterwards.
pub fn chat_live_agents(
    rpc: String,
    chain_id: String,
    generation: i64,
) -> iced::futures::stream::BoxStream<'static, LiveAgentNotice> {
    use iced::futures::StreamExt as _;
    let (sender, receiver) = tokio::sync::mpsc::channel::<LiveAgentNotice>(64);
    tokio::spawn(async move {
        let Ok(client) = rpc_client(&rpc) else {
            return;
        };
        let taken = Taken {
            rpc: rpc.clone(),
            chain_id,
            generation,
        };
        // WHETHER THIS DEVICE MAY READ A RUN'S STDOUT AT ALL, asked once. The
        // node's `run-output:<id>` topic is `Admission::Workspace` BY DESIGN
        // (`noded::stream`: "Write-open / read-gated is deliberate asymmetry"),
        // so the proof is a token out of the node's own workspace directory —
        // which an app dialing a REMOTE node does not have. That is not a
        // transient failure and must not be re-dialed: the pending card still
        // carries the agent, the room, the anchor and a Stop, and says plainly
        // that the progress is out of reach. Reading it from a remote app needs
        // a signed per-run read seam on the node that does not exist yet.
        let output_readable = workspace_at(&rpc).is_some();
        let rows: Rows = Arc::default();
        let mut watchers: BTreeMap<String, Watcher> = BTreeMap::new();
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
                .filter(|record| !record["channel_id"].as_str().unwrap_or_default().is_empty())
                .collect();
            // ONE ROSTER READ PER NEW AGENT, not one per poll: a run's agent
            // cannot be renamed mid-run, and a quiet node must not pay a query
            // every two seconds to learn nothing.
            let unnamed = anchored.iter().any(|record| {
                !labels.contains_key(record["agent_id"].as_str().unwrap_or_default())
            });
            if unnamed {
                labels.extend(agent_labels(&client).await);
                // AND ASKED ONLY ONCE. A run whose agent the roster does not
                // name (it was deleted, or the query failed) falls back to the
                // agent id — without seating that fallback, `unnamed` would
                // stand for as long as the run does and cost a second query
                // every poll for a name that is not coming.
                for record in &anchored {
                    let id = record["agent_id"].as_str().unwrap_or_default();
                    labels
                        .entry(id.to_string())
                        .or_insert_with(|| id.to_string());
                }
            }
            let mut seen = Vec::new();
            for record in anchored {
                let dispatch = record["dispatch_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                seen.push(dispatch.clone());
                let dial = dial_for(
                    output_readable,
                    watchers
                        .get(&dispatch)
                        .map(|watcher| (watcher.handle.is_finished(), watcher.dials)),
                );
                // SEATED ONCE AND THEN LEFT ALONE: a re-dial must not discard
                // the activity the dropped watcher already folded in, and an
                // unwatched row must not be rewritten every poll.
                let unseated = !rows
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains_key(&dispatch);
                if unseated {
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
                        status: if output_readable {
                            "Starting".into()
                        } else {
                            OUTPUT_UNAVAILABLE.into()
                        },
                        ..LiveAgentRow::default()
                    };
                    rows.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(dispatch.clone(), row);
                }
                let dials = match dial {
                    Dial::Unreadable | Dial::Watching | Dial::GaveUp => continue,
                    Dial::First => 1,
                    Dial::Again(tried) => tried + 1,
                };
                watchers.insert(
                    dispatch.clone(),
                    Watcher {
                        handle: tokio::spawn(watch_live_output(
                            taken.clone(),
                            dispatch,
                            rows.clone(),
                            sender.clone(),
                        )),
                        dials,
                    },
                );
            }
            // WHAT LEFT THE PENDING SET, read off the ROWS and not off the
            // watchers. A device that may not read output has no watchers at
            // all, so a sweep over their keys would have left every settled
            // run's card standing on screen for the life of the session.
            let gone: Vec<String> = rows
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .keys()
                .filter(|dispatch| !seen.contains(dispatch))
                .cloned()
                .collect();
            for dispatch in gone {
                if let Some(watcher) = watchers.remove(&dispatch) {
                    watcher.handle.abort();
                }
                rows.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&dispatch);
            }
            if sender.send(snapshot(&taken, &rows)).await.is_err() {
                break;
            }
            tokio::time::sleep(PENDING_POLL).await;
        }
        for watcher in watchers.into_values() {
            watcher.handle.abort();
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
    taken: Taken,
    dispatch: String,
    rows: Rows,
    sender: tokio::sync::mpsc::Sender<LiveAgentNotice>,
) {
    let rpc = taken.rpc.clone();
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
                if sender.send(snapshot(&taken, &rows)).await.is_err() {
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
        let _ = sender.send(snapshot(&taken, &rows)).await;
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

    /// A DROPPED OUTPUT STREAM IS RE-DIALED, and the presence of a handle is not
    /// evidence that anything is watching. `contains_key` was the whole test
    /// before, and a finished task stays in the map — so one transient websocket
    /// failure left the run unwatched for the rest of its life while the card
    /// sat on whatever status it had reached.
    #[test]
    fn a_finished_watcher_is_redialed_until_the_budget_runs_out() {
        assert_eq!(
            dial_for(true, None),
            Dial::First,
            "nothing is watching it yet"
        );
        assert_eq!(
            dial_for(true, Some((false, 1))),
            Dial::Watching,
            "a live watcher is left alone — never a second one for one run"
        );
        assert_eq!(
            dial_for(true, Some((true, 1))),
            Dial::Again(1),
            "its socket dropped, and the handle sitting in the map said nothing"
        );
        assert_eq!(
            dial_for(true, Some((true, MAX_OUTPUT_DIALS - 1))),
            Dial::Again(MAX_OUTPUT_DIALS - 1),
            "the last attempt inside the budget"
        );
        assert_eq!(
            dial_for(true, Some((true, MAX_OUTPUT_DIALS))),
            Dial::GaveUp,
            "past the budget the row keeps the failure it last reported"
        );
    }

    /// AN APP WITH NO LOCAL WORKSPACE NEVER DIALS. The node gates
    /// `run-output:<id>` on a token out of its own workspace directory
    /// (`Admission::Workspace`, deliberately), so a Mac or any app pointed at a
    /// REMOTE node cannot read a run's stdout. That is an entitlement, not a
    /// flaky socket: re-dialing it five times would buy nothing but noise, and
    /// the card still earns its place from the pending poll alone.
    #[test]
    fn a_device_that_may_not_read_output_never_dials_for_it() {
        for watcher in [None, Some((false, 0)), Some((true, 2))] {
            assert_eq!(
                dial_for(false, watcher),
                Dial::Unreadable,
                "no state of a watcher makes an unentitled read dialable"
            );
        }
        assert_eq!(
            OUTPUT_UNAVAILABLE, "Working · progress unavailable from this device",
            "and the row says so, rather than showing a run that looks stalled"
        );
    }

    /// THE CONNECTION GUARD, and the endpoint is the WEAKEST third of it. A
    /// workspace switch brings the node back on the same loopback port, so a
    /// reading still in flight from the chain she left carries the url she is
    /// on — it has to be refused on the chain id or the connect attempt, and a
    /// caller that refuses it must leave the current rows alone rather than
    /// assign its emptiness.
    #[test]
    fn a_reading_from_a_connection_she_has_left_is_refused() {
        let here = "http://127.0.0.1:8844";
        let notice = LiveAgentNotice {
            rpc: here.into(),
            chain_id: "testnet#abcd".into(),
            generation: 7,
            rows: vec![LiveAgentRow {
                channel_id: "general".into(),
                anchor_seq: 2,
                agent: "Chief Duck".into(),
                ..LiveAgentRow::default()
            }],
        };
        assert!(
            !live_agents_stale(&notice, here, "testnet#abcd", 7),
            "the reading for the connection on screen stands"
        );
        assert!(
            live_agents_stale(&notice, "http://127.0.0.1:9844", "testnet#abcd", 7),
            "another endpoint"
        );
        assert!(
            live_agents_stale(&notice, here, "othernet#0f0f", 7),
            "SAME URL, NEW CHAIN — a workspace switch keeps the port, so the \
             url alone would have called this reading current"
        );
        assert!(
            live_agents_stale(&notice, here, "testnet#abcd", 8),
            "same url and chain, but a reconnect has happened since"
        );
    }
}
