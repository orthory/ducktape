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

/// The row a send paints before the block lands. Same arrangement as the
/// reaction fold above: the client mints the row, the shell supplies the
/// identity it must be attributed to, so the pending row groups under the
/// reader's previous message instead of breaking the run and re-grouping when
/// the settle delta replaces it.
pub fn optimistic_message(
    messages: Vec<ChatMessage>,
    body: String,
    message_id: String,
) -> Vec<ChatMessage> {
    bounded_chat_window(chat::client::optimistic_message(
        messages,
        body,
        message_id,
        rpc::cached_user_key().as_deref(),
        // THE SYNCHRONOUS READ: an Ice extern cannot await, and the mint must
        // paint this frame. The directory is warm by any send — see
        // `cached_account_names`.
        &cached_account_names(),
    ))
}

/// The thread rail owns a root plus one sliding reply page. The server cursor
/// remains authoritative for older/newer pages; mounted rich rows stay bounded
/// while a hot thread keeps receiving replies.
pub fn optimistic_thread_message(
    messages: Vec<ChatMessage>,
    body: String,
    message_id: String,
) -> Vec<ChatMessage> {
    bounded_thread_window(chat::client::optimistic_message(
        messages,
        body,
        message_id,
        rpc::cached_user_key().as_deref(),
        // THE SYNCHRONOUS READ: an Ice extern cannot await, and the mint must
        // paint this frame. The directory is warm by any send — see
        // `cached_account_names`.
        &cached_account_names(),
    ))
}

