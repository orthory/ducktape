use super::*;

pub(crate) async fn load_workspace(
    rpc: &RpcClient,
    channel_id: Option<&str>,
    page_id: Option<&str>,
    generation: i64,
) -> Result<WorkspaceData, String> {
    // Three independent trees. Serialized, chat's first paint waited on a
    // status probe, then on the whole pages plane — a page index plus the
    // consensus `/v1/query` blocks read — none of which the message list
    // renders. Concurrent, the console opens on the slowest leg, not their sum.
    let (status, chat, pages) = tokio::try_join!(
        async { rpc.status().await.map_err(String::from) },
        load_chat_data(rpc, channel_id),
        load_pages_data(rpc, page_id)
    )?;
    let tip = tip_from_status(status)?;
    Ok(WorkspaceData {
        generation,
        rpc: rpc.origin().to_string(),
        status: tip.status,
        height: tip.height,
        channels: chat.channels,
        messages: chat.messages,
        has_older_history: chat.has_older_history,
        active_channel: chat.active_channel,
        active_channel_name: chat.active_channel_name,
        active_channel_archived: chat.active_channel_archived,
        active_channel_members_only: chat.active_channel_members_only,
        huddle_roster: chat.huddle_roster,
        channel_members: chat.channel_members,
        pages: pages.pages,
        blocks: pages.blocks,
        active_page: pages.active_page,
        active_page_title: pages.active_page_title,
        active_page_parent: pages.active_page_parent,
        comment_thread_total: pages.comment_thread_total,
        commented_block_hits: pages.commented_block_hits,
    })
}

fn tip_from_status(status: NodeStatus) -> Result<Tip, String> {
    let height = i64::try_from(status.height).map_err(|_| "node height exceeds i64")?;
    Ok(Tip {
        height,
        status: format!("Connected · block {height}"),
    })
}

/// Where a cold start lands when nothing was asked for. The wire orders
/// channels by ID, so "the first one" is an accident of naming — in the demo
/// workspace it is a `channel-1786073…` created minutes ago with nothing in it,
/// and the console opens on "No messages yet" while three rooms carrying
/// hundreds of messages sit under it. Land on somewhere with something to read.
///
/// Each fallback answers a workspace the one above it cannot: every room empty,
/// then every room archived. The last rung keeps the old behaviour so a landing
/// still happens rather than the console opening on no channel at all.
pub(crate) fn landing_channel(channels: &[ChatChannel]) -> Option<&ChatChannel> {
    let has_traffic = |channel: &&ChatChannel| !channel.archived && channel.head_seq > 0;
    let is_open = |channel: &&ChatChannel| !channel.archived;
    channels
        .iter()
        .find(has_traffic)
        .or_else(|| channels.iter().find(is_open))
        .or_else(|| channels.first())
}

