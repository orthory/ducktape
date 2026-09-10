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
        // the launch window's key-state reading, on the same file: one
        // classifier, so Settings and the wallet list cannot disagree about it.
        let (key_path, key_state) = match session_key_path(&rpc) {
            Err(_) => ("(unset)".to_string(), "unlocatable".to_string()),
            Ok(path) => (path.display().to_string(), key_state_of(&path)),
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

/// The ring as the Node tab's slot paints it — owned, because the slot is a
/// host surface that outlives the call that drew it.
pub fn node_log_timeline(
    state: NodeLogTimelineState,
    source: String,
) -> iced::Element<'static, NodeLogTimelineEvent> {
    use iced::widget::{Space, button, column, container, row, text};
    use iced::{Border, Color, Font, Length};
    use ui_lang_components::ui::log_timeline::{LogTimelineEvent, log_timeline};
    use ui_lang_components::ui::theme::DARK;

    let inspection = state.timeline.inspect(node_log_timeline_config());
    let mono = Font {
        family: iced::font::Family::Name(design::fonts::FAMILY_MONO),
        ..Font::DEFAULT
    };
    // ALWAYS A BUTTON, NEVER A BUTTON-OR-A-TEXT. A `button` carries widget
    // state and a `text` carries none, so alternating the two at one position
    // hands iced a state slot whose type changed under it — `Tree`'s downcast
    // then aborts the process (`iced_core widget/tree.rs`), and this position
    // flips the moment a line arrives while the reader is scrolled back. The
    // resting state is the same button with no `on_press`, which is how iced
    // spells "not pressable", and the label carries the difference.
    let following_tail = inspection.following_tail;
    let tail_label = match following_tail {
        true => "LIVE".to_owned(),
        false => format!("RESUME · {} NEW", inspection.unread_count),
    };
    let tail_color = match following_tail {
        true => DARK.palette.success,
        false => DARK.palette.foreground,
    };
    let tail: iced::Element<'_, NodeLogTimelineEvent> =
        button(text(tail_label).size(10).font(mono).color(tail_color))
            .padding([3, 7])
            .style(move |theme, status| match following_tail {
                // resting: the word IS the status, so it wears no chrome
                true => button::Style {
                    background: None,
                    text_color: DARK.palette.success,
                    ..button::text(theme, status)
                },
                false => button::secondary(theme, status),
            })
            .on_press_maybe((!following_tail).then_some(LogTimelineEvent::ResumeTail))
            .into();
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
    // THE LIST IS ALWAYS MOUNTED, and the empty note rides ON it rather than
    // instead of it. `log_timeline` is a stateful virtual list and the note is
    // a plain container: swapping one for the other at this position is the
    // crash above, and this position swaps the FIRST time a line arrives —
    // which is every visit to this tab. A stack keeps both children present
    // with stable types; the note draws nothing when its text is empty.
    let empty_note = match (state.visible.is_empty(), state.lines.is_empty()) {
        (false, _) => "",
        (true, true) => "Waiting for the node's log ring…",
        (true, false) => "No lines match this filter.",
    };
    let timeline: iced::Element<'static, NodeLogTimelineEvent> = log_timeline(
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
    );
    let body = iced::widget::stack![
        timeline,
        container(
            text(empty_note)
                .size(12)
                .font(mono)
                .color(DARK.palette.muted_foreground),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill),
    ];
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
        true => (
            trim_time_to_millis(first),
            fields.next().unwrap_or_default(),
        ),
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
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
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
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
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

/// One curated skill as the record carries it: a duckfs subtree, pinned at a
/// snapshot or tracking the committed head (an empty `source_snapshot`), and
/// whether its body is the agent's persona (`always`) or read on demand.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentSkill {
    pub name: String,
    pub source_prefix: String,
    pub source_snapshot: String,
    pub always: bool,
}

impl From<runs::SkillRef> for AgentSkill {
    fn from(skill: runs::SkillRef) -> Self {
        Self {
            name: skill.name,
            source_prefix: skill.source_prefix,
            source_snapshot: skill.source_snapshot.unwrap_or_default(),
            always: matches!(skill.load, runs::LoadMode::Always),
        }
    }
}

impl From<AgentSkill> for runs::SkillRef {
    fn from(skill: AgentSkill) -> Self {
        let pinned = !skill.source_snapshot.is_empty();
        Self {
            name: skill.name,
            source_prefix: skill.source_prefix,
            source_snapshot: pinned.then_some(skill.source_snapshot),
            load: if skill.always {
                runs::LoadMode::Always
            } else {
                runs::LoadMode::OnDemand
            },
        }
    }
}

/// The resource grant, list by list: the record's caps as the register shows
/// them and the editor hands them back.
#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentCaps {
    pub forge_read: Vec<String>,
    pub forge_push: Vec<String>,
    pub duckfs_read: Vec<String>,
    pub duckfs_write: Vec<String>,
    pub tools: Vec<String>,
    pub secrets: Vec<String>,
    pub pages_write: Vec<String>,
    pub subagent_budget: i64,
}

impl From<runs::ResourceCaps> for AgentCaps {
    fn from(caps: runs::ResourceCaps) -> Self {
        Self {
            forge_read: caps.forge_read,
            forge_push: caps.forge_push,
            duckfs_read: caps.duckfs_read,
            duckfs_write: caps.duckfs_write,
            tools: caps.tools,
            secrets: caps.secrets,
            pages_write: caps.pages_write,
            subagent_budget: i64::from(caps.subagent_budget),
        }
    }
}

