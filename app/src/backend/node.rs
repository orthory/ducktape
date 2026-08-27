use super::*;

/// The device-local settings facts: where this app points and what identity it
/// holds locally. Node status belongs to [`NodeFacts`].
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct SettingsFacts {
    pub generation: i64,
    pub key_path: String,
    pub key_state: String,
    /// This workspace's directory on this device — the Node overview's data dir.
    pub data_dir: String,
    pub open_tabs: i64,
    /// THE VIEWER'S OWN KEY, full hex — the `me` every membership test needs.
    /// `ChatMember.key` is `member_id(..)` at full width, and the account card
    /// carries an account NUMBER, not a key, so neither the account card nor
    /// the node key can answer "is this row me". Empty on a device with no user
    /// key, which `post_gate` reads as "not seated" — the honest answer when
    /// there is no identity to seat.
    pub user_key: String,
}

/// The NETWORK card's Data dir row.
/// Load the settings facts: the local user key's location and state, the
/// workspace directory, and the persisted tab count.
pub async fn load_settings_facts(
    rpc: String,
    generation: i64,
) -> Result<SettingsFacts, HydrationError> {
    async {
        let (key_path, key_state) = match user_key_path() {
            Err(_) => ("(unset)".to_string(), "unlocatable".to_string()),
            Ok(path) => {
                let state = match std::fs::read(&path) {
                    Err(_) => "absent",
                    Ok(bytes) if bytes.starts_with(ENCRYPTED_KEY_PREFIX.as_bytes()) => "encrypted",
                    Ok(_) => "PLAINTEXT — secure it",
                };
                (path.display().to_string(), state.to_string())
            }
        };
        let tabs = load_doc_tabs(rpc.clone()).await;
        let data_dir = workspace_at(&rpc)
            .map(|(_, dir)| dir.display().to_string())
            .or_else(|| ducktape_home().map(|home| home.display().to_string()))
            .unwrap_or_default();
        Ok::<_, String>(SettingsFacts {
            generation,
            key_path,
            key_state,
            data_dir,
            open_tabs: count_i64(tabs.len()),
            user_key: local_user_key()
                .await
                .map(|key| hex_encode(&key))
                .unwrap_or_default(),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Forget this endpoint's persisted doc tabs.
pub async fn clear_doc_tabs(rpc: String) -> bool {
    save_doc_tabs(rpc, Vec::new()).await
}

/// One log line for the operator pane.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct NodeLogLine {
    pub cursor: String,
    pub line: String,
}

pub type NodeLogTimelineEvent = ui_lang_components::ui::log_timeline::LogTimelineEvent<String>;

/// Retained native timeline state plus the bounded rows it renders.
///
/// Clone snapshots the same mounted widget state; the old value is replaced by
/// the Ice assignment that requested the clone.
#[derive(Debug)]
pub struct NodeLogTimelineState {
    timeline: ui_lang_components::ui::log_timeline::LogTimelineState<String>,
    lines: Arc<[NodeLogLine]>,
    visible: Arc<[NodeLogLine]>,
    filter: String,
}

impl Clone for NodeLogTimelineState {
    fn clone(&self) -> Self {
        Self {
            timeline: self.timeline.update_snapshot(),
            lines: Arc::clone(&self.lines),
            visible: Arc::clone(&self.visible),
            filter: self.filter.clone(),
        }
    }
}

const NODE_LOG_LIMIT: usize = 4_096;
const NODE_LOG_TRIM: usize = 1_024;

fn node_log_timeline_config() -> ui_lang_components::ui::log_timeline::VirtualListConfig {
    ui_lang_components::ui::log_timeline::VirtualListConfig::new(26.0)
        .expect("node log row geometry is fixed")
        .overscan(4)
}

pub fn node_log_timeline_state() -> NodeLogTimelineState {
    NodeLogTimelineState {
        timeline: ui_lang_components::ui::log_timeline::LogTimelineState::new(
            ui_lang_components::ui::log_timeline::VirtualListId::new("node-log-timeline"),
        ),
        lines: Arc::from([]),
        visible: Arc::from([]),
        filter: String::new(),
    }
}

pub fn node_log_timeline_reset() -> NodeLogTimelineState {
    node_log_timeline_state()
}

pub fn node_log_timeline_push(
    mut state: NodeLogTimelineState,
    line: NodeLogLine,
) -> NodeLogTimelineState {
    // ponytail: a bounded linear duplicate guard is smaller than retaining a
    // second cursor index; revisit only if the 4,096-line ceiling moves.
    let duplicate = state.lines.iter().any(|held| held.cursor == line.cursor);
    if duplicate {
        return state;
    }
    let mut lines = Vec::from(state.lines.as_ref());
    lines.push(line);
    if lines.len() > NODE_LOG_LIMIT {
        lines.drain(..NODE_LOG_TRIM);
    }
    state.lines = lines.into();
    node_log_timeline_reconcile(state)
}

pub fn node_log_timeline_filter(
    mut state: NodeLogTimelineState,
    filter: String,
) -> NodeLogTimelineState {
    state.filter = filter.trim().to_lowercase();
    node_log_timeline_reconcile(state)
}

fn node_log_timeline_reconcile(mut state: NodeLogTimelineState) -> NodeLogTimelineState {
    let visible: Arc<[NodeLogLine]> = state
        .lines
        .iter()
        .filter(|line| state.filter.is_empty() || line.line.to_lowercase().contains(&state.filter))
        .cloned()
        .collect::<Vec<_>>()
        .into();
    let config = node_log_timeline_config();
    let append = state
        .timeline
        .reconcile(&visible, |line| line.cursor.clone(), config);
    if append.is_err() {
        state
            .timeline
            .replace(&visible, |line| line.cursor.clone(), config)
            .expect("node log cursors are unique");
    }
    state.visible = visible;
    state
}

pub fn node_log_timeline_apply(
    mut state: NodeLogTimelineState,
    event: NodeLogTimelineEvent,
) -> NodeLogTimelineState {
    state.timeline.apply(event, node_log_timeline_config());
    state
}

pub fn node_log_timeline<'a>(
    state: &'a NodeLogTimelineState,
    source: &'a str,
) -> iced::Element<'a, NodeLogTimelineEvent> {
    use iced::widget::{Space, button, column, container, row, text};
    use iced::{Border, Color, Font, Length};
    use ui_lang_components::ui::log_timeline::{LogTimelineEvent, log_timeline};
    use ui_lang_components::ui::theme::DARK;

    let inspection = state.timeline.inspect(node_log_timeline_config());
    let mono = Font {
        family: iced::font::Family::Name(design::fonts::FAMILY_MONO),
        ..Font::DEFAULT
    };
    let tail: iced::Element<'_, NodeLogTimelineEvent> = if inspection.following_tail {
        text("LIVE")
            .size(10)
            .font(mono)
            .color(DARK.palette.success)
            .into()
    } else {
        button(
            text(format!("RESUME · {} NEW", inspection.unread_count))
                .size(10)
                .font(mono),
        )
        .padding([3, 7])
        .on_press(LogTimelineEvent::ResumeTail)
        .into()
    };
    let header = row![
        text("NODE LOG")
            .size(10)
            .font(mono)
            .color(DARK.palette.foreground),
        text(source)
            .size(10)
            .font(mono)
            .color(DARK.palette.muted_foreground),
        Space::new().width(Length::Fill),
        tail,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    let body: iced::Element<'_, NodeLogTimelineEvent> = if state.visible.is_empty() {
        let message = if state.lines.is_empty() {
            "Waiting for the node's log ring…"
        } else {
            "No lines match this filter."
        };
        container(
            text(message)
                .size(12)
                .font(mono)
                .color(DARK.palette.muted_foreground),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else {
        log_timeline(
            &state.timeline,
            &state.visible,
            node_log_timeline_config(),
            "Node log",
            |line| line.cursor.clone(),
            |line| line.line.clone(),
            |_, line, _selected| {
                let parts = split_log_line(line.line.clone());
                let level_color = match parts.level.as_str() {
                    "ERROR" => DARK.palette.destructive,
                    "WARN" => DARK.palette.warning,
                    "INFO" => DARK.palette.success,
                    "DEBUG" | "TRACE" => DARK.palette.muted_foreground,
                    _ => Color::TRANSPARENT,
                };
                row![
                    // 24 mono chars at size 11 (Geist Mono, 0.6 em advance)
                    // need ~158 px; 150 let the tail paint over the level.
                    text(parts.time)
                        .size(11)
                        .font(mono)
                        .color(DARK.palette.muted_foreground)
                        .width(170),
                    text(parts.level)
                        .size(11)
                        .font(mono)
                        .color(level_color)
                        .width(48),
                    text(parts.message)
                        .size(11)
                        .font(mono)
                        .color(DARK.palette.foreground),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .into()
            },
            |event| event,
            &DARK,
        )
    };
    container(column![header, body].spacing(10))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(15)
        .style(|_| container::Style {
            background: Some(DARK.palette.background.into()),
            text_color: Some(DARK.palette.foreground),
            border: Border {
                color: DARK.palette.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// The node's live log ring as an app stream — reconnects with backoff and
/// resumes from the last cursor, exactly like the module stream.
pub fn node_logs(rpc: String) -> iced::futures::stream::BoxStream<'static, NodeLogLine> {
    struct State {
        rpc: String,
        cursor: Option<String>,
        stream: Option<
            iced::futures::stream::BoxStream<'static, ducktape_rpc::Result<ducktape_rpc::LogLine>>,
        >,
        retry_attempt: u32,
    }
    iced::futures::stream::unfold(
        State {
            rpc,
            cursor: None,
            stream: None,
            retry_attempt: 0,
        },
        |mut state| async move {
            loop {
                if state.stream.is_none() && state.retry_attempt > 0 {
                    tokio::time::sleep(retry_delay(state.retry_attempt)).await;
                }
                if state.stream.is_none() {
                    let Ok(rpc) = rpc_client(&state.rpc) else {
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        continue;
                    };
                    match rpc.log_events(state.cursor.clone()).await {
                        Ok(stream) => state.stream = Some(stream),
                        Err(_) => {
                            state.retry_attempt = state.retry_attempt.saturating_add(1);
                            continue;
                        }
                    }
                }
                match state
                    .stream
                    .as_mut()
                    .expect("stream initialized")
                    .next()
                    .await
                {
                    Some(Ok(line)) => {
                        state.retry_attempt = 0;
                        state.cursor = Some(line.cursor.clone());
                        return Some((
                            NodeLogLine {
                                cursor: line.cursor,
                                line: line.line,
                            },
                            state,
                        ));
                    }
                    Some(Err(_)) | None => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                    }
                }
            }
        },
    )
    .boxed()
}

/// One tracing line, split for the dark log console's three columns.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LogParts {
    pub time: String,
    pub level: String,
    pub message: String,
}

/// The ring's tracing timer prints microseconds (`…T09:12:44.918273Z`, 27
/// chars) but the console column is sized for milliseconds — an iced text
/// widget never clips itself, so the extra digits paint over the level
/// column. Trim the fraction to three digits; any other shape passes through.
fn trim_time_to_millis(time: &str) -> String {
    let Some((secs, frac)) = time.rsplit_once('.') else {
        return time.to_string();
    };
    let Some(digits) = frac.strip_suffix('Z') else {
        return time.to_string();
    };
    let trimmable = digits.len() > 3 && digits.bytes().all(|b| b.is_ascii_digit());
    if !trimmable {
        return time.to_string();
    }
    format!("{secs}.{}Z", &digits[..3])
}

/// Split `2026-07-27T09:12:44.918Z  INFO ducktape::join: admitted` into its
/// three columns. A line that does not carry a level is all message.
pub fn split_log_line(line: String) -> LogParts {
    const LEVELS: [&str; 5] = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
    let mut fields = line.split_whitespace();
    let Some(first) = fields.next() else {
        return LogParts {
            time: String::new(),
            level: String::new(),
            message: line,
        };
    };
    let timestamped =
        first.contains(':') && first.chars().next().is_some_and(|c| c.is_ascii_digit());
    let (time, level_field) = match timestamped {
        true => (trim_time_to_millis(first), fields.next().unwrap_or_default()),
        false => (String::new(), first),
    };
    if !LEVELS.contains(&level_field) {
        return LogParts {
            time,
            level: String::new(),
            message: line,
        };
    }
    let cut = line
        .find(level_field)
        .map_or(line.len(), |at| at + level_field.len());
    LogParts {
        time,
        level: level_field.to_string(),
        message: line[cut..].trim_start().to_string(),
    }
}

#[cfg(test)]
mod log_timeline_tests {
    use super::*;

    #[test]
    fn timeline_keeps_unique_history_and_replaces_on_filter_changes() {
        let mut state = node_log_timeline_state();
        state = node_log_timeline_push(
            state,
            NodeLogLine {
                cursor: "1".into(),
                line: "INFO admitted resident".into(),
            },
        );
        state = node_log_timeline_push(
            state,
            NodeLogLine {
                cursor: "1".into(),
                line: "duplicate cursor".into(),
            },
        );
        state = node_log_timeline_push(
            state,
            NodeLogLine {
                cursor: "2".into(),
                line: "WARN retrying dial".into(),
            },
        );

        assert_eq!(state.lines.len(), 2);
        assert_eq!(state.visible.len(), 2);
        assert_eq!(
            state
                .timeline
                .inspect(node_log_timeline_config())
                .list
                .logical_items,
            2
        );

        state = node_log_timeline_filter(state, " warn ".into());
        assert_eq!(state.visible.len(), 1);
        assert_eq!(state.visible[0].cursor, "2");
        assert_eq!(
            state
                .timeline
                .inspect(node_log_timeline_config())
                .list
                .logical_items,
            1
        );
    }
}

/// The node's consensus/storage facts — everything `/v1/status` publishes that
/// the two-field `Status` type drops, plus the mesh sample's live/total.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct NodeFacts {
    /// The daemon identity, full hex so the operator surface can copy the key
    /// that membership and peer records actually carry.
    pub public_key: String,
    /// The daemon's build version, verbatim off `/v1/status` (its own
    /// `CARGO_PKG_VERSION`). A build/commit SHA is NOT published anywhere, so
    /// the version line carries the version alone.
    pub version: String,
    pub root_hash: String,
    /// The three consensus facts are OPTION on purpose: `operations.consensus`
    /// is absent on a resident, a joiner and the embedded local daemon
    /// "rather than being filled with misleading zeroes", so a plain i64 would
    /// print a hard 0 as if it were measured.
    pub view: Option<i64>,
    pub quorum: Option<i64>,
    pub reachable_validators: Option<i64>,
    /// These two are under the SAME absent-on-a-resident `operations` object as
    /// the trio above, so they get the same honesty — carried as [`UNMEASURED`]
    /// rather than a plain `0`, which both renderers already print as `—`.
    pub last_finalized_at: i64,
    pub checkpoint_height: i64,
    /// THE HEAD FROM THE SAME DOCUMENT AS [`Self::checkpoint_height`]. A
    /// checkpoint means nothing except against the head it was sampled with, so
    /// the two travel together or not at all: Settings used to draw them from
    /// two separate `/v1/status` calls and printed CHECKPOINT h 422,563 above
    /// HEIGHT h 422,553 — an order no node is ever in. See [`served_height`]
    /// for why a wire `0` lands here as [`UNMEASURED`].
    pub height: i64,
    /// The node's own lifecycle phase — `starting`, `recovering`, `joining`,
    /// `syncing`, `validating`, `serving`, `draining`, `halted`.
    ///
    /// THE ONLY TRUSTWORTHY DISCRIMINANT for whether a sync is happening. The
    /// `sync` block beside it is written by `begin_sync` and never cleared, so
    /// a node that finished syncing hours ago still carries the last run.
    pub phase: String,
    /// Unix seconds the phase last changed; [`UNMEASURED`] when unpublished.
    pub phase_since: i64,
    /// The sync run's heights, [`UNMEASURED`] when the node has published none.
    pub sync_target: i64,
    pub sync_applied: i64,
    /// CUMULATIVE since boot and never reset, so these are a total rather than
    /// a state — which is why they belong on a detail surface and not on a
    /// badge. Absence really is zero here: a count of nothing IS zero.
    pub sync_retries: i64,
    pub sync_failures: i64,
    /// The last sync error, SELF-CLEARING: `record_sync_progress` puts it back
    /// to `None` the moment the node advances. Present therefore means "the
    /// most recent attempt failed and nothing has moved since", which is a
    /// fact about now rather than a scar.
    pub sync_last_error: String,
}

/// A DEFAULT IS A DOCUMENT NO NODE HAS PUBLISHED, so its three numbers are
/// [`UNMEASURED`] and not zero.
///
/// `derive(Default)` gave them `0`, which is the one value this whole file
/// exists to keep off the screen: `height_label(0)` renders `h 0` and
/// `relative_time(0)` renders nothing, so a defaulted document prints a
/// measured head and a measured checkpoint for a node that has served neither.
/// It is inert today: both arms of `overview_from` construct a default (the
/// struct literal is evaluated before the status arm overwrites `facts`), but
/// only the peers frame's copy survives, and every one of the six `keep_i64` /
/// `keep_str` guards in `node_overview_sample` discards it on
/// `facts_answered == false`. Inert is not the same as right, which is why the
/// invariant is written here rather than left loaded on a public struct.
impl Default for NodeFacts {
    fn default() -> Self {
        Self {
            public_key: String::new(),
            version: String::new(),
            root_hash: String::new(),
            view: None,
            quorum: None,
            reachable_validators: None,
            last_finalized_at: UNMEASURED,
            checkpoint_height: UNMEASURED,
            height: UNMEASURED,
            phase: String::new(),
            phase_since: UNMEASURED,
            sync_target: UNMEASURED,
            sync_applied: UNMEASURED,
            sync_retries: 0,
            sync_failures: 0,
            sync_last_error: String::new(),
        }
    }
}

/// Load the node facts from the raw status document.
/// A section the node omits for its role stays `None` — the status projection
/// leaves it out rather than filling it with misleading numbers, and so do we.
/// The facts a `/v1/status` document carries — the ONE reader, shared by the
/// HTTP load and the pushed `status` snapshot, for the same reason
/// [`peer_rows`] is shared.
pub(crate) fn node_facts(status: &serde_json::Value) -> NodeFacts {
    let operations = &status["operations"];
    let consensus = &operations["consensus"];
    let sync = &operations["sync"];
    NodeFacts {
        public_key: status["public_key"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        version: status["version"].as_str().unwrap_or_default().to_string(),
        root_hash: status["root_hash"].as_str().unwrap_or_default().to_string(),
        view: consensus["view"].as_i64(),
        quorum: consensus["quorum"].as_i64(),
        reachable_validators: consensus["reachable_validators"].as_i64(),
        last_finalized_at: operations["last_finalized_at"]
            .as_i64()
            .unwrap_or(UNMEASURED),
        checkpoint_height: operations["storage"]["checkpoint_height"]
            .as_i64()
            .unwrap_or(UNMEASURED),
        height: served_height(&status["height"]),
        phase: operations["phase"].as_str().unwrap_or_default().to_string(),
        phase_since: operations["phase_since"].as_i64().unwrap_or(UNMEASURED),
        sync_target: sync["target_height"].as_i64().unwrap_or(UNMEASURED),
        sync_applied: sync["applied_height"].as_i64().unwrap_or(UNMEASURED),
        sync_retries: sync["retries"].as_i64().unwrap_or(0),
        sync_failures: sync["failures"].as_i64().unwrap_or(0),
        sync_last_error: sync["last_error"].as_str().unwrap_or_default().to_string(),
    }
}

/// The one sentence all three surfaces print for what the node is doing.
///
/// Progress rides ONLY while `sync_in_progress`. `operations.sync` is never
/// cleared, so printing it whenever it exists leaves a finished run's numbers
/// on screen for good — and a reader cannot tell a live count from a fossil.
pub fn sync_label(phase: &str, applied: i64, target: i64) -> String {
    if phase.is_empty() {
        return String::new();
    }
    let name = capitalized(phase);
    let measured = applied >= 0 && target >= 0;
    if !sync_in_progress(phase) || !measured {
        return name;
    }
    format!(
        "{name} {} / {}",
        grouped_digits(applied),
        grouped_digits(target)
    )
}

/// The node spells its phases lowercase on the wire; a reader reads prose.
fn capitalized(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

/// Whether the node is catching up RIGHT NOW.
///
/// The phase, and only the phase. `operations.sync` is never cleared, so its
/// presence says a sync once happened — not that one is happening.
pub(crate) fn sync_in_progress(phase: &str) -> bool {
    phase == "syncing"
}

pub async fn load_node_facts(rpc: String) -> Result<NodeFacts, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status_json().await?;
        Ok(node_facts(&status))
    }
    .await
    .map_err(app_error)
}

/// The head a status document actually serves, or [`UNMEASURED`] when it
/// serves none.
///
/// **A wire `0` is not a measurement — it is the node's own "no boundary
/// served" sentinel**, written at three independent sites: `NodeStatus`'s
/// `Default` ("zeroed boundary facts are the honest answer before any boundary
/// is served"), the validator's `node.finalized().map(|f| f.height)
/// .unwrap_or(0)`, and the replica's `None => (0, String::new(), Vec::new())`.
///
/// That last one is why this matters beside a checkpoint. A resident takes it
/// whenever it stops serving — a range-pruned backfill, an unresolvable pruned
/// view, an epoch cutover — and each of those three sites sets `serving = None`
/// and republishes while passing the LIVE `replica_prev_ckpt`, which only ever
/// climbs. Read as a measurement, one honest document then renders
/// `HEIGHT h 0` above `CHECKPOINT h 425,981`: a measured zero AND the very
/// inversion the pair is supposed to make impossible. Read as absence, it
/// renders `HEIGHT h —` — which is exactly what a node serving no boundary
/// knows about the head.
fn served_height(height: &serde_json::Value) -> i64 {
    match height.as_i64() {
        Some(head) if head > 0 => head,
        _ => UNMEASURED,
    }
}

/// What an `operations` reading the node did not publish carries.
///
/// The rule is already written twice — `NodeFacts`'s consensus trio is
/// `Option` "rather than being filled with misleading zeroes", and `state/node.ice`
/// says an absent reading "must print `—`, never a measured `0`". The two
/// `i64` fields beside them had no way to say it, because `0` is a legal
/// height and a legal timestamp.
///
/// NEGATIVE is that way: `height_label` already renders `< 0` as `h —`, so
/// this reuses a contract the renderer had rather than inventing one. Naming
/// it keeps the `-1` from reading as arithmetic at the fill site.
pub const UNMEASURED: i64 = -1;

/// A consensus fact the node did not publish for this role reads `—`, never a
/// zero. The view has no way to branch on an absent value itself.
pub fn optional_number(value: Option<i64>) -> String {
    match value {
        Some(number) => grouped_digits(number),
        None => "—".into(),
    }
}

/// One direct peer, as `GET /v1/peers` actually reports it.
///
/// There is NO per-peer height on that surface — the envelope carries this
/// node's own, and stamping it on every row would print the same number beside
/// every peer and call it theirs. `role` is the standing the peers view does
/// carry (`validator` / `resident`), absent on a lane that cannot read the
/// valset — and absent renders as nothing, which is the honest answer.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PeerRow {
    pub key: String,
    pub role: String,
    pub live: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PeersData {
    pub generation: i64,
    pub peers: Vec<PeerRow>,
}

/// THE NODE'S OWN STATUS, PUSHED, ON EVERY TAB.
///
/// Cheap to hold anywhere the console is standing: the node answers `status`
/// from a cell it publishes at each boundary, and the snapshot debounce means
/// one read per heartbeat. That is what lets a sync reading follow the reader
/// around instead of living on one tab — the node's phase is a fact about the
/// node, not about the surface you happen to have open.
pub fn node_status_live(rpc: String) -> iced::futures::stream::BoxStream<'static, NodeFacts> {
    snapshot_stream(rpc, Snapshot::Status)
}

/// THE DIRECT-PEER SAMPLE, PUSHED, ONLY WHERE IT IS DRAWN.
///
/// Every sample encodes the node's ENTIRE metrics registry, so the Ice `when`
/// gate on this subscription is the whole budget: leaving the tab stops the
/// encode at the source rather than throttling it here.
pub fn node_peers_live(rpc: String) -> iced::futures::stream::BoxStream<'static, PeersData> {
    snapshot_stream(rpc, Snapshot::Peers)
}

/// Which snapshot topic a stream carries, and how its document is read.
///
/// One discriminant rather than two copies of the reconnect loop: the loops
/// were identical and the only difference was the topic and the reader.
#[derive(Clone, Copy)]
enum Snapshot {
    Status,
    Peers,
}

/// One snapshot topic, reconnecting with backoff, parsed with the SAME reader
/// the HTTP load uses.
///
/// A dropped socket is not a reason to blank the surface: the rows on screen
/// were true when they were sampled. Rebuild the subscription and keep them
/// until a fresher sample replaces them.
fn snapshot_stream<T: Send + 'static>(
    rpc: String,
    topic: Snapshot,
) -> iced::futures::stream::BoxStream<'static, T>
where
    Snapshot: SnapshotReader<T>,
{
    struct State {
        rpc: String,
        stream: Option<
            iced::futures::stream::BoxStream<'static, ducktape_rpc::Result<serde_json::Value>>,
        >,
        retry_attempt: u32,
    }
    iced::futures::stream::unfold(
        State {
            rpc,
            stream: None,
            retry_attempt: 0,
        },
        move |mut state| async move {
            loop {
                if state.stream.is_none() && state.retry_attempt > 0 {
                    tokio::time::sleep(retry_delay(state.retry_attempt)).await;
                }
                if state.stream.is_none() {
                    let Ok(client) = rpc_client(&state.rpc) else {
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        continue;
                    };
                    let opened = match topic {
                        Snapshot::Status => client.status_events().await,
                        Snapshot::Peers => client.peers_events().await,
                    };
                    match opened {
                        Ok(stream) => state.stream = Some(stream),
                        Err(_) => {
                            state.retry_attempt = state.retry_attempt.saturating_add(1);
                            continue;
                        }
                    }
                }
                match state
                    .stream
                    .as_mut()
                    .expect("stream initialized")
                    .next()
                    .await
                {
                    Some(Ok(document)) => {
                        state.retry_attempt = 0;
                        return Some((topic.read(&document), state));
                    }
                    Some(Err(_)) | None => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                    }
                }
            }
        },
    )
    .boxed()
}

