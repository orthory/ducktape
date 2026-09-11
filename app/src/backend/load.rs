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
    // The directory before any row: every author, member and huddle tile below
    // is named through it, and this is the load that runs on boot and on every
    // resync — including the one an identity op triggers.
    //
    // A directory that cannot be read yet — a resident whose identity index is
    // still folding — is a room named by key hex, never a room that failed to
    // open: this must not be the `?` that takes the whole chat load down.
    if let Err(error) = refresh_names(rpc).await {
        tracing::debug!(
            target: "ducktape::chat",
            %error,
            "the name directory was not read; keys render as hex"
        );
    }
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
    let facts = ReaderFacts::current().await;
    let huddle_roster = active_wire_channel.map_or_else(Vec::new, |info| {
        huddle_roster(&info.channel.huddle, facts.reader())
    });
    // NO TIMELINE. The chat view reads its own room's messages off the index;
    // what the app still needs of a room is its record, its huddle roster and
    // its member roll — the roster the call's media leg hangs on, and the roll
    // the composers complete mentions against.
    let channel_members = match active_channel.is_empty() {
        true => Vec::new(),
        false => load_channel_members(rpc, &active_channel, facts.names()).await?,
    };
    Ok(ChatData {
        generation: 0,
        channels,
        active_channel,
        active_channel_name,
        active_channel_archived,
        active_channel_members_only,
        huddle_roster,
        channel_members,
    })
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
    reader: ChatReader<'_>,
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
    // The roster is named through the reader handed in — the directory as
    // last read, never a filling query: `load_channel_row` awaits this inside
    // the live stream's decoder fold, where a `/v1/query` would freeze every
    // subscriber for as long as the node's select loop is busy.
    let roster = huddle_roster(&info.channel.huddle, reader);
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
/// What is left is two INDEPENDENT reads: the channel's own row (for the
/// huddle roster) and the member roll. They run concurrently, so a switch
/// costs one round trip — and no timeline: the chat view reads its own room's
/// messages off the index, on the kernel contract.
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
) -> Result<ChatData, String> {
    // Awaited before the fan-out so the cached identity is warm for every leg:
    // there is no single-flight, and two cold callers would each spawn the
    // CLI. Same reason `load_chat_data` awaits it above its own read.
    let facts = ReaderFacts::current().await;
    let (room, channel_members) = tokio::try_join!(
        load_channel_facts(rpc, channel_id, facts.reader()),
        load_channel_members(rpc, channel_id, facts.names())
    )?;
    let Some((channel, huddle_roster)) = room else {
        return load_chat_data(rpc, None).await;
    };
    Ok(ChatData {
        generation: 0,
        channels: vec![channel.clone()],
        active_channel: channel.id,
        active_channel_name: channel.name,
        active_channel_archived: channel.archived,
        active_channel_members_only: channel.members_only,
        huddle_roster,
        channel_members,
    })
}

pub(crate) async fn load_channel_members(
    rpc: &RpcClient,
    channel_id: &str,
    names: &NameDirectory,
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
            let id = member_id(&member.party);
            ChatMember {
                label: names.member_label(id),
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

/// ONE ROOT-INDEX PAGE, with the cursor the node handed back verified against
/// the rows it came with. The chat TAB reads its own windows now; what is left
/// on this side is the forge item's discussion, which is a channel's newest
/// page and nothing else.
async fn query_roots(
    rpc: &RpcClient,
    channel_id: &str,
    before_seq: Option<u64>,
) -> Result<Vec<MsgRow>, String> {
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
    Ok(roots)
}

pub(crate) async fn load_messages(
    rpc: &RpcClient,
    channel_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let roots = query_roots(rpc, channel_id, None).await?;
    let facts = ReaderFacts::current().await;
    let mut messages: Vec<ChatMessage> = roots
        .into_iter()
        .map(|row| chat_message(row, facts.reader()))
        .collect();
    mark_message_groups(&mut messages);
    Ok(messages)
}

/// ONE THREAD WITH ITS WHOLE CONVERSATION. The grouped page query already
/// carries every comment, so the card never asks a second time.
pub(crate) fn page_comment_thread(thread: ThreadRow, names: &NameDirectory) -> PageCommentThread {
    let comments: Vec<PageComment> = thread
        .comments
        .into_iter()
        .filter(|comment| !comment.deleted)
        .enumerate()
        .map(|(index, comment)| page_comment(index + 1, comment, names))
        .collect();
    let comment_count = count_i64(comments.len());
    let count_label = if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    };
    PageCommentThread {
        id: thread.id,
        target: thread.target,
        author: author_display(&thread.opener, names),
        meta: if thread.resolved {
            format!("{count_label} · resolved")
        } else {
            count_label
        },
        resolved: thread.resolved,
        comment_count,
        comments,
    }
}

/// `created_at` is a block HEIGHT on a validator network and unix millis on a
/// single-writer noded (the same hybrid `ChatMessage::time` carries), so the
/// slot beside an author cannot be an age. It names the comment's place in
/// its thread instead, and says when one was edited.
fn page_comment(ordinal: usize, comment: CommentRow, names: &NameDirectory) -> PageComment {
    let edited = comment.edited_at.is_some();
    let ordinal = count_i64(ordinal);
    PageComment {
        id: comment.id,
        ordinal,
        author: author_display(&comment.author, names),
        meta: if edited {
            format!("#{ordinal} · edited")
        } else {
            format!("#{ordinal}")
        },
        text: comment.text,
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
    // THE CHIP COUNTS WHAT IS OUTSTANDING. A resolved thread is filed away —
    // the card keeps it behind its own toggle — so counting it here made the
    // header disagree with every margin badge under it.
    let comment_thread_total = count_i64(threads.iter().filter(|row| !row.resolved).count());
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