impl AgentCaps {
    fn into_resource_caps(self) -> Result<runs::ResourceCaps, String> {
        let subagent_budget = u32::try_from(self.subagent_budget)
            .map_err(|_| "the subagent budget must be a whole number of calls".to_string())?;
        Ok(runs::ResourceCaps {
            forge_read: self.forge_read,
            forge_push: self.forge_push,
            duckfs_read: self.duckfs_read,
            duckfs_write: self.duckfs_write,
            tools: self.tools,
            secrets: self.secrets,
            pages_write: self.pages_write,
            subagent_budget,
        })
    }
}

/// One configured model: its record, whole, with its live-run fact.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub initials: String,
    pub capability: String,
    pub status: String,
    /// The current controller of the model's programmable account, by name.
    pub owner_handle: String,
    /// That controller's account number, decimal: the one principal whose
    /// signature may change this record.
    pub controller: String,
    /// this agent holds a RUN in flight right now — the runs module's pending
    /// register, NOT `status`. `ModelStatus` is only Active|Paused and Active
    /// is the registration default, so it says "not paused", never "working".
    pub live: bool,
    pub allowed_actions: Vec<String>,
    pub caps: AgentCaps,
    pub skills: Vec<AgentSkill>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentsData {
    pub generation: i64,
    pub agents: Vec<AgentRow>,
    /// every run the journal lists, newest dispatch first — the tracker
    pub runs: Vec<RunRow>,
    /// Every capability tag a node on this network announces — what a
    /// record's `capability` can be dispatched on today.
    pub capabilities: Vec<String>,
    /// The action vocabulary a grant draws from.
    pub actions: Vec<String>,
}

