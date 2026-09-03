use super::*;
use ::chat;
use ::forge;
use identity::{AccountView, IdentityQuery, IdentityReply};

/// One UI publication may carry at most this many consecutive chat deltas.
/// The cap bounds one reducer pass; the capacity-one publication gate below
/// waits for the UI to finish that pass before the stream reads another one.
/// No clock participates in batching or fairness.
pub(crate) const LIVE_CHAT_BATCH_LIMIT: usize = 64;

enum PendingLiveEvent {
    Event(ducktape_rpc::Result<ModuleEvent>),
    Closed,
    Update(Box<LiveUpdate>),
}

struct LiveEventState {
    rpc: String,
    cursors: BTreeMap<String, String>,
    stream: Option<ducktape_rpc::ModuleEventStream>,
    pending: Option<PendingLiveEvent>,
    retry_attempt: u32,
    publication_gate: Arc<tokio::sync::Semaphore>,
}

/// Merge one later, consecutive chat publication into `batch` when the shared
/// production cap permits it. A returned update is a non-chat publication (or
/// the next full chat batch) that the caller must preserve without reordering.
fn merge_live_chat_batch(batch: &mut LiveUpdate, mut next: LiveUpdate) -> Option<LiveUpdate> {
    let both_chat = batch.kind == crate::LiveKind::Chat && next.kind == crate::LiveKind::Chat;
    let fits = batch.chat.len().saturating_add(next.chat.len()) <= LIVE_CHAT_BATCH_LIMIT;
    if !both_chat || !fits {
        return Some(next);
    }
    batch.chat.append(&mut next.chat);
    batch.status = next.status;
    batch.height = next.height;
    None
}

/// Deterministic seam for allocation/update-count probes. Its input is the
/// sequence of already-ready, already-folded publications; production uses
/// the same [`merge_live_chat_batch`] decision while greedily polling the live
/// socket. Non-chat publications are ordering barriers.
#[cfg(test)]
pub(crate) fn batch_live_updates(updates: Vec<LiveUpdate>) -> Vec<LiveUpdate> {
    let mut emitted = Vec::new();
    for update in updates {
        let Some(batch) = emitted.last_mut() else {
            emitted.push(update);
            continue;
        };
        if let Some(update) = merge_live_chat_batch(batch, update) {
            emitted.push(update);
        }
    }
    emitted
}

/// `attempt` backs the connect off exactly as `live_resync_load` backs off a
/// live sync — 1s doubling to a 16s cap. The steady-state path has always
/// retried forever; the connect that GETS you there gave up after one failure,
/// which is the wrong way round. A transient failure is not rare: a `/v1/query`
/// can block until the node writes its next checkpoint (issue #1018), which is
/// longer than the RPC client's 30s timeout, so a healthy node hands the
/// console an "error sending request" often enough to matter.
///
/// `generation` rides through to the reply so a connect answering for an
/// endpoint you have since left is dropped unread — the same guard the page
/// and chat planes learned in #970.
pub async fn connect(
    rpc: String,
    attempt: i64,
    generation: i64,
) -> Result<WorkspaceData, HydrationError> {
    if attempt > 0 {
        tokio::time::sleep(retry_delay(u32::try_from(attempt).unwrap_or(u32::MAX))).await;
    }
    let result = async {
        let rpc = rpc_client(&rpc)?;
        load_workspace(&rpc, None, None, generation).await
    }
    .await;
    // SAY WHAT ACTUALLY FAILED. This threw the cause away with `|_|` and
    // asserted a diagnosis it had not made: "Check the endpoint and node" is
    // the one thing the reader can act on, and it is wrong whenever the node is
    // answering fine and the failure is a timeout, an unreadable reply, or a
    // broken signer. Measured while debugging this very screen — the node was
    // serving `/v1/status` in under a millisecond and the app still said to go
    // check it.
    //
    // `user_error` is the translator the rest of the app already routes
    // through: it names the signer, the key, a refused password, a slow node
    // and a garbled reply, and falls through to the raw message rather than
    // inventing one.
    // A GENERATION, NOT AN `AppError`. The failure arm retries, so it must be
    // able to tell ITS OWN failure from one belonging to a connect chain that
    // has since been abandoned — otherwise two chains both retry forever and
    // each one's generation bump can reject the other's success. `AppError`
    // carries `committed`, which a read has no use for; `HydrationError` is
    // what every other loader here already fails with.
    result.map_err(|cause| HydrationError {
        generation,
        message: user_error(cause.to_string()),
    })
}

pub fn live_events(rpc: String) -> iced::futures::stream::BoxStream<'static, LiveUpdate> {
    iced::futures::stream::unfold(
        LiveEventState {
            rpc,
            cursors: BTreeMap::new(),
            stream: None,
            pending: None,
            retry_attempt: 0,
            publication_gate: Arc::new(tokio::sync::Semaphore::new(1)),
        },
        |mut state| async move {
            // One subscription item may exist outside this stream at a time.
            // iced drops the generated LiveUpdated message after `update`;
            // that drop is the acknowledgement that releases this permit.
            let publication_permit = state
                .publication_gate
                .clone()
                .acquire_owned()
                .await
                .expect("the live publication gate stays open");
            if state.stream.is_none() && state.retry_attempt > 0 {
                tokio::time::sleep(retry_delay(state.retry_attempt)).await;
            }
            if state.stream.is_none() {
                let connected = async {
                    let rpc = rpc_client(&state.rpc)?;
                    // THE PLANES THIS CONSOLE DRAWS. The first four fold; the
                    // rest reload the one plane they name (see `folded_update`).
                    //
                    // A module a node does not index no longer takes the others
                    // down — it comes back as `Refused` and that plane alone
                    // stays cold (`ModuleEvent::Refused`). `bin/noded` indexes
                    // no `valset` and no `governance`, so this list is only
                    // safe at all because of that.
                    rpc.module_events(
                        vec![
                            "chat".to_string(),
                            "pages".to_string(),
                            "inbox".to_string(),
                            "forge".to_string(),
                            "valset".to_string(),
                            "governance".to_string(),
                            "identity".to_string(),
                            "agent".to_string(),
                            "runs".to_string(),
                            "files".to_string(),
                        ],
                        state.cursors.clone(),
                    )
                    .await
                    .map_err(Into::into)
                }
                .await;
                match connected {
                    Ok(stream) => state.stream = Some(stream),
                    Err(error) => {
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        let mut update = live_retry(error);
                        update.permit = LivePermit::held(publication_permit);
                        return Some((update, state));
                    }
                }
            }
            let mut skipped_ready_frames = 0usize;
            loop {
                let event = match state.pending.take() {
                    Some(PendingLiveEvent::Event(event)) => Some(event),
                    Some(PendingLiveEvent::Closed) => None,
                    Some(PendingLiveEvent::Update(update)) => {
                        let mut update = *update;
                        update.permit = LivePermit::held(publication_permit);
                        return Some((update, state));
                    }
                    None => {
                        state
                            .stream
                            .as_mut()
                            .expect("stream initialized above")
                            .next()
                            .await
                    }
                };
                let mut update = match event {
                    Some(Ok(ModuleEvent::Ready { cursors })) => {
                        state.cursors = cursors;
                        state.retry_attempt = 0;
                        live_update(crate::LiveKind::Ready, "Live", -1)
                    }
                    Some(Ok(ModuleEvent::Changed { module, cursor, op })) => {
                        state.cursors.insert(format!("module:{module}"), cursor);
                        match folded_update(&state.rpc, &module, *op).await {
                            Some(update) => update,
                            // invisible to the UI (hook registration) — keep
                            // draining without emitting.
                            None => {
                                skipped_ready_frames += 1;
                                let exhausted_fairness_budget =
                                    skipped_ready_frames >= LIVE_CHAT_BATCH_LIMIT;
                                if exhausted_fairness_budget {
                                    tokio::task::yield_now().await;
                                    skipped_ready_frames = 0;
                                }
                                continue;
                            }
                        }
                    }
                    // THIS PLANE IS DEAD FOR THIS CONNECTION; THE OTHERS ARE
                    // NOT. Keep draining — the whole point is that a module
                    // this node does not index no longer takes chat and pages
                    // down with it.
                    //
                    // NOT surfaced, and saying so rather than pretending: the
                    // refusal arrives just before `ready`, and `live_updated`
                    // assigns `status` as its first statement, so any message
                    // put here is overwritten microseconds later. Showing it
                    // needs a per-plane field that no surface reads yet.
                    Some(Ok(ModuleEvent::Refused { .. })) => {
                        skipped_ready_frames += 1;
                        let exhausted_fairness_budget =
                            skipped_ready_frames >= LIVE_CHAT_BATCH_LIMIT;
                        if exhausted_fairness_budget {
                            tokio::task::yield_now().await;
                            skipped_ready_frames = 0;
                        }
                        continue;
                    }
                    Some(Ok(ModuleEvent::Lagged { module, cursor })) => {
                        state.cursors.insert(format!("module:{module}"), cursor);
                        live_resync(&module, -1)
                    }
                    // THE HEAD MOVES ON BLOCKS, NOT ON OPS. Height used to come
                    // only from a folded op, so a chain whose four subscribed
                    // modules were quiet left the console reading a frozen
                    // block number — on an idle chain, forever. The node has
                    // been sending this every block the whole time (the
                    // heartbeat rides the block wake, nop fillers included);
                    // the client threw it away by declaring the frame a unit
                    // variant, so the height never survived deserialization.
                    //
                    // It carries no cursor: a heartbeat is not a topic and
                    // resuming does not replay one. And it triggers NO load —
                    // see `ModuleEvent::Tip`.
                    //
                    // No de-duplication here, deliberately: the handler stops a
                    // tip immediately after the head assignment
                    // (`handlers/lifecycle.ice`), so a repeated height costs two
                    // scalar writes and no fold. Suppressing it would buy that
                    // back at the price of carrying a last-height in this state,
                    // and the fold path — the part that actually cost something
                    // — is already unreachable.
                    Some(Ok(ModuleEvent::Tip { height })) => live_update(
                        crate::LiveKind::Tip,
                        &format!("Live · block {height}"),
                        i64::try_from(height).unwrap_or(i64::MAX),
                    ),
                    Some(Err(error)) => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        live_retry(error.into())
                    }
                    None => {
                        state.stream = None;
                        state.retry_attempt = state.retry_attempt.saturating_add(1);
                        live_retry("RPC stream closed".into())
                    }
                };
                if update.kind == crate::LiveKind::Chat {
                    update = collect_ready_chat_updates(&mut state, update).await;
                }
                update.permit = LivePermit::held(publication_permit);
                return Some((update, state));
            }
        },
    )
    .boxed()
}