pub(crate) async fn load_chat_data(
    rpc: &RpcClient,
    requested: Option<&str>,
) -> Result<ChatData, String> {
    let mut wire_channels = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: ChatViewReply = rpc
            .view(
                "chat",
                &ChatViewQuery::Channels {
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let ChatViewReply::Channels {
            channels: page,
            has_more,
            next_after,
        } = reply
        else {
            return Err("node returned an invalid channel list".into());
        };
        wire_channels.extend(page);
        if !has_more {
            break;
        }
        after = next_after;
        if after.is_none() {
            break;
        }
    }
    let channels = wire_channels
        .iter()
        .map(|info| ChatChannel {
            id: info.channel.id.clone(),
            name: info.channel.name.clone(),
            archived: info.channel.archived,
            members_only: info.channel.post_policy == PostPolicy::MembersOnly,
            huddle_count: count_i64(info.channel.huddle.len()),
            head_seq: number_i64(info.head_seq),
        })
        .collect::<Vec<_>>();
    let active_channel = requested
        .filter(|id| channels.iter().any(|channel| channel.id == *id))
        .map(str::to_string)
        .or_else(|| landing_channel(&channels).map(|channel| channel.id.clone()))
        .unwrap_or_default();
    let active_channel_name = channels
        .iter()
        .find(|channel| channel.id == active_channel)
        .map(|channel| channel.name.clone())
        .unwrap_or_default();
    // WHAT THE READER IS LOOKING AT, recorded where it is decided. This load
    // runs on every room open and every chat resync, and it is the only place
    // that knows both the room names and which one won — the live decoder,
    // which needs both to word and to suppress a desktop notification, may not
    // query for either inside the fold. See `notify`.
    note_rooms(&channels, &active_channel);
    let active_wire_channel = wire_channels
        .iter()
        .find(|info| info.channel.id == active_channel);
    let active_channel_archived = active_wire_channel.is_some_and(|info| info.channel.archived);
    let active_channel_members_only =
        active_wire_channel.is_some_and(|info| info.channel.post_policy == PostPolicy::MembersOnly);
    let me = local_user_key().await;
    let names = account_names(rpc).await;
    let huddle_roster = active_wire_channel.map_or_else(Vec::new, |info| {
        huddle_roster(&info.channel.huddle, me.as_deref(), &names)
    });
    // Both read only the active channel, which is decided above — the member
    // roll has no business sitting in front of the timeline. `local_user_key`
    // is awaited before this so the cached identity is warm for both legs
    // (there is no single-flight; two cold callers would each spawn the CLI).
    let (channel_members, message_page) = match active_channel.is_empty() {
        true => (Vec::new(), RootPage::default()),
        false => tokio::try_join!(
            load_channel_members(rpc, &active_channel),
            load_messages(rpc, &active_channel)
        )?,
    };
    Ok(ChatData {
        generation: 0,
        channels,
        messages: message_page.messages,
        has_older_history: message_page.has_more,
        active_channel,
        active_channel_name,
        active_channel_archived,
        active_channel_members_only,
        huddle_roster,
        channel_members,
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_has_more: false,
    })
}

/// Which rows a channel window opens on: the live tail, or a page centred on
/// one older message a search hit named.
#[derive(Clone, Copy)]
pub(crate) enum MessageWindow {
    Tail,
    Around(u64),
}

/// One channel's row and its huddle roster, read from the index view. The
/// roster length is not derivable from an op, so the row still has to be read.
///
/// `None` IS AN ANSWER, NOT A FAILURE. A room this node cannot see is the
/// ordinary state of three things: a resident whose chat index has not folded
/// yet (a fresh join spends minutes in `joining` with every `Channel` query
/// answering `null`), a room id remembered from a network this session has
/// since left, and a landing id that never existed here. None of those is a
/// broken node, so none of them may reach the reader as a red banner over the
/// timeline — each caller decides what an unseen room means for it.
pub(crate) async fn load_channel_facts(
    rpc: &RpcClient,
    channel_id: &str,
    me: Option<&[u8]>,
) -> Result<Option<(ChatChannel, Vec<HuddleParticipant>)>, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::Channel {
                channel_id: channel_id.to_string(),
            },
        )
        .await?;
    let ChatViewReply::Channel(record) = reply else {
        return Err("node returned an invalid channel record".into());
    };
    let Some(info) = record else {
        return Ok(None);
    };
    // THE CACHED DIRECTORY, NEVER A FILLING READ: `load_channel_row` awaits this
    // function inside the live stream's decoder fold, where a `/v1/query` would
    // freeze every subscriber (issue #1018). The connect's chat load warms it.
    let roster = huddle_roster(&info.channel.huddle, me, &cached_account_names());
    Ok(Some((
        ChatChannel {
            id: info.channel.id,
            name: info.channel.name,
            archived: info.channel.archived,
            members_only: info.channel.post_policy == PostPolicy::MembersOnly,
            huddle_count: count_i64(info.channel.huddle.len()),
            head_seq: number_i64(info.head_seq),
        },
        roster,
    )))
}