/// Load model configurations with current account controllers and run activity.
/// The model's registration origin does not change when control transfers.
pub async fn load_agents(rpc: String, generation: i64) -> Result<AgentsData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let reply: runs::RunsReply = client
            .query(
                "runs",
                &runs::RunsQuery::Model {
                    query: runs::ModelQuery::Agents,
                },
            )
            .await?;
        let runs::RunsReply::Model(runs::ModelReply::Agents(records)) = reply else {
            return Err("the runs module returned the wrong model roster reply".into());
        };
        let (accounts, working, capabilities, recent) = tokio::join!(
            read_accounts(&client),
            agents_with_a_run_in_flight(&client),
            announced_capabilities(&client),
            recent_runs(&client)
        );
        let controllers: BTreeMap<u64, u64> = accounts?
            .into_iter()
            .filter_map(|account| match account.control {
                identity::Control::Program { controller, .. }
                | identity::Control::Revoked { controller } => Some((account.number, controller)),
                identity::Control::Keys => None,
            })
            .collect();
        let names = names();
        let agents = records
            .into_iter()
            .map(|record| {
                let status = match record.status {
                    runs::ModelStatus::Active => "active",
                    runs::ModelStatus::Paused => "paused",
                }
                .to_string();
                let controller = controllers
                    .get(&record.account)
                    .ok_or_else(|| "the model account has no program controller".to_string())?;
                let owner_handle = author_display(&format!("acct:{controller}"), &names);
                Ok(AgentRow {
                    live: working.contains(&record.agent_id),
                    initials: initials_of(&record.display_name),
                    capability: record.capability,
                    id: record.agent_id,
                    name: record.display_name,
                    status,
                    owner_handle,
                    controller: controller.to_string(),
                    allowed_actions: record.allowed_actions,
                    caps: record.caps.into(),
                    skills: record.skills.into_iter().map(AgentSkill::from).collect(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        // the tracker names agents the way the register does. A node whose
        // runs journal cannot answer (no mapper installed, a fold still
        // catching up) lists no runs rather than losing the register.
        let names_by_id: BTreeMap<String, String> = agents
            .iter()
            .map(|agent| (agent.id.clone(), agent.name.clone()))
            .collect();
        let runs = recent
            .unwrap_or_default()
            .into_iter()
            .map(|run| run_row(run, &names_by_id))
            .collect();
        Ok(AgentsData {
            generation,
            agents,
            runs,
            capabilities,
            actions: runs::KNOWN_ACTIONS
                .iter()
                .map(|action| (*action).to_string())
                .collect(),
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Every capability tag some node announces, sorted and deduped — the
/// executors a record can name and be dispatched on. A node that cannot
/// answer the registry offers none, never a guess.
async fn announced_capabilities(rpc: &RpcClient) -> Vec<String> {
    let Ok(capability::CapabilityReply::All(registry)) = rpc
        .query::<_, capability::CapabilityReply>("capability", &capability::CapabilityQuery::All)
        .await
    else {
        return Vec::new();
    };
    registry
        .into_iter()
        .flat_map(|(_, tags)| tags)
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
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

/// One run of an agent — a dispatch and what became of it — read off the
/// runs journal (`runs::index`), every stamp rendered for the register the
/// Agents view draws: the view owns no clock and no chain height.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
pub struct RunRow {
    pub run_id: String,
    /// the run's address: what a `duck://run/` link names and what the
    /// journal read is keyed by
    pub dispatch_id: String,
    pub agent_id: String,
    pub agent_name: String,
    /// what the run answers: a channel message, a job, or a calling run
    pub origin: String,
    /// `dispatched`, `running`, `accepted`, `rejected` or `failed`
    pub state: String,
    /// the dispatch height, rendered — a consensus counter, never a clock
    pub dispatched: String,
    /// the settlement height, rendered; "" while the run is in flight
    pub settled: String,
    pub attempt: i64,
    /// the executing node's key, abbreviated; "" before a session opened
    pub holder: String,
    pub actions: i64,
    pub degraded: bool,
    /// the failure excerpt of a failed run
    pub reason: String,
    pub output_ref: String,
    /// 0 when the run opened no PR
    pub pr_number: i64,
}

/// One journal entry of the run the reader opened.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
pub struct JournalEntry {
    /// the commit height, rendered
    pub height: String,
    pub kind: String,
    pub summary: String,
}

/// One place a run touched, as the chip the run panel draws: where it was
/// called from (`relation` "from") or what its receipts landed on ("touched").
/// `url` is the duck:// address the open plane warps to; "" for a place the
/// protocol has no address for yet, which draws as a label alone.
#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize)]
pub struct RunLink {
    pub relation: String,
    /// `chat`, `page`, `forge`, `file`, `task`, `job`, `module`,
    /// `conversation`, `run` or `output` — the glyph the chip wears
    pub kind: String,
    pub label: String,
    pub url: String,
}

/// A chip's label is one line: the text's words, single-spaced. How much of
/// it a chip shows is the view's call, not a count picked here.
fn chip_label(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A page block as a chip names it: the page's opening line, then the
/// block's own when the block is not the page itself. The block's page is
/// what the link opens, the block its anchor.
async fn page_block_link(
    client: &RpcClient,
    chain: &str,
    block_id: &str,
) -> Result<Option<(String, String)>, String> {
    let Some(block) = view_block(client, block_id).await? else {
        return Ok(None);
    };
    let is_page = block.page_id == block.block_id;
    let label = match is_page {
        true => chip_label(&block.text),
        false => match view_block(client, &block.page_id).await? {
            Some(root) => chip_label(&format!("{} · {}", root.text, block.text)),
            None => chip_label(&block.text),
        },
    };
    Ok(Some((
        label,
        duck_page_block_link(block.page_id, block.block_id, chain.to_owned()),
    )))
}

/// One block off pages' view lane; `None` for an id the index does not hold.
async fn view_block(
    client: &RpcClient,
    block_id: &str,
) -> Result<Option<::pages::index::PageBlockRow>, String> {
    let reply: ::pages::index::PagesViewReply = client
        .view(
            "pages",
            &::pages::index::PagesViewQuery::GetBlock {
                block_id: block_id.to_owned(),
            },
        )
        .await?;
    match reply {
        ::pages::index::PagesViewReply::Block(block) => Ok(block),
        _ => Err("the pages index returned the wrong reply to a block read".into()),
    }
}

/// The block a comment thread is anchored to, off pages' view lane; `None`
/// for a thread the index does not hold.
async fn view_thread_target(client: &RpcClient, thread_id: &str) -> Result<Option<String>, String> {
    let reply: ::pages::index::PagesViewReply = client
        .view(
            "pages",
            &::pages::index::PagesViewQuery::GetThread {
                thread_id: thread_id.to_owned(),
            },
        )
        .await?;
    match reply {
        ::pages::index::PagesViewReply::Thread(thread) => Ok(thread.map(|thread| thread.target)),
        _ => Err("the pages index returned the wrong reply to a thread read".into()),
    }
}

/// The chip for one place. Chat, forge, page and run places carry an
/// address; the rest name what they are until the protocol addresses them.
/// A page place whose block the index no longer holds, or whose read failed,
/// names its id and carries no address: the run's journal still opens, one
/// chip short of a link.
async fn run_link(
    client: &RpcClient,
    chain: &str,
    relation: &str,
    place: runs::index::RunPlace,
) -> RunLink {
    use runs::index::RunPlace;
    let link = |kind: &str, label: String, url: String| RunLink {
        relation: relation.to_owned(),
        kind: kind.to_owned(),
        label,
        url,
    };
    match place {
        RunPlace::ChatMessage { channel_id, seq } => link(
            "chat",
            format!("#{channel_id} · msg {seq}"),
            duck_channel_message_link(channel_id, height_i64(seq), chain.to_owned()),
        ),
        RunPlace::Channel { channel_id } => link(
            "chat",
            format!("#{channel_id}"),
            duck_channel_link(channel_id, chain.to_owned()),
        ),
        RunPlace::PageBlock { block_id } => match page_block_link(client, chain, &block_id).await {
            Ok(Some((label, url))) => link("page", label, url),
            _ => link("page", format!("block {block_id}"), String::new()),
        },
        RunPlace::PageThread { thread_id } => {
            let resolved = match view_thread_target(client, &thread_id).await {
                Ok(Some(target)) => page_block_link(client, chain, &target).await,
                _ => Ok(None),
            };
            match resolved {
                Ok(Some((label, url))) => link("page", label, url),
                _ => link("page", format!("thread {thread_id}"), String::new()),
            }
        }
        RunPlace::Page { page_id, title } => link(
            "page",
            chip_label(&title),
            duck_page_link(page_id, chain.to_owned()),
        ),
        RunPlace::Job { job_id } => link("job", format!("job {job_id}"), String::new()),
        RunPlace::Task { task_id } => link("task", format!("task {task_id}"), String::new()),
        RunPlace::File { path } => link("file", chip_label(&path), String::new()),
        RunPlace::Module { module_id } => {
            link("module", format!("module {module_id}"), String::new())
        }
        RunPlace::Conversation { conversation_id } => link(
            "conversation",
            chip_label(&format!("conversation {conversation_id}")),
            String::new(),
        ),
        RunPlace::Run { dispatch_id } => link(
            "run",
            format!("run {}", short_pubkey(&dispatch_id)),
            duck_run_link(dispatch_id, chain.to_owned()),
        ),
        RunPlace::ForgeItem { repo, number } => link(
            "forge",
            format!("{repo}#{number}"),
            duck_forge_item_link(repo, height_i64(number), chain.to_owned()),
        ),
        RunPlace::Output { output_ref } => link("output", chip_label(&output_ref), String::new()),
    }
}

/// Every chip of one run: its origin first, then each place its receipts
/// touched in journal order.
async fn run_links(client: &RpcClient, chain: &str, run: runs::index::RunView) -> Vec<RunLink> {
    let mut links = Vec::with_capacity(run.places.len() + 1);
    if let Some(origin) = run.origin {
        links.push(run_link(client, chain, "from", origin).await);
    }
    for place in run.places {
        links.push(run_link(client, chain, "touched", place).await);
    }
    links
}

/// The journal of one run, and the SCOPE it was read in.
///
/// The dispatch id alone is not a scope. Two networks can carry the same run,
/// and a reconnect to the same endpoint is a different session — so a read
/// started on network A, answering after the app moved to B with that same run
/// open, would install A's journal under B. The fields below are what make the
/// comparison identify the OPERATION rather than its subject: `link` is the
/// app's `connect_generation`, `account` the seated account, and `op` a fresh
/// per-dispatch nonce.
#[derive(Clone, Debug, Default, Hash, PartialEq, serde::Serialize)]
pub struct RunJournal {
    pub dispatch_id: String,
    pub entries: Vec<JournalEntry>,
    /// the run's origin and every place it touched, as chips
    pub links: Vec<RunLink>,
    pub rpc: String,
    pub network: String,
    pub link: i64,
    pub account: String,
    pub op: i64,
    /// the read's own refusal, as one sentence; "" when it answered
    pub error: String,
}

pub fn empty_run_journal() -> RunJournal {
    RunJournal::default()
}

/// Whether this journal still belongs to what the app is showing.
///
/// Both a success AND a failure meet this fence, which is why
/// [`load_run_journal`] is infallible: an `Err` arm cannot carry the scope it
/// happened in, so a refusal about A would surface — and be attributed —
/// under B.
#[allow(clippy::too_many_arguments)]
pub fn journal_in_scope(
    journal: &RunJournal,
    rpc: &str,
    network: &str,
    link: i64,
    account: &str,
    op: i64,
    dispatch_id: &str,
) -> bool {
    journal.rpc == rpc
        && journal.network == network
        && journal.link == link
        && journal.account == account
        && journal.op == op
        && journal.dispatch_id == dispatch_id
}

/// What a run answers, in the tracker's words.
fn run_origin(
    channel_id: &str,
    anchor_seq: u64,
    job_id: &Option<String>,
    delegation_id: &Option<String>,
) -> String {
    if let Some(job_id) = job_id {
        return format!("job {job_id}");
    }
    if let Some(delegation_id) = delegation_id {
        return format!("called by a run · {delegation_id}");
    }
    format!("#{channel_id} · msg {anchor_seq}")
}

fn outcome_word(outcome: runs::RunOutcome) -> &'static str {
    match outcome {
        runs::RunOutcome::ResultAccepted => "accepted",
        runs::RunOutcome::ActionRejected => "rejected",
        runs::RunOutcome::Failed => "failed",
    }
}

fn height_i64(height: u64) -> i64 {
    i64::try_from(height).unwrap_or(i64::MAX)
}

/// The tracker's row for one journal run; `names` maps agent ids to the
/// display names the register carries, and an agent the register does not
/// list is named by its id.
fn run_row(run: runs::index::RunView, names: &BTreeMap<String, String>) -> RunRow {
    let agent_name = names
        .get(&run.agent_id)
        .cloned()
        .unwrap_or_else(|| run.agent_id.clone());
    let origin = run_origin(
        &run.channel_id,
        run.anchor_seq,
        &run.job_id,
        &run.delegation_id,
    );
    // the only forge item among a run's touched places is the PR its sink
    // opened or updated; the item it was called on is its origin
    let pr_number = run
        .places
        .iter()
        .find_map(|place| match place {
            runs::index::RunPlace::ForgeItem { number, .. } => Some(height_i64(*number)),
            _ => None,
        })
        .unwrap_or(0);
    let mut row = RunRow {
        run_id: run.run_id,
        dispatch_id: run.dispatch_id,
        agent_id: run.agent_id,
        agent_name,
        origin,
        state: "dispatched".into(),
        dispatched: height_label_short(height_i64(run.dispatched.height)),
        settled: String::new(),
        attempt: 0,
        holder: String::new(),
        actions: height_i64(run.actions),
        degraded: false,
        reason: String::new(),
        output_ref: String::new(),
        pr_number,
    };
    match run.state {
        runs::index::RunState::Dispatched => {}
        runs::index::RunState::Running { attempt, holder } => {
            row.state = "running".into();
            row.attempt = i64::from(attempt);
            row.holder = short_pubkey(&holder);
        }
        runs::index::RunState::Settled {
            outcome,
            reason,
            degraded,
            executing_node,
            output_ref,
            at,
        } => {
            row.state = outcome_word(outcome).into();
            row.settled = height_label_short(height_i64(at.height));
            row.holder = short_pubkey(&executing_node);
            row.degraded = degraded;
            row.reason = reason.unwrap_or_default();
            row.output_ref = output_ref.unwrap_or_default();
        }
    }
    row
}

/// One journal fact in the tracker's words: its kind, and a one-line
/// summary of what the module committed.
fn journal_entry(row: runs::index::JournalRow) -> JournalEntry {
    let (kind, summary) = match row.fact {
        runs::RunFact::Dispatched {
            agent_id,
            channel_id,
            anchor_seq,
            job_id,
            delegation_id,
            ..
        } => (
            "dispatched",
            format!(
                "for {agent_id} from {}",
                run_origin(&channel_id, anchor_seq, &job_id, &delegation_id)
            ),
        ),
        runs::RunFact::SessionOpened { attempt, holder } => (
            "session opened",
            format!("on {} · attempt {attempt}", short_pubkey(&holder)),
        ),
        runs::RunFact::Acted {
            request_id,
            lane,
            operation,
            result,
        } => ("acted", action_summary(lane, &operation, &result, &request_id)),
        runs::RunFact::Settled {
            outcome,
            reason,
            degraded,
            executing_node,
            output_ref,
            pr,
        } => {
            let mut parts = vec![outcome_word(outcome).to_string()];
            if degraded {
                parts.push("degraded".into());
            }
            parts.extend(reason);
            if executing_node != "unknown" {
                parts.push(format!("on {}", short_pubkey(&executing_node)));
            }
            parts.extend(output_ref);
            parts.extend(pr.map(|pr| pr_label(&pr)));
            ("settled", parts.join(" · "))
        }
        runs::RunFact::ResultActionRefused { request_id } => ("result action refused", request_id),
        runs::RunFact::PrLinked { pr } => ("pr linked", pr_label(&pr)),
    };
    JournalEntry {
        height: height_label_short(height_i64(row.height)),
        kind: kind.into(),
        summary,
    }
}

fn pr_label(pr: &runs::PrRef) -> String {
    format!("PR {}#{}", pr.repo, pr.number)
}

/// One staged action in the tracker's words: the operation, the lane that
/// admitted it, what its receipt says it did, and the receipt id.
fn action_summary(
    lane: runs::LaneKind,
    operation: &str,
    result: &serde_json::Value,
    request_id: &str,
) -> String {
    let lane = match lane {
        runs::LaneKind::Live => "live",
        runs::LaneKind::Final => "final",
    };
    let mut parts = vec![operation.to_string(), lane.to_string()];
    receipt_leaves(result, &mut parts);
    parts.push(request_id.to_string());
    parts.join(" · ")
}

/// The scalar leaves of a receipt, each as `key value`, in key order; a
/// nested object contributes its leaves under their own keys and a null
/// leaf says nothing.
fn receipt_leaves(value: &serde_json::Value, out: &mut Vec<String>) {
    let serde_json::Value::Object(fields) = value else {
        return;
    };
    for (key, value) in fields {
        match value {
            serde_json::Value::Null => {}
            serde_json::Value::Object(_) => receipt_leaves(value, out),
            serde_json::Value::String(text) => out.push(format!("{key} {text}")),
            other => out.push(format!("{key} {other}")),
        }
    }
}

/// Every run the journal lists, newest dispatch first.
async fn recent_runs(client: &RpcClient) -> Result<Vec<runs::index::RunView>, String> {
    let reply: runs::index::RunsViewReply = client
        .view(
            "runs",
            &runs::index::RunsViewQuery::Recent {
                agent_id: None,
                limit: None,
            },
        )
        .await?;
    let runs::index::RunsViewReply::Runs(runs) = reply else {
        return Err("the runs journal returned the wrong reply to a recent-runs read".into());
    };
    Ok(runs)
}

/// The tracker's rows, for the workspace search: agents named by id.
pub async fn load_agent_runs(rpc: String) -> Result<Vec<RunRow>, AppError> {
    async {
        let client = rpc_client(&rpc)?;
        let runs = recent_runs(&client).await?;
        Ok(runs
            .into_iter()
            .map(|run| run_row(run, &BTreeMap::new()))
            .collect())
    }
    .await
    .map_err(app_error)
}

/// The journal of one run, fact by fact, in the scope it was read in. An empty
/// id is the reader closing the journal: nothing is read and the empty journal
/// comes back at once.
///
/// Infallible on purpose: the answer carries its own scope, and a `Result`'s
/// error arm cannot. A refusal that arrives
/// without its scope has nowhere safe to be shown once the reader has moved to
/// another network with the same run open.
#[allow(clippy::too_many_arguments)]
pub async fn load_run_journal(
    rpc: String,
    network: String,
    link: i64,
    account: String,
    op: i64,
    dispatch_id: String,
) -> RunJournal {
    let scope = RunJournal {
        dispatch_id: dispatch_id.clone(),
        rpc: rpc.clone(),
        network: network.clone(),
        link,
        account,
        op,
        ..RunJournal::default()
    };
    if dispatch_id.is_empty() {
        return scope;
    }
    match read_run_journal(&rpc, &network, &dispatch_id).await {
        Ok((entries, links)) => RunJournal {
            entries,
            links,
            ..scope
        },
        Err(error) => RunJournal {
            error: app_error(error).message,
            ..scope
        },
    }
}

/// The run's journal lines and its chips. `chain` is the network the links
/// are spelled for, so a chip opened later on another network refuses
/// instead of resolving its ids against the wrong store.
async fn read_run_journal(
    rpc: &str,
    chain: &str,
    dispatch_id: &str,
) -> Result<(Vec<JournalEntry>, Vec<RunLink>), String> {
    let client = rpc_client(rpc)?;
    let reply: runs::index::RunsViewReply = client
        .view(
            "runs",
            &runs::index::RunsViewQuery::Run {
                dispatch_id: dispatch_id.to_owned(),
            },
        )
        .await?;
    let runs::index::RunsViewReply::Run(detail) = reply else {
        return Err("the runs journal returned the wrong reply to a run read".into());
    };
    let Some(detail) = detail else {
        return Ok((Vec::new(), Vec::new()));
    };
    let links = run_links(&client, chain, detail.run).await;
    let entries = detail.journal.into_iter().map(journal_entry).collect();
    Ok((entries, links))
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
        let operation = match paused {
            true => runs::ModelMsg::PauseModel { agent_id },
            false => runs::ModelMsg::ResumeModel { agent_id },
        };
        let payload = runs::encode_msg(&runs::RunsMsg::ConfigureModel { operation });
        signed_write(&rpc, "runs", payload, password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The editor's record as the Agents view hands it back: every field the
/// controller may set, in one piece. The view holds the drafts; this is what
/// leaves it with the save.
#[derive(Debug, serde::Deserialize)]
pub struct AgentDraft {
    pub agent_id: String,
    pub display_name: String,
    pub capability: String,
    pub allowed_actions: Vec<String>,
    pub caps: AgentCaps,
    pub skills: Vec<AgentSkill>,
}

impl AgentDraft {
    fn decode(draft: &str) -> Result<Self, String> {
        serde_json::from_str(draft)
            .map_err(|error| format!("the agent draft does not decode: {error}"))
    }
}

/// Rewrite an agent's record with the editor's draft. Every editable field is
/// sent, so the record afterwards IS the draft; the registry decides whether
/// the signing account controls it.
pub async fn save_agent(rpc: String, password: String, draft: String) -> Result<bool, AppError> {
    async {
        let draft = AgentDraft::decode(&draft)?;
        let agent_id = required_id(draft.agent_id, "agent")?;
        let rpc = rpc_client(&rpc)?;
        let operation = runs::ModelMsg::UpdateModel {
            agent_id,
            display_name: Some(draft.display_name),
            capability: Some(draft.capability),
            allowed_actions: Some(draft.allowed_actions),
            recipe_hash: None,
            caps: Some(draft.caps.into_resource_caps()?),
            skills: Some(draft.skills.into_iter().map(runs::SkillRef::from).collect()),
        };
        let payload = runs::encode_msg(&runs::RunsMsg::ConfigureModel { operation });
        signed_write(&rpc, "runs", payload, password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Bring a new agent into the register: provision its keyless program account
/// under the signing account (`controller`, the wallet's own account number),
/// read that account back, and register the draft against it. Two committed
/// writes and one read, in order; the first write is a full block, so the
/// read never runs ahead of it.
pub async fn register_agent(
    rpc: String,
    password: String,
    controller: String,
    draft: String,
) -> Result<bool, AppError> {
    async {
        let draft = AgentDraft::decode(&draft)?;
        let controller: u64 = controller.parse().map_err(|_| {
            "registering an agent needs an account to control it — create one in Settings first"
                .to_string()
        })?;
        runs::validate_agent_id(&draft.agent_id)?;
        let display_name = draft.display_name.trim().to_owned();
        if display_name.is_empty() {
            return Err("give the agent a display name".to_string());
        }
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "agent",
            ::agent::encode_msg(&::agent::AgentMsg::Provision {
                name: display_name.clone(),
                program: runs::model_program(&draft.agent_id),
            }),
            password.clone(),
        )
        .await?;
        let account = newest_program_account(&rpc, controller, &display_name).await?;
        let operation = runs::ModelMsg::RegisterModel {
            account,
            agent_id: draft.agent_id,
            display_name,
            capability: draft.capability,
            allowed_actions: draft.allowed_actions,
            recipe_hash: None,
            caps: Some(draft.caps.into_resource_caps()?),
            skills: Some(draft.skills.into_iter().map(runs::SkillRef::from).collect()),
        };
        let payload = runs::encode_msg(&runs::RunsMsg::ConfigureModel { operation });
        signed_write(&rpc, "runs", payload, password).await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The highest-numbered agent-executed program account named `name` under
/// `controller`. Accounts are numbered upward with no gaps, so after a
/// provision the newest match IS the account it minted, whatever older
/// accounts share the name.
async fn newest_program_account(
    rpc: &RpcClient,
    controller: u64,
    name: &str,
) -> Result<u64, String> {
    let page_limit =
        usize::try_from(identity::MAX_QUERY_LIMIT).expect("the identity page cap fits a usize");
    let mut newest = None;
    let mut from: identity::AccountNumber = 0;
    loop {
        let reply: identity::IdentityReply = rpc
            .query(
                "identity",
                &identity::IdentityQuery::Controlled {
                    by: controller,
                    from,
                    limit: identity::MAX_QUERY_LIMIT,
                },
            )
            .await?;
        let identity::IdentityReply::Accounts(page) = reply else {
            return Err("the identity module returned the wrong reply".to_string());
        };
        let page_is_last = page.len() < page_limit;
        let Some(last) = page.last().map(|account| account.number) else {
            break;
        };
        let runs_agent_program = |account: &identity::AccountView| {
            matches!(
                &account.control,
                identity::Control::Program { executor, .. } if executor == "agent"
            )
        };
        newest = page
            .iter()
            .filter(|account| account.name == name && runs_agent_program(account))
            .map(|account| account.number)
            .max()
            .or(newest);
        if page_is_last {
            break;
        }
        from = last + 1;
    }
    newest
        .ok_or_else(|| format!("the program account for {name:?} was not found after provisioning"))
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
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
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
            identity::IdentityReply::Accounts(_)
            | identity::IdentityReply::Resolved(_)
            | identity::IdentityReply::Gen(_) => {
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
        identity::IdentityReply::Account(_)
        | identity::IdentityReply::Accounts(_)
        | identity::IdentityReply::Resolved(_) => {
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
        identity::IdentityReply::Accounts(_)
        | identity::IdentityReply::Resolved(_)
        | identity::IdentityReply::Gen(_) => {
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

/// The ceremony owns its callback socket. Cancelling the UI task or timing out
/// drops the listener and any partial request along with the wait.
async fn browser_ceremony(request: authpage::Request) -> Result<authpage::Outcome, String> {
    let listener = authpage::Listener::bind()
        .await
        .map_err(|e| format!("auth callback: {e}"))?;
    let callback = listener.callback_url();
    let url = authpage::request_url(authpage::AUTH_PAGE, &request, &callback);
    let op = request_op(&request);
    let opened = authpage::open_browser(&url);
    if !opened {
        tracing::warn!(target: "ducktape::auth", event = "ceremony_failed", surface = "browser", op, reason = "no_browser_opener");
        return Err("no browser opener on this machine (xdg-open / open)".to_string());
    }
    tracing::info!(target: "ducktape::auth", event = "ceremony_shown", surface = "browser", op);
    let outcome = tokio::time::timeout(CEREMONY_TIMEOUT, listener.wait())
        .await
        .map_err(|_| "the browser did not answer in time".to_string())
        .and_then(|outcome| outcome);
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
            chain_id: chain_id.to_string(),
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
        let number = authpage::assertion_account(
            &chain_id,
            &browser_ceremony(authpage::account_request()).await?,
        )?;
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
        let (_, proof) = authpage::login_consent(&chain_id, &consent)?;
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
    let waiting = relay.wait(CEREMONY_TIMEOUT);
    tokio::pin!(waiting);
    // The countdown: the same QR re-sent each second with the time it has
    // left, so the screen can show it. The first tick is a second away —
    // the reading above already carries the full ceiling.
    let second = Duration::from_secs(1);
    let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + second, second);
    let outcome = loop {
        tokio::select! {
            answered = &mut waiting => {
                break answered;
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
            chain_id: chain_id.to_string(),
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
        let number = authpage::assertion_account(&chain_id, &named)?;
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
        let (_, proof) = authpage::login_consent(&chain_id, &consent)?;
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

    /// A relay that answers 204 `absent` times, then `json` once and exits.
    fn fake_relay(absent: usize, json: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for (served, stream) in listener.incoming().take(absent + 1).enumerate() {
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

    const ASSERTION: &str = r#"{"op":"get","credentialId":"AQ","authenticatorData":"AQ","clientDataJSON":"AQ","signature":"AQ","userHandle":"6zD6Woip0W_PPk0EWZGNZdwjPHgvY2dqMFHQVJ7xyIwqAAAAAAAAAA"}"#;

    /// Invalidating the UI stream must close the request already at the relay,
    /// even if that relay never sends a response or the next countdown tick.
    #[tokio::test]
    async fn cancelling_a_ceremony_stream_closes_the_pending_relay_request() {
        use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, BufReader};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let mut stream = ceremony_stream(move |mut tx| async move {
            qr_ceremony(
                &base,
                authpage::Request::Get { challenge: [7; 32] },
                "Confirm with the passkey.",
                &mut tx,
            )
            .await?;
            Ok(())
        });
        let receive_request = async {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = BufReader::new(socket);
            loop {
                let mut line = String::new();
                let read = socket.read_line(&mut line).await.unwrap();
                assert_ne!(read, 0, "the request must reach the relay before cancellation");
                let headers_complete = line == "\r\n";
                if headers_complete {
                    return socket;
                }
            }
        };
        let mut socket = {
            let consume = async {
                while stream.next().await.is_some() {}
            };
            tokio::select! {
                socket = receive_request => socket,
                () = consume => panic!("the unanswered ceremony ended before cancellation"),
            }
        };
        drop(stream);
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
    }

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
                user_handle: Some(handle),
                ..
            } if handle == authpage::UserHandle::new("demo#a1b2c3d4", 42)
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

#[cfg(test)]
mod journal_summary_tests {
    //! An action's journal line says what the run did, from the receipt the
    //! module minted, not from the model's input: the reader sees where a
    //! reply landed and which emoji a reaction set, on whichever lane.

    use super::*;

    #[test]
    fn an_action_reads_as_operation_lane_receipt_and_id() {
        let entry = journal_entry(runs::index::JournalRow {
            height: 309,
            time: 0,
            fact: runs::RunFact::Acted {
                request_id: "action/abc/ack".into(),
                lane: runs::LaneKind::Live,
                operation: "react".into(),
                result: serde_json::json!({"channel_id": "engineering", "seq": 2, "emoji": "👀"}),
            },
        });
        assert_eq!(entry.kind, "acted");
        assert_eq!(
            entry.summary,
            "react · live · channel_id engineering · emoji 👀 · seq 2 · action/abc/ack"
        );
    }

    #[test]
    fn a_final_lane_reply_names_its_destination_from_the_nested_receipt() {
        let entry = journal_entry(runs::index::JournalRow {
            height: 579,
            time: 0,
            fact: runs::RunFact::Acted {
                request_id: "result/abc/0".into(),
                lane: runs::LaneKind::Final,
                operation: "reply".into(),
                result: serde_json::json!({
                    "destination": {"channel_id": "engineering", "kind": "chat", "thread": 12},
                    "id": "agent/abc",
                }),
            },
        });
        assert_eq!(
            entry.summary,
            "reply · final · channel_id engineering · kind chat · thread 12 · id agent/abc · result/abc/0"
        );
    }

    #[test]
    fn a_module_authored_effect_with_no_receipt_reads_as_its_label_alone() {
        let entry = journal_entry(runs::index::JournalRow {
            height: 580,
            time: 0,
            fact: runs::RunFact::Acted {
                request_id: "result/abc/1".into(),
                lane: runs::LaneKind::Final,
                operation: "forge".into(),
                result: serde_json::Value::Null,
            },
        });
        assert_eq!(entry.summary, "forge · final · result/abc/1");
    }
}

#[cfg(test)]
mod run_journal_scope_tests {
    //! The run id is the journal's SUBJECT. What makes an answer identifiable
    //! is the operation it came from — the endpoint, the chain, the connection
    //! revision, the seated account and the dispatch nonce.

    use super::*;

    fn read(network: &str, link: i64, op: i64) -> RunJournal {
        RunJournal {
            dispatch_id: "run-7".into(),
            links: Vec::new(),
            entries: vec![JournalEntry {
                height: "41".into(),
                kind: "dispatched".into(),
                summary: "network A said so".into(),
            }],
            rpc: "http://node".into(),
            network: network.into(),
            link,
            account: "7".into(),
            op,
            error: String::new(),
        }
    }

    /// THE BUG THIS FENCE EXISTS FOR. A journal read starts on network A for
    /// run-7; the reader switches to B, which has a run-7 of its own and opens
    /// it; A's answer lands. Matching on the run id alone installs A's journal
    /// under B, and the entries are indistinguishable from B's own.
    #[test]
    fn a_delayed_journal_from_another_network_never_installs_under_this_one() {
        let from_a = read("duck-a", 4, 9);
        let live = |network, link, op| {
            journal_in_scope(&from_a, "http://node", network, link, "7", op, "run-7")
        };
        assert!(live("duck-a", 4, 9), "its own scope installs");
        // the SAME run id, the SAME endpoint, the SAME account — another chain
        assert!(!live("duck-b", 4, 9), "network A's journal installed under B");
        // a reconnect to the same endpoint is a different session
        assert!(!live("duck-a", 5, 9));
        // and a second read of the same run on the same link is a second
        // operation: the first answering last must not overwrite it
        assert!(!live("duck-a", 4, 10));
    }

    /// A REFUSAL IS SCOPED TOO. `load_run_journal` is infallible precisely so a
    /// failure carries the scope it happened in; an `Err` arm cannot, and its
    /// message would be attributed to whatever the reader has open now.
    #[test]
    fn a_refusal_carries_the_scope_it_was_refused_in() {
        let refused = RunJournal {
            entries: Vec::new(),
            error: "the runs journal did not answer".into(),
            ..read("duck-a", 4, 9)
        };
        assert!(journal_in_scope(
            &refused,
            "http://node",
            "duck-a",
            4,
            "7",
            9,
            "run-7"
        ));
        assert!(!journal_in_scope(
            &refused,
            "http://node",
            "duck-b",
            4,
            "7",
            9,
            "run-7"
        ));
        assert!(!refused.error.is_empty(), "and it still says what happened");
    }

    /// Closing the journal is an empty id: nothing is read, and the answer is
    /// still scoped so the close cannot be undone by a read in flight.
    #[tokio::test]
    async fn closing_the_journal_reads_nothing_and_still_carries_its_scope() {
        let closed = load_run_journal(
            "http://node".into(),
            "duck-a".into(),
            4,
            "7".into(),
            9,
            String::new(),
        )
        .await;
        assert!(closed.entries.is_empty());
        assert!(closed.error.is_empty(), "a close is not a failure");
        assert!(journal_in_scope(&closed, "http://node", "duck-a", 4, "7", 9, ""));
    }
}