/// Greedily take only chat frames that are ready *now*. The first frame of any
/// other kind is parked for the next unfold, so a pages/forge/tip/error frame
/// cannot be overtaken by later chat traffic. Dropping a pending `next()`
/// future is safe: it owns no frame and the boxed stream retains its socket.
async fn collect_ready_chat_updates(
    state: &mut LiveEventState,
    mut batch: LiveUpdate,
) -> LiveUpdate {
    // Count consumed CHAT FRAMES, not only visible deltas. Hook registration
    // folds to `None`; without this separate budget an always-ready run of
    // invisible chat frames could monopolise this one stream poll forever.
    let mut consumed = batch.chat.len();
    while consumed < LIVE_CHAT_BATCH_LIMIT {
        let ready = state
            .stream
            .as_mut()
            .expect("a chat update came from an initialized stream")
            .next()
            .now_or_never();
        let Some(event) = ready else {
            break;
        };
        let Some(event) = event else {
            state.pending = Some(PendingLiveEvent::Closed);
            break;
        };
        let (module, cursor, op) = match event {
            Ok(ModuleEvent::Changed { module, cursor, op }) => (module, cursor, op),
            other => {
                state.pending = Some(PendingLiveEvent::Event(other));
                break;
            }
        };
        let is_chat = module == "chat";
        if !is_chat {
            state.pending = Some(PendingLiveEvent::Event(Ok(ModuleEvent::Changed {
                module,
                cursor,
                op,
            })));
            break;
        }
        consumed += 1;
        state.cursors.insert("module:chat".into(), cursor);
        let Some(update) = folded_update(&state.rpc, "chat", *op).await else {
            continue;
        };
        if let Some(update) = merge_live_chat_batch(&mut batch, update) {
            state.pending = Some(PendingLiveEvent::Update(Box::new(update)));
            break;
        }
    }
    batch
}

/// The complete chat-owned result of one live batch. Ice crosses the extern
/// boundary once with each list, then assigns the result fields; no delta in
/// the batch can wander through Pages, Bell, or Forge lifecycle reducers.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLiveFold {
    pub messages_changed: bool,
    pub thread_messages_changed: bool,
    pub has_older_history: bool,
    pub selected_message_seq: i64,
    pub selected_message_rev: i64,
    pub message_action: crate::MessageAction,
    pub message_edit_draft: String,
    pub thread_selected_seq: i64,
    pub thread_selected_rev: i64,
    pub thread_message_action: crate::MessageAction,
    pub thread_edit_draft: String,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub thread_messages: Vec<ChatMessage>,
    pub channel_members: Vec<ChatMember>,
    pub channel_reads: Vec<ChannelRead>,
    pub rooms: Vec<ChatSidebarRow>,
    pub dm_rows: Vec<DmSidebarRow>,
    pub unread_marker_seq: i64,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub post_refusal: String,
    pub forge_discussion: Vec<ChatMessage>,
    /// A huddle roster change in the active channel needs the canonical roster
    /// read that a delta cannot derive.
    pub refresh_chat: bool,
}

struct ChatFoldState {
    channels: Vec<ChatChannel>,
    messages: Vec<ChatMessage>,
    thread_messages: Vec<ChatMessage>,
    channel_members: Vec<ChatMember>,
    forge_discussion: Vec<ChatMessage>,
    active_channel: String,
    active_thread_seq: i64,
    forge_item_channel: String,
    history_view: bool,
    messages_changed: bool,
    thread_messages_changed: bool,
    refresh_chat: bool,
}

fn pending_row_matches(messages: &[ChatMessage], id: &str) -> bool {
    messages
        .iter()
        .any(|message| message.pending && message.id == id)
}

fn contains_committed_seq(messages: &[ChatMessage], seq: i64) -> bool {
    messages
        .iter()
        .any(|message| !message.pending && message.seq == seq)
}

fn accepts_edit(messages: &[ChatMessage], seq: i64, rev: i64) -> bool {
    messages.iter().any(|message| {
        !message.pending && message.seq == seq && !message.deleted && message.rev < rev
    })
}

fn fold_channel_created(state: &mut ChatFoldState, channel: ChatChannel) {
    state.channels = chat::client::insert_channel(std::mem::take(&mut state.channels), channel);
}

fn fold_channel_renamed(state: &mut ChatFoldState, channel_id: String, name: String) {
    state.channels =
        chat::client::rename_channel(std::mem::take(&mut state.channels), &channel_id, name);
}

fn fold_channel_archived(state: &mut ChatFoldState, channel_id: String, archived: bool) {
    state.channels =
        chat::client::archive_channel(std::mem::take(&mut state.channels), &channel_id, archived);
}

fn fold_posted(state: &mut ChatFoldState, channel_id: String, seq: i64, message: ChatMessage) {
    state.channels =
        chat::client::advance_channel_head(std::mem::take(&mut state.channels), &channel_id, seq);
    let is_active_channel = channel_id == state.active_channel;
    let settles_pending = is_active_channel && pending_row_matches(&state.messages, &message.id);
    let folds_active_window = is_active_channel && (!state.history_view || settles_pending);
    if folds_active_window {
        let inserts_committed = !contains_committed_seq(&state.messages, seq);
        state.messages_changed |= settles_pending || inserts_committed;
        state.messages = chat::client::merge_posted_message(
            std::mem::take(&mut state.messages),
            message.clone(),
        );
    }
    let updates_forge_discussion = channel_id == state.forge_item_channel;
    if updates_forge_discussion {
        state.forge_discussion = chat::client::merge_posted_message(
            std::mem::take(&mut state.forge_discussion),
            message,
        );
    }
}

fn fold_reply(
    state: &mut ChatFoldState,
    channel_id: String,
    seq: i64,
    root_seq: i64,
    message: ChatMessage,
) {
    state.channels =
        chat::client::advance_channel_head(std::mem::take(&mut state.channels), &channel_id, seq);
    let is_active_channel = channel_id == state.active_channel;
    let settles_pending =
        is_active_channel && pending_row_matches(&state.thread_messages, &message.id);
    let folds_active_window = is_active_channel && (!state.history_view || settles_pending);
    if folds_active_window {
        let updates_root = contains_committed_seq(&state.messages, root_seq);
        state.messages_changed |= updates_root;
        state.messages =
            chat::client::bump_reply_summary(std::mem::take(&mut state.messages), root_seq);
    }
    let updates_open_thread = is_active_channel && root_seq == state.active_thread_seq;
    if updates_open_thread {
        state.thread_messages_changed = true;
        let thread =
            chat::client::bump_reply_summary(std::mem::take(&mut state.thread_messages), root_seq);
        state.thread_messages = chat::client::merge_thread_reply(thread, message);
    }
    let updates_forge_discussion = channel_id == state.forge_item_channel;
    if updates_forge_discussion {
        state.forge_discussion =
            chat::client::bump_reply_summary(std::mem::take(&mut state.forge_discussion), root_seq);
    }
}

fn fold_edited(state: &mut ChatFoldState, channel_id: String, seq: i64, message: ChatMessage) {
    let folds_active_window = channel_id == state.active_channel && !state.history_view;
    if folds_active_window {
        state.messages_changed |= accepts_edit(&state.messages, seq, message.rev);
        state.messages =
            chat::client::merge_message_edit(std::mem::take(&mut state.messages), seq, &message);
    }
    let updates_open_thread = channel_id == state.active_channel && state.active_thread_seq > 0;
    if updates_open_thread {
        state.thread_messages_changed |= accepts_edit(&state.thread_messages, seq, message.rev);
        state.thread_messages = chat::client::merge_message_edit(
            std::mem::take(&mut state.thread_messages),
            seq,
            &message,
        );
    }
    let updates_forge_discussion = channel_id == state.forge_item_channel;
    if updates_forge_discussion {
        state.forge_discussion = chat::client::merge_message_edit(
            std::mem::take(&mut state.forge_discussion),
            seq,
            &message,
        );
    }
}

