use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chat::{AuthorRef, ChatMsg, ChatQuery, ChatReply, PostPolicy};
use ducktape_rpc::{Client as RpcClient, ModuleEvent, Status as NodeStatus};
use iced::futures::StreamExt as _;
use pages::{BlockKind, NewBlock, PageMsg, PageQuery, PageReply};
use tokio::io::AsyncWriteExt as _;
use zeroize::{Zeroize as _, Zeroizing};

const DEFAULT_RPC: &str = "http://127.0.0.1:8844";
const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;
const MAX_KEY_FILE_BYTES: u64 = 64 * 1024;
const MAX_FRAME_HEX_BYTES: usize = 3 * 1024 * 1024;
const ENCRYPTED_KEY_PREFIX: &str = "ducktape-user-key-v1:";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const CHAT_TIMELINE_ROOT_LIMIT: usize = 128;

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatChannel {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub members_only: bool,
    pub huddle_count: i64,
    pub head_seq: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatReaction {
    pub emoji: String,
    pub count: i64,
    pub reacted_by_me: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatMember {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    pub seq: i64,
    pub author: String,
    pub meta: String,
    pub body: String,
    pub pending: bool,
    pub rev: i64,
    pub edited: bool,
    pub deleted: bool,
    pub reply_count: i64,
    pub thread_seq: i64,
    pub reactions: Vec<ChatReaction>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatData {
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub active_channel_huddle_count: i64,
    pub channel_members: Vec<ChatMember>,
    pub selected_message_seq: i64,
    pub selected_message_rev: i64,
    pub selected_message_body: String,
    pub active_thread_seq: i64,
    pub thread_target_seq: i64,
    pub thread_messages: Vec<ChatMessage>,
    pub thread_next_reply_offset: i64,
    pub thread_has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
struct ThreadData {
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ThreadLoadData {
    pub generation: i64,
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ThreadPageData {
    pub generation: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LiveThreadData {
    pub generation: i64,
    pub channel_id: String,
    pub root_seq: i64,
    pub target_seq: i64,
    pub messages: Vec<ChatMessage>,
    pub next_reply_offset: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatSearchHit {
    pub channel_id: String,
    pub seq: i64,
    pub root_seq: i64,
    pub author: String,
    pub text: String,
    pub meta: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatSearchData {
    pub generation: i64,
    pub hits: Vec<ChatSearchHit>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageItem {
    pub id: String,
    pub title: String,
    pub parent: String,
    pub prefix: String,
    pub child_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageBlock {
    pub key: i64,
    pub id: String,
    pub parent: String,
    pub kind: String,
    pub text: String,
    pub pending: bool,
    pub checked: bool,
    pub prefix: String,
    pub child_count: i64,
    pub mark_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PagesData {
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    pub selected_block_id: String,
    pub selected_block_kind: String,
    pub selected_block_text: String,
    pub selected_block_checked: bool,
    pub page_title_selected: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageCommentThread {
    pub id: String,
    pub author: String,
    pub meta: String,
    pub resolved: bool,
    pub comment_count: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageComment {
    pub id: String,
    pub ordinal: i64,
    pub author: String,
    pub meta: String,
    pub text: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlockThreadListData {
    pub generation: i64,
    pub target: String,
    pub from: i64,
    pub threads: Vec<PageCommentThread>,
    pub total: i64,
    pub next_from: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlockCommentData {
    pub generation: i64,
    pub target: String,
    pub thread_id: String,
    pub from: i64,
    pub comments: Vec<PageComment>,
    pub next_from: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BlockCommentsRefreshData {
    pub generation: i64,
    pub target: String,
    pub threads: Vec<PageCommentThread>,
    pub total: i64,
    pub threads_next_from: i64,
    pub threads_has_more: bool,
    pub thread_id: String,
    pub comments: Vec<PageComment>,
    pub comments_next_from: i64,
    pub comments_has_more: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageSearchHit {
    pub page_id: String,
    pub block_id: String,
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageSearchData {
    pub generation: i64,
    pub hits: Vec<PageSearchHit>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AutosaveResult {
    pub generation: i64,
    pub written: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct WorkspaceData {
    pub generation: i64,
    pub rpc: String,
    pub status: String,
    pub height: i64,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub active_channel_name: String,
    pub active_channel_archived: bool,
    pub active_channel_members_only: bool,
    pub active_channel_huddle_count: i64,
    pub channel_members: Vec<ChatMember>,
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AppError {
    pub message: String,
    pub committed: bool,
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self {
            message,
            committed: false,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct HydrationError {
    pub generation: i64,
    pub message: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LiveUpdate {
    pub kind: String,
    pub status: String,
    pub height: i64,
}

pub fn optimistic_message(mut messages: Vec<ChatMessage>, body: String) -> Vec<ChatMessage> {
    messages.push(ChatMessage {
        id: "pending".into(),
        seq: -1,
        author: "You".into(),
        meta: "Sending…".into(),
        body,
        pending: true,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        reactions: Vec::new(),
    });
    messages
}

pub fn rollback_messages(mut messages: Vec<ChatMessage>, keep_pending: bool) -> Vec<ChatMessage> {
    if keep_pending {
        return messages;
    }
    messages.retain(|message| !message.pending);
    messages
}

pub fn append_thread_page(messages: Vec<ChatMessage>, next: Vec<ChatMessage>) -> Vec<ChatMessage> {
    messages
        .into_iter()
        .chain(next)
        .map(|message| (message.seq, message))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

pub fn finish_thread_reply(mut messages: Vec<ChatMessage>, reply: ChatMessage) -> Vec<ChatMessage> {
    messages.retain(|message| !message.pending && message.seq != reply.seq);
    messages.push(reply);
    messages
}

pub fn thread_offset_after_reply(offset: i64, has_more: bool) -> i64 {
    if offset < 0 || has_more {
        offset
    } else {
        offset.saturating_add(1)
    }
}

pub fn optimistic_block(
    mut blocks: Vec<PageBlock>,
    after_id: String,
    kind: String,
    text: String,
) -> Vec<PageBlock> {
    let selected_index = blocks.iter().position(|block| block.id == after_id);
    let (insert_at, parent, prefix) = match selected_index {
        Some(index) => {
            let selected = &blocks[index];
            let selected_depth = selected.prefix.len();
            let insert_at = blocks
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, block)| block.prefix.len() <= selected_depth)
                .map_or(blocks.len(), |(index, _)| index);
            (insert_at, selected.parent.clone(), selected.prefix.clone())
        }
        None => (blocks.len(), String::new(), String::new()),
    };
    let id = loop {
        let id = fresh_id("pending-block");
        if blocks.iter().all(|block| block.id != id) {
            break id;
        }
    };
    blocks.insert(
        insert_at,
        PageBlock {
            key: page_block_key(&id),
            id,
            parent,
            kind,
            text,
            pending: true,
            checked: false,
            prefix,
            child_count: 0,
            mark_count: 0,
        },
    );
    blocks
}

pub fn rollback_blocks(mut blocks: Vec<PageBlock>, keep_pending: bool) -> Vec<PageBlock> {
    if keep_pending {
        return blocks;
    }
    blocks.retain(|block| !block.pending);
    blocks
}

pub fn append_page_comment_threads(
    threads: Vec<PageCommentThread>,
    next: Vec<PageCommentThread>,
) -> Vec<PageCommentThread> {
    threads
        .into_iter()
        .chain(next)
        .map(|thread| (thread.id.clone(), thread))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

pub fn append_page_comments(
    comments: Vec<PageComment>,
    next: Vec<PageComment>,
) -> Vec<PageComment> {
    comments
        .into_iter()
        .chain(next)
        .map(|comment| (comment.ordinal, comment))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

pub fn restore_draft(current: String, pending: String, keep_pending: bool) -> String {
    if keep_pending {
        return current;
    }
    if current.is_empty() { pending } else { current }
}

pub fn remember_failed_draft(
    existing: String,
    current: String,
    pending: String,
    committed: bool,
) -> String {
    if committed || current.is_empty() || pending.is_empty() {
        return existing;
    }
    if existing.is_empty() {
        return pending;
    }
    format!("{existing}\n{pending}")
}

pub fn retain_for_endpoint(value: String, current: String, next: String) -> String {
    if current == next {
        value
    } else {
        String::new()
    }
}

pub fn mutation_failure_phase(committed: bool) -> String {
    if committed { "recovering" } else { "idle" }.into()
}

fn committed_message_change(phase: &str, committed: bool) -> bool {
    committed && matches!(phase, "message-edit" | "message-delete")
}

pub fn message_seq_after_failure(current: i64, phase: String, committed: bool) -> i64 {
    if committed_message_change(&phase, committed) {
        0
    } else {
        current
    }
}

pub fn message_text_after_failure(current: String, phase: String, committed: bool) -> String {
    if committed_message_change(&phase, committed) {
        String::new()
    } else {
        current
    }
}

pub fn message_action_after_failure(current: String, phase: String, committed: bool) -> String {
    if committed_message_change(&phase, committed) {
        "toolbar".into()
    } else {
        current
    }
}

pub fn refreshed_block_draft(
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
    autosave_status: String,
) -> String {
    let has_local_edit = matches!(autosave_status.as_str(), "saving" | "error");
    if selected_id.is_empty() || has_local_edit {
        return current;
    }
    blocks
        .into_iter()
        .find(|block| block.id == selected_id)
        .map_or(current, |block| block.text)
}

pub fn remember_orphaned_block_drafts(
    mut drafts: Vec<String>,
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
    autosave_status: String,
) -> Vec<String> {
    let has_local_edit = matches!(autosave_status.as_str(), "saving" | "error");
    if has_local_edit && selected_block_missing(&blocks, &selected_id) {
        append_recovered_draft(&mut drafts, current);
    }
    drafts
}

pub fn remember_orphaned_comment_drafts(
    mut drafts: Vec<String>,
    blocks: Vec<PageBlock>,
    selected_id: String,
    current: String,
) -> Vec<String> {
    if selected_block_missing(&blocks, &selected_id) {
        append_recovered_draft(&mut drafts, current);
    }
    drafts
}

pub fn remove_recovered_draft(mut drafts: Vec<String>, recovered: String) -> Vec<String> {
    if let Some(index) = drafts.iter().position(|draft| draft == &recovered) {
        drafts.remove(index);
    }
    drafts
}

pub fn retain_drafts_for_endpoint(
    drafts: Vec<String>,
    current: String,
    next: String,
) -> Vec<String> {
    if current == next { drafts } else { Vec::new() }
}

pub fn refreshed_selected_block(blocks: Vec<PageBlock>, selected_id: String) -> String {
    if selected_block_missing(&blocks, &selected_id) {
        String::new()
    } else {
        selected_id
    }
}

pub fn retain_selected_string(value: String, selected_id: String) -> String {
    if selected_id.is_empty() {
        String::new()
    } else {
        value
    }
}

pub fn retain_selected_i64(value: i64, selected_id: String) -> i64 {
    if selected_id.is_empty() { 0 } else { value }
}

pub fn retain_selected_comment_threads(
    threads: Vec<PageCommentThread>,
    selected_id: String,
) -> Vec<PageCommentThread> {
    if selected_id.is_empty() {
        Vec::new()
    } else {
        threads
    }
}

pub fn retain_selected_comments(
    comments: Vec<PageComment>,
    selected_id: String,
) -> Vec<PageComment> {
    if selected_id.is_empty() {
        Vec::new()
    } else {
        comments
    }
}

pub fn cancel_missing_block_autosave(
    rpc: String,
    generation: i64,
    blocks: Vec<PageBlock>,
    selected_id: String,
) -> i64 {
    if selected_block_missing(&blocks, &selected_id) {
        let key = scope_key(rpc.trim().to_string(), selected_id);
        autosaves()
            .lock()
            .expect("autosave lock poisoned")
            .remove(&key);
        return generation.saturating_add(1);
    }
    generation
}

fn selected_block_missing(blocks: &[PageBlock], selected_id: &str) -> bool {
    !selected_id.is_empty() && !blocks.iter().any(|block| block.id == selected_id)
}

fn append_recovered_draft(drafts: &mut Vec<String>, draft: String) {
    let should_append = !draft.is_empty() && !drafts.iter().any(|current| current == &draft);
    if should_append {
        drafts.push(draft);
    }
}

pub fn scope_key(scope: String, id: String) -> String {
    format!("{scope}\0{id}")
}

pub async fn defer_focus(scope: String) -> String {
    scope
}

pub fn focus_next() -> iced::Task<()> {
    iced::widget::operation::focus_next()
}

pub fn block_action_menu_y(pointer_y: f64, viewport_height: f64) -> f64 {
    let below = (pointer_y - 4.0).max(0.0);
    let below_fits = below + 190.0 <= viewport_height;
    if below_fits {
        below
    } else {
        (pointer_y - 190.0).max(0.0)
    }
}

struct Tip {
    height: i64,
    status: String,
}

fn rpc_client(input: &str) -> Result<RpcClient, String> {
    let configured = if input.trim().is_empty() {
        std::env::var("DUCKTAPE_NODE").unwrap_or_else(|_| DEFAULT_RPC.to_string())
    } else {
        input.trim().to_string()
    };
    RpcClient::new(&configured).map_err(Into::into)
}

pub fn canonical_endpoint(input: String) -> String {
    let configured = input.trim();
    rpc_client(configured)
        .map(|rpc| rpc.origin().to_string())
        .unwrap_or_else(|_| configured.to_string())
}

pub async fn connect(rpc: String) -> Result<WorkspaceData, AppError> {
    let result = async {
        let rpc = rpc_client(&rpc)?;
        load_workspace(&rpc, None, None, 0).await
    }
    .await;
    result.map_err(|_| AppError {
        message: "Could not connect. Check the endpoint and node.".into(),
        committed: false,
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
                    rpc.module_events(
                        vec!["chat".to_string(), "pages".to_string()],
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
            let event = state
                .stream
                .as_mut()
                .expect("stream initialized above")
                .next()
                .await;
            match event {
                Some(Ok(ModuleEvent::Ready { cursors })) => {
                    state.cursors = cursors;
                    state.retry_attempt = 0;
                    Some((live_update("ready", "Live", -1), state))
                }
                Some(Ok(ModuleEvent::Changed {
                    module,
                    cursor,
                    height,
                })) => {
                    state.cursors.insert(format!("module:{module}"), cursor);
                    let height = i64::try_from(height).unwrap_or(i64::MAX);
                    Some((
                        live_update("changed", &format!("Live · block {height}"), height),
                        state,
                    ))
                }
                Some(Ok(ModuleEvent::Lagged { module, cursor })) => {
                    state.cursors.insert(format!("module:{module}"), cursor);
                    Some((live_update("changed", "Live · resyncing", -1), state))
                }
                Some(Err(error)) => {
                    state.stream = None;
                    state.retry_attempt = state.retry_attempt.saturating_add(1);
                    Some((live_retry(error.into()), state))
                }
                None => {
                    state.stream = None;
                    state.retry_attempt = state.retry_attempt.saturating_add(1);
                    Some((live_retry("RPC stream closed".into()), state))
                }
            }
        },
    )
    .boxed()
}

pub async fn refresh(
    rpc: String,
    channel_id: String,
    page_id: String,
    generation: i64,
) -> Result<WorkspaceData, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        load_workspace(
            &rpc,
            (!channel_id.is_empty()).then_some(channel_id.as_str()),
            (!page_id.is_empty()).then_some(page_id.as_str()),
            generation,
        )
        .await
    }
    .await
    .map_err(|message| HydrationError {
        generation,
        message,
    })
}

pub async fn retry_refresh(
    rpc: String,
    channel_id: String,
    page_id: String,
    generation: i64,
    attempt: i64,
) -> Result<WorkspaceData, HydrationError> {
    let attempt = u32::try_from(attempt).unwrap_or(u32::MAX);
    tokio::time::sleep(retry_delay(attempt)).await;
    refresh(rpc, channel_id, page_id, generation).await
}

pub async fn load_chat(rpc: String, channel_id: String) -> Result<ChatData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        load_chat_data(&rpc, Some(&channel_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn load_chat_hit(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
) -> Result<ChatData, AppError> {
    async {
        let root_seq = positive_sequence(root_seq)?;
        let target_seq = positive_sequence(target_seq)?;
        let rpc = rpc_client(&rpc)?;
        let mut chat = load_chat_data(&rpc, Some(&channel_id)).await?;
        chat.messages = load_messages_around(&rpc, &channel_id, root_seq).await?;
        let root = chat
            .messages
            .iter()
            .find(|message| message.seq == number_i64(root_seq))
            .cloned()
            .ok_or_else(|| "message was not found".to_string())?;
        chat.selected_message_seq = root.seq;
        chat.selected_message_rev = root.rev;
        chat.selected_message_body.clone_from(&root.body);
        if target_seq == root_seq {
            return Ok(chat);
        }
        let reply = load_message_at(&rpc, &channel_id, target_seq).await?;
        if reply.head.thread != Some(root_seq) {
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
    .map_err(app_error)
}

pub async fn create_channel(
    rpc: String,
    password: String,
    name: String,
) -> Result<ChatData, AppError> {
    async {
        let name = bounded_text(name, "channel name", 128)?;
        let channel_id = fresh_id("channel");
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel_id.clone(),
                name,
                post_policy: PostPolicy::Open,
            }),
            password,
        )
        .await?;
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn rename_channel(
    rpc: String,
    password: String,
    channel_id: String,
    name: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn archive_channel(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn unarchive_channel(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn add_channel_member(
    rpc: String,
    password: String,
    channel_id: String,
    member_key: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn remove_channel_member(
    rpc: String,
    password: String,
    channel_id: String,
    member_key: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn join_huddle(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn leave_huddle(
    rpc: String,
    password: String,
    channel_id: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn send_message(
    rpc: String,
    password: String,
    channel_id: String,
    body: String,
) -> Result<ChatData, AppError> {
    async {
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
                message_id: fresh_id("message"),
                blocks: vec![chat::Block::paragraph(body)],
                thread: None,
                as_agent: None,
            }),
            password,
        )
        .await?;
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn load_thread(
    rpc: String,
    channel_id: String,
    root_seq: i64,
    target_seq: i64,
    through_reply_offset: i64,
    committed_reply: bool,
    generation: i64,
) -> Result<ThreadLoadData, HydrationError> {
    let result = async {
        let root_seq = positive_sequence(root_seq)?;
        let target_seq = u64::try_from(target_seq).unwrap_or(0);
        let is_sparse_target = !committed_reply && through_reply_offset < 0 && target_seq > 0;
        let requested_reply_offset = u64::try_from(through_reply_offset)
            .unwrap_or(0)
            .min(chat::MAX_THREAD_REPLIES as u64);
        let through_reply_offset = if committed_reply {
            chat::MAX_THREAD_REPLIES as u64
        } else {
            requested_reply_offset
        };
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
            message,
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
        let thread = query_thread_page(&rpc, &channel_id, root_seq, from).await?;
        let reply_count = thread.replies.len() as u64;
        let current_user = local_user_key().await;
        let next_reply_offset = from.saturating_add(reply_count);
        let page_is_full = reply_count == chat::MAX_QUERY_LIMIT;
        let thread_cap_reached = next_reply_offset >= chat::MAX_THREAD_REPLIES as u64;
        Ok(ThreadPageData {
            generation,
            messages: thread
                .replies
                .into_iter()
                .map(|message| chat_message(message, current_user.as_deref()))
                .collect(),
            next_reply_offset: number_i64(next_reply_offset),
            has_more: page_is_full && !thread_cap_reached,
        })
    }
    .await;
    result.map_err(|message| HydrationError {
        generation,
        message,
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
        false,
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
    body: String,
) -> Result<ChatMessage, AppError> {
    async {
        let root_seq = positive_sequence(root_seq)?;
        let body = bounded_text(body, "reply", 16 * 1024)?;
        let rpc = rpc_client(&rpc)?;
        let message_id = fresh_id("message");
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: channel_id.clone(),
                message_id: message_id.clone(),
                blocks: vec![chat::Block::paragraph(body)],
                thread: Some(root_seq),
                as_agent: None,
            }),
            password,
        )
        .await?;
        let reply = load_reply_by_id(&rpc, &message_id, &channel_id, root_seq)
            .await
            .map_err(committed_error)?;
        let current_user = local_user_key().await;
        Ok(chat_message(reply, current_user.as_deref()))
    }
    .await
}

pub async fn edit_message(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    base_rev: i64,
    body: String,
) -> Result<ChatData, AppError> {
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
                blocks: vec![chat::Block::paragraph(body)],
                base_rev: Some(base_rev),
            }),
            password,
        )
        .await?;
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn delete_message(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn add_reaction(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    emoji: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn remove_reaction(
    rpc: String,
    password: String,
    channel_id: String,
    seq: i64,
    emoji: String,
) -> Result<ChatData, AppError> {
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
        load_chat_data(&rpc, Some(&channel_id))
            .await
            .map_err(committed_error)
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
        let reply: chat::index::ChatViewReply = rpc
            .view(
                "chat",
                &serde_json::json!({
                    "search": {
                        "text": text,
                        "channel_id": (!channel_id.is_empty()).then_some(channel_id),
                        "limit": 50
                    }
                }),
            )
            .await?;
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
        message,
    })
}

pub async fn load_page(
    rpc: String,
    page_id: String,
    selected_block_id: String,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let pages = load_pages_data(&rpc, Some(&page_id)).await?;
        Ok(with_selected_block(pages, &selected_block_id))
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
        message,
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
        message,
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
        message,
    })
}

pub async fn create_block_thread(
    rpc: String,
    password: String,
    target: String,
    text: String,
    generation: i64,
) -> Result<BlockCommentData, AppError> {
    async {
        let target = required_id(target, "block")?;
        let text = bounded_text(text, "comment", 16 * 1024)?;
        let thread_id = fresh_id("thread");
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

pub async fn autosave_page_title(
    rpc: String,
    password: String,
    page_id: String,
    title: String,
) -> Result<bool, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string());
        }
        let title = bounded_exact_text(title, "page title", 512)?;
        debounced_page_text(rpc, password, page_id, title).await
    }
    .await
    .map_err(app_error)
}

pub async fn autosave_block_text(
    rpc: String,
    password: String,
    block_id: String,
    kind: String,
    text: String,
    generation: i64,
) -> Result<AutosaveResult, HydrationError> {
    let result = async {
        let kind = parse_block_kind(&kind)?;
        let text = bounded_updated_block_text(kind, text)?;
        debounced_page_text(rpc, password, block_id, text).await
    }
    .await;
    result
        .map(|written| AutosaveResult {
            generation,
            written,
        })
        .map_err(|message| HydrationError {
            generation,
            message,
        })
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

pub async fn add_block(
    rpc: String,
    password: String,
    page_id: String,
    after_id: String,
    kind: String,
    text: String,
) -> Result<PagesData, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string().into());
        }
        let kind = parse_block_kind(&kind)?;
        let text = bounded_new_block_text(kind, text)?;
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let root = blocks
            .first()
            .filter(|block| block.kind == BlockKind::Page)
            .ok_or_else(|| "page block was not found".to_string())?;
        let selected = blocks.iter().find(|block| block.id == after_id);
        let parent = selected
            .and_then(|block| block.parent.clone())
            .unwrap_or_else(|| page_id.clone());
        let after = selected
            .map(|block| block.id.clone())
            .or_else(|| root.children.last().cloned());
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent,
                after,
                block: NewBlock {
                    id: fresh_id(if kind == BlockKind::Page {
                        "page"
                    } else {
                        "block"
                    }),
                    kind,
                    text,
                    marks: Vec::new(),
                },
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

pub async fn save_block(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
    kind: String,
    text: String,
) -> Result<PagesData, AppError> {
    async {
        let kind = parse_block_kind(&kind)?;
        let text = bounded_updated_block_text(kind, text)?;
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let block = blocks
            .iter()
            .find(|block| block.id == block_id)
            .ok_or_else(|| "block was not found".to_string())?;
        let text_changed = block.text != text;
        let kind_changed = block.kind != kind;
        if text_changed {
            signed_write(
                &rpc,
                "pages",
                pages::encode_msg(&PageMsg::UpdateText {
                    block_id: block_id.clone(),
                    text,
                    marks: None,
                }),
                password.clone(),
            )
            .await?;
            if kind_changed {
                signed_write(
                    &rpc,
                    "pages",
                    pages::encode_msg(&PageMsg::SetKind { block_id, kind }),
                    password,
                )
                .await
                .map_err(committed_error)?;
            }
            return load_pages_data(&rpc, Some(&page_id))
                .await
                .map_err(committed_error);
        }
        if kind_changed {
            signed_write(
                &rpc,
                "pages",
                pages::encode_msg(&PageMsg::SetKind { block_id, kind }),
                password,
            )
            .await?;
            return load_pages_data(&rpc, Some(&page_id))
                .await
                .map_err(committed_error);
        }
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(app_error)
    }
    .await
}

pub async fn set_block_checked(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
    checked: bool,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::SetChecked { block_id, checked }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)
    }
    .await
}

pub async fn move_block(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
    direction: String,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let blocks = load_page_blocks(&rpc, &page_id).await?;
        let (parent, after) = block_move(&blocks, &block_id, &direction)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::MoveBlock {
                block_id,
                parent,
                after,
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

pub async fn remove_block(
    rpc: String,
    password: String,
    page_id: String,
    block_id: String,
) -> Result<PagesData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::RemoveBlock { block_id }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id))
            .await
            .map_err(committed_error)
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
        let pages::index::PagesViewReply::Hits(hits) = reply;
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
        message,
    })
}

async fn load_workspace(
    rpc: &RpcClient,
    channel_id: Option<&str>,
    page_id: Option<&str>,
    generation: i64,
) -> Result<WorkspaceData, String> {
    let tip = tip_from_status(rpc.status().await?)?;
    let chat = load_chat_data(rpc, channel_id).await?;
    let pages = load_pages_data(rpc, page_id).await?;
    Ok(WorkspaceData {
        generation,
        rpc: rpc.origin().to_string(),
        status: tip.status,
        height: tip.height,
        channels: chat.channels,
        messages: chat.messages,
        active_channel: chat.active_channel,
        active_channel_name: chat.active_channel_name,
        active_channel_archived: chat.active_channel_archived,
        active_channel_members_only: chat.active_channel_members_only,
        active_channel_huddle_count: chat.active_channel_huddle_count,
        channel_members: chat.channel_members,
        pages: pages.pages,
        blocks: pages.blocks,
        active_page: pages.active_page,
        active_page_title: pages.active_page_title,
        active_page_parent: pages.active_page_parent,
    })
}

fn tip_from_status(status: NodeStatus) -> Result<Tip, String> {
    let height = i64::try_from(status.height).map_err(|_| "node height exceeds i64")?;
    Ok(Tip {
        height,
        status: format!("Connected · block {height}"),
    })
}

async fn load_chat_data(rpc: &RpcClient, requested: Option<&str>) -> Result<ChatData, String> {
    let reply: ChatReply = rpc.query("chat", &ChatQuery::Channels).await?;
    let wire_channels = match reply {
        ChatReply::Channels(channels) => channels,
        _ => return Err("node returned an invalid channel list".into()),
    };
    let channels = wire_channels
        .iter()
        .map(|channel| ChatChannel {
            id: channel.id.clone(),
            name: channel.name.clone(),
            archived: channel.archived,
            members_only: channel.post_policy == PostPolicy::MembersOnly,
            huddle_count: count_i64(channel.huddle.len()),
            head_seq: number_i64(channel.head_seq),
        })
        .collect::<Vec<_>>();
    let active_channel = requested
        .filter(|id| channels.iter().any(|channel| channel.id == *id))
        .map(str::to_string)
        .or_else(|| channels.first().map(|channel| channel.id.clone()))
        .unwrap_or_default();
    let active_channel_name = channels
        .iter()
        .find(|channel| channel.id == active_channel)
        .map(|channel| channel.name.clone())
        .unwrap_or_default();
    let active_wire_channel = wire_channels
        .iter()
        .find(|channel| channel.id == active_channel);
    let active_channel_archived = active_wire_channel.is_some_and(|channel| channel.archived);
    let active_channel_members_only =
        active_wire_channel.is_some_and(|channel| channel.post_policy == PostPolicy::MembersOnly);
    let active_channel_huddle_count =
        active_wire_channel.map_or(0, |channel| count_i64(channel.huddle.len()));
    let active_channel_head_seq = active_wire_channel.map_or(0, |channel| channel.head_seq);
    let channel_members = if active_channel.is_empty() {
        Vec::new()
    } else {
        load_channel_members(rpc, &active_channel).await?
    };
    let messages = if active_channel.is_empty() {
        Vec::new()
    } else {
        load_messages(rpc, &active_channel, active_channel_head_seq).await?
    };
    Ok(ChatData {
        channels,
        messages,
        active_channel,
        active_channel_name,
        active_channel_archived,
        active_channel_members_only,
        active_channel_huddle_count,
        channel_members,
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_next_reply_offset: 0,
        thread_has_more: false,
    })
}

async fn load_channel_members(
    rpc: &RpcClient,
    channel_id: &str,
) -> Result<Vec<ChatMember>, String> {
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::Members {
                channel_id: channel_id.to_string(),
            },
        )
        .await?;
    let members = match reply {
        ChatReply::Members(members) => members,
        _ => return Err("node returned an invalid channel member list".into()),
    };
    Ok(members
        .into_iter()
        .map(|member| ChatMember {
            label: short_hex(&member),
            key: hex_encode(&member),
        })
        .collect())
}

async fn load_messages(
    rpc: &RpcClient,
    channel_id: &str,
    head_seq: u64,
) -> Result<Vec<ChatMessage>, String> {
    let mut cursor = head_seq;
    let mut roots = Vec::new();
    while cursor > 0 && roots.len() < CHAT_TIMELINE_ROOT_LIMIT {
        let limit = chat::MAX_QUERY_LIMIT.min(cursor);
        let from_seq = cursor - limit + 1;
        let reply: ChatReply = rpc
            .query(
                "chat",
                &ChatQuery::MessagesRange {
                    channel_id: channel_id.to_string(),
                    from_seq,
                    limit,
                },
            )
            .await?;
        let messages = match reply {
            ChatReply::Messages(messages) => messages,
            _ => return Err("node returned an invalid message list".into()),
        };
        roots.extend(
            messages
                .into_iter()
                .filter(|message| message.head.thread.is_none()),
        );
        if from_seq == 1 {
            break;
        }
        cursor = from_seq - 1;
    }
    roots.sort_by_key(|message| message.seq);
    let excess = roots.len().saturating_sub(CHAT_TIMELINE_ROOT_LIMIT);
    roots.drain(..excess);
    let current_user = local_user_key().await;
    Ok(roots
        .into_iter()
        .map(|message| chat_message(message, current_user.as_deref()))
        .collect())
}

async fn load_messages_around(
    rpc: &RpcClient,
    channel_id: &str,
    seq: u64,
) -> Result<Vec<ChatMessage>, String> {
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::MessagesAround {
                channel_id: channel_id.to_string(),
                seq,
                limit: chat::MAX_QUERY_LIMIT,
            },
        )
        .await?;
    let messages = match reply {
        ChatReply::Messages(messages) => messages,
        _ => return Err("node returned an invalid message window".into()),
    };
    let current_user = local_user_key().await;
    Ok(messages
        .into_iter()
        .filter(|message| message.head.thread.is_none())
        .map(|message| chat_message(message, current_user.as_deref()))
        .collect())
}

async fn load_message_at(
    rpc: &RpcClient,
    channel_id: &str,
    seq: u64,
) -> Result<chat::MessageView, String> {
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::MessagesAround {
                channel_id: channel_id.to_string(),
                seq,
                limit: 1,
            },
        )
        .await?;
    let messages = match reply {
        ChatReply::Messages(messages) => messages,
        _ => return Err("node returned an invalid message window".into()),
    };
    messages
        .into_iter()
        .find(|message| message.seq == seq)
        .ok_or_else(|| "message was not found".into())
}

async fn load_reply_by_id(
    rpc: &RpcClient,
    message_id: &str,
    channel_id: &str,
    root_seq: u64,
) -> Result<chat::MessageView, String> {
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::Message {
                message_id: message_id.to_string(),
            },
        )
        .await?;
    let message = match reply {
        ChatReply::Message(Some(message)) => message,
        _ => return Err("reply was not found".into()),
    };
    let is_expected_reply =
        message.channel_id == channel_id && message.head.thread == Some(root_seq);
    if !is_expected_reply {
        return Err("node returned an invalid thread reply".into());
    }
    Ok(message)
}

async fn query_thread_page(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    from: u64,
) -> Result<chat::Thread, String> {
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::Thread {
                channel_id: channel_id.to_string(),
                root_seq,
                from,
                limit: chat::MAX_QUERY_LIMIT,
            },
        )
        .await?;
    match reply {
        ChatReply::Thread(Some(thread)) => Ok(thread),
        _ => Err("thread was not found".into()),
    }
}

async fn load_sparse_thread_data(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    target_seq: u64,
) -> Result<ThreadData, String> {
    let root = load_message_at(rpc, channel_id, root_seq).await?;
    let target = load_message_at(rpc, channel_id, target_seq).await?;
    if target.head.thread != Some(root_seq) {
        return Err("search result does not belong to the selected thread".into());
    }
    let current_user = local_user_key().await;
    Ok(ThreadData {
        root_seq: number_i64(root_seq),
        target_seq: number_i64(target_seq),
        messages: vec![
            chat_message(root, current_user.as_deref()),
            chat_message(target, current_user.as_deref()),
        ],
        next_reply_offset: -1,
        has_more: false,
    })
}

async fn load_thread_data(
    rpc: &RpcClient,
    channel_id: &str,
    root_seq: u64,
    through_reply_offset: u64,
) -> Result<ThreadData, String> {
    if channel_id.is_empty() || root_seq == 0 {
        return Ok(ThreadData {
            root_seq: 0,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        });
    }

    let mut root = None;
    let mut replies = Vec::new();
    let mut from = 0_u64;
    let has_more = loop {
        let thread = query_thread_page(rpc, channel_id, root_seq, from).await?;
        if root.is_none() {
            root = Some(thread.root);
        }
        let page_len = thread.replies.len() as u64;
        replies.extend(thread.replies);
        from = from.saturating_add(page_len);
        let page_is_full = page_len == chat::MAX_QUERY_LIMIT;
        let thread_cap_reached = from >= chat::MAX_THREAD_REPLIES as u64;
        let has_more = page_is_full && !thread_cap_reached;
        let first_page_is_enough = through_reply_offset == 0;
        let requested_offset_is_loaded = from >= through_reply_offset;
        if !has_more || first_page_is_enough || requested_offset_is_loaded {
            break has_more;
        }
    };
    let root = root.ok_or_else(|| "thread was not found".to_string())?;
    let current_user = local_user_key().await;
    let messages = std::iter::once(root)
        .chain(replies)
        .map(|message| chat_message(message, current_user.as_deref()))
        .collect();
    Ok(ThreadData {
        root_seq: number_i64(root_seq),
        target_seq: 0,
        messages,
        next_reply_offset: number_i64(from),
        has_more,
    })
}

fn chat_message(message: chat::MessageView, current_user: Option<&[u8]>) -> ChatMessage {
    let edited = message.head.rev > 0;
    let meta = if edited {
        format!("#{} · edited", message.seq)
    } else {
        format!("#{}", message.seq)
    };
    ChatMessage {
        id: message.head.message_id,
        seq: number_i64(message.seq),
        author: author_name(&message.head.author),
        meta,
        body: if message.head.deleted {
            "Message deleted".into()
        } else {
            message_body(&message.head.blocks)
        },
        pending: false,
        rev: i64::from(message.head.rev),
        edited,
        deleted: message.head.deleted,
        reply_count: number_i64(message.head.reply_count),
        thread_seq: number_i64(message.head.thread.unwrap_or(0)),
        reactions: message
            .reactions
            .into_iter()
            .map(|reaction| {
                let reacted_by_me = reacted_by_user(&reaction.reactors, current_user);
                ChatReaction {
                    emoji: reaction.emoji,
                    count: count_i64(reaction.reactors.len()),
                    reacted_by_me,
                }
            })
            .collect(),
    }
}

fn reacted_by_user(reactors: &BTreeSet<AuthorRef>, current_user: Option<&[u8]>) -> bool {
    current_user.is_some_and(|current_user| {
        reactors
            .iter()
            .any(|reactor| matches!(reactor, AuthorRef::User(key) if key == current_user))
    })
}

async fn query_block_threads(
    rpc: &RpcClient,
    target: &str,
    from: u32,
    generation: i64,
) -> Result<BlockThreadListData, String> {
    let reply: PageReply = rpc
        .query(
            "pages",
            &PageQuery::ThreadsForTarget {
                target: target.to_string(),
                from,
                limit: 0,
            },
        )
        .await?;
    let PageReply::ThreadPage(page) = reply else {
        return Err("node returned an invalid comment thread page".into());
    };
    if page.target != target {
        return Err("node returned comment threads for another block".into());
    }
    let has_more = page.next_from.is_some();
    Ok(BlockThreadListData {
        generation,
        target: page.target,
        from: i64::from(from),
        threads: page.threads.into_iter().map(page_comment_thread).collect(),
        total: i64::from(page.total),
        next_from: page.next_from.map_or(0, i64::from),
        has_more,
    })
}

async fn query_block_comment_page(
    rpc: &RpcClient,
    target: &str,
    thread_id: &str,
    from: u32,
    generation: i64,
) -> Result<Option<BlockCommentData>, String> {
    let reply: PageReply = rpc
        .query(
            "pages",
            &PageQuery::CommentsForThread {
                thread_id: thread_id.to_string(),
                from,
                limit: 0,
            },
        )
        .await?;
    let PageReply::CommentPage(page) = reply else {
        return Err("node returned an invalid comment page".into());
    };
    let Some(page) = page else {
        return Ok(None);
    };
    let is_expected_thread = page.thread.id == thread_id && page.thread.target == target;
    if !is_expected_thread {
        return Err("node returned comments for another block".into());
    }
    let has_more = page.next_from.is_some();
    Ok(Some(BlockCommentData {
        generation,
        target: page.thread.target,
        thread_id: page.thread.id,
        from: i64::from(from),
        comments: page.comments.into_iter().map(page_comment).collect(),
        next_from: page.next_from.map_or(0, i64::from),
        has_more,
    }))
}

fn page_comment_thread(thread: pages::CommentThread) -> PageCommentThread {
    let comment_count = i64::from(thread.comment_count);
    let count_label = if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    };
    PageCommentThread {
        id: thread.id,
        author: page_author_name(&thread.opener),
        meta: if thread.resolved {
            format!("{count_label} · resolved")
        } else {
            count_label
        },
        resolved: thread.resolved,
        comment_count,
    }
}

fn page_comment(item: pages::CommentItem) -> PageComment {
    let edited = item.comment.edited_at.is_some();
    PageComment {
        id: item.comment.id,
        ordinal: i64::from(item.ordinal),
        author: page_author_name(&item.comment.author),
        meta: if edited {
            format!("#{} · edited", item.ordinal)
        } else {
            format!("#{}", item.ordinal)
        },
        text: item.comment.text,
    }
}

fn page_author_name(author: &pages::AuthorRef) -> String {
    match author {
        pages::AuthorRef::User(key) => format!("user {}", short_hex(key)),
        pages::AuthorRef::Agent { agent_id, .. } => format!("@{agent_id}"),
        pages::AuthorRef::Module(module) => module.clone(),
        pages::AuthorRef::System => "system".into(),
    }
}

async fn load_pages_data(rpc: &RpcClient, requested: Option<&str>) -> Result<PagesData, String> {
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
            selected_block_id: String::new(),
            selected_block_kind: String::new(),
            selected_block_text: String::new(),
            selected_block_checked: false,
            page_title_selected: false,
        });
    }
    let wire_blocks = load_page_blocks(rpc, &active_page).await?;
    let active_page_title = wire_blocks
        .first()
        .map(|block| block.text.clone())
        .unwrap_or_default();
    let blocks = page_blocks(wire_blocks, &active_page);
    Ok(PagesData {
        pages,
        blocks,
        active_page,
        active_page_title,
        active_page_parent,
        selected_block_id: String::new(),
        selected_block_kind: String::new(),
        selected_block_text: String::new(),
        selected_block_checked: false,
        page_title_selected: false,
    })
}

fn page_blocks(wire_blocks: Vec<pages::Block>, active_page: &str) -> Vec<PageBlock> {
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
            mark_count: count_i64(block.marks.len()),
        })
        .collect()
}

fn page_block_key(id: &str) -> i64 {
    // ponytail: session-wide interning is collision-free; scope it per workspace
    // only if retaining every visited block id becomes measurable.
    static KEYS: OnceLock<Mutex<BTreeMap<String, i64>>> = OnceLock::new();
    let mut keys = KEYS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(key) = keys.get(id) {
        return *key;
    }
    let key = count_i64(keys.len());
    keys.insert(id.to_owned(), key);
    key
}

fn with_selected_block(mut pages: PagesData, selected_block_id: &str) -> PagesData {
    if !selected_block_id.is_empty() && selected_block_id == pages.active_page {
        pages.page_title_selected = true;
        return pages;
    }
    let Some(block) = pages
        .blocks
        .iter()
        .find(|block| block.id == selected_block_id)
    else {
        return pages;
    };
    pages.selected_block_id.clone_from(&block.id);
    pages.selected_block_kind.clone_from(&block.kind);
    pages.selected_block_text.clone_from(&block.text);
    pages.selected_block_checked = block.checked;
    pages
}

async fn load_page_blocks(rpc: &RpcClient, page_id: &str) -> Result<Vec<pages::Block>, String> {
    let mut blocks = Vec::new();
    let mut after = None;
    loop {
        let reply: PageReply = rpc
            .query(
                "pages",
                &PageQuery::GetPage {
                    page_id: page_id.to_string(),
                    after: after.clone(),
                    limit: 0,
                },
            )
            .await?;
        let page = match reply {
            PageReply::Page(Some(page)) => page,
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

async fn load_page_index(rpc: &RpcClient) -> Result<Vec<pages::PageMeta>, String> {
    let mut pages = Vec::new();
    let mut after = None;
    loop {
        let reply: PageReply = rpc
            .query(
                "pages",
                &PageQuery::ListPages {
                    after: after.clone(),
                    limit: 0,
                },
            )
            .await?;
        let page = match reply {
            PageReply::PageList(page) => page,
            _ => return Err("node returned an invalid page list".into()),
        };
        pages.extend(page.pages);
        let Some(next) = page.next_after else {
            return Ok(pages);
        };
        if after.as_ref() == Some(&next) {
            return Err("node repeated the page-list cursor".into());
        }
        after = Some(next);
    }
}

fn page_items(wire_pages: Vec<pages::PageMeta>) -> Vec<PageItem> {
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

async fn signed_write(
    rpc: &RpcClient,
    target: &str,
    payload: Vec<u8>,
    password: String,
) -> Result<(), String> {
    if payload.is_empty() || payload.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{target} transaction exceeds the signed payload limit"
        ));
    }
    let frame = sign_frame(target, &payload, password).await?;
    rpc.submit_frame(frame).await.map_err(Into::into)
}

async fn sign_frame(target: &str, payload: &[u8], mut password: String) -> Result<Vec<u8>, String> {
    let key = user_key_path()?;
    let encrypted = key_is_encrypted(&key)?;
    let payload_hex = hex_encode(payload);
    let input = signing_input(encrypted, &password, &payload_hex);
    password.zeroize();
    let input = Zeroizing::new(input?);
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("sign-frame")
        .arg("--key")
        .arg(&key)
        .arg("--target")
        .arg(target)
        .arg("--seq")
        .arg(next_sequence().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        format!("could not start the ducktape signer ({error}); build node-bin or set DUCKTAPE_BIN")
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ducktape signer stdin is unavailable".to_string())?;
    stdin
        .write_all(&input)
        .await
        .map_err(|error| format!("could not send payload to signer: {error}"))?;
    drop(stdin);
    let output = tokio::time::timeout(RPC_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "ducktape signer timed out".to_string())?
        .map_err(|error| format!("ducktape signer failed: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ducktape signer refused the transaction: {}",
            bounded_detail(&detail)
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| "ducktape signer returned non-UTF-8 output".to_string())?;
    let frame_hex = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "ducktape signer returned no frame".to_string())?;
    hex_decode(frame_hex.trim())
}

fn signing_input(encrypted: bool, password: &str, payload_hex: &str) -> Result<Vec<u8>, String> {
    let invalid_password = password.len() > 16 * 1024
        || password
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, 0 | b'\n' | b'\r'));
    if invalid_password {
        return Err("key password is too long or contains a line delimiter".into());
    }
    if encrypted && password.is_empty() {
        return Err("the local user key is locked; enter its password".into());
    }
    let mut input = Vec::with_capacity(password.len() + payload_hex.len() + 2);
    if encrypted {
        input.extend_from_slice(password.as_bytes());
        input.push(b'\n');
    }
    input.extend_from_slice(payload_hex.as_bytes());
    input.push(b'\n');
    Ok(input)
}

async fn local_user_key() -> Option<Vec<u8>> {
    // ponytail: cache the launch identity; restart the app after replacing user.key.
    static KEY: tokio::sync::OnceCell<Option<Vec<u8>>> = tokio::sync::OnceCell::const_new();
    KEY.get_or_init(read_local_user_key).await.clone()
}

async fn read_local_user_key() -> Option<Vec<u8>> {
    let key = user_key_path().ok()?;
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("key")
        .arg("status")
        .arg("--key")
        .arg(key)
        .kill_on_drop(true);
    let output = tokio::time::timeout(RPC_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() || output.stdout.len() > 256 {
        return None;
    }
    parse_user_key_status(std::str::from_utf8(&output.stdout).ok()?)
}

fn parse_user_key_status(status: &str) -> Option<Vec<u8>> {
    let mut fields = status.split_whitespace();
    let kind = fields.next()?;
    if !matches!(kind, "plaintext" | "encrypted") {
        return None;
    }
    let key = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    public_key(key, "local user key").ok()
}

fn user_key_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DUCKTAPE_USER_KEY") {
        return Ok(path.into());
    }
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(root).join("user.key"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape/user.key"))
        .ok_or_else(|| "cannot locate local user.key; set DUCKTAPE_USER_KEY".to_string())
}

fn key_is_encrypted(path: &std::path::Path) -> Result<bool, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    if metadata.len() > MAX_KEY_FILE_BYTES {
        return Err("local user key file is unexpectedly large".into());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let mut prefix = [0; ENCRYPTED_KEY_PREFIX.len()];
    let read = file
        .read(&mut prefix)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let encrypted = read == prefix.len() && prefix == ENCRYPTED_KEY_PREFIX.as_bytes();
    prefix.zeroize();
    Ok(encrypted)
}

fn ducktape_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("DUCKTAPE_BIN") {
        return path.into();
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(sibling) = current.parent().map(|parent| parent.join("ducktape"))
        && sibling.is_file()
    {
        return sibling;
    }
    PathBuf::from("ducktape")
}

fn bounded_text(value: String, field: &str, limit: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > limit || value.chars().any(|character| character == '\0') {
        return Err(format!("{field} must be between 1 and {limit} bytes"));
    }
    Ok(value.to_string())
}

fn bounded_exact_text(value: String, field: &str, limit: usize) -> Result<String, String> {
    let invalid = value.len() > limit || value.chars().any(|character| character == '\0');
    if invalid {
        return Err(format!(
            "{field} must be at most {limit} bytes and contain no NUL"
        ));
    }
    Ok(value)
}

fn required_id(value: String, subject: &str) -> Result<String, String> {
    bounded_text(value, &format!("{subject} id"), 512)
}

fn public_key(value: &str, field: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    let expected = chat::HUDDLE_NODE_KEY_BYTES * 2;
    if value.len() != expected {
        return Err(format!("{field} must be {expected} hexadecimal characters"));
    }
    hex_decode(value).map_err(|_| format!("{field} must be hexadecimal"))
}

fn positive_sequence(value: i64) -> Result<u64, String> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "message sequence must be positive".into())
}

fn app_error(message: String) -> AppError {
    message.into()
}

fn committed_error(message: String) -> AppError {
    AppError {
        message,
        committed: true,
    }
}

fn retry_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(4);
    Duration::from_secs(1_u64 << exponent)
}

fn number_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn live_update(kind: &str, status: &str, height: i64) -> LiveUpdate {
    LiveUpdate {
        kind: kind.into(),
        status: status.into(),
        height,
    }
}

fn live_retry(_message: String) -> LiveUpdate {
    LiveUpdate {
        kind: "retrying".into(),
        status: "Reconnecting…".into(),
        height: -1,
    }
}

fn message_body(blocks: &[chat::Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            chat::Block::Paragraph(spans) => span_text(spans),
            chat::Block::Code { lang, text } => match lang {
                Some(lang) => format!("{lang}\n{text}"),
                None => text.clone(),
            },
            chat::Block::Quote(spans) => format!("“{}”", span_text(spans)),
            chat::Block::Divider => "────────".into(),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn span_text(spans: &[chat::Span]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

fn author_name(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(key) => format!("user {}", short_hex(key)),
        AuthorRef::Agent { agent_id, .. } => format!("@{agent_id}"),
        AuthorRef::Module(module) => module.clone(),
        AuthorRef::System => "system".into(),
    }
}

fn short_hex(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in bytes.iter().take(4) {
        let _ = write!(output, "{byte:02x}");
    }
    if bytes.len() > 4 {
        output.push('…');
    }
    output
}

const fn block_kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Page => "Page",
        BlockKind::Paragraph => "Text",
        BlockKind::Heading1 => "Heading 1",
        BlockKind::Heading2 => "Heading 2",
        BlockKind::Heading3 => "Heading 3",
        BlockKind::Bulleted => "Bullet",
        BlockKind::Numbered => "Number",
        BlockKind::Todo => "Todo",
        BlockKind::Toggle => "Toggle",
        BlockKind::Quote => "Quote",
        BlockKind::Code => "Code",
        BlockKind::Callout => "Callout",
        BlockKind::Divider => "Divider",
    }
}

fn parse_block_kind(kind: &str) -> Result<BlockKind, String> {
    match kind {
        "Page" => Ok(BlockKind::Page),
        "Text" => Ok(BlockKind::Paragraph),
        "Heading 1" => Ok(BlockKind::Heading1),
        "Heading 2" => Ok(BlockKind::Heading2),
        "Heading 3" => Ok(BlockKind::Heading3),
        "Bullet" => Ok(BlockKind::Bulleted),
        "Number" => Ok(BlockKind::Numbered),
        "Todo" => Ok(BlockKind::Todo),
        "Toggle" => Ok(BlockKind::Toggle),
        "Quote" => Ok(BlockKind::Quote),
        "Code" => Ok(BlockKind::Code),
        "Callout" => Ok(BlockKind::Callout),
        "Divider" => Ok(BlockKind::Divider),
        _ => Err("choose a valid block type".into()),
    }
}

fn bounded_new_block_text(kind: BlockKind, text: String) -> Result<String, String> {
    if kind == BlockKind::Divider {
        return Ok(String::new());
    }
    let field = if kind == BlockKind::Page {
        "page title"
    } else {
        "block text"
    };
    let limit = if kind == BlockKind::Page {
        512
    } else {
        64 * 1024
    };
    if text.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    bounded_exact_text(text, field, limit)
}

fn bounded_updated_block_text(kind: BlockKind, text: String) -> Result<String, String> {
    if kind == BlockKind::Divider {
        return Ok(String::new());
    }
    if kind == BlockKind::Page {
        return bounded_exact_text(text, "page title", 512);
    }
    bounded_exact_text(text, "block text", 64 * 1024)
}

async fn debounced_page_text(
    rpc: String,
    mut password: String,
    block_id: String,
    text: String,
) -> Result<bool, String> {
    let key = format!("{rpc}\0{block_id}");
    let ticket = begin_autosave(&key);
    tokio::time::sleep(Duration::from_millis(400)).await;
    if !autosave_is_current(&key, ticket) {
        password.zeroize();
        return Ok(false);
    }
    // ponytail: one writer is enough before a live network; shard by RPC only if latency proves it.
    let _writer = autosave_writer().lock().await;
    if !autosave_is_current(&key, ticket) {
        password.zeroize();
        return Ok(false);
    }
    let result = async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::UpdateText {
                block_id,
                text: text.clone(),
                marks: None,
            }),
            password,
        )
        .await
    }
    .await;
    let current = finish_autosave(&key, ticket);
    if !current {
        return Ok(false);
    }
    result?;
    Ok(true)
}

fn autosaves() -> &'static std::sync::Mutex<BTreeMap<String, u64>> {
    static AUTOSAVES: OnceLock<std::sync::Mutex<BTreeMap<String, u64>>> = OnceLock::new();
    AUTOSAVES.get_or_init(Default::default)
}

fn autosave_writer() -> &'static tokio::sync::Mutex<()> {
    static WRITER: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    WRITER.get_or_init(Default::default)
}

pub fn cancel_autosaves(rpc: String, generation: i64) -> i64 {
    let prefix = format!("{}\0", rpc.trim());
    autosaves()
        .lock()
        .expect("autosave lock poisoned")
        .retain(|key, _| !key.starts_with(&prefix));
    generation.saturating_add(1)
}

fn begin_autosave(key: &str) -> u64 {
    static TICKET: AtomicU64 = AtomicU64::new(1);
    let ticket = TICKET.fetch_add(1, Ordering::Relaxed);
    autosaves()
        .lock()
        .expect("autosave lock poisoned")
        .insert(key.to_string(), ticket);
    ticket
}

fn autosave_is_current(key: &str, ticket: u64) -> bool {
    autosaves().lock().expect("autosave lock poisoned").get(key) == Some(&ticket)
}

fn finish_autosave(key: &str, ticket: u64) -> bool {
    let mut autosaves = autosaves().lock().expect("autosave lock poisoned");
    let is_current = autosaves.get(key) == Some(&ticket);
    if !is_current {
        return false;
    }
    autosaves.remove(key);
    true
}

fn block_move(
    blocks: &[pages::Block],
    block_id: &str,
    direction: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let block = blocks
        .iter()
        .find(|block| block.id == block_id)
        .ok_or_else(|| "block was not found".to_string())?;
    let parent_id = block
        .parent
        .as_deref()
        .ok_or_else(|| "top-level pages cannot move inside their own document".to_string())?;
    let parent = blocks
        .iter()
        .find(|block| block.id == parent_id)
        .ok_or_else(|| "block parent was not found".to_string())?;
    let index = parent
        .children
        .iter()
        .position(|child| child == block_id)
        .ok_or_else(|| "block is missing from its parent".to_string())?;
    match direction {
        "up" if index > 0 => Ok((
            Some(parent.id.clone()),
            index
                .checked_sub(2)
                .map(|index| parent.children[index].clone()),
        )),
        "down" if index + 1 < parent.children.len() => Ok((
            Some(parent.id.clone()),
            Some(parent.children[index + 1].clone()),
        )),
        "indent" if index > 0 => {
            let new_parent = blocks
                .iter()
                .find(|block| block.id == parent.children[index - 1])
                .ok_or_else(|| "previous block was not found".to_string())?;
            Ok((
                Some(new_parent.id.clone()),
                new_parent.children.last().cloned(),
            ))
        }
        "outdent" => {
            let promotes_page = block.kind == BlockKind::Page && parent.parent.is_none();
            if promotes_page {
                return Ok((None, None));
            }
            let grandparent = parent
                .parent
                .clone()
                .ok_or_else(|| "block is already at the top level".to_string())?;
            Ok((Some(grandparent), Some(parent.id.clone())))
        }
        "up" => Err("block is already first".into()),
        "down" => Err("block is already last".into()),
        "indent" => Err("block needs a previous sibling to indent under".into()),
        _ => Err("choose a valid block move".into()),
    }
}

fn bounded_detail(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "no detail".into();
    }
    value.chars().take(300).collect()
}

fn next_sequence() -> u64 {
    static SEQUENCE: OnceLock<AtomicU64> = OnceLock::new();
    SEQUENCE
        .get_or_init(|| AtomicU64::new(epoch_nanos() as u64))
        .fetch_add(1, Ordering::Relaxed)
}

fn fresh_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", epoch_nanos(), next_sequence())
}

