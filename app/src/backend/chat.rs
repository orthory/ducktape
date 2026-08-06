use super::*;
use ::chat;

/// One reaction tap folded into the visible rows at CLICK time — the chip
/// must not wait for the block. Rides the canonical reactor-set fold, so the
/// settled delta replays over it without drifting the count. No cached
/// identity (boot race) = no optimistic fold; the resync renders it instead.
pub fn reaction_applied(
    messages: Vec<ChatMessage>,
    seq: i64,
    emoji: String,
    added: bool,
) -> Vec<ChatMessage> {
    let Some(key) = rpc::cached_user_key() else {
        return messages;
    };
    let reactor = format!("user:{}", hex_encode(&key));
    chat::client::optimistic_reaction(messages, seq, emoji, added, reactor)
}

pub async fn rename_channel(
    rpc: String,
    password: String,
    channel_id: String,
    name: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let name = bounded_text(name, "channel name", 128)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::RenameChannel {
                channel_id: channel_id.clone(),
                name,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn archive_channel(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetChannelArchived {
                channel_id: channel_id.clone(),
                archived: true,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn unarchive_channel(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetChannelArchived {
                channel_id: channel_id.clone(),
                archived: false,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn add_channel_member(
    rpc: String,
    password: String,
    channel_id: String,
    member_key: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let user = public_key(&member_key, "member public key")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetMembership {
                channel_id: channel_id.clone(),
                user,
                member: true,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn remove_channel_member(
    rpc: String,
    password: String,
    channel_id: String,
    member_key: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let user = public_key(&member_key, "member public key")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::SetMembership {
                channel_id: channel_id.clone(),
                user,
                member: false,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

/// The reaction picker's emoji, in grid order — a frequently-used seed the
/// view lays out 8 per row. Adding here is the whole "add an emoji" change.
pub fn reaction_palette() -> Vec<String> {
    [
        "👍", "❤️", "😄", "😂", "😮", "😢", "🎉", "👀", //
        "🙌", "🔥", "✅", "❌", "💯", "🚀", "🤔", "😅", //
        "🙏", "👏", "💪", "✨", "⚡", "🐛", "📌", "❓", //
        "🦆", "🤝", "😴", "🧠", "➕", "🎯", "🚧", "🏁",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// One participant of a channel's live huddle — the roster is consensus state
/// (`HuddleMember{user, node, joined_at}`), not a count.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HuddleParticipant {
    pub key: String,
    pub label: String,
    pub initials: String,
    pub is_agent: bool,
    pub is_you: bool,
    pub joined_at: i64,
    /// The member's NODE key (hex) — the overlay identity the call hub fans
    /// media out to; `call_recipients` steers with exactly these.
    pub node: String,
}

/// Render the on-chain huddle roster, marking the row this device holds.
///
/// `HuddleEntry.user` is the kernel's BARE user id (`op.origin.id`) — the
/// same vocabulary `MemberRow` speaks — and NOT `MsgRow.author`'s
/// `user:{hex}`. This function compared against the prefixed form for as
/// long as it existed, so `is_you` never matched a real roster row and the
/// LIVE pill/leave/timer surface was unreachable; the fixture that covered
/// it had invented prefixed entries. Match the wire, prefix only to reuse
/// the author renderer for the label.
pub(crate) fn huddle_roster(
    members: &[chat::index::HuddleEntry],
    me: Option<&[u8]>,
) -> Vec<HuddleParticipant> {
    let mine = me.map(hex_encode);
    members
        .iter()
        .map(|member| {
            let label = author_name(&format!("user:{}", member.user));
            HuddleParticipant {
                initials: initials_of(&label),
                // The module refuses non-User authors ("only external users
                // may join a huddle"), so every roster row is a person.
                is_agent: false,
                is_you: mine.as_deref() == Some(member.user.as_str()),
                joined_at: number_i64(member.joined_at),
                key: member.user.clone(),
                node: member.node.clone(),
                label,
            }
        })
        .collect()
}

/// Am *I* in this huddle — the discriminant that splits the `Huddle` start
/// button from the LIVE pill with its ✕ Leave.
pub fn huddle_self(roster: Vec<HuddleParticipant>) -> bool {
    roster.iter().any(|participant| participant.is_you)
}

/// The call fan-out set: every roster peer's node key, self excluded — the
/// shape `CallClientControl::Recipients` wants.
pub fn huddle_recipient_nodes(roster: Vec<HuddleParticipant>) -> Vec<String> {
    roster
        .into_iter()
        .filter(|participant| !participant.is_you)
        .map(|participant| participant.node)
        .collect()
}

// The huddle's elapsed clock is a LOCAL session fact on a NATIVE `every 1s`
// subscription — ui-lang ships one, so this app has no tick stream of its own.

pub async fn join_huddle(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        let status = rpc.status().await.map_err(|error| error.to_string())?;
        let node = public_key(&status.public_key, "node public key")?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::JoinHuddle {
                channel_id: channel_id.clone(),
                node,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn leave_huddle(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<bool, AppError> {
    async {
        let channel_id = required_id(channel_id, "channel")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::LeaveHuddle {
                channel_id: channel_id.clone(),
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn send_message(
    rpc: String,
    password: String,
    channel_id: String,
    message_id: String,
    body: String,
    members: Vec<ChatMember>,
) -> Result<SendReceipt, OptimisticMutationError> {
    let operation_id = message_id.clone();
    let operation_scope = channel_id.clone();
    let operation_body = body.clone();
    let result = async {
        if channel_id.is_empty() {
            return Err("choose a channel first".to_string().into());
        }
        let body = bounded_text(body, "message", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: channel_id.clone(),
                message_id: required_id(message_id, "message")?,
                blocks: parse_message_with_members(&body, &members),
                thread: None,
                as_agent: None,
            }),
            password,
        )
        .await?;
        Ok(())
    }
    .await;
    result
        .map(|()| SendReceipt {
            operation_id: operation_id.clone(),
            channel_id: operation_scope.clone(),
        })
        .map_err(|cause: AppError| OptimisticMutationError {
            message: cause.message,
            committed: cause.committed,
            operation_id,
            scope_id: operation_scope,
            body: operation_body,
        })
}

pub async fn load_thread(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    through_reply_offset: i64,
    generation: i64,
) -> Result<ThreadLoadData, HydrationError> {
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let target_seq = u64::try_from(target_seq).unwrap_or(0);
        let is_sparse_target = through_reply_offset < 0 && target_seq > 0;
        let through_reply_offset = u64::try_from(through_reply_offset)
            .unwrap_or(0)
            .min(chat::MAX_THREAD_REPLIES as u64);
        let rpc = rpc_client(&rpc)?;
        if is_sparse_target {
            return load_sparse_thread_data(&rpc, &channel_id, root_seq, target_seq).await;
        }
        let mut thread =
            load_thread_data(&rpc, &channel_id, root_seq, through_reply_offset).await?;
        let target_is_loaded = target_seq > 0
            && thread
                .messages
                .iter()
                .any(|message| message.seq == number_i64(target_seq));
        if target_is_loaded {
            thread.target_seq = number_i64(target_seq);
        }
        Ok(thread)
    }
    .await;
    result
        .map(|thread| ThreadLoadData {
            generation,
            root_seq: thread.root_seq,
            target_seq: thread.target_seq,
            messages: thread.messages,
            next_reply_offset: thread.next_reply_offset,
            has_more: thread.has_more,
        })
        .map_err(|message| HydrationError {
            generation,
            message: user_error(message),
        })
}

pub async fn load_thread_page(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    from: i64,
    generation: i64,
) -> Result<ThreadPageData, HydrationError> {
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let from = u64::try_from(from).map_err(|_| "invalid thread offset".to_string())?;
        let rpc = rpc_client(&rpc)?;
        let thread = query_thread_page(&rpc, &channel_id, root_seq).await?;
        let total = thread.replies.len() as u64;
        let start = from.min(total);
        let page_len = CHAT_VIEW_PAGE_LIMIT.min(total - start);
        let next_reply_offset = start + page_len;
        let page_is_full = page_len == CHAT_VIEW_PAGE_LIMIT;
        let thread_cap_reached = next_reply_offset >= chat::MAX_THREAD_REPLIES as u64;
        let has_more = page_is_full && !thread_cap_reached;
        let current_user = local_user_key().await;
        let messages = thread
            .replies
            .into_iter()
            .skip(start as usize)
            .take(page_len as usize)
            .map(|row| chat_message(row, current_user.as_deref()))
            .collect();
        Ok(ThreadPageData {
            generation,
            messages,
            next_reply_offset: number_i64(next_reply_offset),
            has_more,
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}

pub async fn refresh_live_thread(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    through_reply_offset: i64,
    generation: i64,
) -> Result<LiveThreadData, HydrationError> {
    if channel_id.is_empty() || root_seq <= 0 {
        return Ok(LiveThreadData {
            generation,
            channel_id,
            root_seq: 0,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        });
    }
    load_thread(
        rpc,
        channel_id.clone(),
        root_seq,
        target_seq,
        through_reply_offset,
        generation,
    )
    .await
    .map(|thread| LiveThreadData {
        generation: thread.generation,
        channel_id,
        root_seq: thread.root_seq,
        target_seq: thread.target_seq,
        messages: thread.messages,
        next_reply_offset: thread.next_reply_offset,
        has_more: thread.has_more,
    })
}

pub async fn send_reply(
    rpc: String,
    password: String,
    channel_id: String,
    root_seq: i64,
    message_id: String,
    body: String,
    members: Vec<ChatMember>,
) -> Result<SendReceipt, OptimisticMutationError> {
    let operation_id = message_id.clone();
    let operation_scope = channel_id.clone();
    let operation_body = body.clone();
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let body = bounded_text(body, "reply", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        let message_id = required_id(message_id, "message")?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: channel_id.clone(),
                message_id: message_id.clone(),
                blocks: parse_message_with_members(&body, &members),
                thread: Some(root_seq),
                as_agent: None,
            }),
            password,
        )
        .await?;
        Ok(())
    }
    .await;
    result
        .map(|()| SendReceipt {
            operation_id: operation_id.clone(),
            channel_id: operation_scope.clone(),
        })
        .map_err(|cause: AppError| OptimisticMutationError {
            message: cause.message,
            committed: cause.committed,
            operation_id,
            scope_id: operation_scope,
            body: operation_body,
        })
}

pub async fn edit_message(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    base_rev: i64,
    body: String,
    members: Vec<ChatMember>,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let base_rev =
            u32::try_from(base_rev).map_err(|_| "invalid message revision".to_string())?;
        let body = bounded_text(body, "message", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::EditMessage {
                channel_id: channel_id.clone(),
                seq,
                blocks: parse_message_with_members(&body, &members),
                base_rev: Some(base_rev),
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn delete_message(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::DeleteMessage {
                channel_id: channel_id.clone(),
                seq,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn add_reaction(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    emoji: String,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let emoji = bounded_text(emoji, "reaction", chat::MAX_EMOJI_BYTES)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::AddReaction {
                channel_id: channel_id.clone(),
                seq,
                emoji,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn remove_reaction(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    emoji: String,
) -> Result<bool, AppError> {
    async {
        let seq = positive_sequence(seq)?;
        let emoji = bounded_text(emoji, "reaction", chat::MAX_EMOJI_BYTES)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::RemoveReaction {
                channel_id: channel_id.clone(),
                seq,
                emoji,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
}

pub async fn search_chat(
    rpc: String,
    channel_id: String,
    text: String,
    generation: i64,
) -> Result<ChatSearchData, HydrationError> {
    let result = async {
        let text = bounded_text(text, "search", 512)?;
        let rpc = rpc_client(&rpc)?;
        // a `#tag` query filters by the exact hashtag (the index's tag
        // postings); anything else is full-text search.
        let query = match text.strip_prefix('#') {
            Some(tag) if !tag.is_empty() => serde_json::json!({
                "tag_search": {
                    "tag": tag.to_lowercase(),
                    "channel_id": (!channel_id.is_empty()).then_some(channel_id),
                    "limit": 50
                }
            }),
            _ => serde_json::json!({
                "search": {
                    "text": text,
                    "channel_id": (!channel_id.is_empty()).then_some(channel_id),
                    "limit": 50
                }
            }),
        };
        let reply: chat::index::ChatViewReply = rpc.view("chat", &query).await?;
        let chat::index::ChatViewReply::Hits(hits) = reply else {
            return Err("chat search returned an invalid reply".into());
        };
        Ok(ChatSearchData {
            generation,
            hits: hits
                .into_iter()
                .map(|hit| ChatSearchHit {
                    channel_id: hit.channel_id,
                    seq: number_i64(hit.seq),
                    root_seq: number_i64(hit.thread.unwrap_or(hit.seq)),
                    author: hit.author,
                    text: hit.text,
                    meta: format!("#{}", hit.seq),
                })
                .collect(),
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}

pub async fn load_page(rpc: String, page_id: String) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        load_pages_data(&rpc, Some(&page_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn load_block_threads(
    rpc: String,
    target: String,
    from: i64,
    generation: i64,
) -> Result<BlockThreadListData, HydrationError> {
    let result = async {
        let target = required_id(target, "block")?;
        let from = u32::try_from(from).map_err(|_| "invalid comment offset".to_string())?;
        let rpc = rpc_client(&rpc)?;
        query_block_threads(&rpc, &target, from, generation).await
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Every comment thread on a PAGE, not on one block: the same
/// `ThreadsForTargets` query, asked for the page and all of its blocks at once.
/// `target` comes back as the page id — the rail is document-scoped.
pub async fn load_page_threads(
    rpc: String,
    page_id: String,
    generation: i64,
) -> Result<BlockThreadListData, HydrationError> {
    let result = async {
        let page_id = required_id(page_id, "page")?;
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let block_ids: Vec<String> = blocks.into_iter().map(|block| block.id).collect();
        let threads: Vec<PageCommentThread> = query_page_thread_rows(&rpc, &page_id, &block_ids)
            .await?
            .into_iter()
            .map(page_comment_thread)
            .collect();
        let total = count_i64(threads.len());
        Ok(BlockThreadListData {
            generation,
            target: page_id,
            from: 0,
            threads,
            total,
            next_from: 0,
            has_more: false,
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}

pub async fn load_block_comment_page(
    rpc: String,
    target: String,
    thread_id: String,
    from: i64,
    generation: i64,
) -> Result<BlockCommentData, HydrationError> {
    let result = async {
        let target = required_id(target, "block")?;
        let thread_id = required_id(thread_id, "comment thread")?;
        let from = u32::try_from(from).map_err(|_| "invalid comment offset".to_string())?;
        let rpc = rpc_client(&rpc)?;
        query_block_comment_page(&rpc, &target, &thread_id, from, generation)
            .await?
            .ok_or_else(|| "comment thread was not found".to_string())
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}

pub async fn refresh_block_comments(
    rpc: String,
    target: String,
    thread_id: String,
    generation: i64,
) -> Result<BlockCommentsRefreshData, HydrationError> {
    let result = async {
        if target.is_empty() {
            return Ok(BlockCommentsRefreshData {
                generation,
                target,
                threads: Vec::new(),
                total: 0,
                threads_next_from: 0,
                threads_has_more: false,
                thread_id: String::new(),
                comments: Vec::new(),
                comments_next_from: 0,
                comments_has_more: false,
            });
        }
        let target = required_id(target, "block")?;
        let rpc = rpc_client(&rpc)?;
        let threads = query_block_threads(&rpc, &target, 0, generation).await?;
        let comments = if thread_id.is_empty() {
            BlockCommentData {
                generation,
                target: target.clone(),
                thread_id,
                from: 0,
                comments: Vec::new(),
                next_from: 0,
                has_more: false,
            }
        } else {
            let thread_id = required_id(thread_id, "comment thread")?;
            query_block_comment_page(&rpc, &target, &thread_id, 0, generation)
                .await?
                .unwrap_or(BlockCommentData {
                    generation,
                    target: target.clone(),
                    thread_id: String::new(),
                    from: 0,
                    comments: Vec::new(),
                    next_from: 0,
                    has_more: false,
                })
        };
        Ok(BlockCommentsRefreshData {
            generation,
            target,
            threads: threads.threads,
            total: threads.total,
            threads_next_from: threads.next_from,
            threads_has_more: threads.has_more,
            thread_id: comments.thread_id,
            comments: comments.comments,
            comments_next_from: comments.next_from,
            comments_has_more: comments.has_more,
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}

pub async fn post_block_comment(
    rpc: String,
    password: String,
    target: String,
    thread_id: String,
    text: String,
    generation: i64,
) -> Result<BlockCommentData, AppError> {
    async {
        let target = required_id(target, "block")?;
        let text = bounded_text(text, "comment", 16 * 1024)?;
        let thread_id = comment_thread_id(thread_id)?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::AddComment {
                thread_id: thread_id.clone(),
                comment_id: fresh_id("comment"),
                target: target.clone(),
                text,
                anchor: None,
                mentions: Vec::new(),
                as_agent: None,
            }),
            password,
        )
        .await?;
        query_block_comment_page(&rpc, &target, &thread_id, 0, generation)
            .await
            .and_then(|page| page.ok_or_else(|| "comment thread was not found".to_string()))
            .map_err(committed_error)
    }
    .await
}

/// Flip a thread's resolved flag. The node's own op; the caller reloads the
/// rail to pick the new state up.
pub async fn resolve_comment_thread(
    rpc: String,
    password: String,
    thread_id: String,
    resolved: bool,
) -> Result<bool, AppError> {
    async {
        let thread_id = required_id(thread_id, "comment thread")?;
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::ResolveThread {
                thread_id,
                resolved,
            }),
            password,
        )
        .await?;
        Ok(true)
    }
    .await
    .map_err(app_error)
}

pub(crate) fn comment_thread_id(thread_id: String) -> Result<String, String> {
    if thread_id.is_empty() {
        Ok(fresh_id("thread"))
    } else {
        required_id(thread_id, "comment thread")
    }
}

pub async fn create_page(
    rpc: String,
    password: String,
    title: String,
) -> Result<PagesData, AppError> {
    async {
        let title = bounded_text(title, "page title", 512)?;
        let page_id = fresh_id("page");
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: page_id.clone(),
                title,
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn delete_page(
    rpc: String,
    password: String,
    page_id: String,
) -> Result<PagesData, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string().into());
        }
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::RemoveBlock {
                block_id: page_id.clone(),
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, None).await.map_err(committed_error)
    }
    .await
}

pub async fn search_pages(
    rpc: String,
    page_id: String,
    text: String,
    generation: i64,
) -> Result<PageSearchData, HydrationError> {
    let result = async {
        let text = bounded_text(text, "search", 512)?;
        let rpc = rpc_client(&rpc)?;
        let reply: pages::index::PagesViewReply = rpc
            .view(
                "pages",
                &serde_json::json!({
                    "search": {
                        "text": text,
                        "page_id": (!page_id.is_empty()).then_some(page_id),
                        "limit": 50
                    }
                }),
            )
            .await?;
        let pages::index::PagesViewReply::Hits(hits) = reply else {
            return Err("page search returned an invalid reply".into());
        };
        Ok(PageSearchData {
            generation,
            hits: hits
                .into_iter()
                .map(|hit| PageSearchHit {
                    page_id: hit.page_id,
                    block_id: hit.block_id,
                    kind: block_kind_name(hit.kind).into(),
                    text: hit.text,
                })
                .collect(),
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message: user_error(message),
    })
}