fn fold_deleted(state: &mut ChatFoldState, channel_id: String, seq: i64) {
    let folds_active_window = channel_id == state.active_channel && !state.history_view;
    if folds_active_window {
        state.messages_changed |= contains_committed_seq(&state.messages, seq);
        state.messages = chat::client::tombstone_message(std::mem::take(&mut state.messages), seq);
    }
    let updates_open_thread = channel_id == state.active_channel && state.active_thread_seq > 0;
    if updates_open_thread {
        state.thread_messages_changed |= contains_committed_seq(&state.thread_messages, seq);
        state.thread_messages =
            chat::client::tombstone_message(std::mem::take(&mut state.thread_messages), seq);
    }
    let updates_forge_discussion = channel_id == state.forge_item_channel;
    if updates_forge_discussion {
        state.forge_discussion =
            chat::client::tombstone_message(std::mem::take(&mut state.forge_discussion), seq);
    }
}

#[allow(clippy::too_many_arguments)]
fn fold_reaction(
    state: &mut ChatFoldState,
    channel_id: String,
    seq: i64,
    emoji: String,
    added: bool,
    reactor: String,
    by_me: bool,
) {
    let folds_active_window = channel_id == state.active_channel && !state.history_view;
    if folds_active_window {
        state.messages_changed |= contains_committed_seq(&state.messages, seq);
        state.messages = chat::client::merge_message_reaction(
            std::mem::take(&mut state.messages),
            seq,
            &emoji,
            added,
            &reactor,
            by_me,
        );
    }
    let updates_open_thread = channel_id == state.active_channel && state.active_thread_seq > 0;
    if updates_open_thread {
        state.thread_messages_changed |= contains_committed_seq(&state.thread_messages, seq);
        state.thread_messages = chat::client::merge_message_reaction(
            std::mem::take(&mut state.thread_messages),
            seq,
            &emoji,
            added,
            &reactor,
            by_me,
        );
    }
    let updates_forge_discussion = channel_id == state.forge_item_channel;
    if updates_forge_discussion {
        state.forge_discussion = chat::client::merge_message_reaction(
            std::mem::take(&mut state.forge_discussion),
            seq,
            &emoji,
            added,
            &reactor,
            by_me,
        );
    }
}

fn fold_membership(state: &mut ChatFoldState, channel_id: String, added: bool, member: ChatMember) {
    let updates_active_members = channel_id == state.active_channel;
    if updates_active_members {
        state.channel_members = chat::client::apply_membership(
            std::mem::take(&mut state.channel_members),
            added,
            member,
        );
    }
}

fn fold_channel_refresh(state: &mut ChatFoldState, channel_id: String) {
    state.refresh_chat |= channel_id == state.active_channel;
}

fn fold_channel_updated(state: &mut ChatFoldState, channel_id: String, channel: ChatChannel) {
    state.refresh_chat |= channel_id == state.active_channel;
    state.channels =
        chat::client::replace_channel(std::mem::take(&mut state.channels), &channel_id, channel);
}

/// Fold one ordered live chat batch in one Rust ownership domain. Lists move
/// into this function once, then each delta mutates those owned lists in
/// sequence. This replaces the former Ice handler's repeated by-value extern
/// calls, which deep-cloned the whole timeline for every operation.
#[allow(clippy::too_many_arguments)]
pub fn fold_live_chat(
    deltas: Vec<ChatDelta>,
    channels: Vec<ChatChannel>,
    messages: Vec<ChatMessage>,
    thread_messages: Vec<ChatMessage>,
    channel_members: Vec<ChatMember>,
    mut channel_reads: Vec<ChannelRead>,
    dm_peers: Vec<DmPeer>,
    me: String,
    active_channel: String,
    active_thread_seq: i64,
    history_view: bool,
    chat_visible: bool,
    has_older_history: bool,
    unread_boundary: i64,
    mut active_channel_name: String,
    mut active_channel_archived: bool,
    mut active_channel_members_only: bool,
    forge_discussion: Vec<ChatMessage>,
    forge_item_channel: String,
    selected_message_seq: i64,
    selected_message_rev: i64,
    message_action: crate::MessageAction,
    message_edit_draft: String,
    thread_selected_seq: i64,
    thread_selected_rev: i64,
    thread_message_action: crate::MessageAction,
    thread_edit_draft: String,
) -> ChatLiveFold {
    // Read before the timeline moves into the fold: the floor it ends on is
    // compared against this to see whether the render window evicted history.
    let floor_before = oldest_committed_seq(&messages);
    let mut state = ChatFoldState {
        channels,
        messages,
        thread_messages,
        channel_members,
        forge_discussion,
        active_channel,
        active_thread_seq,
        forge_item_channel,
        history_view,
        messages_changed: false,
        thread_messages_changed: false,
        refresh_chat: false,
    };
    for delta in deltas {
        match delta {
            ChatDelta::ChannelCreated { channel } => fold_channel_created(&mut state, channel),
            ChatDelta::ChannelRenamed { channel_id, name } => {
                fold_channel_renamed(&mut state, channel_id, name)
            }
            ChatDelta::ChannelArchived {
                channel_id,
                archived,
            } => fold_channel_archived(&mut state, channel_id, archived),
            ChatDelta::Posted {
                channel_id,
                seq,
                message,
            } => fold_posted(&mut state, channel_id, seq, message),
            ChatDelta::Reply {
                channel_id,
                seq,
                root_seq,
                message,
            } => fold_reply(&mut state, channel_id, seq, root_seq, message),
            ChatDelta::Edited {
                channel_id,
                seq,
                message,
            } => fold_edited(&mut state, channel_id, seq, message),
            ChatDelta::Deleted { channel_id, seq } => fold_deleted(&mut state, channel_id, seq),
            ChatDelta::Reaction {
                channel_id,
                seq,
                emoji,
                added,
                reactor,
                by_me,
            } => fold_reaction(&mut state, channel_id, seq, emoji, added, reactor, by_me),
            ChatDelta::Membership {
                channel_id,
                added,
                member,
            } => fold_membership(&mut state, channel_id, added, member),
            ChatDelta::ChannelRefresh { channel_id } => {
                fold_channel_refresh(&mut state, channel_id)
            }
            ChatDelta::ChannelUpdated {
                channel_id,
                channel,
            } => fold_channel_updated(&mut state, channel_id, channel),
        }
    }
    let ChatFoldState {
        channels,
        messages,
        thread_messages,
        channel_members,
        forge_discussion,
        active_channel,
        history_view,
        messages_changed,
        thread_messages_changed,
        refresh_chat,
        ..
    } = state;
    let reads_live_tail = !history_view && chat_visible;

    if let Some(channel) = channels.iter().find(|channel| channel.id == active_channel) {
        active_channel_name.clone_from(&channel.name);
        active_channel_archived = channel.archived;
        active_channel_members_only = channel.members_only;
    }
    let seated = seated_in(&channel_members, &me);
    let post_refusal = if active_channel_archived {
        "channel_archived".into()
    } else if active_channel_members_only && !seated {
        "members_only".into()
    } else {
        String::new()
    };

    if reads_live_tail {
        let head = channels
            .iter()
            .find(|channel| channel.id == active_channel)
            .map_or(0, |channel| channel.head_seq);
        match channel_reads
            .iter_mut()
            .find(|read| read.channel == active_channel)
        {
            Some(read) => read.seq = read.seq.max(head),
            None => channel_reads.push(ChannelRead {
                channel: active_channel.clone(),
                seq: head,
            }),
        }
    }
    let unread_marker_seq = if unread_boundary <= 0 {
        0
    } else {
        messages
            .iter()
            .find(|message| message.seq > unread_boundary)
            .map_or(0, |message| message.seq)
    };
    let rooms = chat_sidebar_rooms(channels.clone(), dm_peers.clone(), channel_reads.clone());
    let dm_rows = chat_sidebar_dms(channels.clone(), dm_peers, channel_reads.clone());

    // THE SERVER OWNS THIS FLAG; THE FOLD MAY ONLY RAISE IT.
    //
    // It used to be recomputed here as "the oldest loaded root has seq > 1",
    // which is a guess and a wrong one: thread replies consume root sequences
    // without becoming roots, so a channel's very first message routinely sits
    // at seq 40 and "Load older messages" stood over the true beginning of every
    // busy room forever. What a live fold DOES know is whether it pushed the
    // floor up — `bounded_chat_window` evicts from the oldest edge to hold the
    // render window — and rows this window dropped are older history by
    // definition, whatever the page load last said.
    //
    // A window that held no committed row has no floor to lose: the first live
    // arrival in an empty room raises the floor from 0 to its own seq, which is
    // growth, not eviction.
    let window_had_a_floor = floor_before > 0;
    let evicted_the_floor = window_had_a_floor && oldest_committed_seq(&messages) > floor_before;
    let has_older_history = has_older_history || evicted_the_floor;
    let selection = message_selection_after_window_ref(
        &messages,
        selected_message_seq,
        selected_message_rev,
        message_action,
        message_edit_draft,
    );
    let thread_selection = message_selection_after_window_ref(
        &thread_messages,
        thread_selected_seq,
        thread_selected_rev,
        thread_message_action,
        thread_edit_draft,
    );
    ChatLiveFold {
        messages_changed,
        thread_messages_changed,
        has_older_history,
        selected_message_seq: selection.seq,
        selected_message_rev: selection.rev,
        message_action: selection.action,
        message_edit_draft: selection.draft,
        thread_selected_seq: thread_selection.seq,
        thread_selected_rev: thread_selection.rev,
        thread_message_action: thread_selection.action,
        thread_edit_draft: thread_selection.draft,
        channels,
        messages,
        thread_messages,
        channel_members,
        channel_reads,
        rooms,
        dm_rows,
        unread_marker_seq,
        active_channel_name,
        active_channel_archived,
        active_channel_members_only,
        post_refusal,
        forge_discussion,
        refresh_chat,
    }
}