/// How one snapshot topic's document becomes the value the console holds.
trait SnapshotReader<T> {
    fn read(&self, document: &serde_json::Value) -> T;
}

impl SnapshotReader<NodeFacts> for Snapshot {
    fn read(&self, document: &serde_json::Value) -> NodeFacts {
        node_facts(document)
    }
}

impl SnapshotReader<PeersData> for Snapshot {
    fn read(&self, document: &serde_json::Value) -> PeersData {
        PeersData {
            generation: -1,
            peers: peer_rows(document),
        }
    }
}

/// The peer rows a `/v1/peers` document carries — the ONE reader, shared by
/// the HTTP load and the pushed `peers` snapshot. A second copy of these key
/// names is exactly how the table came to read three the node never served.
///
/// THE KEYS THE NODE ACTUALLY SERVES. This read `key`/`height`/`live` and
/// `crates/noded/src/peers.rs` serves none of the three, so every row rendered a
/// blank name, a zero, and an offline dot — for peers that were connected.
fn peer_rows(reply: &serde_json::Value) -> Vec<PeerRow> {
    reply["peers"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|peer| PeerRow {
            key: short_label(peer["peer"].as_str().unwrap_or_default()),
            role: peer["role"].as_str().unwrap_or_default().to_string(),
            live: peer["connected"].as_bool().unwrap_or(false),
        })
        .collect()
}

