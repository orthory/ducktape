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
        // the launch window's `user_key_state` reading, on the same file: one
        // classifier, so Settings and the wallet list cannot disagree about it.
        let (key_path, key_state) = match user_key_path() {
            Err(_) => ("(unset)".to_string(), "unlocatable".to_string()),
            Ok(path) => {
                let state = keystore::userkey::key_file_state(&path);
                (path.display().to_string(), state.as_str().to_string())
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
    /// The chain id every chain-scoped user proof (an `AddKey` consent) is
    /// minted for; "" on a daemon that serves no chain.
    pub chain_id: String,
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
            chain_id: String::new(),
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
        chain_id: status["chain_id"].as_str().unwrap_or_default().to_string(),
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
    /// no modules registry (the daemon's default set does not).
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
/// every module, and the modules registry (where a network runs one) adds the
/// active code hash and any armed swap.
///
/// The registry half is BEST EFFORT on purpose — the daemon's default module
/// set has no `modules`, and a network without one still has a real,
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
                let registry = code.get(&id);
                let pending =
                    registry.map_or(serde_json::Value::Null, |entry| entry["pending"].clone());
                ModuleRow {
                    category: module["category"].as_str().unwrap_or_default().to_string(),
                    root: short_digest(module["root"].as_str().unwrap_or_default()),
                    code_hash: registry
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
                    ready: pending_is_ready(&pending),
                    id,
                }
            })
            .collect();
        Ok(ModulesData { rows })
    }
    .await
    .map_err(app_error)
}

/// whether a `ScheduledSwap`'s readiness latch has closed: `ready_at` is the
/// block it closed in, `null` until then (and the whole `pending` is `null`
/// when nothing is scheduled).
fn pending_is_ready(pending: &serde_json::Value) -> bool {
    !pending["ready_at"].is_null()
}

#[cfg(test)]
mod module_row_tests {
    use super::pending_is_ready;

    /// the Modules row's readiness flag keys on `ScheduledSwap.ready_at` —
    /// the block the latch closed in, `null` until then. the literal is the
    /// real `modules::interface::{ModuleCode, ScheduledSwap}` serde field
    /// set (both `deny_unknown_fields`); this crate cannot decode the typed
    /// struct (no `modules` dependency), so the field names are pinned here.
    #[test]
    fn a_pending_swap_is_ready_once_ready_at_is_set() {
        let entry = |ready_at: serde_json::Value| {
            serde_json::json!({
                "module_id": "x",
                "active_code_hash": [],
                "history": [],
                "pending": {
                    "name": "n",
                    "activation_height": 9,
                    "code_hash": [],
                    "readiness": [],
                    "ready_at": ready_at,
                }
            })
        };
        assert!(pending_is_ready(&entry(serde_json::json!(6))["pending"]));
        assert!(!pending_is_ready(
            &entry(serde_json::Value::Null)["pending"]
        ));
        // nothing scheduled: the whole `pending` is null.
        assert!(!pending_is_ready(&serde_json::Value::Null));
    }
}

/// `ModulesQuery::ModuleStatus` keyed by module id, empty when this network
/// runs no modules registry.
async fn module_code_by_id(client: &RpcClient) -> BTreeMap<String, serde_json::Value> {
    let Ok(reply) = client
        .query::<_, serde_json::Value>("modules", &serde_json::json!("module_status"))
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
    pub key_rows: Vec<AccountKeyRow>,
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
            key_rows: Vec::new(),
        }
    }
}

/// One key association as the settings card lists it: the scheme token the
/// CLI prints, the hex key, the label ("" when none) and the admission time.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AccountKeyRow {
    pub scheme: String,
    pub pubkey: String,
    pub label: String,
    pub added_at: i64,
}

fn key_row(key: identity::KeyView) -> AccountKeyRow {
    AccountKeyRow {
        scheme: scheme_token(key.scheme).to_string(),
        pubkey: hex_encode(&key.pubkey),
        label: key.label.unwrap_or_default(),
        added_at: i64::try_from(key.added_at).unwrap_or(i64::MAX),
    }
}

fn scheme_token(scheme: identity::KeyScheme) -> &'static str {
    match scheme {
        identity::KeyScheme::Ed25519 => "ed25519",
        identity::KeyScheme::Secp256k1 => "secp256k1",
        identity::KeyScheme::Secp256r1 => "secp256r1",
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
            key_rows: account.keys.into_iter().map(key_row).collect(),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The chain a network names, read once off `/v1/status` — the welcome step
/// runs before the console's status stream exists, and every key consent is
/// chain-scoped.
pub async fn chain_id_of(rpc: String) -> Result<String, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let status = client.status_json().await?;
        named_chain(node_facts(&status).chain_id)
    }
    .await
    .map_err(app_error)
}