/// Fold one applied op into a live update. A decode failure (payload or
/// stamp) degrades to a scoped resync of that module — a CLIENT reloads,
/// never wedges. `None` = the op is invisible to this UI.
pub(crate) async fn folded_update(
    rpc: &str,
    module: &str,
    op: ducktape_rpc::StreamOp,
) -> Option<LiveUpdate> {
    let height = i64::try_from(op.height).unwrap_or(i64::MAX);
    // WHAT THE STREAM DELIVERED, THE RELOADS BEHIND IT MUST NOT PREDATE.
    //
    // A push reports APPLICATION, which is the acceptance gap closed and the
    // FOLD gap still open: the node's block loop writes the op feed and the
    // index folds behind it on its own runner (only the sim's
    // `wait_folds_drained` ever joins the two). Every structural pages op sets
    // `load_pages` below, and that reload reads the folded view — so without
    // this the pane installs a tree that predates the very op that prompted
    // it, with no further op coming to correct it.
    //
    // Recorded HERE, once, rather than carried down through the update, the
    // handler's debounce and the resync extern's argument list: this is where
    // the height is learned, and `load_pages_data` already waits on the record
    // (`rpc.rs`, `SEEN_BLOCKS`).
    if let Ok(client) = rpc_client(rpc) {
        note_module_block(&client, module, op.height);
    }
    let Some(payload) = op
        .payload
        .as_ref()
        .and_then(|value| serde_json::to_vec(value).ok())
    else {
        return Some(live_resync(module, height));
    };
    match module {
        "chat" => {
            // THE DIRECTORY AS LAST READ, NOT A FRESH READ: this is inside the
            // live decoder fold, where a query would freeze every subscriber
            // for as long as the node's select loop is busy (issue #1018). It
            // is warm by the connect that opened this stream.
            let facts = ReaderFacts::current().await;
            let origin_kind = stream_origin_kind(&op.origin.kind);
            let folded = chat::client::delta_from_op(
                &payload,
                op.assigned.as_ref(),
                origin_kind,
                op.origin.id.as_deref(),
                facts.reader(),
                op.height,
            );
            let delta = match folded {
                Ok(Some(delta)) => delta,
                Ok(None) => return None,
                Err(_) => return Some(live_resync("chat", height)),
            };
            // huddle membership is roster-derived — reload the one channel
            // row from its canonical record instead of guessing the count.
            let delta = match delta {
                ChatDelta::ChannelRefresh { channel_id } => {
                    let channel = match load_channel_row(rpc, &channel_id).await {
                        Ok(Some(channel)) => channel,
                        Ok(None) | Err(_) => return Some(live_resync("chat", height)),
                    };
                    ChatDelta::ChannelUpdated {
                        channel_id,
                        channel,
                    }
                }
                ready => ready,
            };
            Some(LiveUpdate {
                kind: crate::LiveKind::Chat,
                status: format!("Live · block {height}"),
                height,
                module: "chat".into(),
                load_chat: false,
                load_pages: false,
                debounce: false,
                chat: vec![delta],
                pages: PagesDelta::default(),
                bell: BellDelta::default(),
                forge: ForgeRefresh::default(),
                permit: LivePermit::default(),
            })
        }
        "inbox" => {
            // the same derivation `local_member` uses: the bell folds only the
            // ops naming THIS user's queue, and a queue is named for its owner.
            let member = local_member().await?;
            let origin_kind = stream_origin_kind(&op.origin.kind);
            let folded = inbox::client::delta_from_op(
                &payload,
                op.assigned.as_ref(),
                origin_kind,
                op.origin.id.as_deref(),
                &member,
            );
            match folded {
                Ok(Some(bell)) => Some(LiveUpdate {
                    kind: crate::LiveKind::Bell,
                    status: format!("Live · block {height}"),
                    height,
                    module: "inbox".into(),
                    load_chat: false,
                    load_pages: false,
                    debounce: false,
                    chat: Vec::new(),
                    pages: PagesDelta::default(),
                    bell,
                    forge: ForgeRefresh::default(),
                    permit: LivePermit::default(),
                }),
                Ok(None) => None,
                Err(_) => None,
            }
        }
        "pages" => match pages::client::delta_from_op(&payload) {
            Ok(delta) => {
                let folded = delta.kind == "text";
                Some(LiveUpdate {
                    kind: crate::LiveKind::Pages,
                    status: format!("Live · block {height}"),
                    height,
                    module: "pages".into(),
                    load_chat: false,
                    // A TEXT EDIT FOLDS; EVERYTHING ELSE RELOADS. The page autosave
                    // commits one `UpdateText` per tick while a reader types, and
                    // each used to buy a `load_pages_data`: three SEQUENTIAL
                    // queries — the page index, every block of the open page, and a
                    // `ThreadsForTargets` whose body carries every block id. Your
                    // own keystrokes came back on your own stream and made you
                    // re-read the document you were typing into, against a read
                    // path that is checkpoint-gated.
                    load_pages: !folded,
                    // nothing to coalesce when nothing is fetched.
                    debounce: !folded,
                    chat: Vec::new(),
                    pages: delta,
                    bell: BellDelta::default(),
                    forge: ForgeRefresh::default(),
                    permit: LivePermit::default(),
                })
            }
            Err(_) => Some(live_resync("pages", height)),
        },
        "forge" => match forge::client::refresh_from_op(&payload) {
            Ok(refresh) => Some(LiveUpdate {
                kind: crate::LiveKind::Forge,
                status: format!("Live · block {height}"),
                height,
                module: "forge".into(),
                load_chat: false,
                load_pages: false,
                // pushes arrive in bursts (one op per ref batch, then the
                // tracker follow-ups) — coalesce the reloads like pages does.
                debounce: true,
                chat: Vec::new(),
                pages: PagesDelta::default(),
                bell: BellDelta::default(),
                forge: refresh,
                permit: LivePermit::default(),
            }),
            Err(_) => Some(live_resync("forge", height)),
        },
        // THE RELOAD PLANES. No client fold exists for these modules and none
        // is worth writing: a validator set changes when someone joins, a
        // proposal when someone votes, an account when someone renames a
        // device. Human-rate, all of them — so the op is a signal that ONE
        // plane is stale, and the handler refetches exactly that one.
        //
        // Reading rather than folding costs a checkpoint-gated query
        // (`connect`), which is why this is not the answer for chat or pages.
        // At these rates it is the right trade: no fold to keep correct, and
        // nothing at all on a block that does not touch them.
        //
        // `runs` rides here for a different reason than the rest: NOTHING ON
        // SCREEN DRAWS A RUN. The fact it feeds lives in another module's
        // projection — an `AgentRow.live` is `agents_with_a_run_in_flight`
        // reading `runs`' pending register, joined onto a row `agent` owns
        // (`backend/node.rs`) — so there is no local state for a fold to fold
        // INTO and the op can only be a signal to refetch the agents
        // projection. It commits at agent-TURN rate (a run claimed, a run
        // finished), which is the same human rate as the others, so the trade
        // above holds. Without it the Forge seat's live dot had no off-tab
        // refresh path at all and stayed dark until Agents was opened.
        "valset" | "governance" | "identity" | "agent" | "runs" | "files" => {
            Some(live_plane(module, height))
        }
        _ => None,
    }
}

fn stream_origin_kind(kind: &ducktape_rpc::StreamOriginKind) -> &'static str {
    match kind {
        ducktape_rpc::StreamOriginKind::External => "external",
        ducktape_rpc::StreamOriginKind::Module => "module",
        ducktape_rpc::StreamOriginKind::System => "system",
    }
}

/// One channel's row rebuilt from the index view — the huddle roster length is
/// not derivable from the op, so the row still has to be read.
///
/// THE VIEW LANE, NOT `/v1/query`. This is awaited inside the live stream's
/// decoder fold, so a `/v1/query` here freezes every subscriber's fold for as
/// long as the node's select loop is busy writing a checkpoint (issue #1018).
/// `ChatViewQuery::Channel` reads the same `ChannelInfo` off an MVCC snapshot,
/// off-loop — identical payload, no checkpoint tax.
///
/// An unseen row is `None`, and the caller turns it into a scoped resync rather
/// than a banner: the op named a channel this node's index cannot answer for
/// yet, and a reload is the only thing that heals that.
pub(crate) async fn load_channel_row(
    rpc: &str,
    channel_id: &str,
) -> Result<Option<ChatChannel>, String> {
    let rpc = rpc_client(rpc)?;
    let room = load_channel_facts(&rpc, channel_id, ChatReader::nobody()).await?;
    Ok(room.map(|(channel, _roster)| channel))
}

/// One scoped catch-up load, flag-selected per plane: the chat slices
/// (channel list + active window + members) and/or the pages slices (page
/// list + active blocks). Runs on stream `ready` (the subscribe→hydrate
/// ordering race), on a `resync` (lag or an unfoldable op), and debounced
/// after pages deltas — never per chat commit. Unloaded planes come back
/// with their `*_loaded` flag false and the handler keeps current state.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LiveRefresh {
    pub generation: i64,
    /// The `pages_fold_serial` the request snapshotted, echoed back so
    /// `live_resynced` can tell whether a text fold landed while this reply
    /// was in flight (#1041). A mismatch gates ONLY the fold-owned fields —
    /// page titles and block texts — never the structural pages half.
    pub fold_serial: i64,
    pub chat_loaded: bool,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub has_older_history: bool,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub huddle_roster: Vec<HuddleParticipant>,
    pub channel_members: Vec<ChatMember>,
    pub pages_loaded: bool,
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    pub comment_thread_total: i64,
    pub commented_block_hits: Vec<String>,
}