/// Load the peers standing view.
pub async fn load_peers(rpc: String, generation: i64) -> Result<PeersData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let reply = rpc.peers().await?;
        Ok(PeersData {
            generation,
            peers: peer_rows(&reply),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// One registered module, as the node itself reports it.
///
/// There is no MARKETPLACE behind this row and there cannot be: a publisher, a
/// verification badge, an install count and a catalog description exist in no
/// module, no index and no manifest. This is the INSTALLED/RUNTIME truth —
/// what is registered, at which code, with which swap pending.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ModuleRow {
    pub id: String,
    /// `workspace` | `developer` | `automation` | `system` — the presentation
    /// category the status projection attaches by id. Never consensus state.
    pub category: String,
    /// The module's own state root, short form.
    pub root: String,
    /// The active component's sha256, short form. Empty when this network runs
    /// no lifecycle module (the daemon's default set does not).
    pub code_hash: String,
    /// The scheduled swap's target hash, short form; empty when none is armed.
    pub pending_hash: String,
    /// The pending swap's activation height (0 when none is armed).
    pub activation_height: i64,
    /// Validators that have verified the pending bytes locally.
    pub readiness: i64,
    /// The pending swap has full coverage and will activate at its height.
    pub ready: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ModulesData {
    pub rows: Vec<ModuleRow>,
}

/// The registered module set: `/v1/status` publishes id, root and category for
/// every module, and the lifecycle module (where a network runs one) adds the
/// active code hash and any armed swap.
///
/// The lifecycle half is BEST EFFORT on purpose — the daemon's default module
/// set has no `lifecycle`, and a network without one still has a real,
/// complete registered set to show.
pub async fn load_modules(rpc: String) -> Result<ModulesData, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status_json().await?;
        let code = module_code_by_id(&client).await;
        let rows = status["modules"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|module| {
                let id = module["id"].as_str().unwrap_or_default().to_string();
                let lifecycle = code.get(&id);
                let pending =
                    lifecycle.map_or(serde_json::Value::Null, |entry| entry["pending"].clone());
                ModuleRow {
                    category: module["category"].as_str().unwrap_or_default().to_string(),
                    root: short_digest(module["root"].as_str().unwrap_or_default()),
                    code_hash: lifecycle
                        .map(|entry| {
                            short_digest(&hex_encode(&json_bytes(&entry["active_code_hash"])))
                        })
                        .unwrap_or_default(),
                    pending_hash: short_digest(&hex_encode(&json_bytes(&pending["code_hash"]))),
                    activation_height: pending["activation_height"].as_i64().unwrap_or(0),
                    readiness: count_i64(
                        pending["readiness"]
                            .as_array()
                            .map_or(0, |signals| signals.len()),
                    ),
                    ready: pending["ready"].as_bool().unwrap_or(false),
                    id,
                }
            })
            .collect();
        Ok(ModulesData { rows })
    }
    .await
    .map_err(app_error)
}