/// Test seam: Ice reads extern structs but cannot construct one.
pub fn account_data_none(generation: i64) -> AccountData {
    AccountData::none(generation)
}

/// A network pick's gate: no password means a read-only session with no key
/// to probe an account for — the console opens outright.
pub fn pick_gate(password: &str) -> crate::PickGate {
    match password.is_empty() {
        true => crate::PickGate::ReadOnly,
        false => crate::PickGate::Probe,
    }
}

/// The probe's answer as the discriminant the launch window branches on.
pub fn account_probe(found: bool) -> crate::AccountProbe {
    match found {
        true => crate::AccountProbe::Found,
        false => crate::AccountProbe::Missing,
    }
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

/// Found an account with this device's key as its first member. The frame
/// signature is the key's possession proof; the name is display-only.
pub async fn create_account(rpc: String, password: String, name: String) -> Result<bool, AppError> {
    async {
        let name = bounded_text(name, "account name", identity::MAX_NAME_LEN)?;
        let client = rpc_client(&rpc)?;
        signed_write(
            &client,
            "identity",
            identity::encode_msg(&identity::IdentityMsg::Create {
                name,
                scheme: identity::KeyScheme::Ed25519,
            }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Mint the `AddKey` ticket that admits another device's pasted ed25519 key
/// to this device's account: this device (a member) consents to the key at
/// its CURRENT generation on `chain_id`, and the other device submits the
/// ticket verbatim ([`join_with_ticket`], or `ducktape account key join`).
/// The consent is single-use — the module advances the generation on
/// admission.
pub async fn mint_key_ticket(
    rpc: String,
    password: String,
    chain_id: String,
    pubkey: String,
    label: String,
) -> Result<String, AppError> {
    async {
        let chain_id = named_chain(chain_id)?;
        let new_key = hex_decode(pubkey.trim())?;
        let wellformed = identity::KeyScheme::Ed25519.pubkey_wellformed(&new_key);
        if !wellformed {
            return Err("that is not a well-formed ed25519 public key".to_string());
        }
        let label = optional_label(label)?;
        let client = rpc_client(&rpc)?;
        let msg = consented_add_key(
            &client,
            password,
            &chain_id,
            identity::KeyScheme::Ed25519,
            &new_key,
            label,
        )
        .await?;
        Ok(add_key_ticket(&msg))
    }
    .await
    .map_err(app_error)
}

/// The ticket text: ONE json line, exactly the `AddKey` payload the joining
/// key signs into its frame.
fn add_key_ticket(msg: &identity::IdentityMsg) -> String {
    String::from_utf8(identity::encode_msg(msg)).expect("json is utf-8")
}

/// An `AddKey` is chain-scoped; a node that has not named its chain yet
/// cannot be consented on.
fn named_chain(chain_id: String) -> Result<String, String> {
    if chain_id.is_empty() {
        return Err(
            "the connected node has not named its chain yet — a key consent is chain-scoped"
                .to_string(),
        );
    }
    Ok(chain_id)
}

fn optional_label(label: String) -> Result<Option<String>, String> {
    match label.trim() {
        "" => Ok(None),
        text => Ok(Some(bounded_text(
            text.to_string(),
            "key label",
            identity::MAX_LABEL_LEN,
        )?)),
    }
}

/// A key's current generation — what a consent signs, so it is single-use.
async fn key_generation(client: &RpcClient, key: &[u8]) -> Result<u64, String> {
    let reply: identity::IdentityReply = client
        .query(
            "identity",
            &identity::IdentityQuery::KeyGen { key: key.to_vec() },
        )
        .await?;
    match reply {
        identity::IdentityReply::Gen(generation) => Ok(generation),
        identity::IdentityReply::Account(_) | identity::IdentityReply::Accounts(_) => {
            Err("the identity module returned the wrong reply".to_string())
        }
    }
}

/// How long a consent this app mints stays spendable, in blocks —
/// `consensus_time` is a block height and a validator network heartbeats about
/// once a second, so this is roughly a day. There is no revoke op: this window
/// IS how a mis-issued ticket dies.
const CONSENT_TTL: u64 = 86_400;

/// The `AddKey` this device consents to for `new_key` (of `scheme`) at its
/// current generation, into THIS device's account, spendable for
/// [`CONSENT_TTL`] blocks.
async fn consented_add_key(
    client: &RpcClient,
    password: String,
    chain_id: &str,
    scheme: identity::KeyScheme,
    new_key: &[u8],
    label: Option<String>,
) -> Result<identity::IdentityMsg, String> {
    let generation = key_generation(client, new_key).await?;
    let account = own_account(client).await?.number;
    let expires_at = consent_expiry(client).await?;
    let authorizer = sign_add_key_consent(
        password, chain_id, scheme, new_key, generation, account, expires_at,
    )
    .await?;
    Ok(identity::IdentityMsg::AddKey {
        scheme,
        label,
        authorizer,
    })
}

/// The `expires_at` a consent minted right now carries.
async fn consent_expiry(client: &RpcClient) -> Result<u64, String> {
    Ok(client
        .status()
        .await
        .map_err(|error| error.to_string())?
        .height
        + CONSENT_TTL)
}

/// The account this device's key belongs to, by the canonical resolver.
async fn own_account(client: &RpcClient) -> Result<identity::AccountView, String> {
    let Some(key) = local_user_key().await else {
        return Err("this device has no user key".to_string());
    };
    account_reply(
        client
            .query("identity", &identity::IdentityQuery::OfKey { key })
            .await?,
    )?
    .ok_or_else(|| "this device's key belongs to no account yet".to_string())
}

fn account_reply(reply: identity::IdentityReply) -> Result<Option<identity::AccountView>, String> {
    match reply {
        identity::IdentityReply::Account(account) => Ok(account),
        identity::IdentityReply::Accounts(_) | identity::IdentityReply::Gen(_) => {
            Err("the identity module returned the wrong reply".to_string())
        }
    }
}

fn identity_msg(msg: &identity::IdentityMsg) -> sdk::Msg {
    sdk::Msg {
        target: "identity".into(),
        payload: identity::encode_msg(msg),
    }
}

// ============================================================================
// browser ceremonies (`authpage`)
// ============================================================================

/// How long a browser touch may take before the app gives up on it.
const CEREMONY_TIMEOUT: Duration = Duration::from_secs(300);

/// One browser round trip, off the async runtime (the callback listener is a
/// blocking accept): open the page, block for its result. On timeout the
/// listener is poked with an abandon result so its thread ends too.
async fn browser_ceremony(request: authpage::Request) -> Result<authpage::Outcome, String> {
    let listener = authpage::Listener::bind().map_err(|e| format!("auth callback: {e}"))?;
    let callback = listener.callback_url();
    let url = authpage::request_url(authpage::AUTH_PAGE, &request, &callback);
    let op = request_op(&request);
    let opened = authpage::open_browser(&url);
    if !opened {
        tracing::warn!(target: "ducktape::auth", event = "ceremony_failed", surface = "browser", op, reason = "no_browser_opener");
        return Err("no browser opener on this machine (xdg-open / open)".to_string());
    }
    tracing::info!(target: "ducktape::auth", event = "ceremony_shown", surface = "browser", op);
    let waiting = tokio::task::spawn_blocking(move || listener.wait());
    let answered = tokio::time::timeout(CEREMONY_TIMEOUT, waiting).await;
    let outcome = match answered {
        Ok(joined) => joined.map_err(|_| "the browser ceremony did not finish".to_string())?,
        Err(_elapsed) => {
            authpage::abandon(&callback, "no answer from the browser");
            Err("the browser did not answer in time".to_string())
        }
    };
    match &outcome {
        Ok(_) => {
            tracing::info!(target: "ducktape::auth", event = "ceremony_answered", surface = "browser", op)
        }
        Err(reason) => {
            tracing::warn!(target: "ducktape::auth", event = "ceremony_failed", surface = "browser", op, reason)
        }
    }
    outcome
}

/// Register a NEW passkey on this device's account: ceremony 1 creates it
/// (the page hands back its key), this device consents, ceremony 2 has the
/// passkey sign its own `AddKey` frame — possession proven by the assertion.
pub async fn register_passkey(
    rpc: String,
    password: String,
    chain_id: String,
    label: String,
) -> Result<bool, AppError> {
    async {
        let chain_id = named_chain(chain_id)?;
        let label = optional_label(label)?;
        require_password(&password)?;
        let client = rpc_client(&rpc)?;
        let account = own_account(&client).await?;
        let registered = browser_ceremony(authpage::Request::Create {
            challenge: authpage::create_challenge(),
            user: account.number,
            name: account.name,
        })
        .await?;
        let authpage::Outcome::Create { public_key, .. } = registered else {
            return Err("expected a passkey registration".to_string());
        };
        let msg = consented_add_key(
            &client,
            password,
            &chain_id,
            identity::KeyScheme::Secp256r1,
            &public_key,
            label,
        )
        .await?;
        let (request, preimage) =
            authpage::passkey_frame_request(&public_key, next_sequence(), &identity_msg(&msg));
        let signed = browser_ceremony(request).await?;
        submit_raw_frame(
            &client,
            "identity",
            authpage::passkey_frame(preimage, &signed)?,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Link an Ethereum wallet to this device's account: touch 1 reveals its
/// key, this device consents, touch 2 has the wallet sign its own `AddKey`
/// frame.
pub async fn link_wallet(
    rpc: String,
    password: String,
    chain_id: String,
    label: String,
) -> Result<bool, AppError> {
    async {
        let chain_id = named_chain(chain_id)?;
        let label = optional_label(label)?;
        require_password(&password)?;
        let client = rpc_client(&rpc)?;
        own_account(&client).await?;
        let reveal = authpage::reveal_message();
        let touch = browser_ceremony(authpage::Request::Eth {
            message: reveal.clone(),
        })
        .await?;
        let pubkey = authpage::wallet_pubkey(&reveal, &touch)?;
        let msg = consented_add_key(
            &client,
            password,
            &chain_id,
            identity::KeyScheme::Secp256k1,
            &pubkey,
            label,
        )
        .await?;
        let (request, preimage) =
            authpage::wallet_frame_request(&pubkey, next_sequence(), &identity_msg(&msg));
        let touch = browser_ceremony(request).await?;
        submit_raw_frame(
            &client,
            "identity",
            authpage::wallet_frame(preimage, &touch)?,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Admit THIS device into an account by a passkey's consent. TWO browser
/// touches: a consent names the account it admits into, and only the passkey
/// knows which that is — touch 1 asks (`userHandle`), touch 2 is the assertion
/// over this key's `AddKey` preimage for that account. This device signs the
/// frame (the key being admitted).
pub async fn login_with_passkey(
    rpc: String,
    password: String,
    chain_id: String,
    label: String,
) -> Result<bool, AppError> {
    async {
        let chain_id = named_chain(chain_id)?;
        let label = optional_label(label)?;
        require_password(&password)?;
        let Some(device_key) = local_user_key().await else {
            return Err("this device has no user key".to_string());
        };
        let client = rpc_client(&rpc)?;
        let generation = key_generation(&client, &device_key).await?;
        let number =
            authpage::assertion_account(&browser_ceremony(authpage::account_request()).await?)?;
        let account = account_reply(
            client
                .query("identity", &identity::IdentityQuery::Get { number })
                .await?,
        )?
        .ok_or_else(|| format!("the passkey names account {number}, unknown to this node"))?;
        let expires_at = consent_expiry(&client).await?;
        let consent = browser_ceremony(authpage::login_request(
            &chain_id,
            &device_key,
            generation,
            number,
            expires_at,
        ))
        .await?;
        let (_, proof) = authpage::login_consent(&consent)?;
        let msg = authpage::login_add_key(
            &chain_id,
            &device_key,
            generation,
            &account,
            label,
            proof,
            expires_at,
        )?;
        signed_write(&client, "identity", identity::encode_msg(&msg), password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

// ============================================================================
// QR ceremonies — the browser is a phone that scanned the app's screen
// ============================================================================

/// One reading of a ceremony the launch window (or the Settings card) is
/// showing: `show_qr` carries the URL to render, `working` a line of what
/// the app is doing between touches, `done`/`failed` close the stream.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct CeremonyStep {
    pub phase: String,
    pub qr: String,
    pub detail: String,
    /// `show_qr` only: how long the code stays good, `m:ss`, re-sent every
    /// second; empty on every other phase.
    pub left: String,
}

impl CeremonyStep {
    fn working(detail: &str) -> Self {
        Self {
            phase: "working".into(),
            qr: String::new(),
            detail: detail.into(),
            left: String::new(),
        }
    }

    fn show_qr(url: String, detail: &str, left: Duration) -> Self {
        Self {
            phase: "show_qr".into(),
            qr: url,
            detail: detail.into(),
            left: authpage::countdown(left),
        }
    }

    fn done() -> Self {
        Self {
            phase: "done".into(),
            qr: String::new(),
            detail: String::new(),
            left: String::new(),
        }
    }

    fn failed(message: String) -> Self {
        Self {
            phase: "failed".into(),
            qr: String::new(),
            detail: message,
            left: String::new(),
        }
    }
}

/// Test seam: Ice reads extern structs but cannot construct one.
pub fn ceremony_step(phase: String, qr: String, detail: String) -> CeremonyStep {
    CeremonyStep {
        phase,
        qr,
        detail,
        left: String::new(),
    }
}

/// Which welcome door a ceremony came through: a name was typed only on the
/// create path.
pub fn welcome_door(name_draft: &str) -> crate::WelcomeDoor {
    match name_draft.trim().is_empty() {
        true => crate::WelcomeDoor::Login,
        false => crate::WelcomeDoor::Create,
    }
}

/// The step's phase as the discriminant the handlers branch on.
pub fn ceremony_phase(step: &CeremonyStep) -> crate::CeremonyPhase {
    match step.phase.as_str() {
        "show_qr" => crate::CeremonyPhase::ShowQr,
        "working" => crate::CeremonyPhase::Working,
        "done" => crate::CeremonyPhase::Done,
        _ => crate::CeremonyPhase::Failed,
    }
}

type StepSender = iced::futures::channel::mpsc::Sender<CeremonyStep>;

/// Hand one reading to the UI; a closed receiver means the lane was
/// invalidated (a cancel), which ends the ceremony as an error nobody reads.
async fn step(tx: &mut StepSender, step: CeremonyStep) -> Result<(), String> {
    use iced::futures::SinkExt as _;
    tx.send(step)
        .await
        .map_err(|_| "the ceremony was cancelled".to_string())
}

/// The page op a request asks for — the `op` field of its fragment, for logs.
fn request_op(request: &authpage::Request) -> &'static str {
    match request {
        authpage::Request::Create { .. } => "create",
        authpage::Request::Get { .. } => "get",
        authpage::Request::Eth { .. } => "eth",
    }
}

/// One browser ceremony run ON A PHONE: mint a relay slot, hand the URL to
/// the UI as a QR under `detail` (the line the screen shows beside it), then
/// wait for the phone's answer under the same ceiling the desktop path uses.
/// `relay_base` is the auth host (tests point it at a fake).
pub(crate) async fn qr_ceremony(
    relay_base: &str,
    request: authpage::Request,
    detail: &str,
    tx: &mut StepSender,
) -> Result<authpage::Outcome, String> {
    let relay = authpage::Relay::at(relay_base);
    let url = authpage::request_url(authpage::AUTH_PAGE, &request, &relay.callback_url());
    let op = request_op(&request);
    tracing::info!(target: "ducktape::auth", event = "ceremony_shown", surface = "phone", op, relay = %relay.id);
    step(
        tx,
        CeremonyStep::show_qr(url.clone(), detail, CEREMONY_TIMEOUT),
    )
    .await?;
    let started = std::time::Instant::now();
    let waiting = tokio::task::spawn_blocking(move || relay.wait(CEREMONY_TIMEOUT));
    tokio::pin!(waiting);
    // The countdown: the same QR re-sent each second with the time it has
    // left, so the screen can show it. The first tick is a second away —
    // the reading above already carries the full ceiling.
    let second = Duration::from_secs(1);
    let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + second, second);
    let outcome = loop {
        tokio::select! {
            joined = &mut waiting => {
                break joined.map_err(|_| "the ceremony did not finish".to_string())?;
            }
            _ = ticks.tick() => {
                let left = CEREMONY_TIMEOUT.saturating_sub(started.elapsed());
                step(tx, CeremonyStep::show_qr(url.clone(), detail, left)).await?;
            }
        }
    };
    match &outcome {
        Ok(_) => {
            tracing::info!(target: "ducktape::auth", event = "ceremony_answered", surface = "phone", op)
        }
        Err(reason) => {
            tracing::warn!(target: "ducktape::auth", event = "ceremony_failed", surface = "phone", op, reason)
        }
    }
    outcome
}

/// Run `body` as a step stream: every `Err` becomes a `failed` step, `Ok` a
/// `done` one. The body is driven BY the stream's own polls (no spawn, so
/// no runtime handle is assumed), and every reading — the closing one too —
/// travels the one channel, so the UI sees them in order. Dropping the
/// stream (a lane invalidation) drops the body mid-await: the cancel.
fn ceremony_stream<F, Fut>(body: F) -> iced::futures::stream::BoxStream<'static, CeremonyStep>
where
    F: FnOnce(StepSender) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
{
    use iced::futures::{SinkExt as _, StreamExt as _};
    let (tx, rx) = iced::futures::channel::mpsc::channel::<CeremonyStep>(8);
    let mut closing = tx.clone();
    let driving = async move {
        let last = match body(tx).await {
            Ok(()) => {
                tracing::info!(target: "ducktape::auth", event = "ceremony_stream_done");
                CeremonyStep::done()
            }
            Err(message) => {
                tracing::warn!(target: "ducktape::auth", event = "ceremony_stream_failed", reason = %message);
                CeremonyStep::failed(message)
            }
        };
        let _ = closing.send(last).await;
    };
    let driver = iced::futures::stream::once(driving).filter_map(|()| async { None });
    iced::futures::stream::select(rx, driver).boxed()
}

/// Create the account with this device's key (no touch), then register a
/// passkey from the phone: QR 1 creates it, this device consents, QR 2 has
/// the passkey sign its own admission.
pub fn create_account_by_qr(
    rpc: String,
    password: String,
    chain_id: String,
    name: String,
) -> iced::futures::stream::BoxStream<'static, CeremonyStep> {
    ceremony_stream(move |mut tx| async move {
        let chain_id = named_chain(chain_id)?;
        require_password(&password)?;
        step(&mut tx, CeremonyStep::working("Creating the account…")).await?;
        create_account(rpc.clone(), password.clone(), name)
            .await
            .map_err(|e| e.message)?;
        add_passkey_steps(&mut tx, &rpc, password, &chain_id, None).await
    })
}

/// Register a passkey on the account this device's key already belongs to.
pub fn add_passkey_by_qr(
    rpc: String,
    password: String,
    chain_id: String,
    label: String,
) -> iced::futures::stream::BoxStream<'static, CeremonyStep> {
    ceremony_stream(move |mut tx| async move {
        let chain_id = named_chain(chain_id)?;
        let label = optional_label(label)?;
        require_password(&password)?;
        add_passkey_steps(&mut tx, &rpc, password, &chain_id, label).await
    })
}

/// QR 1 (create) → this device consents → QR 2 (the passkey signs its own
/// `AddKey`) → submit. The phone half of `register_passkey`.
async fn add_passkey_steps(
    tx: &mut StepSender,
    rpc: &str,
    password: String,
    chain_id: &str,
    label: Option<String>,
) -> Result<(), String> {
    let client = rpc_client(rpc)?;
    let account = own_account(&client).await?;
    let registered = qr_ceremony(
        authpage::AUTH_PAGE,
        authpage::Request::Create {
            challenge: authpage::create_challenge(),
            user: account.number,
            name: account.name,
        },
        "Scan 1 of 2 — your phone creates the passkey.",
        tx,
    )
    .await?;
    let authpage::Outcome::Create { public_key, .. } = registered else {
        return Err("expected a passkey registration".to_string());
    };
    step(tx, CeremonyStep::working("Consenting to the new key…")).await?;
    let msg = consented_add_key(
        &client,
        password,
        chain_id,
        identity::KeyScheme::Secp256r1,
        &public_key,
        label,
    )
    .await?;
    let (request, preimage) =
        authpage::passkey_frame_request(&public_key, next_sequence(), &identity_msg(&msg));
    let signed = qr_ceremony(
        authpage::AUTH_PAGE,
        request,
        "Scan 2 of 2 — confirm with the passkey you just made.",
        tx,
    )
    .await?;
    step(tx, CeremonyStep::working("Submitting…")).await?;
    submit_raw_frame(
        &client,
        "identity",
        authpage::passkey_frame(preimage, &signed)?,
    )
    .await?;
    Ok(())
}

/// Admit THIS device by a passkey's consent given on the phone: two QRs, one
/// per touch — the first asks the passkey which account it speaks for, the
/// second is the consent, bound to that account. The phone half of
/// `login_with_passkey`.
pub fn login_by_qr(
    rpc: String,
    password: String,
    chain_id: String,
) -> iced::futures::stream::BoxStream<'static, CeremonyStep> {
    ceremony_stream(move |mut tx| async move {
        let chain_id = named_chain(chain_id)?;
        require_password(&password)?;
        let Some(device_key) = local_user_key().await else {
            return Err("this device has no user key".to_string());
        };
        let client = rpc_client(&rpc)?;
        let generation = key_generation(&client, &device_key).await?;
        let named = qr_ceremony(
            authpage::AUTH_PAGE,
            authpage::account_request(),
            "Confirm with the passkey that belongs to your account.",
            &mut tx,
        )
        .await?;
        let number = authpage::assertion_account(&named)?;
        step(&mut tx, CeremonyStep::working("Reading the account…")).await?;
        let account = account_reply(
            client
                .query("identity", &identity::IdentityQuery::Get { number })
                .await?,
        )?
        .ok_or_else(|| format!("the passkey names account {number}, unknown to this node"))?;
        let expires_at = consent_expiry(&client).await?;
        let consent = qr_ceremony(
            authpage::AUTH_PAGE,
            authpage::login_request(&chain_id, &device_key, generation, number, expires_at),
            "Confirm once more to admit this device to the account.",
            &mut tx,
        )
        .await?;
        let (_, proof) = authpage::login_consent(&consent)?;
        step(&mut tx, CeremonyStep::working("Joining the account…")).await?;
        let msg = authpage::login_add_key(
            &chain_id,
            &device_key,
            generation,
            &account,
            None,
            proof,
            expires_at,
        )?;
        signed_write(&client, "identity", identity::encode_msg(&msg), password).await?;
        Ok(())
    })
}

/// A pasted ticket is an `AddKey` or it is refused HERE, before any signature
/// — the module would refuse a stray `SetName` too, but under a name that
/// says nothing about tickets.
fn add_key_ticket_bytes(ticket: &str) -> Result<Vec<u8>, String> {
    let ticket = ticket.trim();
    let is_add_key = matches!(
        identity::decode_msg(ticket.as_bytes())?,
        identity::IdentityMsg::AddKey { .. }
    );
    if !is_add_key {
        return Err(
            "that is not an add-key ticket (mint one on a device that is already a member)"
                .to_string(),
        );
    }
    Ok(ticket.as_bytes().to_vec())
}

/// Join the account a ticket names with THIS device's key: the ticket bytes
/// ride verbatim (the member's consent is over them), signed by the key being
/// admitted.
pub async fn join_with_ticket(
    rpc: String,
    password: String,
    ticket: String,
) -> Result<bool, AppError> {
    async {
        let payload = add_key_ticket_bytes(&ticket)?;
        let client = rpc_client(&rpc)?;
        signed_write(&client, "identity", payload, password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Remove one key from this device's account (member-gated; the module
/// refuses the last key).
pub async fn remove_account_key(
    rpc: String,
    password: String,
    pubkey: String,
) -> Result<bool, AppError> {
    async {
        let key = hex_decode(pubkey.trim())?;
        let client = rpc_client(&rpc)?;
        signed_write(
            &client,
            "identity",
            identity::encode_msg(&identity::IdentityMsg::RemoveKey { key }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

#[cfg(test)]
mod account_ticket_tests {
    use super::*;

    fn member() -> ed25519::PrivateKey {
        ed25519::PrivateKey::from_seed(41)
    }

    /// The ticket the app mints IS the `AddKey` the CLI's `key join` submits:
    /// one line, decodes to the message, and the consent verifies under the
    /// module's own namespace at the minted generation — and at no other.
    #[test]
    fn a_ticket_is_the_add_key_the_cli_accepts() {
        let new_key = ed25519::PrivateKey::from_seed(42)
            .public_key()
            .as_ref()
            .to_vec();
        let authorizer = workspace_config::ed25519_authorizer(
            &member(),
            "chain-a",
            identity::KeyScheme::Ed25519,
            &new_key,
            3,
            11,
            900,
        );
        let ticket = add_key_ticket(&identity::IdentityMsg::AddKey {
            scheme: identity::KeyScheme::Ed25519,
            label: Some("phone".into()),
            authorizer,
        });
        assert_eq!(ticket.lines().count(), 1, "one json line, pasteable");
        let identity::IdentityMsg::AddKey {
            scheme,
            label,
            authorizer,
        } = identity::decode_msg(ticket.as_bytes()).unwrap()
        else {
            panic!("a ticket is an AddKey");
        };
        assert_eq!(scheme, identity::KeyScheme::Ed25519);
        assert_eq!(label.as_deref(), Some("phone"));
        assert_eq!(authorizer.key, member().public_key().as_ref());
        assert_eq!(authorizer.account, 11);
        assert_eq!(authorizer.expires_at, 900);
        let preimage = |generation, account, expires_at| {
            identity::add_key_preimage(
                "chain-a",
                identity::KeyScheme::Ed25519,
                &new_key,
                generation,
                account,
                expires_at,
            )
        };
        let verifies = |generation, account, expires_at| {
            identity::KeyScheme::Ed25519.verify(
                &authorizer.key,
                identity::IDENTITY_ADD_KEY_NS,
                &preimage(generation, account, expires_at),
                &authorizer.proof,
            )
        };
        assert!(verifies(3, 11, 900), "the consent is over the minted terms");
        assert!(!verifies(4, 11, 900), "and is single-use");
        assert!(!verifies(3, 12, 900), "account-bound");
        assert!(!verifies(3, 11, 901), "expiry-bound");
        assert_eq!(
            add_key_ticket_bytes(&format!("  {ticket}\n")).unwrap(),
            ticket.as_bytes(),
            "the joining frame carries the ticket bytes verbatim, whitespace trimmed"
        );
    }

    #[test]
    fn a_non_add_key_ticket_is_refused_before_any_signature() {
        let stray = String::from_utf8(identity::encode_msg(&identity::IdentityMsg::SetName {
            name: "x".into(),
        }))
        .unwrap();
        let err = add_key_ticket_bytes(&stray).unwrap_err();
        assert!(err.contains("not an add-key ticket"), "{err}");
        assert!(add_key_ticket_bytes("not json").is_err());
    }
}

#[cfg(test)]
mod qr_ceremony_tests {
    use super::*;
    use std::io::{BufRead as _, BufReader, Write as _};

    /// a relay that answers 204 `absent` times, then `json` once, then 204.
    fn fake_relay(absent: usize, json: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for (served, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let is_the_answer = served == absent;
                let response = match is_the_answer {
                    true => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{json}",
                        json.len()
                    ),
                    false => "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_string(),
                };
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        base
    }

    const ASSERTION: &str = r#"{"op":"get","credentialId":"AQ","authenticatorData":"AQ","clientDataJSON":"AQ","signature":"AQ","userHandle":"KgAAAAAAAAA"}"#;

    /// the first reading is the QR — the auth page URL carrying this relay's
    /// slot as its callback — and the outcome is the phone's answer.
    #[tokio::test(flavor = "current_thread")]
    async fn a_qr_ceremony_shows_the_url_then_yields_the_outcome() {
        let base = fake_relay(1, ASSERTION);
        let (mut tx, mut rx) = iced::futures::channel::mpsc::channel::<CeremonyStep>(8);
        let outcome = qr_ceremony(
            &base,
            authpage::Request::Get {
                challenge: [7u8; 32],
            },
            "Confirm with the passkey.",
            &mut tx,
        )
        .await
        .unwrap();
        assert!(matches!(
            outcome,
            authpage::Outcome::Get {
                user_handle: Some(42),
                ..
            }
        ));
        let shown = rx.next().await.unwrap();
        assert_eq!(shown.phase, "show_qr");
        assert!(
            shown
                .qr
                .starts_with("https://auth.ducktape.industries/#op=get&challenge="),
            "{}",
            shown.qr
        );
        // the callback is percent-encoded into the fragment: `/r/` survives as %2Fr%2F
        assert!(
            shown.qr.contains("&cb=http%3A%2F%2F127.0.0.1"),
            "{}",
            shown.qr
        );
        assert!(shown.qr.contains("%2Fr%2F"), "{}", shown.qr);
    }

    /// the stream shape: readings in order, and the closing one last.
    #[tokio::test(flavor = "current_thread")]
    async fn a_ceremony_stream_ends_with_its_closing_step_in_order() {
        let steps: Vec<CeremonyStep> = ceremony_stream(|mut tx| async move {
            step(&mut tx, CeremonyStep::working("one")).await?;
            step(&mut tx, CeremonyStep::working("two")).await?;
            Err("boom".to_string())
        })
        .collect()
        .await;
        let phases: Vec<&str> = steps.iter().map(|s| s.phase.as_str()).collect();
        assert_eq!(phases, ["working", "working", "failed"]);
        assert_eq!(steps[1].detail, "two");
        assert_eq!(steps[2].detail, "boom");
        let done: Vec<CeremonyStep> = ceremony_stream(|_tx| async move { Ok(()) }).collect().await;
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].phase, "done");
    }
}