/// `planes` is `chat` | `pages` | `both` — the flat Ice surface's
/// discriminant for which slices to load ([`resync_planes`] builds it).
// the Ice extern boundary is a flat parameter list by construction — the
// same allowance every other wide extern in this module carries.
#[allow(clippy::too_many_arguments)]
pub async fn live_resync_load(
    rpc: String,
    channel_id: String,
    page_id: String,
    planes: String,
    debounce: bool,
    generation: i64,
    fold_serial: i64,
    attempt: i64,
) -> Result<LiveRefresh, HydrationError> {
    if debounce {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if attempt > 0 {
        tokio::time::sleep(retry_delay(u32::try_from(attempt).unwrap_or(u32::MAX))).await;
    }
    async {
        let rpc = rpc_client(&rpc)?;
        let mut refresh = LiveRefresh {
            generation,
            fold_serial,
            chat_loaded: false,
            channels: Vec::new(),
            messages: Vec::new(),
            has_older_history: false,
            active_channel: String::new(),
            active_channel_name: String::new(),
            active_channel_archived: false,
            active_channel_members_only: false,
            huddle_roster: Vec::new(),
            channel_members: Vec::new(),
            pages_loaded: false,
            pages: Vec::new(),
            blocks: Vec::new(),
            active_page: String::new(),
            active_page_title: String::new(),
            active_page_parent: String::new(),
            comment_thread_total: 0,
            commented_block_hits: Vec::new(),
        };
        let load_chat = planes == "chat" || planes == "both";
        let load_pages = planes == "pages" || planes == "both";
        // `both` is the arm the boot pays: the stream's `ready` event resyncs
        // each plane the moment the console connects. The two planes share
        // nothing, so they run together rather than one behind the other.
        let (chat, pages) = tokio::try_join!(
            async {
                match load_chat {
                    true => load_chat_data(
                        &rpc,
                        (!channel_id.is_empty()).then_some(channel_id.as_str()),
                    )
                    .await
                    .map(Some),
                    false => Ok(None),
                }
            },
            async {
                match load_pages {
                    true => {
                        // A LIVE REFRESH FOLLOWS SOMEONE ELSE'S WRITE, and the
                        // op that prompted it was recorded when it arrived
                        // (`folded_update`), so this read waits for the fold to
                        // carry it — the structural half of the reply is
                        // applied unconditionally, with no later op to correct
                        // a tree that predates the push.
                        load_pages_data(&rpc, (!page_id.is_empty()).then_some(page_id.as_str()))
                            .await
                            .map(Some)
                    }
                    false => Ok(None),
                }
            }
        )?;
        if let Some(chat) = chat {
            refresh.chat_loaded = true;
            refresh.channels = chat.channels;
            refresh.messages = chat.messages;
            refresh.has_older_history = chat.has_older_history;
            refresh.active_channel = chat.active_channel;
            refresh.active_channel_name = chat.active_channel_name;
            refresh.active_channel_archived = chat.active_channel_archived;
            refresh.active_channel_members_only = chat.active_channel_members_only;
            refresh.huddle_roster = chat.huddle_roster;
            refresh.channel_members = chat.channel_members;
        }
        if let Some(pages) = pages {
            refresh.pages_loaded = true;
            refresh.pages = pages.pages;
            refresh.blocks = pages.blocks;
            refresh.active_page = pages.active_page;
            refresh.active_page_title = pages.active_page_title;
            refresh.active_page_parent = pages.active_page_parent;
            refresh.comment_thread_total = pages.comment_thread_total;
            refresh.commented_block_hits = pages.commented_block_hits;
        }
        Ok(refresh)
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Did this live update say `want`'s plane went stale?
///
/// AN EXTERN, not a `let`, because the Ice checker cannot type a subscription
/// payload's field inside one (`handlers/overlays.ice` records the same
/// limitation) — which is why `forge_live_hit` is one too. Taking the wanted
/// module as an argument keeps it to a single predicate for every plane.
pub fn plane_live_hit(kind: crate::LiveKind, module: String, want: String) -> bool {
    kind == crate::LiveKind::Plane && module == want
}

/// Did this live update touch the AGENTS projection — from either module?
///
/// TWO MODULES, ONE ROW. `agent` owns the registration and `runs` owns the
/// liveness: `AgentRow.live` is `agents_with_a_run_in_flight` reading `runs`'
/// pending register, joined on in `load_agents`. So a run starting or ending
/// changes what the Forge seat's dot draws while `agent` commits nothing at
/// all — the reason the dot went dark for a whole turn once the tab move
/// stopped refetching off-tab.
///
/// Named rather than spelled inline for the same reason [`plane_live_hit`] is:
/// the Ice checker cannot type a subscription payload's field inside a `let`.
pub fn agents_plane_hit(kind: crate::LiveKind, module: String) -> bool {
    kind == crate::LiveKind::Plane && (module == "agent" || module == "runs")
}

/// The planes discriminant for [`live_resync_load`].
pub fn resync_planes(load_chat: bool, load_pages: bool) -> String {
    match (load_chat, load_pages) {
        (true, true) => "both".into(),
        (true, false) => "chat".into(),
        (false, true) => "pages".into(),
        (false, false) => String::new(),
    }
}

// per-field keepers: apply a refreshed value only when its plane loaded —
// the Ice handler assigns every field unconditionally and these self-select.

/// The channel keeper folds rather than replaces — [`upsert_channel_rows`]
/// states why — and it owns the loaded pick so the fold is never paid for on a
/// plane-only resync, which is most of them. Written as an argument
/// (`keep_channels(loaded, upsert_channel_rows(channels, next), channels)`) the
/// upsert ran on every pages-only refresh and was thrown away one call later.
/// Same early-return shape as [`resynced_messages`] below.
///
/// EXCEPT ACROSS A NETWORK. `chain_moved` says the list on screen was learned
/// from a chain this node is no longer on — a workspace switch under a console
/// that never reconnected, because the endpoint did not change — and a fold has
/// no way to express "that room does not exist here": it only ever adds. So the
/// one thing that can be true of the previous network's rooms is that they are
/// gone, and the answer replaces the list outright.
pub fn keep_channels(
    loaded: bool,
    chain_moved: bool,
    next: Vec<ChatChannel>,
    current: Vec<ChatChannel>,
) -> Vec<ChatChannel> {
    if !loaded {
        return current;
    }
    if chain_moved {
        return next;
    }
    upsert_channel_rows(current, next)
}

/// The oldest COMMITTED root's seq, or 0 for a window holding none. Pending
/// sends carry a negative seq and answer for nothing — the same rule
/// `oldest_committed` states in `load.rs`.
fn oldest_committed_seq(rows: &[ChatMessage]) -> i64 {
    rows.iter()
        .find(|row| !row.pending && row.seq > 0)
        .map_or(0, |row| row.seq)
}

/// The committed `seq` range of a timeline window, or `None` when it holds no
/// committed row at all. Pending sends carry `seq == -1` and answer for nothing
/// (the same rule `oldest_committed` states in `load.rs`).
fn committed_seq_span(rows: &[ChatMessage]) -> Option<(i64, i64)> {
    let mut seqs = rows
        .iter()
        .filter(|row| !row.pending && row.seq > 0)
        .map(|row| row.seq);
    let first = seqs.next()?;
    Some(seqs.fold((first, first), |(oldest, newest), seq| {
        (oldest.min(seq), newest.max(seq))
    }))
}

/// FOLD THE RESYNC'S PAGE ONTO THE WINDOW ON SCREEN — do not replace it.
///
/// [`load_chat_data`] answers with the latest root-index page no matter how
/// far back the reader has paged, so assigning it back threw away every
/// "Load older" page she had
/// loaded — and, the scrollable staying mounted at `anchor-y=end`, clamped her
/// offset onto the top of the suddenly-short window. The trigger is ordinary: a
/// huddle join/leave in the room on screen, a websocket reconnect, a chat op the
/// delta path cannot fold, or any of the three chat failure resyncs.
///
/// So the tail path merges with [`merge_message_send_result`] — union by `seq`
/// with the canonical row winning on `rev`, pending rows re-appended at the
/// tail — and regroups, because the retained rows and the canonical page each
/// carry the author runs of their own page and the seam between them would
/// otherwise draw a duplicate (or swallow a) run header.
///
/// A SPLICE THAT DOES NOT TOUCH REPLACES INSTEAD, and the fresh page wins.
/// Merging two windows that do not overlap leaves a HOLE in the middle that
/// nothing can ever page in: "Load older" walks back from `oldest_message_seq`,
/// which is now the far-back end, so it steps past the gap forever
/// (`handlers/chat.ice` states the same hazard for the search window). This is
/// what `ModuleEvent::Lagged` can produce: the missed ops are never replayed,
/// so the canonical page can start past the newest row on screen. One
/// overlapping `seq` is the whole test — thread replies leave gaps in the root
/// sequence, so "the pages abut" is not `+1`. A paged window that still
/// overlaps the canonical tail remains continuous and keeps the rows the
/// reader loaded.
pub fn resynced_messages(
    loaded: bool,
    chain_moved: bool,
    next: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
) -> Vec<ChatMessage> {
    // the plane-only resync, which is most of them: no chat came back, so the
    // window on screen IS the answer and the merge below is never paid for.
    if !loaded {
        return current;
    }
    // ACROSS A NETWORK NOTHING MERGES — see `keep_channels`. The rows on screen
    // were read from a chain this node is no longer on; a `seq` that overlaps
    // one in the new network's room is a coincidence, not continuity.
    if chain_moved {
        return next;
    }
    let pages_overlap = match (committed_seq_span(&next), committed_seq_span(&current)) {
        (Some((oldest_canonical, _)), Some((_, newest_held))) => oldest_canonical <= newest_held,
        _ => false,
    };
    let splice_is_continuous = pages_overlap;
    if !splice_is_continuous {
        return merge_pending_messages(next, current, current_channel, next_channel);
    }
    let mut merged = merge_message_send_result(next, current, current_channel, next_channel);
    mark_message_groups(&mut merged);
    bounded_chat_window(merged)
}

pub fn keep_members(
    loaded: bool,
    next: Vec<ChatMember>,
    current: Vec<ChatMember>,
) -> Vec<ChatMember> {
    if loaded { next } else { current }
}

/// Everything a chat load says about the huddle — the one rule, in one place,
/// for the five folds that used to spell it out in four lines each.
///
/// A LOAD CARRIES THE ROSTER OF THE CHANNEL IT LOADED, AND THAT IS NOT ALWAYS
/// THE HUDDLE'S. The docked pill and the popped panel follow you onto every
/// other room and every other screen — that is what they are FOR — so reading
/// "am I in a huddle" off the room you happen to be looking at answered no the
/// moment you clicked a second channel. And that answer is not cosmetic:
/// `huddle_joined` is the media leg's subscription gate, so a channel click cut
/// the audio and video of the call you were in, closed the window, and blanked
/// the `huddle_channel` that `leave_huddle_here` needs — leaving you on the
/// on-chain roster with no control left that could take you off it.
///
/// So: while joined, a load of ANY OTHER channel says nothing about the huddle
/// and changes nothing about it. A load of the huddle's own channel (or any
/// load at all while not joined) answers in full, and a resync that carried no
/// chat at all (`loaded == false`) answers not at all.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct HuddleAfterLoad {
    pub joined: bool,
    pub roster: Vec<HuddleParticipant>,
    pub channel: String,
    pub channel_name: String,
}

// Eight, because the rule compares two whole huddles — the standing one and
// the one the load carries. Folding either half into a struct only moves the
// four names to the call site, where five folds would each build it by hand.
#[allow(clippy::too_many_arguments)]
pub fn huddle_after_load(
    loaded: bool,
    joined: bool,
    channel: String,
    channel_name: String,
    roster: Vec<HuddleParticipant>,
    loaded_channel: String,
    loaded_channel_name: String,
    loaded_roster: Vec<HuddleParticipant>,
) -> HuddleAfterLoad {
    let standing = HuddleAfterLoad {
        joined,
        roster,
        channel,
        channel_name,
    };
    let speaks_for_the_huddle = loaded && (!joined || loaded_channel == standing.channel);
    if !speaks_for_the_huddle {
        return standing;
    }
    let joined_now = huddle_self(loaded_roster.clone());
    if !joined_now {
        return HuddleAfterLoad::default();
    }
    HuddleAfterLoad {
        joined: true,
        roster: loaded_roster,
        channel: loaded_channel,
        channel_name: loaded_channel_name,
    }
}

pub fn keep_pages(loaded: bool, next: Vec<PageItem>, current: Vec<PageItem>) -> Vec<PageItem> {
    if loaded { next } else { current }
}

pub fn keep_page_hits(
    loaded: bool,
    next: Vec<PageSearchHit>,
    current: Vec<PageSearchHit>,
) -> Vec<PageSearchHit> {
    if loaded { next } else { current }
}

/// Does this pages reply still answer for the page the app is on?
///
/// A live resync is issued with whatever page was active AT THE TIME and then
/// runs several queries. A mutation landing in between leaves the reply
/// speaking for a document nobody is looking at — measured on a page create:
/// the create's own reload corrected the selection to the new page, and a
/// resync issued a moment earlier for the OLD page answered afterwards and
/// pulled the reader back to it. Its blocks, title and comment counts are all
/// for that other document, so none of them may land.
///
/// Two replies are still current: one that resolved to the page the app is on,
/// and one whose index no longer holds that page at all — there the reply's
/// fallback is the honest answer and the app must follow it.
pub fn pages_reply_answers_current(pages: Vec<PageItem>, replied: String, current: String) -> bool {
    current.is_empty() || replied == current || !pages.iter().any(|page| page.id == current)
}

pub fn keep_strs(loaded: bool, next: Vec<String>, current: Vec<String>) -> Vec<String> {
    if loaded { next } else { current }
}

pub fn keep_blocks(loaded: bool, next: Vec<PageBlock>, current: Vec<PageBlock>) -> Vec<PageBlock> {
    if loaded { next } else { current }
}

/// Fold one committed text edit into the open document's blocks.
///
/// The whole fold, because text is the whole of what an `UpdateText` changes
/// that this shell can draw: `page_blocks` copies `block.text` verbatim and
/// carries no mark field, so this produces exactly what a reload would have.
///
/// A `block_id` this list does not hold is NOT an error — block ids are minted,
/// so it belongs to a page nobody is looking at, and the reader of that page
/// gets it from their own load. Same for any non-`text` delta: those already
/// carry `load_pages`, and folding them would mean re-deriving `prefix`,
/// `child_count` and sibling order from an op that names none of them.
pub fn apply_page_text(mut blocks: Vec<PageBlock>, delta: PagesDelta) -> Vec<PageBlock> {
    if delta.kind != "text" {
        return blocks;
    }
    let Some(block) = blocks.iter_mut().find(|block| block.id == delta.block_id) else {
        return blocks;
    };
    block.text = delta.text;
    blocks
}

/// Fold a committed RENAME into the open page's title.
///
/// `UpdateText` against a PAGE block is the rename op — there is no other one
/// (`pages::interface`, `block_ops`) — and the module-side classifier calls it
/// `text` like any other edit, because a payload naming one block id cannot
/// know which ids are pages. This shell can: `active_page` IS that id.
///
/// Without this the rename folded into NOTHING and was not merely invisible.
/// `page_blocks` drops the page head (`load.rs`, `.skip(1)`), so the block fold
/// never matches it; `load_pages` is false because the op classified as text;
/// and the title is line 0 of the buffer, so the reader's next keystroke made
/// `save_page_document` compare its stale line 0 against a FRESH read of the
/// node and write the old title back over the new one. A rename by anyone else
/// was reverted on chain by the next person to type.
pub fn apply_page_title(title: String, delta: PagesDelta, active_page: String) -> String {
    let renames_open_page = delta.kind == "text" && delta.block_id == active_page;
    if renames_open_page {
        return delta.text;
    }
    title
}

/// The same rename, in the page list — for ANY page, not just the open one.
///
/// The list holds page ids, so the delta's `block_id` landing in it IS the
/// test, and the module is what makes that sound: page ids ARE block ids in
/// one global namespace, and both writers enforce global uniqueness against
/// the whole store — `InsertBlock` refuses an id that already exists
/// (`block_ops.rs`, `PageError::DuplicateBlock`) and `CreatePage` refuses one
/// already held by a non-page block (`page_ops.rs`). So a body block's id can
/// never equal a page id, and a body edit can never rewrite a row here. (The
/// app's own `fresh_id` prefixes are a local convention and prove nothing —
/// ids are client-minted, and another writer need not use them.)
///
/// The empty title becomes `Untitled` because THAT IS WHAT A RELOAD PRODUCES
/// (`load.rs`, `page_items`), and a fold must land exactly where the reload it
/// replaces would have. Clearing line 0 submits an `UpdateText` with empty
/// text, so this is reachable from this very editor; without it the row and
/// the doc tab would go blank until some unrelated op bought a reload.
/// `active_page_title` deliberately does NOT normalize — a reload leaves it
/// verbatim (`load.rs`, `active_page_title`), so folding it verbatim matches.
pub fn apply_page_rename(mut pages: Vec<PageItem>, delta: PagesDelta) -> Vec<PageItem> {
    if delta.kind != "text" {
        return pages;
    }
    let Some(page) = pages.iter_mut().find(|page| page.id == delta.block_id) else {
        return pages;
    };
    page.title = match delta.text.is_empty() {
        true => "Untitled".into(),
        false => delta.text,
    };
    pages
}

/// Did this delta FOLD into pages state rather than buy a reload?
///
/// The stream decoder's own classification (`load_pages: !folded` above): a
/// `text` delta — a body edit or a rename — lands through `apply_page_text` /
/// `apply_page_title` / `apply_page_rename` and fetches nothing. Everything
/// else sets `load_pages`, which bumps the hydration generation and orphans
/// any resync reply in flight. That asymmetry is what makes one serial
/// sufficient for #1041: text folds are the ONLY pages writes that can land
/// inside a still-current reply window, so a moved serial names exactly the
/// folded fields as the divergence.
pub fn pages_delta_folds(delta: PagesDelta) -> bool {
    delta.kind == "text"
}

/// The row-title half of #1041's selective merge.
///
/// A reply the fold outran takes its page LIST from the reply — the list is
/// the whole index, and a row the reply carries that state does not is
/// structural news the read was issued for — but every shared row keeps the
/// title STATE holds: inside a still-current window the only way the two can
/// disagree is a rename that folded after the reply's reads executed. When
/// the reads DID see the rename the two titles agree, so over-keeping (the
/// serial cannot tell those orderings apart) costs nothing.
pub fn keep_folded_page_titles(
    fold_outran_reply: bool,
    next: Vec<PageItem>,
    current: Vec<PageItem>,
) -> Vec<PageItem> {
    if !fold_outran_reply {
        return next;
    }
    let folded_titles: BTreeMap<&str, &str> = current
        .iter()
        .map(|page| (page.id.as_str(), page.title.as_str()))
        .collect();
    next.into_iter()
        .map(|mut page| {
            if let Some(title) = folded_titles.get(page.id.as_str()) {
                page.title = (*title).to_string();
            }
            page
        })
        .collect()
}

/// The block-TEXT half of the same merge. Structure — which blocks exist,
/// their order, kind, depth — is the reply's: it is what the read was issued
/// for, and no structural delta can land inside a still-current window
/// without orphaning the reply. Text is the fold's: `apply_page_text` writes
/// nothing else, and a folded text lost to a pre-fold reply is not merely
/// stale on screen — a clean buffer rebuilt from it makes the reader's next
/// keystroke plan the OLD text back onto the chain (`document_plan` is a
/// two-way diff, and body lines have no authorship guard the way the title
/// has `title_write_owed`). Pending optimistic blocks are no fold, so they
/// are not an overlay source; `merge_pending_blocks` re-seats them.
pub fn keep_folded_block_texts(
    fold_outran_reply: bool,
    next: Vec<PageBlock>,
    current: Vec<PageBlock>,
) -> Vec<PageBlock> {
    if !fold_outran_reply {
        return next;
    }
    let folded_texts: BTreeMap<&str, &str> = current
        .iter()
        .filter(|block| !block.pending)
        .map(|block| (block.id.as_str(), block.text.as_str()))
        .collect();
    next.into_iter()
        .map(|mut block| {
            if let Some(text) = folded_texts.get(block.id.as_str()) {
                block.text = (*text).to_string();
            }
            block
        })
        .collect()
}

pub fn keep_str(loaded: bool, next: &str, current: &str) -> String {
    if loaded { next } else { current }.to_owned()
}

pub fn keep_forge_phase(
    loaded: bool,
    next: crate::ForgePhase,
    current: crate::ForgePhase,
) -> crate::ForgePhase {
    if loaded { next } else { current }
}

pub fn keep_bool(loaded: bool, next: bool, current: bool) -> bool {
    if loaded { next } else { current }
}

pub fn keep_i64(loaded: bool, next: i64, current: i64) -> i64 {
    if loaded { next } else { current }
}

/// The channel the reader just clicked, without cloning or re-paging the
/// channel list. Its timeline is one root-index page; no head hint is needed.
///
/// A GENERATION, NOT AN `AppError` — the same reason [`connect`] fails with
/// one. Nothing serializes these any more (`choose_channel` takes every click
/// and drops the superseded REPLY), so a failure has to be able to say which
/// switch it belongs to: without that, B erroring after the reader has clicked
/// on to C clears `loading` under C, swapping C's plate for "No messages yet",
/// and writes B's error into the banner. `committed` is what `AppError` adds,
/// and a room switch has nothing to commit.
pub async fn load_channel_window(
    rpc: String,
    channel_id: String,
    generation: i64,
) -> Result<ChatData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let mut chat = load_channel_window_data(&rpc, &channel_id, MessageWindow::Tail).await?;
        chat.generation = generation;
        Ok(chat)
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The reply a search hit points at, when it points at one at all. A hit on the
/// thread ROOT is answered by the window around it and needs no second read.
async fn load_hit_reply(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    target_seq: u64,
) -> Result<Option<MsgRow>, String> {
    if target_seq == root_seq {
        return Ok(None);
    }
    load_message_at(rpc, channel_id, target_seq).await.map(Some)
}

/// Same generation-carrying failure as [`load_channel_window`], same reason.
pub async fn load_chat_hit(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    generation: i64,
) -> Result<ChatData, HydrationError> {
    async {
        let root_seq = positive_sequence(root_seq)?;
        let target_seq = positive_sequence(target_seq)?;
        let rpc = rpc_client(&rpc)?;
        // THREE SEQUENTIAL PHASES, CONCURRENT. This used to re-page the channel
        // list, walk the channel's live tail, THROW that walk away for a window
        // around the hit, and only then read the reply — the slowest navigation
        // in the app, with the pane on the loading plate for all of it. The
        // window and the reply are independent of the channel's row and its
        // member roll, so the whole thing is one round trip now.
        let (mut chat, reply) = tokio::try_join!(
            load_channel_window_data(&rpc, &channel_id, MessageWindow::Around(root_seq)),
            load_hit_reply(&rpc, &channel_id, root_seq, target_seq)
        )?;
        let root = chat
            .messages
            .iter()
            .find(|message| message.seq == number_i64(root_seq))
            .cloned()
            .ok_or_else(|| "message was not found".to_string())?;
        chat.generation = generation;
        chat.selected_message_seq = root.seq;
        chat.selected_message_rev = root.rev;
        chat.selected_message_body.clone_from(&root.body);
        let Some(reply) = reply else {
            return Ok(chat);
        };
        if reply.thread != Some(root_seq) {
            return Err("search result does not belong to the selected thread".into());
        }
        let facts = ReaderFacts::current().await;
        chat.active_thread_seq = root.seq;
        chat.thread_target_seq = number_i64(target_seq);
        chat.thread_messages = vec![root, chat_message(reply, facts.reader())];
        Ok(chat)
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Land on the channel that was just made.
///
/// `submit_frame` returns when the node ACCEPTS a transaction, not when it
/// applies one, so a reload issued immediately after can read an index that
/// does not hold the new channel yet. `load_chat_data` is RIGHT to drop a
/// requested id it cannot see and fall back to `channels.first()` — a live
/// refresh can legitimately name a channel that has since been archived away —
/// so the correction belongs in the callers that know their id is good.
///
/// The fallback is not cosmetic here. Opening a DM left the DM header on
/// screen while `active_channel` pointed at whatever unrelated public room
/// sorted first, so the composer under that header posted into it.
///
/// `members` is seeded rather than left for the refresh because `post_gate`
/// refuses a members-only channel the viewer is not seated in: an empty list
/// would gate the composer of the DM you just opened.
fn landed_on_channel(
    mut data: ChatData,
    channel_id: String,
    name: String,
    members_only: bool,
    members: Vec<ChatMember>,
) -> ChatData {
    if data.active_channel == channel_id {
        return data;
    }
    data.active_channel = channel_id;
    data.active_channel_name = name;
    data.active_channel_archived = false;
    data.active_channel_members_only = members_only;
    data.huddle_roster = Vec::new();
    data.channel_members = members;
    data.messages = Vec::new();
    data.has_older_history = false;
    data.selected_message_seq = 0;
    data.selected_message_rev = 0;
    data.selected_message_body = String::new();
    data.active_thread_seq = 0;
    data.thread_target_seq = 0;
    data.thread_messages = Vec::new();
    data.thread_has_more = false;
    data
}

pub async fn create_channel(
    rpc: String,
    password: String,
    name: String,
    members_only: bool,
    generation: i64,
) -> Result<ChatData, AppError> {
    async {
        let name = bounded_text(name, "channel name", 128)?;
        let landing_name = name.clone();
        let channel_id = fresh_id("channel");
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel_id.clone(),
                name,
                post_policy: match members_only {
                    true => PostPolicy::MembersOnly,
                    false => PostPolicy::Open,
                },
            }),
            password,
        )
        .await?;
        let data = load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)?;
        let mut data = landed_on_channel(data, channel_id, landing_name, members_only, Vec::new());
        data.generation = generation;
        Ok(data)
    }
    .await
}