/// `LifecycleQuery::ModuleStatus` keyed by module id, empty when this network
/// runs no lifecycle module.
async fn module_code_by_id(client: &RpcClient) -> BTreeMap<String, serde_json::Value> {
    let Ok(reply) = client
        .query::<_, serde_json::Value>("lifecycle", &serde_json::json!("module_status"))
        .await
    else {
        return BTreeMap::new();
    };
    reply["module_status"]["modules"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let id = entry["module_id"].as_str()?.to_string();
            Some((id, entry))
        })
        .collect()
}

/// One registered agent, rendered from its registry record and live-run fact.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub initials: String,
    pub capability: String,
    pub status: String,
    /// the external key shortened for display, else the origin's variant tag.
    pub owner_handle: String,
    /// this agent holds a RUN in flight right now — the runs module's pending
    /// register, NOT `status`. `AgentStatus` is only Active|Paused and Active
    /// is the registration default, so it says "not paused", never "working".
    pub live: bool,
    pub skill_count: i64,
    pub cap_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentsData {
    pub generation: i64,
    pub agents: Vec<AgentRow>,
}

/// The owner origin rendered as a handle. An external origin carries raw key
/// bytes; a module/system origin reads as its own name.
fn agent_owner_handle(owner: &serde_json::Value) -> String {
    let Some(tagged) = owner.as_object() else {
        return owner.as_str().unwrap_or_default().to_string();
    };
    let Some((variant, payload)) = tagged.iter().next() else {
        return String::new();
    };
    if variant != "external" {
        return payload.as_str().unwrap_or(variant.as_str()).to_string();
    }
    short_label(&hex_encode(&json_bytes(payload)))
}