/// One channel's window, WITHOUT re-paging the channel list.
///
/// This is the SWITCH path's loader; [`load_chat_data`] is the cold-boot and
/// resync one, where the list itself is the thing being learned. A channel
/// click already holds that list in state and the live fold keeps it fresh, so
/// re-paging it put a whole extra round trip — more on a workspace past one
/// page — in front of the first row the reader was waiting for.
///
/// What is left is three INDEPENDENT reads: the channel's own row (for the
/// huddle roster), the member roll, and the messages. They run concurrently,
/// so a switch costs one round trip: the timeline leg is one root-index page,
/// independent of the number of replies in the channel.
///
/// The answer CARRIES BACK ONLY THE ROW THIS REFRESHED. Handing the pre-click
/// channel snapshot back would have the reducer revert every delta the live
/// stream folded during the round trip — a peer's post in a third room and its
/// unread badge, a channel created, renamed or archived. See
/// `upsert_channel_rows`, which folds this row into the list on screen instead
/// of replacing it.
///
/// The window is authoritative about WHERE IT LANDED, which is not the same as
/// always landing where it was asked. A room this node cannot see is not an
/// error the reader can act on — it is a resident whose index has not folded
/// yet, or an id remembered from a network this session has left — so it falls
/// through to the cold load, which names the landing channel it settles on (or
/// no channel at all, on a workspace with nothing to read yet) and carries the
/// whole list back with it. Every reducer downstream lands on
/// `next.active_channel`, so the answer stays truthful either way; what it must
/// never do is put "channel record was not found" over the timeline.
pub(crate) async fn load_channel_window_data(
    rpc: &RpcClient,
    channel_id: &str,
    window: MessageWindow,
) -> Result<ChatData, String> {
    // Awaited before the fan-out so the cached identity is warm for every leg:
    // there is no single-flight, and three cold callers would each spawn the
    // CLI. Same reason `load_chat_data` awaits it above its own join.
    let me = local_user_key().await;
    let messages_leg = async {
        match window {
            MessageWindow::Tail => load_messages(rpc, channel_id).await,
            MessageWindow::Around(seq) => {
                let messages = load_messages_around(rpc, channel_id, seq).await?;
                let floor = oldest_committed(&messages).map_or(0, |message| message.seq);
                let has_more = older_roots_exist(rpc, channel_id, floor).await?;
                Ok(RootPage { messages, has_more })
            }
        }
    };
    let (facts, channel_members, message_page) = tokio::try_join!(
        load_channel_facts(rpc, channel_id, me.as_deref()),
        load_channel_members(rpc, channel_id),
        messages_leg
    )?;
    let Some((channel, huddle_roster)) = facts else {
        return load_chat_data(rpc, None).await;
    };
    Ok(ChatData {
        generation: 0,
        channels: vec![channel.clone()],
        messages: message_page.messages,
        has_older_history: message_page.has_more,
        active_channel: channel.id,
        active_channel_name: channel.name,
        active_channel_archived: channel.archived,
        active_channel_members_only: channel.members_only,
        huddle_roster,
        channel_members,
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_has_more: false,
    })
}

pub(crate) async fn load_channel_members(
    rpc: &RpcClient,
    channel_id: &str,
) -> Result<Vec<ChatMember>, String> {
    let mut members = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: ChatViewReply = rpc
            .view(
                "chat",
                &ChatViewQuery::Members {
                    channel_id: channel_id.to_string(),
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let ChatViewReply::Members {
            members: page,
            has_more,
            next_after,
        } = reply
        else {
            return Err("node returned an invalid channel member list".into());
        };
        members.extend(page);
        if !has_more {
            break;
        }
        after = next_after;
        if after.is_none() {
            break;
        }
    }
    Ok(members
        .into_iter()
        .map(|member| {
            let id = member_id(&member.user);
            ChatMember {
                label: short_label(id),
                key: id.to_string(),
            }
        })
        .collect())
}

/// The member's key id: the part after `user:` in a rendered member handle,
/// or the whole handle when it carries no such prefix.
pub(crate) fn member_id(user: &str) -> &str {
    user.strip_prefix("user:").unwrap_or(user)
}

pub async fn load_older_messages(
    rpc: String,
    channel_id: String,
    before_seq: i64,
) -> Result<HistoryPageData, AppError> {
    let result = async {
        let rpc = rpc_client(&rpc)?;
        let before = u64::try_from(before_seq).unwrap_or(0);
        let page = query_roots(&rpc, &channel_id, Some(before)).await?;
        let current_user = local_user_key().await;
        let names = account_names(&rpc).await;
        let messages: Vec<ChatMessage> = page
            .roots
            .into_iter()
            .map(|row| chat_message(row, current_user.as_deref(), &names))
            .collect();
        Ok((messages, page.has_more))
    }
    .await;
    result
        .map(|(messages, has_more)| HistoryPageData {
            channel_id,
            messages,
            has_more,
        })
        .map_err(app_error)
}

pub(crate) async fn load_messages_around(
    rpc: &RpcClient,
    channel_id: &str,
    seq: u64,
) -> Result<Vec<ChatMessage>, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::MessagesAround {
                channel_id: channel_id.to_string(),
                seq,
                limit: Some(CHAT_VIEW_PAGE_LIMIT),
            },
        )
        .await?;
    let ChatViewReply::Messages(rows) = reply else {
        return Err("node returned an invalid message window".into());
    };
    let current_user = local_user_key().await;
    let names = account_names(rpc).await;
    Ok(rows
        .into_iter()
        .filter(|row| row.thread.is_none())
        .map(|row| chat_message(row, current_user.as_deref(), &names))
        .collect())
}

pub(crate) async fn load_message_at(
    rpc: &RpcClient,
    channel_id: &str,
    seq: u64,
) -> Result<MsgRow, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::MessagesAround {
                channel_id: channel_id.to_string(),
                seq,
                limit: Some(1),
            },
        )
        .await?;
    let ChatViewReply::Messages(rows) = reply else {
        return Err("node returned an invalid message window".into());
    };
    rows.into_iter()
        .find(|row| row.seq == seq)
        .ok_or_else(|| "message was not found".into())
}