/// [`chat::client::mark_message_groups`] as a value fold, for the reducer.
///
/// The timeline calls it on the vec it just pushed an optimistic row onto. The
/// thread rail does NOT: its vec is `[root] ++ replies` and the root renders as
/// its own divided block, so a whole-vec pass folds the first reply under the
/// root and swallows its header. The rail's replies-only marking lives inside
/// `bounded_thread_window`, which every rail writer already folds through.
pub fn mark_author_runs(mut messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    chat::client::mark_message_groups(&mut messages);
    messages
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
    names: &AuthorNames,
) -> Vec<HuddleParticipant> {
    let mine = me.map(hex_encode);
    members
        .iter()
        .map(|member| {
            let label = author_name(&format!("user:{}", member.user), names);
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
///
/// Excludes by BOTH `is_you` (this device's own roster row) AND `self_node`
/// (this device's node key, whichever roster row carries it): a member's
/// `node_proof` only proves that member's user holds the node key it names,
/// never that the name is unique — a stale or replayed roster row can still
/// carry another user's `user` field alongside THIS node's key, and fanning
/// media to your own node is a loopback echo regardless of whose row it rides
/// in on.
pub fn huddle_recipient_nodes(
    roster: Vec<HuddleParticipant>,
    self_node: Option<&str>,
) -> Vec<String> {
    roster
        .into_iter()
        .filter(|participant| !participant.is_you)
        .filter(|participant| Some(participant.node.as_str()) != self_node)
        .map(|participant| participant.node)
        .collect()
}

/// THE FAN-OUT SET, READ FROM CONSENSUS — the live call session's own poll
/// (`crate::call`), not the roster on screen.
///
/// The hub gates admission on this set at BOTH ends: a datagram from a peer
/// the local session does not list is dropped at demux, media and 1 Hz
/// presence beacon alike. So a peer who joins after us has to enter the set
/// from somewhere, and the only thing that used to re-steer it was a peer
/// beacon — which their join could not deliver, because they were not in the
/// set yet. Two people joining a huddle therefore heard and saw nothing of
/// each other, forever.
///
/// It is read here rather than taken from `huddle_roster` because that field
/// belongs to the channel the user is LOOKING at, which need not be the
/// channel they are huddling in.
pub(crate) async fn huddle_fanout_nodes(
    rpc: &str,
    channel_id: &str,
) -> Result<Vec<String>, String> {
    let client = rpc_client(rpc)?;
    let me = local_user_key().await;
    let status = client.status().await.map_err(|error| error.to_string())?;
    let self_node = (!status.public_key.is_empty()).then_some(status.public_key);
    // A huddle in a room this node cannot see has no fan-out set — an empty
    // one, not a failure: the poll re-reads on its own cadence and picks the
    // roster up as soon as the index answers for the room.
    let facts = load_channel_facts(&client, channel_id, me.as_deref()).await?;
    let roster = facts.map_or_else(Vec::new, |(_channel, roster)| roster);
    Ok(huddle_recipient_nodes(roster, self_node.as_deref()))
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
        // Proof of possession: THIS node signs the join under its own key —
        // never asserted by the joiner — so the roster can only ever name a
        // node that agreed to route this user's media (issue #1792).
        let user = local_user_key()
            .await
            .ok_or_else(|| "no local user key to join a huddle as".to_string())?;
        let (_, node_proof_hex) = rpc
            .huddle_node_proof(&channel_id, &hex_encode(&user))
            .await
            .map_err(|error| error.to_string())?;
        let node_proof = hex_decode(&node_proof_hex)
            .map_err(|_| "huddle node proof must be hexadecimal".to_string())?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::JoinHuddle {
                channel_id: channel_id.clone(),
                node,
                node_proof,
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

/// WHO A SEND'S `@handles` MAY REACH: the network's ACCOUNT DIRECTORY unioned
/// with this room's explicit roster.
///
/// The roster alone was the whole answer, and the UI resets it to `[]` for
/// every open-policy channel (only a members-only room ever loads one) — so
/// `@orthory` in `#general`, the room everybody actually writes in, parsed as
/// plain text and minted no `Mark::Mention` for the module to fan out. The
/// directory is a person's name wherever they can be read.
async fn mention_candidates(rpc: &RpcClient, members: &[ChatMember]) -> MentionCandidates {
    MentionCandidates::new(&account_names(rpc).await, members)
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
                blocks: parse_message_with_mentions(
                    &body,
                    &mention_candidates(&rpc, &members).await,
                ),
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
            // A message is not in a thread; the stream's composer is keyed
            // by its room alone.
            thread_seq: 0,
            body: operation_body,
        })
}

pub async fn load_thread(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    generation: i64,
) -> Result<ThreadLoadData, HydrationError> {
    let result = load_thread_window(rpc, channel_id, root_seq, target_seq).await;
    result
        .map(|thread| ThreadLoadData {
            generation,
            root_seq: thread.root_seq,
            target_seq: thread.target_seq,
            messages: thread.messages,
            next_reply_seq: thread.next_reply_seq,
            has_more: thread.has_more,
        })
        .map_err(|message| HydrationError {
            generation,
            message: user_error(message),
        })
}

async fn load_thread_window(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
) -> Result<ThreadData, String> {
    let root_seq = positive_sequence(root_seq)?;
    let target_seq = u64::try_from(target_seq).unwrap_or(0);
    let rpc = rpc_client(&rpc)?;
    if target_seq > 0 {
        return load_sparse_thread_data(&rpc, &channel_id, root_seq, target_seq).await;
    }
    load_thread_data(&rpc, &channel_id, root_seq).await
}

pub async fn load_thread_page(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    after_reply_seq: i64,
    generation: i64,
) -> Result<ThreadPageData, HydrationError> {
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let after_reply_seq = u64::try_from(after_reply_seq).ok().filter(|seq| *seq > 0);
        let rpc = rpc_client(&rpc)?;
        let thread = query_thread_page(&rpc, &channel_id, root_seq, after_reply_seq).await?;
        let next_reply_seq = number_i64(thread.next_reply_seq.unwrap_or(0));
        let current_user = local_user_key().await;
        let names = account_names(&rpc).await;
        let messages = thread
            .replies
            .into_iter()
            .map(|row| chat_message(row, current_user.as_deref(), &names))
            .collect();
        Ok(ThreadPageData {
            generation,
            messages,
            next_reply_seq,
            has_more: thread.has_more,
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
) -> Result<LiveThreadData, AppError> {
    if channel_id.is_empty() || root_seq <= 0 {
        return Ok(LiveThreadData {
            channel_id,
            root_seq: 0,
            messages: Vec::new(),
        });
    }
    let root_seq = positive_sequence(root_seq).map_err(app_error)?;
    let rpc = rpc_client(&rpc).map_err(app_error)?;
    load_thread_data(&rpc, &channel_id, root_seq)
        .await
        .map(|thread| LiveThreadData {
            channel_id,
            root_seq: thread.root_seq,
            messages: thread.messages,
        })
        .map_err(app_error)
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
    let operation_thread = root_seq;
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
                blocks: parse_message_with_mentions(
                    &body,
                    &mention_candidates(&rpc, &members).await,
                ),
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
            thread_seq: operation_thread,
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
                blocks: parse_message_with_mentions(
                    &body,
                    &mention_candidates(&rpc, &members).await,
                ),
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
) -> Result<ChatSearchData, AppError> {
    // The reader, for the same `you` rendering the timeline does — a search hit
    // was showing the RAW wire author (`user:3f8dc8…773`, the full 64-hex key
    // with its prefix) as the row's headline, above the text it matched.
    let current_user = local_user_key().await;
    let result = async {
        let text = bounded_text(text, "search", 512)?;
        let rpc = rpc_client(&rpc)?;
        let names = account_names(&rpc).await;
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
            hits: hits
                .into_iter()
                .map(|hit| ChatSearchHit {
                    // THE ROOM COMES FIRST, because it is the thing a hit is
                    // missing. `#12` alone reads as a CHANNEL in this app —
                    // every channel is written `# General` — while it is
                    // actually the message's sequence number, and the channel
                    // it was found in went unsaid in both the palette and the
                    // sidebar. Every renderer of a hit shows `meta`, so the
                    // room belongs in it rather than composed at one call site
                    // (which is what the Explorer was doing alone).
                    meta: format!("{} · #{}", hit.channel_id, hit.seq),
                    channel_id: hit.channel_id,
                    seq: number_i64(hit.seq),
                    root_seq: number_i64(hit.thread.unwrap_or(hit.seq)),
                    author: author_display(&hit.author, current_user.as_deref(), &names),
                    text: hit.text,
                })
                .collect(),
        })
    }
    .await;
    result.map_err(app_error)
}

pub async fn load_page(rpc: String, page_id: String) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        load_pages_data(&rpc, Some(&page_id)).await
    }
    .await
    .map_err(app_error)
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
        // "" is `page_edited`'s "not my turn" — its parallel always fires
        // both runs; an empty target answers empty without touching the node
        // (and no open rail matches an empty target, so the answer is inert).
        if page_id.is_empty() {
            return Ok(BlockThreadListData {
                generation,
                target: String::new(),
                from: 0,
                threads: Vec::new(),
                total: 0,
                next_from: 0,
                has_more: false,
            });
        }
        let page_id = required_id(page_id, "page")?;
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let block_ids: Vec<String> = blocks.into_iter().map(|block| block.id).collect();
        let names = account_names(&rpc).await;
        let threads: Vec<PageCommentThread> = query_page_thread_rows(&rpc, &page_id, &block_ids)
            .await?
            .into_iter()
            .map(|thread| page_comment_thread(thread, &names))
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

/// Hand a WEB link to the OS opener — the `DuckKind::Web` arm of the open
/// plane, and its only caller. Only http(s) leaves the app this way
/// (this passes a string to a shell command, and the scheme gate is the trust
/// boundary); every other scheme is the open plane's own to resolve.
pub async fn open_external_url(url: String) -> Result<bool, AppError> {
    async {
        // "" is `page_edited`'s "not my turn" (see its parallel) — nothing
        // was asked, nothing opens.
        if url.is_empty() {
            return Ok(false);
        }
        let is_web = url.starts_with("http://") || url.starts_with("https://");
        if !is_web {
            return Err("only web links are handed to the system browser".to_string());
        }
        let opener = match std::env::consts::OS {
            "macos" => "open",
            "windows" => "explorer",
            _ => "xdg-open",
        };
        tokio::process::Command::new(opener)
            .arg(&url)
            .spawn()
            .map_err(|error| format!("could not open the link: {error}"))?;
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

/// The cheapest thing the pages view can be asked: zero targets, so the guest
/// scans nothing and answers an empty group list. [`await_fold`] reads only
/// the reply's watermark header, so the body should cost as little as the lane
/// allows.
pub(crate) fn empty_pages_probe() -> PagesViewQuery {
    PagesViewQuery::ThreadsForTargets {
        targets: Vec::new(),
    }
}

pub async fn create_page(
    rpc: String,
    password: String,
    title: String,
) -> Result<PagesData, AppError> {
    async {
        let title = bounded_text(title, "page title", pages::MAX_PAGE_TITLE_LEN)?;
        let page_id = fresh_id("page");
        let rpc = rpc_client(&rpc)?;
        let height = signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: page_id.clone(),
                title: title.clone(),
            }),
            password,
        )
        .await?;
        // WAIT FOR THE FOLD, THEN RELOAD. `submit_frame` returns when the node
        // ACCEPTS a transaction, not when it applies one, and the pages read
        // model is folded behind the block loop — so a reload fired straight
        // after the write reads an index that predates it. The view lane
        // answers how far the fold has consumed the op feed, so this waits for
        // it to reach the block that took the write.
        await_fold(&rpc, "pages", &empty_pages_probe(), height).await;
        // The wait above already covers this reload, so it passes None rather
        // than paying a second probe for the same watermark.
        let mut data = load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)?;
        // LAND ON THE PAGE THAT WAS JUST MADE. The wait above narrows the
        // window; it does not close it (a boundary stamp leaves no watermark,
        // a busy block can park the fold mid-batch, the budget is bounded on
        // purpose), so the correction stays the guarantee — measured, not
        // assumed: the reload asked for the new id, was handed the first page
        // in the list instead, and reported the new id absent from that list.
        // `load_pages_data` is right to drop an id it cannot see (a live
        // refresh or a save can legitimately name a page that has since been
        // deleted, and must follow the fallback). This is the one caller that
        // KNOWS its id is good, so the correction belongs here: press Enter on
        // a title and you are on that page, not on whichever one sorts first.
        // It is a no-op whenever the fold did arrive.
        if data.active_page != page_id {
            data.active_page = page_id;
            data.active_page_title = title;
            data.active_page_parent = String::new();
            data.blocks = Vec::new();
            data.comment_thread_total = 0;
            data.commented_block_hits = Vec::new();
        }
        Ok(data)
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
        let height = signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::RemoveBlock {
                block_id: page_id.clone(),
            }),
            password,
        )
        .await?;
        await_fold(&rpc, "pages", &empty_pages_probe(), height).await;
        // Same as `create_page`: the wait above covers this reload.
        let mut data = load_pages_data(&rpc, None).await.map_err(committed_error)?;
        // DROP WHAT WAS JUST DELETED. Same acceptance-vs-application gap as
        // `create_page`, read the other way round and narrowed by the same
        // wait: this reload can still see the removed page, so it stayed in
        // the sidebar, stayed selectable, and re-installed its blocks into the
        // editor when picked — a document the network no longer has. The
        // correction below is idempotent, so it costs nothing once the fold
        // has arrived and remains the guarantee when it has not.
        // `RemoveBlock` deletes the whole SUBTREE
        // (pages/src/store.rs walks children with no page-kind stop), so every
        // descendant goes with it, not just the row that was asked for.
        let doomed = descendants_of(&data.pages, &page_id);
        data.pages.retain(|page| !doomed.contains(&page.id));
        if doomed.contains(&data.active_page) {
            // Re-resolve exactly as `load_pages_data` would have, had the index
            // already caught up: the first surviving page, or nothing.
            data.active_page = data
                .pages
                .first()
                .map(|page| page.id.clone())
                .unwrap_or_default();
            data.active_page_title = String::new();
            data.active_page_parent = String::new();
            data.blocks = Vec::new();
            data.comment_thread_total = 0;
            data.commented_block_hits = Vec::new();
        }
        Ok(data)
    }
    .await
}