/// One peer of the DM directory. There is no `status`: presence has no source
/// anywhere in the product, and a dot that always reads "offline" is a lie.
///
/// `is_agent` is always false today — see [`load_dm_peers`].
///
/// `channel_id` is the pair's deterministic two-party channel id
/// (`dm_channel_id(me, key)`), computed once at load time rather than at
/// every render. The prepared DIRECT projection uses it to attach the row's
/// scalar unread reading when channels or read cursors move.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct DmPeer {
    pub key: String,
    pub name: String,
    pub initials: String,
    pub is_agent: bool,
    pub channel_id: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct DmPeersData {
    pub generation: i64,
    pub peers: Vec<DmPeer>,
}

/// The people this device can open a DM with, one row per identity account.
///
/// Registered agents are NOT here: a DM is a chat channel seated on public
/// KEYS, and an agent id is an arbitrary string that [`open_dm`]'s
/// `public_key()` would reject outright. Until an agent is addressable as a
/// channel member, an agent row in this directory would be a button that
/// cannot work.
///
/// The row is keyed on the account NUMBER (decimal), so every key of a
/// multi-device account reaches the same row, and the channel id hashes the
/// PAIR OF ACCOUNT NUMBERS so both ends of one DM land on the same room.
pub async fn load_dm_peers(rpc: String, generation: i64) -> Result<DmPeersData, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let me = local_user_key().await;
        // The same read that refreshes the name directory: an identity op
        // reloads this directory, and every label on screen moves with it.
        let accounts = read_accounts(&client).await?;
        // self is the account THIS key is a member of (a key holds at most one).
        let is_mine = |account: &AccountView| {
            me.as_ref()
                .is_some_and(|me| account.keys.iter().any(|key| &key.pubkey == me))
        };
        let my_number = accounts
            .iter()
            .find(|account| is_mine(account))
            .map(|account| account.number.to_string());
        let mut peers: Vec<DmPeer> = Vec::new();
        for account in accounts {
            if is_mine(&account) {
                continue;
            }
            let key = account.number.to_string();
            let name = account.name;
            let channel_id = my_number
                .as_ref()
                .map(|mine| dm_channel_id(mine.clone(), key.clone()))
                .unwrap_or_default();
            peers.push(DmPeer {
                initials: initials_of(&name),
                is_agent: false,
                key,
                name,
                channel_id,
            });
        }
        Ok(DmPeersData { generation, peers })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The two-party channel id of a pair of member keys, derived by the chat