#[derive(Default)]
pub(crate) struct RootPage {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) has_more: bool,
}

struct RootRows {
    roots: Vec<MsgRow>,
    has_more: bool,
}

/// IS THERE ANY ROOT OLDER THAN `floor` — the server's answer, for a window the
/// server was never asked to page.
///
/// The tail and older-page loads carry the index's own `has_more` back with
/// them. A search-hit window does not: `MessagesAround` is a slice centred on
/// one message and says nothing about what lies before it, and the guess that
/// stood in for it — "the oldest row's seq is greater than 1" — is wrong on
/// every real channel. Root sequences have HOLES: a thread reply consumes a seq
/// without ever becoming a root, so the first message in a busy channel can sit
/// at seq 40 and "Load older messages" stood forever over a timeline with
/// nothing older, paging an empty answer on every click.
///
/// One root before the floor is the whole question, so the page is one row.
async fn older_roots_exist(rpc: &RpcClient, channel_id: &str, floor: i64) -> Result<bool, String> {
    let Ok(floor) = u64::try_from(floor) else {
        return Ok(false);
    };
    if floor == 0 {
        return Ok(false);
    }
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::Roots {
                channel_id: channel_id.to_string(),
                before_seq: Some(floor),
                limit: Some(1),
            },
        )
        .await?;
    let ChatViewReply::Roots { roots, .. } = reply else {
        return Err("node returned an invalid root page".into());
    };
    Ok(!roots.is_empty())
}

async fn query_roots(
    rpc: &RpcClient,
    channel_id: &str,
    before_seq: Option<u64>,
) -> Result<RootRows, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::Roots {
                channel_id: channel_id.to_string(),
                before_seq,
                limit: Some(CHAT_VIEW_PAGE_LIMIT),
            },
        )
        .await?;
    let ChatViewReply::Roots {
        roots,
        has_more,
        next_before_seq,
    } = reply
    else {
        return Err("node returned an invalid root page".into());
    };
    let expected_cursor = if has_more {
        roots.first().map(|row| row.seq)
    } else {
        None
    };
    let roots_are_strictly_ordered = roots.windows(2).all(|pair| pair[0].seq < pair[1].seq);
    let roots_precede_request =
        before_seq.is_none_or(|before| roots.iter().all(|row| row.seq < before));
    let roots_are_timeline_rows = roots.iter().all(|row| row.thread.is_none());
    let page_has_a_cursor_source = !has_more || !roots.is_empty();
    let cursor_is_valid = next_before_seq == expected_cursor;
    if !roots_are_strictly_ordered
        || !roots_precede_request
        || !roots_are_timeline_rows
        || !page_has_a_cursor_source
        || !cursor_is_valid
    {
        return Err("node returned an invalid root cursor".into());
    }
    Ok(RootRows { roots, has_more })
}

pub(crate) async fn load_messages(rpc: &RpcClient, channel_id: &str) -> Result<RootPage, String> {
    let page = query_roots(rpc, channel_id, None).await?;
    let current_user = local_user_key().await;
    let names = account_names(rpc).await;
    let mut messages: Vec<ChatMessage> = page
        .roots
        .into_iter()
        .map(|row| chat_message(row, current_user.as_deref(), &names))
        .collect();
    mark_message_groups(&mut messages);
    Ok(RootPage {
        messages,
        has_more: page.has_more,
    })
}

/// One page of older history, returned with the channel that requested it.
/// The compiler-owned `history` lane drops superseded replies; the channel
/// identity still guards a page whose room changed without another history run.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HistoryPageData {
    pub channel_id: String,
    pub messages: Vec<ChatMessage>,
    pub has_more: bool,
}

/// The oldest COMMITTED row — the only kind that answers for history.
///
/// A pending optimistic row carries a negative seq, which sorts ahead of every
/// real message. Reading `first()` blindly made an in-flight send answer for
/// the top of the timeline: `oldest_message_seq` handed the loader a `-1` that
/// floors to an empty server cursor.
fn oldest_committed(messages: &[ChatMessage]) -> Option<&ChatMessage> {
    messages.iter().find(|message| !message.pending)
}

/// The seq of the oldest loaded root (the ceiling for the next older page).
pub fn oldest_message_seq(messages: Vec<ChatMessage>) -> i64 {
    oldest_committed(&messages).map_or(0, |message| message.seq)
}