/// A page id and every page beneath it. `RemoveBlock` takes the subtree, so a
/// caller correcting a stale read has to take the same set or it leaves orphans
/// pointing at a parent that is gone.
pub(crate) fn descendants_of(pages: &[PageItem], root: &str) -> BTreeSet<String> {
    let mut doomed = BTreeSet::from([root.to_string()]);
    // The index is not depth-ordered, so sweep until nothing new is added.
    loop {
        let before = doomed.len();
        for page in pages {
            if doomed.contains(&page.parent) {
                doomed.insert(page.id.clone());
            }
        }
        if doomed.len() == before {
            return doomed;
        }
    }
}

/// Every hit joined to the TITLE of the page it lives in.
///
/// A hit row from the index names only its page id, so every surface that
/// rendered one had to say something else instead: the Explorer printed the
/// block text as BOTH the row's title and its snippet, the palette showed a
/// bare block kind, and the pages search panel showed an opaque block id.
/// None of the three said which page the match was in.
pub(crate) fn titled_page_hits(
    hits: Vec<pages::index::PageBlockRow>,
    index: Vec<PageRow>,
) -> Vec<PageSearchHit> {
    let titles = index
        .into_iter()
        .map(|page| (page.id, page.title))
        .collect::<BTreeMap<_, _>>();
    hits.into_iter()
        .map(|hit| PageSearchHit {
            // An untitled page reads "Untitled" in the sidebar (`page_items`),
            // so a hit must not read differently. A page id the index does not
            // carry takes the same fallback rather than a blank run.
            page_title: titles
                .get(&hit.page_id)
                .filter(|title| !title.is_empty())
                .cloned()
                .unwrap_or_else(|| "Untitled".to_string()),
            page_id: hit.page_id,
            block_id: hit.block_id,
            kind: block_kind_name(hit.kind).into(),
            text: hit.text,
        })
        .collect()
}

pub async fn search_pages(
    rpc: String,
    page_id: String,
    text: String,
) -> Result<PageSearchData, AppError> {
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
        if hits.is_empty() {
            return Ok(PageSearchData { hits: Vec::new() });
        }
        // The titles live one view over, in the same index — this is the very
        // call the pages sidebar makes. Paid once per search that matched
        // something, never on the empty keystrokes that dominate the palette.
        //
        // A LABEL IS DECORATION AND MUST NEVER DESTROY THE PAYLOAD. Taking `?`
        // here turned a search the node had already ANSWERED into an `Err`, and
        // both readers throw those away without a word: the Explorer's
        // `if let Ok(pages)` (backend/search.rs) drops every page hit from a
        // workspace search, and the palette keeps whichever leg survived. So one
        // failed `ListPages` — a second round trip, on a paged view, after the
        // search already returned — silently emptied page results that existed.
        // An index we could not read leaves every hit on the "Untitled"
        // fallback `titled_page_hits` already takes for an unknown page id.
        let index = load_page_index(&rpc).await.unwrap_or_default();
        Ok(PageSearchData {
            hits: titled_page_hits(hits, index),
        })
    }
    .await;
    result.map_err(app_error)
}