/// Load the agent roster from the canonical registry.
pub async fn load_agents(rpc: String, generation: i64) -> Result<AgentsData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let reply: serde_json::Value = client.query("agent", &serde_json::json!("agents")).await?;
        let working = agents_with_a_run_in_flight(&client).await;
        let agents = reply["agents"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|record| {
                let status = tagged_name(&record["status"]);
                let owner_handle = agent_owner_handle(&record["owner"]);
                let name = record["display_name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let caps = &record["caps"];
                let has_subagent_grant = caps["subagent_budget"].as_i64().unwrap_or(0) > 0;
                let cap_count = [
                    "forge_read",
                    "forge_push",
                    "duckfs_read",
                    "duckfs_write",
                    "tools",
                    "secrets",
                    "pages_write",
                ]
                .into_iter()
                .map(|field| caps[field].as_array().map_or(0, Vec::len))
                .sum::<usize>()
                    + usize::from(has_subagent_grant);
                let id = record["agent_id"].as_str().unwrap_or_default().to_string();
                AgentRow {
                    live: working.contains(&id),
                    initials: initials_of(&name),
                    capability: record["capability"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    skill_count: count_i64(record["skills"].as_array().map_or(0, Vec::len)),
                    cap_count: count_i64(cap_count),
                    id,
                    name,
                    status,
                    owner_handle,
                }
            })
            .collect();
        Ok(AgentsData { generation, agents })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The agents holding a run in flight, from the runs module's pending
/// register — the ONLY place in the product that knows an agent is working.
/// A node that cannot answer the query reports nobody working, never everybody.
async fn agents_with_a_run_in_flight(rpc: &RpcClient) -> BTreeSet<String> {
    let Ok(reply) = rpc
        .query::<_, serde_json::Value>("runs", &serde_json::json!("pending_runs"))
        .await
    else {
        return BTreeSet::new();
    };
    let Some(pending) = reply["pending_runs"].as_array() else {
        return BTreeSet::new();
    };
    pending
        .iter()
        .filter_map(|run| run["agent_id"].as_str().map(str::to_string))
        .collect()
}

/// Whether any agent is engaging work right now — the rail's Forge pulse dot.
pub fn any_agent_active(rows: &[AgentRow]) -> bool {
    rows.iter().any(|row| row.live)
}

/// One agent run indexed by workspace search.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct RunRow {
    pub run_id: String,
    pub agent_id: String,
    pub outcome: String,
    /// A consensus counter (the creation block), NOT a unix stamp — render it
    /// with `height_ago`/`height_label_short`, never with `relative_time`.
    pub created_at: i64,
}

/// Pending runs first, then the delivered ring newest-first. Two queries because
/// the runs module keeps in-flight correlation and settled history separate.
pub async fn load_agent_runs(rpc: String) -> Result<Vec<RunRow>, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        // Two independent reads of one module, awaited one after the other. On a
        // cold `runs` module the first touch measured 54 s on this box, so the
        // serial pair was two ceilings deep for no ordering reason.
        let ask_pending = serde_json::json!("pending_runs");
        let ask_recent = serde_json::json!("recent_runs");
        let (pending, recent) = tokio::join!(
            client.query::<_, serde_json::Value>("runs", &ask_pending),
            client.query::<_, serde_json::Value>("runs", &ask_recent),
        );
        let pending = pending?;
        let recent = recent?;
        let mut runs: Vec<RunRow> = pending["pending_runs"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|record| RunRow {
                run_id: record["run_id"].as_str().unwrap_or_default().to_string(),
                agent_id: record["agent_id"].as_str().unwrap_or_default().to_string(),
                outcome: "running".into(),
                created_at: record["created_at"].as_i64().unwrap_or(0),
            })
            .collect();
        runs.extend(
            recent["recent_runs"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|record| {
                    let outcome = tagged_name(&record["outcome"]);
                    RunRow {
                        run_id: record["run_id"].as_str().unwrap_or_default().to_string(),
                        agent_id: record["agent_id"].as_str().unwrap_or_default().to_string(),
                        created_at: record["created_at"].as_i64().unwrap_or(0),
                        outcome,
                    }
                }),
        );
        Ok(runs)
    }
    .await
    .map_err(app_error)
}