/// Prepend an older page ahead of the current timeline, de-duped by seq, sorted
/// oldest-first, and re-grouped so the seam between pages regroups correctly.
///
/// Pending rows are partitioned out and re-appended at the tail, exactly as
/// [`merge_message_send_result`] does: they have no seq to sort by, and sorting
/// them numerically hoisted an in-flight send to the top of a months-old
/// scrollback.
pub fn prepend_history(messages: Vec<ChatMessage>, older: Vec<ChatMessage>) -> Vec<ChatMessage> {
    const SEAM_ROWS: usize = 64;

    let (mut pending, committed): (Vec<ChatMessage>, Vec<ChatMessage>) =
        messages.into_iter().partition(|message| message.pending);
    let known: BTreeSet<i64> = committed.iter().map(|message| message.seq).collect();
    let mut older: Vec<ChatMessage> = older
        .into_iter()
        .filter(|message| !message.pending && !known.contains(&message.seq))
        .collect();
    older.sort_by_key(|message| message.seq);

    if pending.len() > CHAT_HOT_WINDOW_LIMIT {
        pending.drain(..pending.len() - CHAT_HOT_WINDOW_LIMIT);
    }
    let committed_limit = CHAT_HOT_WINDOW_LIMIT.saturating_sub(pending.len());
    let seam_rows = committed.len().min(SEAM_ROWS).min(committed_limit);
    let older_limit = committed_limit - seam_rows;
    if older.len() > older_limit {
        older.drain(..older.len() - older_limit);
    }

    // Keep the oldest edge of the old window beside the newest edge of the
    // page just loaded. This retains several viewports across the prepend
    // seam, while the next cursor still starts at the first retained row and
    // can reach every row discarded from the older edge on its next walk.
    let mut merged = older;
    let current_limit = committed_limit - merged.len();
    merged.extend(committed.into_iter().take(current_limit));
    merged.extend(pending);
    mark_message_groups(&mut merged);
    merged
}

/// One cursor page of a thread. The view already exposes the exact server
/// cursor; fetching the complete run and slicing it locally made page N
/// re-download pages 0..N-1.
pub(crate) struct ThreadPage {
    pub(crate) root: MsgRow,
    pub(crate) replies: Vec<MsgRow>,
    pub(crate) has_more: bool,
    pub(crate) next_reply_seq: Option<u64>,
}

pub(crate) async fn query_thread_page(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    after_reply_seq: Option<u64>,
) -> Result<ThreadPage, String> {
    let reply: ChatViewReply = rpc
        .view(
            "chat",
            &ChatViewQuery::Thread {
                channel_id: channel_id.to_string(),
                root_seq,
                after_reply_seq,
                limit: Some(CHAT_VIEW_PAGE_LIMIT),
            },
        )
        .await?;
    let ChatViewReply::Thread {
        root,
        replies,
        has_more,
        next_reply_seq,
    } = reply
    else {
        return Err("thread was not found".into());
    };
    let root = root.ok_or_else(|| "thread was not found".to_string())?;
    let expected_cursor = if has_more {
        replies.last().map(|row| row.seq)
    } else {
        None
    };
    let replies_are_strictly_ordered = replies.windows(2).all(|pair| pair[0].seq < pair[1].seq);
    let replies_follow_request =
        after_reply_seq.is_none_or(|after| replies.iter().all(|row| row.seq > after));
    let replies_belong_to_root = replies.iter().all(|row| row.thread == Some(root_seq));
    let page_has_a_cursor_source = !has_more || !replies.is_empty();
    let cursor_is_valid = next_reply_seq == expected_cursor;
    if root.thread.is_some()
        || !replies_are_strictly_ordered
        || !replies_follow_request
        || !replies_belong_to_root
        || !page_has_a_cursor_source
        || !cursor_is_valid
    {
        return Err("node returned an invalid thread cursor".into());
    }
    Ok(ThreadPage {
        root,
        replies,
        has_more,
        next_reply_seq,
    })
}

pub(crate) async fn load_sparse_thread_data(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    target_seq: u64,
) -> Result<ThreadData, String> {
    let root = load_message_at(rpc, channel_id, root_seq).await?;
    let target = load_message_at(rpc, channel_id, target_seq).await?;
    if target.thread != Some(root_seq) {
        return Err("search result does not belong to the selected thread".into());
    }
    let current_user = local_user_key().await;
    let names = account_names(rpc).await;
    Ok(ThreadData {
        root_seq: number_i64(root_seq),
        target_seq: number_i64(target_seq),
        messages: vec![
            chat_message(root, current_user.as_deref(), &names),
            chat_message(target, current_user.as_deref(), &names),
        ],
        next_reply_seq: 0,
        has_more: false,
    })
}