/// module's own client so the app and the module agree on one id.
///
/// It carries NO ':' deliberately: chat refuses a user-authored channel id in
/// the module namespace (`crates/modules/apps/chat/src/lib.rs`,
/// `validate_channel_namespace`), and a DM is created by the pair's own user
/// key. `chat::client`'s test round-trips the minted id against that rule.
pub fn dm_channel_id(a: String, b: String) -> String {
    chat::client::dm_channel_id(&a, &b)
}

/// THE DM PEER IS A READING OF THE ROOM ON SCREEN, NOT A FLAG THAT OUTLIVES IT.
/// `peer` survives only while `channel` is that peer's own two-party channel;
/// every other room answers "".
///
/// `active_dm_peer` decides the whole header, one step removed: it resolves the
/// row (`dm_peer_named`) that the header actually branches on, so a peer this
/// key names draws `DmHeader` with his avatar and name and SUPPRESSES the `#`
/// glyph and `active_channel_name` — while a key whose peer has left the
/// identity roster resolves to the blank row and falls THROUGH to that title
/// (the `empty(active_dm.name)` arms in `screens/chat.ice`), instead of drawing
/// a nameless avatar plate the way branching on the key itself did.
///
/// Cleared by the channel picker alone, it rode a search hit, a create, a
/// reconnect and every resync into another room — Alice's face over #general's
/// timeline, with the room the composer actually posts into never named. So
/// every landing that assigns `active_channel` from a reply re-derives the peer
/// through here, and the field cannot disagree with the room again.
///
/// THE DIRECTORY'S OWN ID DECIDES, for the reason `chat_sidebar_rooms` gives:
/// `DmPeer.channel_id` was derived once, in `load_dm_peers`, from the account
/// number that load resolved for itself. Re-hashing it here against a separate
/// `account_number` reading made the header disagree with the sidebar whenever
/// that reading was late or missing — the peer's own room drew as a `#` channel
/// under his name in DIRECT.
pub fn dm_peer_of_channel(peer: String, peers: Vec<DmPeer>, channel: String) -> String {
    let peer_owns_the_room = peers
        .iter()
        .any(|row| row.key == peer && !row.channel_id.is_empty() && row.channel_id == channel);
    match peer_owns_the_room {
        true => peer,
        false => String::new(),
    }
}

