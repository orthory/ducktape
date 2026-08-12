use super::*;
use ::chat;
use ::forge;

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
    struct State {
        rpc: String,
        cursors: BTreeMap<String, String>,
        stream: Option<ducktape_rpc::ModuleEventStream>,
        retry_attempt: u32,
    }

    iced::futures::stream::unfold(
        State {
            rpc,
            cursors: BTreeMap::new(),
            stream: None,
            retry_attempt: 0,
        },
        |mut state| async move {
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
                        return Some((live_retry(error), state));
                    }
                }
            }
            loop {
                let event = state
                    .stream
                    .as_mut()
                    .expect("stream initialized above")
                    .next()
                    .await;
                let update = match event {
                    Some(Ok(ModuleEvent::Ready { cursors })) => {
                        state.cursors = cursors;
                        state.retry_attempt = 0;
                        live_update("ready", "Live", -1)
                    }
                    Some(Ok(ModuleEvent::Changed { module, cursor, op })) => {
                        state.cursors.insert(format!("module:{module}"), cursor);
                        match folded_update(&state.rpc, &module, *op).await {
                            Some(update) => update,
                            // invisible to the UI (hook registration) — keep
                            // draining without emitting.
                            None => continue,
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
                    Some(Ok(ModuleEvent::Refused { .. })) => continue,
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
                        "tip",
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
                return Some((update, state));
            }
        },
    )
    .boxed()
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
    let Some(payload) = op
        .payload
        .as_ref()
        .and_then(|value| serde_json::to_vec(value).ok())
    else {
        return Some(live_resync(module, height));
    };
    match module {
        "chat" => {
            let current_user = local_user_key().await;
            let origin_kind = stream_origin_kind(&op.origin.kind);
            let folded = chat::client::delta_from_op(
                &payload,
                op.assigned.as_ref(),
                origin_kind,
                op.origin.id.as_deref(),
                current_user.as_deref(),
                op.height,
            );
            let mut delta = match folded {
                Ok(Some(delta)) => delta,
                Ok(None) => return None,
                Err(_) => return Some(live_resync("chat", height)),
            };
            // huddle membership is roster-derived — reload the one channel
            // row from its canonical record instead of guessing the count.
            if delta.kind == "channel-refresh" {
                match load_channel_row(rpc, &delta.channel_id).await {
                    Ok(channel) => {
                        delta.kind = "channel-updated".into();
                        delta.channel = channel;
                    }
                    Err(_) => return Some(live_resync("chat", height)),
                }
            }
            Some(LiveUpdate {
                kind: "chat".into(),
                status: format!("Live · block {height}"),
                height,
                module: "chat".into(),
                load_chat: false,
                load_pages: false,
                debounce: false,
                chat: delta,
                pages: PagesDelta::default(),
                bell: BellDelta::default(),
                forge: ForgeRefresh::default(),
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
                    kind: "bell".into(),
                    status: format!("Live · block {height}"),
                    height,
                    module: "inbox".into(),
                    load_chat: false,
                    load_pages: false,
                    debounce: false,
                    chat: ChatDelta::default(),
                    pages: PagesDelta::default(),
                    bell,
                    forge: ForgeRefresh::default(),
                }),
                Ok(None) => None,
                Err(_) => None,
            }
        }
        "pages" => match pages::client::delta_from_op(&payload) {
            Ok(delta) => {
                let folded = delta.kind == "text";
                Some(LiveUpdate {
                    kind: "pages".into(),
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
                    chat: ChatDelta::default(),
                    pages: delta,
                    bell: BellDelta::default(),
                    forge: ForgeRefresh::default(),
                })
            }
            Err(_) => Some(live_resync("pages", height)),
        },
        "forge" => match forge::client::refresh_from_op(&payload) {
            Ok(refresh) => Some(LiveUpdate {
                kind: "forge".into(),
                status: format!("Live · block {height}"),
                height,
                module: "forge".into(),
                load_chat: false,
                load_pages: false,
                // pushes arrive in bursts (one op per ref batch, then the
                // tracker follow-ups) — coalesce the reloads like pages does.
                debounce: true,
                chat: ChatDelta::default(),
                pages: PagesDelta::default(),
                bell: BellDelta::default(),
                forge: refresh,
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
        "valset" | "governance" | "identity" | "agent" | "files" => {
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
pub(crate) async fn load_channel_row(rpc: &str, channel_id: &str) -> Result<ChatChannel, String> {
    let rpc = rpc_client(rpc)?;
    let (channel, _roster) = load_channel_facts(&rpc, channel_id, None).await?;
    Ok(channel)
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
    pub chat_loaded: bool,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub active_channel_huddle_count: i64,
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
pub async fn live_resync_load(
    rpc: String,
    channel_id: String,
    page_id: String,
    planes: String,
    debounce: bool,
    generation: i64,
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
            chat_loaded: false,
            channels: Vec::new(),
            messages: Vec::new(),
            active_channel: String::new(),
            active_channel_name: String::new(),
            active_channel_archived: false,
            active_channel_members_only: false,
            active_channel_huddle_count: 0,
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
            refresh.active_channel = chat.active_channel;
            refresh.active_channel_name = chat.active_channel_name;
            refresh.active_channel_archived = chat.active_channel_archived;
            refresh.active_channel_members_only = chat.active_channel_members_only;
            refresh.active_channel_huddle_count = chat.active_channel_huddle_count;
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
pub fn plane_live_hit(kind: String, module: String, want: String) -> bool {
    kind == "plane" && module == want
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

pub fn keep_channels(
    loaded: bool,
    next: Vec<ChatChannel>,
    current: Vec<ChatChannel>,
) -> Vec<ChatChannel> {
    if loaded { next } else { current }
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
/// [`load_chat_data`] answers with the LATEST page (the last
/// `CHAT_TIMELINE_ROOT_QUOTA` roots) no matter how far back the reader has
/// paged, so assigning it back threw away every "Load older" page she had
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
/// (`handlers/chat.ice` states the same hazard for the search window). Two
/// landings are non-contiguous — a `history_view` window, which is a snapshot
/// around one old hit; and a tail the client fell too far behind to still
/// reach, which is what `ModuleEvent::Lagged` means: the missed ops are never
/// replayed, so the canonical page can start past the newest row on screen. One
/// overlapping `seq` is the whole test — thread replies leave gaps in the root
/// sequence, so "the pages abut" is not `+1`.
pub fn resynced_messages(
    loaded: bool,
    next: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
    current_channel: String,
    next_channel: String,
    history_view: bool,
) -> Vec<ChatMessage> {
    // the plane-only resync, which is most of them: no chat came back, so the
    // window on screen IS the answer and the merge below is never paid for.
    if !loaded {
        return current;
    }
    let pages_overlap = match (committed_seq_span(&next), committed_seq_span(&current)) {
        (Some((oldest_canonical, _)), Some((_, newest_held))) => oldest_canonical <= newest_held,
        _ => false,
    };
    let splice_is_continuous = !history_view && pages_overlap;
    if !splice_is_continuous {
        return merge_pending_messages(next, current, current_channel, next_channel, String::new());
    }
    let mut merged =
        merge_message_send_result(next, current, current_channel, next_channel, String::new());
    mark_message_groups(&mut merged);
    merged
}

pub fn keep_members(
    loaded: bool,
    next: Vec<ChatMember>,
    current: Vec<ChatMember>,
) -> Vec<ChatMember> {
    if loaded { next } else { current }
}

/// The huddle roster, kept only while this device is IN the huddle. Every chat
/// load carries the roster of the channel it loaded, so a load of any other
/// channel must not leave the popped panel painting strangers — dropping it is
/// the same guard `huddle_channel` itself carries.
pub fn keep_roster(joined: bool, next: Vec<HuddleParticipant>) -> Vec<HuddleParticipant> {
    if joined { next } else { Vec::new() }
}

/// A huddle roster changed on the channel ON SCREEN. The delta path answers
/// the sidebar dot through the refreshed channel row, but `huddle_joined` and
/// `huddle_roster` are recomputed only by a chat reload — and the join/leave
/// ack path carries no roster at all — so exactly this delta must trigger
/// one. Without it the LIVE pill appeared only after a manual channel
/// re-pick (or a restart), which read as "huddle does nothing".
pub fn huddle_refresh_hits(delta: ChatDelta, active_channel: String) -> bool {
    delta.kind == "channel-updated" && delta.channel_id == active_channel
}

/// [`keep_roster`]'s loaded-gated half: a resync that did NOT load chat must
/// leave the roster alone rather than blank it.
pub fn keep_participants(
    loaded: bool,
    next: Vec<HuddleParticipant>,
    current: Vec<HuddleParticipant>,
) -> Vec<HuddleParticipant> {
    if loaded { next } else { current }
}

/// The peers table's own keep: a pushed overview frame answers ONE of the two
/// snapshot topics, so the half it did not carry must survive it.
pub fn keep_peers(loaded: bool, next: Vec<PeerRow>, current: Vec<PeerRow>) -> Vec<PeerRow> {
    if loaded { next } else { current }
}

pub fn keep_pages(loaded: bool, next: Vec<PageItem>, current: Vec<PageItem>) -> Vec<PageItem> {
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

pub fn keep_str(loaded: bool, next: String, current: String) -> String {
    if loaded { next } else { current }
}

pub fn keep_bool(loaded: bool, next: bool, current: bool) -> bool {
    if loaded { next } else { current }
}

pub fn keep_i64(loaded: bool, next: i64, current: i64) -> i64 {
    if loaded { next } else { current }
}

/// The channel the reader just clicked, loaded against the channel list she is
/// already looking at — see [`load_channel_window_data`] for why the list is
/// passed in rather than re-paged.
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
    channels: Vec<ChatChannel>,
    channel_id: String,
    generation: i64,
) -> Result<ChatData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let mut chat =
            load_channel_window_data(&rpc, channels, &channel_id, MessageWindow::Tail).await?;
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
    channels: Vec<ChatChannel>,
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
            load_channel_window_data(&rpc, channels, &channel_id, MessageWindow::Around(root_seq)),
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
        let current_user = local_user_key().await;
        chat.active_thread_seq = root.seq;
        chat.thread_target_seq = number_i64(target_seq);
        chat.thread_messages = vec![root, chat_message(reply, current_user.as_deref())];
        chat.thread_next_reply_offset = -1;
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
    data.active_channel_huddle_count = 0;
    data.huddle_roster = Vec::new();
    data.channel_members = members;
    data.messages = Vec::new();
    data.selected_message_seq = 0;
    data.selected_message_rev = 0;
    data.selected_message_body = String::new();
    data.active_thread_seq = 0;
    data.thread_target_seq = 0;
    data.thread_messages = Vec::new();
    data.thread_next_reply_offset = 0;
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
/// ponytail: the row is keyed on `account_id`, the account's FOUNDING member
/// key, so a multi-device account is reachable only at that key. Pair-wide DMs
/// need account-keyed membership in the chat module itself.
pub async fn load_dm_peers(rpc: String, generation: i64) -> Result<DmPeersData, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let client = rpc_client(&rpc)?;
        let me = local_user_key().await.map(|key| hex_encode(&key));
        let reply: serde_json::Value = client
            .query(
                "identity",
                &serde_json::json!({ "all": { "from": 0, "limit": 256 } }),
            )
            .await?;
        let mut peers: Vec<DmPeer> = Vec::new();
        for account in reply["accounts"].as_array().cloned().unwrap_or_default() {
            // self is any account THIS key is a member of — comparing against
            // `account_id` alone puts a second device in its own directory.
            let mine = account["member_keys"].as_array().is_some_and(|keys| {
                keys.iter().any(|member| {
                    me.as_deref() == Some(hex_encode(&json_bytes(&member["pubkey"])).as_str())
                })
            });
            if mine {
                continue;
            }
            let key = hex_encode(&json_bytes(&account["account_id"]));
            let name = match account["display_name"].as_str() {
                Some(name) if !name.is_empty() => name.to_string(),
                _ => short_label(&key),
            };
            let channel_id = me
                .as_deref()
                .map(|me| dm_channel_id(me.to_string(), key.clone()))
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
/// A device with no user key derives no DM id at all, so it holds no DM — the
/// same answer `chat_sidebar_rooms` gives when `me` is empty.
pub fn dm_peer_of_channel(peer: String, me: String, channel: String) -> String {
    let peer_owns_the_room =
        !peer.is_empty() && !me.is_empty() && dm_channel_id(me, peer.clone()) == channel;
    match peer_owns_the_room {
        true => peer,
        false => String::new(),
    }
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

/// Open the DM with one peer: resolve the deterministic channel when it
/// exists, else create it members-only and seat both keys, then load it.
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
        let peer = public_key(&peer_key, "peer public key")?;
        let me = local_user_key()
            .await
            .ok_or_else(|| "this device has no user key — a DM needs one".to_string())?;
        let channel_id = dm_channel_id(hex_encode(&me), hex_encode(&peer));
        let peer_name = short_label(&hex_encode(&peer));
        let client = rpc_client(&rpc)?;
        let mut existing = load_chat_data(&client, Some(&channel_id)).await?;
        if existing.active_channel == channel_id {
            existing.generation = generation;
            return Ok(existing);
        }
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
        let seated = [&me, &peer]
            .map(|key| {
                let handle = hex_encode(key);
                ChatMember {
                    label: short_label(&handle),
                    key: handle,
                }
            })
            .to_vec();
        for member in [me, peer] {
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
/// she may. A members-only channel she is not seated in refuses her post.
pub fn post_gate(
    archived: bool,
    members_only: bool,
    members: Vec<ChatMember>,
    me: String,
) -> String {
    if archived {
        return "channel_archived".into();
    }
    let seated = members.iter().any(|member| member.key == me);
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