pub(crate) async fn load_thread_data(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
) -> Result<ThreadData, String> {
    if channel_id.is_empty() || root_seq == 0 {
        return Ok(ThreadData {
            root_seq: 0,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_seq: 0,
            has_more: false,
        });
    }

    let thread = query_thread_page(rpc, channel_id, root_seq, None).await?;
    let current_user = local_user_key().await;
    let names = account_names(rpc).await;
    let root = chat_message(thread.root, current_user.as_deref(), &names);
    let mut replies: Vec<ChatMessage> = thread
        .replies
        .into_iter()
        .map(|row| chat_message(row, current_user.as_deref(), &names))
        .collect();
    // The rail draws the stream's run rhythm now, so replies group the same
    // way — but the ROOT renders as its own divided block, so the run starts
    // at the first reply rather than folding it under the root's author.
    mark_message_groups(&mut replies);
    let messages = std::iter::once(root).chain(replies).collect();
    Ok(ThreadData {
        root_seq: number_i64(root_seq),
        target_seq: 0,
        messages,
        next_reply_seq: number_i64(thread.next_reply_seq.unwrap_or(0)),
        has_more: thread.has_more,
    })
}

pub(crate) async fn query_block_comment_page(
    rpc: &RpcClient,
    target: &str,
    thread_id: &str,
    from: u32,
    generation: i64,
) -> Result<Option<BlockCommentData>, String> {
    let reply: PageReply = rpc
        .query(
            "pages",
            &PageQuery::CommentThread {
                thread_id: thread_id.to_string(),
            },
        )
        .await?;
    let names = account_names(rpc).await;
    let PageReply::CommentThread(thread) = reply else {
        return Err("node returned an invalid comment page".into());
    };
    let Some(view) = thread else {
        return Ok(None);
    };
    let is_expected_thread = view.thread.id == thread_id && view.thread.target == target;
    if !is_expected_thread {
        return Err("node returned comments for another block".into());
    }
    // the committed read returns the whole live comment list; the UI's page
    // ordinals are 1-based positions in it, sliced from `from`.
    let comments = view
        .comments
        .into_iter()
        .enumerate()
        .skip(from as usize)
        .map(|(index, comment)| page_comment(index + 1, comment, &names))
        .collect();
    Ok(Some(BlockCommentData {
        generation,
        target: view.thread.target,
        thread_id: view.thread.id,
        from: i64::from(from),
        comments,
        next_from: 0,
        has_more: false,
    }))
}

pub(crate) fn page_comment_thread(thread: ThreadRow, names: &AuthorNames) -> PageCommentThread {
    let comment_count = count_i64(thread.comments.iter().filter(|c| !c.deleted).count());
    let count_label = if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    };
    PageCommentThread {
        id: thread.id,
        target: thread.target,
        author: author_name(&thread.opener, names),
        meta: if thread.resolved {
            format!("{count_label} · resolved")
        } else {
            count_label
        },
        resolved: thread.resolved,
        comment_count,
    }
}

fn page_comment(ordinal: usize, comment: pages::Comment, names: &AuthorNames) -> PageComment {
    let edited = comment.edited_at.is_some();
    let ordinal = count_i64(ordinal);
    PageComment {
        id: comment.id,
        ordinal,
        author: page_author_name(&comment.author, names),
        meta: if edited {
            format!("#{ordinal} · edited")
        } else {
            format!("#{ordinal}")
        },
        text: comment.text,
    }
}

/// THE SAME DIRECTORY THE TIMELINE READS. A page comment's author is a member
/// key like any other, and rendering it as raw hex here while the chat rows
/// beside it say "orthory" is the same defect in a second surface.
fn page_author_name(author: &pages::AuthorRef, names: &AuthorNames) -> String {
    match author {
        pages::AuthorRef::User(key) => author_name(&format!("user:{}", hex_encode(key)), names),
        pages::AuthorRef::Agent { agent_id, .. } => format!("@{agent_id}"),
        pages::AuthorRef::Module(module) => module.clone(),
        pages::AuthorRef::System => "system".into(),
    }
}