fn epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    let valid = !value.is_empty()
        && value.len() <= MAX_FRAME_HEX_BYTES
        && value.len().is_multiple_of(2)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err("ducktape signer returned an invalid frame".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("validated ASCII hex");
            u8::from_str_radix(pair, 16).map_err(|_| "ducktape signer returned invalid hex".into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use commonware_cryptography::{Signer as _, ed25519};
    use iced::futures::StreamExt as _;

    use super::*;

    #[test]
    fn signer_stdin_only_includes_a_password_for_encrypted_keys() {
        assert_eq!(signing_input(false, "ignored", "00").unwrap(), b"00\n");
        assert_eq!(
            signing_input(true, "secret", "00").unwrap(),
            b"secret\n00\n"
        );
        assert!(signing_input(true, "", "00").is_err());
        assert!(signing_input(true, "bad\nsecret", "00").is_err());

        let directory = tempfile::tempdir().unwrap();
        let key = directory.path().join("user.key");
        std::fs::write(&key, format!("{ENCRYPTED_KEY_PREFIX}ciphertext")).unwrap();
        assert!(key_is_encrypted(&key).unwrap());
        std::fs::write(&key, "plaintext-key").unwrap();
        assert!(!key_is_encrypted(&key).unwrap());

        let public_key = "ab".repeat(32);
        assert_eq!(
            parse_user_key_status(&format!("encrypted {public_key}\n")),
            Some(vec![0xab; 32])
        );
        assert!(parse_user_key_status("absent\n").is_none());
        assert!(parse_user_key_status("plaintext not-hex\n").is_none());
        let reactors = BTreeSet::from([AuthorRef::User(vec![0xab; 32]), AuthorRef::System]);
        assert!(reacted_by_user(&reactors, Some(&[0xab; 32])));
        assert!(!reacted_by_user(&reactors, Some(&[0xcd; 32])));
        assert!(!reacted_by_user(&reactors, None));
    }

    #[test]
    fn post_commit_hydration_errors_are_not_retryable() {
        let error = committed_error("read failed".into());
        assert!(error.committed);
        assert_eq!(error.message, "read failed");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chat_and_pages_round_trip_over_signed_frames() {
        let storage = tempfile::tempdir().unwrap();
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().unwrap(),
            simnode::SimOpts {
                auto: true,
                ..Default::default()
            },
        )
        .unwrap();
        let origin = format!("http://{}", sim.addr());
        let rpc = RpcClient::new(&origin).unwrap();
        let signer = ed25519::PrivateKey::from_seed(7);

        submit_test(
            &rpc,
            &signer,
            1,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            2,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "hello-1".into(),
                blocks: vec![chat::Block::paragraph("hello from the app")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            3,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: "welcome".into(),
                title: "Welcome".into(),
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            4,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: None,
                block: NewBlock {
                    id: "intro".into(),
                    kind: BlockKind::Paragraph,
                    text: "A signed page block".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;

        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.channels[0].name, "General");
        assert_eq!(chat.messages[0].body, "hello from the app");
        let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
        assert_eq!(pages.active_page_title, "Welcome");
        assert_eq!(pages.blocks[0].text, "A signed page block");

        let origin = rpc.origin().to_string();
        let selected_page = load_page(origin.clone(), "welcome".into(), "intro".into())
            .await
            .unwrap();
        assert_eq!(selected_page.selected_block_id, "intro");
        assert_eq!(selected_page.selected_block_text, "A signed page block");
        let selected_title = load_page(origin.clone(), "welcome".into(), "welcome".into())
            .await
            .unwrap();
        assert!(selected_title.page_title_selected);
        assert!(selected_title.selected_block_id.is_empty());
        let workspace = connect(origin.clone()).await.unwrap();
        let mut live = live_events(origin.clone());
        let ready = live.next().await.unwrap();
        assert_eq!(ready.kind, "ready");
        submit_test(
            &rpc,
            &signer,
            5,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "hello-2".into(),
                blocks: vec![chat::Block::paragraph("arrived on the next block")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        let changed = live.next().await.unwrap();
        assert_eq!(changed.kind, "changed");
        assert!(changed.height > workspace.height);
        submit_test(
            &rpc,
            &signer,
            6,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "reply-1".into(),
                blocks: vec![chat::Block::paragraph("a threaded reply")],
                thread: Some(1),
                as_agent: None,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            7,
            "chat",
            chat::encode_msg(&ChatMsg::EditMessage {
                channel_id: "general".into(),
                seq: 1,
                blocks: vec![chat::Block::paragraph("hello, edited")],
                base_rev: Some(0),
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            8,
            "chat",
            chat::encode_msg(&ChatMsg::AddReaction {
                channel_id: "general".into(),
                seq: 1,
                emoji: "👍".into(),
            }),
        )
        .await;

        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.active_channel_name, "General");
        assert_eq!(chat.messages[0].body, "hello, edited");
        assert!(chat.messages[0].edited);
        assert_eq!(chat.messages[0].reply_count, 1);
        assert_eq!(chat.messages[0].reactions[0].emoji, "👍");
        let thread = load_thread_data(&rpc, "general", 1, 0).await.unwrap();
        assert_eq!(thread.messages.len(), 2);
        assert_eq!(thread.messages[1].body, "a threaded reply");
        let hit = load_chat_hit(origin.clone(), "general".into(), 1, 3)
            .await
            .unwrap();
        assert_eq!(hit.selected_message_seq, 1);
        assert_eq!(hit.active_thread_seq, 1);
        assert_eq!(hit.thread_target_seq, 3);
        assert_eq!(hit.thread_messages[1].body, "a threaded reply");
        submit_test(
            &rpc,
            &signer,
            9,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: Some("intro".into()),
                block: NewBlock {
                    id: "heading".into(),
                    kind: BlockKind::Heading2,
                    text: "Nested work".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            10,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "heading".into(),
                after: None,
                block: NewBlock {
                    id: "todo".into(),
                    kind: BlockKind::Todo,
                    text: "Ship the editor".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            11,
            "pages",
            pages::encode_msg(&PageMsg::SetChecked {
                block_id: "todo".into(),
                checked: true,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            12,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: Some("heading".into()),
                block: NewBlock {
                    id: "child".into(),
                    kind: BlockKind::Page,
                    text: "Child page".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;

        let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
        assert_eq!(pages.pages[0].id, "welcome");
        assert_eq!(pages.pages[1].id, "child");
        assert_eq!(pages.pages[1].prefix, "  ");
        assert_eq!(pages.blocks[2].id, "todo");
        assert_eq!(pages.blocks[2].prefix, "  ");
        assert!(pages.blocks[2].checked);

        submit_test(
            &rpc,
            &signer,
            13,
            "pages",
            pages::encode_msg(&PageMsg::AddComment {
                thread_id: "thread-live".into(),
                comment_id: "comment-live".into(),
                target: "intro".into(),
                text: "temporary".into(),
                anchor: None,
                mentions: Vec::new(),
                as_agent: None,
            }),
        )
        .await;
        let comments =
            refresh_block_comments(origin.clone(), "intro".into(), "thread-live".into(), 1)
                .await
                .unwrap();
        assert_eq!(comments.thread_id, "thread-live");
        submit_test(
            &rpc,
            &signer,
            14,
            "pages",
            pages::encode_msg(&PageMsg::DeleteComment {
                comment_id: "comment-live".into(),
            }),
        )
        .await;
        let comments =
            refresh_block_comments(origin.clone(), "intro".into(), "thread-live".into(), 2)
                .await
                .unwrap();
        assert!(comments.thread_id.is_empty());
        assert!(comments.comments.is_empty());

        let refreshed = refresh(origin, "general".into(), "welcome".into(), 7)
            .await
            .unwrap();
        assert_eq!(refreshed.generation, 7);
        assert_eq!(refreshed.messages[1].body, "arrived on the next block");
        assert_eq!(refreshed.active_page, "welcome");
        sim.shutdown();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn timeline_pages_past_thread_only_traffic() {
        let storage = tempfile::tempdir().unwrap();
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().unwrap(),
            simnode::SimOpts {
                auto: true,
                ..Default::default()
            },
        )
        .unwrap();
        let origin = format!("http://{}", sim.addr());
        let rpc = RpcClient::new(&origin).unwrap();
        let signer = ed25519::PrivateKey::from_seed(8);

        submit_test(
            &rpc,
            &signer,
            1,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            2,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "root".into(),
                blocks: vec![chat::Block::paragraph("root stays visible")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        for index in 0_u64..257 {
            submit_test(
                &rpc,
                &signer,
                index + 3,
                "chat",
                chat::encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: format!("reply-{index}"),
                    blocks: vec![chat::Block::paragraph(format!("reply {index}"))],
                    thread: Some(1),
                    as_agent: None,
                }),
            )
            .await;
        }

        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].body, "root stays visible");
        let first = load_thread_data(&rpc, "general", 1, 0).await.unwrap();
        assert_eq!(first.messages.len(), 257);
        assert_eq!(first.next_reply_offset, 256);
        assert!(first.has_more);
        let last = load_thread_page(origin.clone(), "general".into(), 1, 256, 9)
            .await
            .unwrap();
        assert_eq!(last.messages.len(), 1);
        assert_eq!(last.messages[0].body, "reply 256");
        assert_eq!(last.next_reply_offset, 257);
        assert!(!last.has_more);
        let sparse = load_thread(origin.clone(), "general".into(), 1, 258, -1, false, 10)
            .await
            .unwrap();
        assert_eq!(sparse.target_seq, 258);
        assert_eq!(sparse.next_reply_offset, -1);
        assert_eq!(sparse.messages.len(), 2);
        assert_eq!(sparse.messages[1].body, "reply 256");
        let committed = load_thread(origin, "general".into(), 1, 258, -1, true, 11)
            .await
            .unwrap();
        assert_eq!(committed.target_seq, 258);
        assert_eq!(committed.messages.len(), 258);
        assert_eq!(committed.messages[257].body, "reply 256");
        assert!(!committed.has_more);
        sim.shutdown();
    }

    #[test]
    fn hydration_retry_is_capped() {
        assert_eq!(retry_delay(1), Duration::from_secs(1));
        assert_eq!(retry_delay(3), Duration::from_secs(4));
        assert_eq!(retry_delay(99), Duration::from_secs(16));
    }

    #[test]
    fn page_block_keys_survive_refresh_reordering() {
        let root = pages::Block {
            id: "page".into(),
            parent: None,
            page: "page".into(),
            kind: BlockKind::Page,
            text: "Page".into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        };
        let block = |id: &str| pages::Block {
            id: id.into(),
            parent: Some("page".into()),
            page: "page".into(),
            kind: BlockKind::Paragraph,
            text: id.into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        };
        let before = page_blocks(
            vec![root.clone(), block("editing"), block("trailing")],
            "page",
        );
        let editing_key = before
            .iter()
            .find(|block| block.id == "editing")
            .unwrap()
            .key;
        let nested = pages::Block {
            id: "nested".into(),
            parent: Some("inserted".into()),
            page: "page".into(),
            kind: BlockKind::Paragraph,
            text: "nested".into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        };
        let after = page_blocks(
            vec![
                root,
                block("inserted"),
                nested,
                block("trailing"),
                block("editing"),
            ],
            "page",
        );

        assert_eq!(
            after
                .iter()
                .find(|block| block.id == "editing")
                .unwrap()
                .key,
            editing_key
        );
        assert_eq!(
            after
                .iter()
                .map(|block| block.key)
                .collect::<BTreeSet<_>>()
                .len(),
            after.len()
        );

        let optimistic =
            optimistic_block(after, "inserted".into(), "Text".into(), "pending".into());
        let pending = &optimistic[2];
        assert!(pending.id.starts_with("pending-block-"));
        assert_eq!(pending.key, page_block_key(&pending.id));
        assert_eq!(pending.parent, "page");
        assert_eq!(optimistic[1].id, "nested");
        assert_eq!(optimistic[3].id, "trailing");
        assert_eq!(
            optimistic
                .iter()
                .map(|block| block.key)
                .collect::<BTreeSet<_>>()
                .len(),
            optimistic.len()
        );
    }

    #[test]
    fn block_action_menu_stays_inside_the_page_viewport() {
        assert_eq!(block_action_menu_y(100.0, 500.0), 96.0);
        assert_eq!(block_action_menu_y(450.0, 500.0), 260.0);
        assert_eq!(block_action_menu_y(2.0, 500.0), 0.0);
    }

    #[test]
    fn refresh_merges_only_clean_block_drafts() {
        let blocks = vec![PageBlock {
            key: 0,
            id: "block".into(),
            parent: "page".into(),
            kind: "Text".into(),
            text: "remote".into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
        }];

        assert_eq!(
            refreshed_block_draft(
                blocks.clone(),
                "block".into(),
                "local".into(),
                "saving".into(),
            ),
            "local"
        );
        assert_eq!(
            refreshed_block_draft(blocks, "block".into(), "local".into(), "saved".into()),
            "remote"
        );
    }

    #[test]
    fn recovered_drafts_are_deduplicated_and_endpoint_scoped() {
        let drafts = remember_orphaned_block_drafts(
            vec!["local".into()],
            Vec::new(),
            "missing".into(),
            "local".into(),
            "error".into(),
        );
        assert_eq!(drafts, ["local"]);
        assert_eq!(
            retain_drafts_for_endpoint(drafts.clone(), "http://node".into(), "http://node".into(),),
            drafts
        );
        assert!(
            retain_drafts_for_endpoint(drafts, "http://node".into(), "http://other".into(),)
                .is_empty()
        );
    }

    #[test]
    fn page_updates_preserve_exact_text() {
        assert_eq!(
            bounded_updated_block_text(BlockKind::Code, "  code\n".into()).unwrap(),
            "  code\n"
        );
        assert_eq!(
            bounded_updated_block_text(BlockKind::Paragraph, String::new()).unwrap(),
            ""
        );
        assert_eq!(
            bounded_exact_text(String::new(), "page title", 512).unwrap(),
            ""
        );
    }

    #[test]
    fn autosave_keeps_only_the_latest_ticket() {
        let key = "autosave-test";
        let first = begin_autosave(key);
        let latest = begin_autosave(key);
        assert!(!autosave_is_current(key, first));
        assert!(autosave_is_current(key, latest));
        assert!(!finish_autosave(key, first));
        assert!(finish_autosave(key, latest));
        assert!(!autosave_is_current(key, latest));
    }

    #[test]
    fn reconnect_cancels_only_the_previous_endpoint_autosaves() {
        let old_key = "http://old\0page";
        let other_key = "http://other\0page";
        let old_ticket = begin_autosave(old_key);
        let other_ticket = begin_autosave(other_key);

        assert_eq!(cancel_autosaves("http://old".into(), 4), 5);
        assert!(!autosave_is_current(old_key, old_ticket));
        assert!(finish_autosave(other_key, other_ticket));
    }

    #[test]
    fn missing_block_cancels_only_its_own_autosave() {
        let title_key = "http://missing-block-test\0page";
        let block_key = "http://missing-block-test\0block";
        let title_ticket = begin_autosave(title_key);
        let block_ticket = begin_autosave(block_key);

        assert_eq!(
            cancel_missing_block_autosave(
                "http://missing-block-test".into(),
                4,
                Vec::new(),
                "block".into(),
            ),
            5
        );
        assert!(!autosave_is_current(block_key, block_ticket));
        assert!(finish_autosave(title_key, title_ticket));
    }

    #[test]
    fn block_moves_follow_visible_sibling_order() {
        let block = |id: &str, parent: Option<&str>, kind, children: &[&str]| pages::Block {
            id: id.into(),
            parent: parent.map(str::to_string),
            page: "page".into(),
            kind,
            text: id.into(),
            marks: Vec::new(),
            checked: false,
            children: children.iter().map(|child| (*child).into()).collect(),
        };
        let blocks = vec![
            block("page", None, BlockKind::Page, &["a", "b"]),
            block("a", Some("page"), BlockKind::Paragraph, &["c"]),
            block("c", Some("a"), BlockKind::Paragraph, &[]),
            block("b", Some("page"), BlockKind::Paragraph, &[]),
        ];

        assert_eq!(
            block_move(&blocks, "b", "up").unwrap(),
            (Some("page".into()), None)
        );
        assert_eq!(
            block_move(&blocks, "a", "down").unwrap(),
            (Some("page".into()), Some("b".into()))
        );
        assert_eq!(
            block_move(&blocks, "b", "indent").unwrap(),
            (Some("a".into()), Some("c".into()))
        );
        assert_eq!(
            block_move(&blocks, "c", "outdent").unwrap(),
            (Some("page".into()), Some("a".into()))
        );

        let page = block("child-page", Some("page"), BlockKind::Page, &[]);
        let parent = block("page", None, BlockKind::Page, &["child-page"]);
        assert_eq!(
            block_move(&[parent, page], "child-page", "outdent").unwrap(),
            (None, None)
        );
    }

    async fn submit_test(
        rpc: &RpcClient,
        signer: &ed25519::PrivateKey,
        sequence: u64,
        target: &str,
        payload: Vec<u8>,
    ) {
        let frame = node::encode_frame(
            signer,
            sequence,
            &sdk::Msg {
                target: target.into(),
                payload,
            },
            None,
        );
        rpc.submit_frame(frame).await.unwrap();
    }
}