/// Pause or resume one agent — owner-gated at the module, not quorum-gated.
pub async fn set_agent_status(
    rpc: String,
    password: String,
    agent_id: String,
    paused: bool,
) -> Result<bool, AppError> {
    async {
        let agent_id = required_id(agent_id, "agent")?;
        let rpc = rpc_client(&rpc)?;
        // `AgentMsg` is snake_case-tagged serde over `sdk::wire` (plain JSON);
        // the app does not depend on the agent crate, so the two owner-gated
        // verbs are written as their wire form.
        let verb = match paused {
            true => "pause_agent",
            false => "resume_agent",
        };
        let payload = serde_json::json!({ verb: { "agent_id": agent_id } });
        signed_write(&rpc, "agent", encode_wire(&payload), password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The local account picture: whether the local user key belongs to an
/// account, and that account's public face. `number` is the decimal account
/// number — "" when there is none.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AccountData {
    pub generation: i64,
    pub exists: bool,
    pub number: String,
    pub name: String,
    pub bio: String,
    pub keys: i64,
}

impl AccountData {
    fn none(generation: i64) -> Self {
        Self {
            generation,
            exists: false,
            number: String::new(),
            name: String::new(),
            bio: String::new(),
            keys: 0,
        }
    }
}

/// Load the account the local user key belongs to (via the canonical
/// resolver, `OfKey`). A device with no user key has no account to load.
pub async fn load_account(rpc: String, generation: i64) -> Result<AccountData, HydrationError> {
    async {
        let Some(key) = local_user_key().await else {
            return Ok(AccountData::none(generation));
        };
        let client = rpc_client(&rpc)?;
        let reply: identity::IdentityReply = client
            .query("identity", &identity::IdentityQuery::OfKey { key })
            .await?;
        let account = match reply {
            identity::IdentityReply::Account(account) => account,
            identity::IdentityReply::Accounts(_) | identity::IdentityReply::Gen(_) => {
                return Err("the identity module returned the wrong reply".to_string());
            }
        };
        let Some(account) = account else {
            return Ok(AccountData::none(generation));
        };
        Ok(AccountData {
            generation,
            exists: true,
            number: account.number.to_string(),
            name: account.name,
            bio: account.bio.unwrap_or_default(),
            keys: count_i64(account.keys.len()),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Rename the account the local user key belongs to (origin-gated: any member
/// key is the authority).
pub async fn set_account_name(
    rpc: String,
    password: String,
    name: String,
) -> Result<bool, AppError> {
    async {
        let name = bounded_text(name, "account name", identity::MAX_NAME_LEN)?;
        let client = rpc_client(&rpc)?;
        signed_write(
            &client,
            "identity",
            identity::encode_msg(&identity::IdentityMsg::SetName { name }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}