pub(crate) async fn load_pages_data(
    rpc: &RpcClient,
    requested: Option<&str>,
) -> Result<PagesData, String> {
    // ONE wait for the whole reload: the page list, the blocks, and the thread
    // panels are three arms of the same fold, so waiting here covers all of
    // them and the block read below finds nothing left outstanding to wait for.
    await_pages_fold(rpc).await;
    let wire_pages = load_page_index(rpc).await?;
    let pages = page_items(wire_pages);
    let active_page = requested
        .filter(|id| pages.iter().any(|page| page.id == *id))
        .map(str::to_string)
        .or_else(|| pages.first().map(|page| page.id.clone()))
        .unwrap_or_default();
    let active_page_parent = pages
        .iter()
        .find(|page| page.id == active_page)
        .map(|page| page.parent.clone())
        .unwrap_or_default();
    if active_page.is_empty() {
        return Ok(PagesData {
            pages,
            blocks: Vec::new(),
            active_page,
            active_page_title: String::new(),
            active_page_parent,
            comment_thread_total: 0,
            commented_block_hits: Vec::new(),
        });
    }
    let wire_blocks = load_page_blocks(rpc, &active_page).await?;
    let active_page_title = wire_blocks
        .first()
        .map(|block| block.text.clone())
        .unwrap_or_default();
    let blocks = page_blocks(wire_blocks, &active_page);
    // One grouped ThreadsForTargets ride-along, so the surface knows its
    // comment story — the header count and the commented-line washes — the
    // moment the page opens, not only after the rail is.
    let block_ids: Vec<String> = blocks.iter().map(|block| block.id.clone()).collect();
    let threads = query_page_thread_rows(rpc, &active_page, &block_ids).await?;
    let comment_thread_total = count_i64(threads.len());
    let commented_block_hits = commented_targets(&active_page, &threads);
    Ok(PagesData {
        pages,
        blocks,
        active_page,
        active_page_title,
        active_page_parent,
        comment_thread_total,
        commented_block_hits,
    })
}

/// Every thread anchored to the page or any of its blocks, one grouped query.
pub(crate) async fn query_page_thread_rows(
    rpc: &RpcClient,
    page_id: &str,
    block_ids: &[String],
) -> Result<Vec<ThreadRow>, String> {
    let mut targets = vec![page_id.to_string()];
    targets.extend(block_ids.iter().cloned());
    let reply: PagesViewReply = rpc
        .view("pages", &PagesViewQuery::ThreadsForTargets { targets })
        .await?;
    let PagesViewReply::Threads(groups) = reply else {
        return Err("node returned an invalid comment thread page".into());
    };
    Ok(groups.into_iter().flat_map(|group| group.threads).collect())
}

/// ONE ENTRY PER UNRESOLVED THREAD, not per block — the repetition IS the
/// count the margin chip spells. This deduplicated, which threw the count away
/// three layers before the chip that needed it: every commented line drew the
/// same three dots whether it carried one stray note or a whole argument, and
/// the only way to tell them apart was to open the rail and read it.
///
/// The page's own id is not a line, so it never marks one. Sorted so equal
/// targets sit together and the fold that counts them is a single pass.
pub(crate) fn commented_targets(page_id: &str, threads: &[ThreadRow]) -> Vec<String> {
    let mut targets: Vec<String> = threads
        .iter()
        .filter(|thread| !thread.resolved && thread.target != page_id)
        .map(|thread| thread.target.clone())
        .collect();
    targets.sort();
    targets
}

pub(crate) fn page_blocks(wire_blocks: Vec<pages::Block>, active_page: &str) -> Vec<PageBlock> {
    let parents = wire_blocks
        .iter()
        .map(|block| (block.id.clone(), block.parent.clone()))
        .collect::<BTreeMap<_, _>>();
    wire_blocks
        .into_iter()
        .skip(1)
        .map(|block| PageBlock {
            key: page_block_key(&block.id),
            prefix: block_prefix(&block, active_page, &parents),
            id: block.id,
            parent: block.parent.unwrap_or_default(),
            kind: block_kind_name(block.kind).into(),
            text: block.text,
            pending: false,
            checked: block.checked,
            child_count: count_i64(block.children.len()),
        })
        .collect()
}

pub(crate) fn page_block_key(id: &str) -> i64 {
    stable_view_key(&format!("page-block:{id}"))
}

/// Collision-free numeric identity for Ice's keyed rows. The language accepts
/// only copyable numeric keys, while the app's durable identities are strings.
pub(crate) fn stable_view_key(identity: &str) -> i64 {
    // ponytail: session-wide interning is collision-free; scope it per workspace
    // only if retaining every visited row identity becomes measurable.
    static KEYS: OnceLock<Mutex<BTreeMap<String, i64>>> = OnceLock::new();
    let mut keys = KEYS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(key) = keys.get(identity) {
        return *key;
    }
    let key = count_i64(keys.len());
    keys.insert(identity.to_owned(), key);
    key
}

pub(crate) async fn load_selected_page_data(
    rpc: &RpcClient,
    page_id: &str,
) -> Result<PagesData, String> {
    load_pages_data(rpc, Some(page_id)).await
}