/// The room a DIRECT row opens — the id `load_dm_peers` derived for that peer,
/// empty when the directory does not name him (or names him with no account
/// number of ours to pair against).
pub fn dm_room_of_peer(peers: Vec<DmPeer>, peer: String) -> String {
    peers
        .into_iter()
        .find(|row| row.key == peer)
        .map(|row| row.channel_id)
        .unwrap_or_default()
}

/// THE DM HEADER'S OWN ROW, resolved where `active_dm_peer` is written.
///
/// The header used to be a filter — `for peer in dm_peers` / `if peer.key ==
/// active_dm_peer` — which the extern-free view can express but which
/// deep-clones every peer AND allocates a per-child scope String, per frame, so
/// that at most one of them renders. A peer who has left the identity roster
/// resolves to the blank row, and the header falls through to the `#` title the
/// way the filter's no-match arm did.
pub fn dm_peer_named(peers: Vec<DmPeer>, key: String) -> DmPeer {
    peers
        .into_iter()
        .find(|peer| peer.key == key)
        .unwrap_or_default()
}

/// The blank peer — "no DM on screen", and the state field's own default.
pub fn no_dm_peer() -> DmPeer {
    DmPeer::default()
}

/// Open the DM with one peer (an account number): resolve the deterministic
/// channel when it exists, else create it members-only and seat every key of
/// the peer's account plus this device's, then load it.
///
/// NOT confidential. `MembersOnly` gates who may POST; every node replicates
/// the channel's plaintext, so a DM is a two-person room, not a private one.
/// Any copy on this surface that promises secrecy is a lie about the wire.
///
/// Fails with a generation for the reason [`load_channel_window`] gives: this
/// is one of the three routes that move the reader between rooms, and a
/// superseded failure must not land under the room she is in now. The writes it
/// makes are idempotent by construction — `dm_channel_id` is deterministic and
/// `SetMembership` is a set — so `committed` had nothing to warn about.
pub async fn open_dm(
    rpc: String,
    password: String,
    peer_key: String,
    generation: i64,
) -> Result<ChatData, HydrationError> {
    async {
        let number: u64 = peer_key
            .trim()
            .parse()
            .map_err(|_| "peer must be an account number".to_string())?;
        let me = local_user_key()
            .await
            .ok_or_else(|| "this device has no user key — a DM needs one".to_string())?;
        let client = rpc_client(&rpc)?;
        let reply: IdentityReply = client
            .query("identity", &IdentityQuery::OfKey { key: me })
            .await?;
        let mine = match reply {
            IdentityReply::Account(account) => account,
            IdentityReply::Accounts(_) | IdentityReply::Gen(_) => {
                return Err("the identity module returned the wrong reply".to_string());
            }
        };
        let mine = mine.ok_or_else(|| "this key is on no account — a DM needs one".to_string())?;
        let channel_id = dm_channel_id(mine.number.to_string(), number.to_string());
        let mut existing = load_chat_data(&client, Some(&channel_id)).await?;
        if existing.active_channel == channel_id {
            existing.generation = generation;
            return Ok(existing);
        }
        let reply: IdentityReply = client
            .query("identity", &IdentityQuery::Get { number })
            .await?;
        let account = match reply {
            IdentityReply::Account(account) => account,
            IdentityReply::Accounts(_) | IdentityReply::Gen(_) => {
                return Err("the identity module returned the wrong reply".to_string());
            }
        };
        let account = account.ok_or_else(|| format!("account {number} does not exist"))?;
        let peer_name = account.name;
        // every key of both accounts is seated, so any device of either end
        // reads and posts in the room.
        let my_keys = mine.keys.into_iter().map(|key| key.pubkey);
        let peer_keys = account.keys.into_iter().map(|key| key.pubkey);
        let members: Vec<Vec<u8>> = my_keys.chain(peer_keys).collect();
        signed_write(
            &client,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel_id.clone(),
                name: peer_name.clone(),
                post_policy: PostPolicy::MembersOnly,
            }),
            password.clone(),
        )
        .await?;
        let names = names();
        let seated = members
            .iter()
            .map(|key| {
                let handle = hex_encode(key);
                ChatMember {
                    label: names.member_label(&handle),
                    key: handle,
                }
            })
            .collect();
        for member in members {
            signed_write(
                &client,
                "chat",
                chat::encode_msg(&ChatMsg::SetMembership {
                    channel_id: channel_id.clone(),
                    user: member,
                    member: true,
                }),
                password.clone(),
            )
            .await?;
        }
        let data = load_chat_data(&client, Some(&channel_id)).await?;
        let mut data = landed_on_channel(data, channel_id, peer_name, true, seated);
        data.generation = generation;
        Ok(data)
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Why the viewer may not post here, as a stable reason token — empty when
/// she may. A members-only channel she is not seated in refuses her post; a
/// seat is hers under any key of her account ([`seated_in`]).
pub fn post_gate(
    archived: bool,
    members_only: bool,
    members: Vec<ChatMember>,
    me: String,
) -> String {
    if archived {
        return "channel_archived".into();
    }
    let seated = seated_in(&members, &me);
    if members_only && !seated {
        return "members_only".into();
    }
    String::new()
}

/// THE BANNER A REFUSED REACTION LEAVES BEHIND — and, on a live channel, the
/// banner already on screen, returned untouched.
///
/// The reaction handlers refuse an archived channel because the module does
/// (`check_post_policy`, reached through `reaction_target`), and until now they
/// refused in silence: no `error`, no state change, no visible difference from
/// a reaction that landed. The surface cannot carry that refusal instead — the
/// quiet message rows are `lazy` on ONE dependency, so `active_channel_archived`
/// never reaches the chips or the one-tap bar, and every row keeps its full
/// hover/press ramp. So the refusal has to speak.
///
/// It carries the banner through rather than clearing it because Ice handlers
/// are straight-line — a `return if` guard, never a branch — so the refusing
/// write happens on the live path too. Opening the ♡ picker is a read: it must
/// not wipe a failed send the reader has not read yet. The three mutations
/// clear the banner on their own line, where they always did.
pub fn reaction_refusal(archived: bool, banner: String) -> String {
    match archived {
        true => "This channel is archived — reactions are closed. Unarchive it from Channel details to react here again.".into(),
        false => banner,
    }
}