/// Wait, briefly, for the pages fold to carry every pages block this client
/// already knows about — its own writes and the ops its live stream delivered
/// (`rpc.rs`, [`SEEN_BLOCKS`]).
///
/// Every pages read below goes through the index view, which folds BEHIND the
/// block loop, so a read fired on the heels of a structural change reads a
/// page that predates it: the moved block back where it was, the deleted line
/// still alive, the line just typed missing — and for the autosave, a missing
/// line is re-INSERTED by the next tick's plan, duplicating it on chain.
///
/// The ordinary read — opening a page, hydrating a boot — knows of nothing
/// outstanding and waits for nothing, so this costs the highest-frequency read
/// in the app exactly zero requests.
async fn await_pages_fold(rpc: &RpcClient) {
    await_seen_fold(rpc, "pages", &empty_pages_probe()).await;
}

/// Every block of one page in PREORDER, off the INDEX VIEW lane.
///
/// This is the app's highest-frequency read — it is what opening a document
/// costs. On `/v1/query` it went through the node's dispatch actor and so paid
/// the select-loop/checkpoint tax of issue #1018; `PagesViewQuery::GetPage`
/// answers the identical `PageBlockPage` off an MVCC snapshot, off-loop
/// (pages' own `tests/index_parity.rs` is the proof that they are identical).
pub(crate) async fn load_page_blocks(
    rpc: &RpcClient,
    page_id: &str,
) -> Result<Vec<pages::Block>, String> {
    await_pages_fold(rpc).await;
    let mut blocks = Vec::new();
    let mut after = None;
    loop {
        let reply: PagesViewReply = rpc
            .view(
                "pages",
                &PagesViewQuery::GetPage {
                    page_id: page_id.to_string(),
                    after: after.clone(),
                    limit: 0,
                },
            )
            .await?;
        let page = match reply {
            PagesViewReply::Page(Some(page)) => page,
            _ => return Err("page was not found".into()),
        };
        blocks.extend(page.blocks);
        let Some(next) = page.next_after else {
            return Ok(blocks);
        };
        if after.as_ref() == Some(&next) {
            return Err("node repeated the page cursor".into());
        }
        after = Some(next);
    }
}

pub(crate) async fn load_page_index(rpc: &RpcClient) -> Result<Vec<PageRow>, String> {
    let mut pages = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let reply: PagesViewReply = rpc
            .view(
                "pages",
                &PagesViewQuery::ListPages {
                    after: after.clone(),
                    limit: None,
                },
            )
            .await?;
        let PagesViewReply::Pages {
            pages: page,
            has_more,
            next_after,
        } = reply
        else {
            return Err("node returned an invalid page list".into());
        };
        pages.extend(page);
        if !has_more {
            return Ok(pages);
        }
        let Some(next) = next_after else {
            return Ok(pages);
        };
        if after.as_ref() == Some(&next) {
            return Err("node repeated the page-list cursor".into());
        }
        after = Some(next);
    }
}

fn page_items(wire_pages: Vec<PageRow>) -> Vec<PageItem> {
    let known = wire_pages
        .iter()
        .map(|page| page.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut children = BTreeMap::<Option<&str>, Vec<usize>>::new();
    for (index, page) in wire_pages.iter().enumerate() {
        let parent = page
            .parent
            .as_deref()
            .filter(|parent| known.contains(parent));
        children.entry(parent).or_default().push(index);
    }
    let mut stack = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|index| (*index, 0_usize))
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut pages = Vec::with_capacity(wire_pages.len());
    while pages.len() < wire_pages.len() {
        let Some((index, depth)) = stack.pop() else {
            let Some(index) = wire_pages
                .iter()
                .position(|page| !visited.contains(page.id.as_str()))
            else {
                break;
            };
            stack.push((index, 0));
            continue;
        };
        let page = &wire_pages[index];
        if !visited.insert(page.id.as_str()) {
            continue;
        }
        let page_children = children.get(&Some(page.id.as_str()));
        pages.push(PageItem {
            id: page.id.clone(),
            title: if page.title.is_empty() {
                "Untitled".into()
            } else {
                page.title.clone()
            },
            parent: page.parent.clone().unwrap_or_default(),
            prefix: "  ".repeat(depth),
            child_count: page_children.map_or(0, |children| count_i64(children.len())),
        });
        if let Some(page_children) = page_children {
            stack.extend(page_children.iter().rev().map(|index| (*index, depth + 1)));
        }
    }
    pages
}

fn block_prefix(
    block: &pages::Block,
    page_id: &str,
    parents: &BTreeMap<String, Option<String>>,
) -> String {
    let mut depth = 0;
    let mut parent = block.parent.as_deref();
    while let Some(parent_id) = parent {
        if parent_id == page_id || depth >= parents.len() {
            break;
        }
        depth += 1;
        parent = parents.get(parent_id).and_then(Option::as_deref);
    }
    "  ".repeat(depth)
}
