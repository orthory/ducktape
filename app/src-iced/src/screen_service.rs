//! Wire adapters for the native user screens.
//!
//! `screens::user` owns presentation and emits typed commands. This module is
//! the single place that translates those commands to the existing node API.

mod chat;
mod files;
mod home;
mod pages;

use self::{chat::*, files::*, home::*, pages::*};

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use reqwest::Url;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt as _;
use unicode_normalization::UnicodeNormalization as _;

use crate::account_service;
use crate::backend::{
    Backend, CachedDeviceRow, CachedNetworkDevices, DeviceStanding, IdentityStatus, Workspace,
};
use crate::profile_service;
use crate::screens::user::{
    AccountKeyKind, AccountProfile, BlockKind, Channel, ChannelContent, ChatData, ChatHit,
    ChatLink, ChatMessage, ChatSpan, ChatTag, ChatThread, Command, Custody, CustodyStatus,
    DeviceNetworkGroup, DeviceRow, FileDiff as ScreenFileDiff, FileEntry as ScreenFileEntry,
    FileKind, FileListing, FilePreview, FilePreviewContent, FileSnapshot as ScreenFileSnapshot,
    HomeData, HuddleMember, InlineMark, MemberKeyRow, PageBlock, PageComment, PageCommentThread,
    PageDocument, PageMeta, PagesData, PostPolicy, Reaction, RelativeAnchor, Screen, ServiceEvent,
    SpanMark, Standing, ThreadMove, WorkspaceRow,
};
use crate::transport::NodeClient;
use crate::user_content_service;

const MAX_FILE_NAME_BYTES: usize = 255;

pub(crate) async fn execute_drop(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
    source: PathBuf,
) -> ServiceEvent {
    action(
        Screen::Files,
        upload_dropped(backend, client, target, source).await,
    )
}

pub async fn execute(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    client: Option<NodeClient>,
    command: Command,
) -> ServiceEvent {
    if matches!(
        &command,
        Command::LinkDevice
            | Command::PollLink
            | Command::ApproveLink { .. }
            | Command::CancelLink
            | Command::ResolveLinkChallenge { .. }
            | Command::GenerateLinkResponse { .. }
            | Command::StartPhoneEnrollment
            | Command::PollPhoneEnrollment
            | Command::ApprovePhoneEnrollment { .. }
            | Command::CancelPhoneEnrollment
            | Command::RemoveMember(_)
            | Command::UnbindNode(_)
            | Command::SetNodeLabel { .. }
            | Command::EnrollTouchId(_)
            | Command::DisableTouchId
    ) {
        return account_service::execute(backend, workspace, client, command).await;
    }
    if matches!(
        &command,
        Command::SaveDisplayName(_)
            | Command::SetDuckName(_)
            | Command::ChooseAvatar
            | Command::SaveProfile { .. }
    ) {
        return match command {
            Command::SaveDisplayName(display_name) => ServiceEvent::HomeProfileFinished(
                profile_service::save_display_name(backend, workspace, client, display_name).await,
            ),
            Command::SetDuckName(handle) => ServiceEvent::HomeProfileFinished(
                profile_service::set_duck_name(workspace, client, handle).await,
            ),
            Command::ChooseAvatar => {
                ServiceEvent::AvatarChosen(profile_service::choose_avatar().await)
            }
            Command::SaveProfile { bio, avatar } => ServiceEvent::HomeProfileFinished(
                profile_service::save_profile(backend, workspace, client, bio, avatar).await,
            ),
            _ => unreachable!("profile commands route before screen match"),
        };
    }
    match command {
        Command::LoadHome => ServiceEvent::HomeLoaded(load_home(backend, workspace, client).await),
        Command::LoadChat { active } => {
            ServiceEvent::ChatLoaded(
                load_chat(backend.as_ref(), client.as_ref(), active.as_deref()).await,
            )
        }
        Command::LoadChannel(channel) => {
            ServiceEvent::ChannelLoaded(load_channel(backend.as_ref(), client.as_ref(), &channel).await)
        }
        Command::LoadPages { active, open_tabs } => {
            ServiceEvent::PagesLoaded(
                load_pages(backend.as_ref(), client, active.as_deref(), open_tabs).await,
            )
        }
        Command::LoadPage(page) => {
            ServiceEvent::PageLoaded(load_page_with_ancestry(backend.as_ref(), client, &page).await)
        }
        Command::CreateChannel { name, policy } => {
            action(Screen::Chat, create_channel(backend.as_ref(), client.as_ref(), name, policy).await)
        }
        Command::SendMessage { channel, body, thread } => {
            action(Screen::Chat, send_message(backend.as_ref(), client.as_ref(), channel, body, thread).await)
        }
        Command::EditMessage {
            channel,
            sequence,
            base_revision,
            body,
        } => action(
            Screen::Chat,
            edit_message(
                backend.as_ref(),
                client.as_ref(),
                channel,
                sequence,
                base_revision,
                body,
            )
            .await,
        ),
        Command::DeleteMessage { channel, sequence } => action(
            Screen::Chat,
            chat_write(
                backend.as_ref(),
                client.as_ref(),
                json!({ "delete_message": { "channel_id": channel, "seq": sequence } }),
            )
            .await,
        ),
        Command::ChooseChatAttachment => ServiceEvent::ChatAttachmentUploaded(
            choose_chat_attachment(backend, client).await,
        ),
        Command::DownloadChatAttachment(path) => action(
            Screen::Chat,
            download_chat_attachment(client.as_ref(), &path).await,
        ),
        Command::RenameChannel { channel, name } => action(
            Screen::Chat,
            chat_write(backend.as_ref(), client.as_ref(), json!({ "rename_channel": { "channel_id": channel, "name": name } })).await,
        ),
        Command::SetChannelArchived { channel, archived } => action(
            Screen::Chat,
            chat_write(backend.as_ref(), client.as_ref(), json!({ "set_channel_archived": { "channel_id": channel, "archived": archived } })).await,
        ),
        Command::LoadThread { channel, root } => {
            ServiceEvent::ThreadLoaded(load_thread(client.as_ref(), &channel, root, backend.as_ref()).await)
        }
        Command::SetReaction { channel, sequence, emoji, remove } => action(
            Screen::Chat,
            set_reaction(backend.as_ref(), client.as_ref(), channel, sequence, emoji, remove).await,
        ),
        Command::SetChannelMembership { channel, key, member } => action(
            Screen::Chat,
            set_membership(backend.as_ref(), client.as_ref(), channel, key, member).await,
        ),
        Command::LoadTags(channel) => {
            ServiceEvent::ChatTagsLoaded(load_chat_tags(client.as_ref(), &channel).await)
        }
        Command::FilterTag { channel, tag } => {
            ServiceEvent::ChatHitsLoaded(filter_chat_tag(client.as_ref(), &channel, &tag).await)
        }
        Command::LoadMessageWindow { channel, sequence } => ServiceEvent::MessageWindowLoaded {
            sequence,
            result: load_message_window(client.as_ref(), &channel, sequence, backend.as_ref()).await,
        },
        Command::SetHuddle { channel, joined } => action(
            Screen::Chat,
            set_huddle(backend.as_ref(), client.as_ref(), channel, joined).await,
        ),
        Command::CreatePage { id, parent } => action(
            Screen::Pages,
            create_page(backend.as_ref(), client.as_ref(), id, parent).await,
        ),
        Command::RenamePage { page, title } => action(
            Screen::Pages,
            pages_write(backend.as_ref(), client.as_ref(), json!({ "update_text": { "block_id": page, "text": title } })).await,
        ),
        Command::SaveBlock { block, .. } => action(
            Screen::Pages,
            pages_write(backend.as_ref(), client.as_ref(), json!({
                "update_text": {
                    "block_id": block.id,
                    "text": block.text,
                    "marks": marks_wire(&block.marks)
                }
            })).await,
        ),
        Command::SetBlockKind { block, kind } => action(
            Screen::Pages,
            pages_write(backend.as_ref(), client.as_ref(), json!({ "set_kind": { "block_id": block, "kind": block_kind_wire(kind) } })).await,
        ),
        Command::ApplySlash { block, kind, text } => action(
            Screen::Pages,
            apply_slash(backend.as_ref(), client.as_ref(), block, kind, text).await,
        ),
        Command::SetBlockChecked { block, checked } => action(
            Screen::Pages,
            pages_write(backend.as_ref(), client.as_ref(), json!({ "set_checked": { "block_id": block, "checked": checked } })).await,
        ),
        Command::RemoveBlock(block) => action(
            Screen::Pages,
            pages_write(backend.as_ref(), client.as_ref(), json!({ "remove_block": { "block_id": block } })).await,
        ),
        Command::DeletePage(page) => action(
            Screen::Pages,
            pages_write(backend.as_ref(), client.as_ref(), json!({ "delete_page": { "page_id": page } })).await,
        ),
        Command::SetPageParent { page, parent } => action(
            Screen::Pages,
            pages_write(
                backend.as_ref(),
                client.as_ref(),
                json!({ "set_page_parent": { "page_id": page, "parent": parent } }),
            )
            .await,
        ),
        Command::SetSpanMark {
            block,
            start,
            end,
            kind,
            active,
        } => action(
            Screen::Pages,
            pages_write(
                backend.as_ref(),
                client.as_ref(),
                json!({
                    "set_span_mark": {
                        "block_id": block,
                        "start": start,
                        "end": end,
                        "kind": inline_mark_wire(kind),
                        "active": active
                    }
                }),
            )
            .await,
        ),
        Command::MoveBlock { block, parent, after } => action(
            Screen::Pages,
            pages_write(
                backend.as_ref(),
                client.as_ref(),
                json!({ "move_block": { "block_id": block, "parent": parent, "after": after } }),
            )
            .await,
        ),
        Command::PasteBlocks {
            parent,
            after,
            blocks,
        } => action(
            Screen::Pages,
            paste_page_blocks(
                backend.as_ref(),
                client.as_ref(),
                parent,
                after,
                blocks,
            )
            .await,
        ),
        Command::AddPageComment {
            thread,
            comment,
            target,
            anchor,
            text,
        } => {
            let mentions = comment_mentions(&text);
            action(
                Screen::Pages,
                pages_write(
                    backend.as_ref(),
                    client.as_ref(),
                    json!({
                        "add_comment": {
                            "thread_id": thread,
                            "comment_id": comment,
                            "target": target,
                            "text": text,
                            "anchor": anchor.map(anchor_wire),
                            "mentions": mentions
                        }
                    }),
                )
                .await,
            )
        }
        Command::ResolvePageComment { thread, resolved } => action(
            Screen::Pages,
            pages_write(
                backend.as_ref(),
                client.as_ref(),
                json!({ "resolve_thread": { "thread_id": thread, "resolved": resolved } }),
            )
            .await,
        ),
        Command::DeletePageComment(comment) => action(
            Screen::Pages,
            pages_write(
                backend.as_ref(),
                client.as_ref(),
                json!({ "delete_comment": { "comment_id": comment } }),
            )
            .await,
        ),
        Command::EditPageComment { comment, text } => {
            let mentions = comment_mentions(&text);
            action(
                Screen::Pages,
                pages_write(
                    backend.as_ref(),
                    client.as_ref(),
                    json!({
                        "edit_comment": {
                            "comment_id": comment,
                            "text": text,
                            "mentions": mentions
                        }
                    }),
                )
                .await,
            )
        }
        Command::ReadPageClipboard(_)
        | Command::FocusPageBlock(_)
        | Command::CommitPageAfter { .. } => action(
            Screen::Pages,
            Err("page desktop actions are handled by the desktop shell".into()),
        ),
        Command::AddBlock { page, kind } => action(
            Screen::Pages,
            pages_write(
                backend.as_ref(),
                client.as_ref(),
                json!({
                    "insert_block": {
                        "parent": page,
                        "after": null,
                        "block": { "id": fresh_id("block"), "kind": block_kind_wire(kind), "text": "" }
                    }
                }),
            ).await,
        ),
        Command::SplitPageBlock { page: _, left, right, thread_moves } => action(
            Screen::Pages,
            split_page_block(backend.as_ref(), client.as_ref(), left, right, thread_moves).await,
        ),
        Command::MergePageBlock { page: _, destination, source, thread_moves } => action(
            Screen::Pages,
            merge_page_block(
                backend.as_ref(),
                client.as_ref(),
                destination,
                source,
                thread_moves,
            )
            .await,
        ),
        Command::LockAccount => action(
            Screen::Home,
            match backend {
                Some(backend) => backend.lock_identity().await,
                None => Err("desktop backend is unavailable".into()),
            },
        ),
        Command::LoadFiles { path } => {
            ServiceEvent::FilesLoaded(load_files(client, path, None).await)
        }
        Command::LoadSnapshot { id, path } => {
            ServiceEvent::FilesLoaded(load_files(client, path, id).await)
        }
        Command::LoadFile { path, snapshot } => {
            ServiceEvent::FileLoaded(load_file(client, path, snapshot).await)
        }
        Command::CreateFolder { parent, name } => action(
            Screen::Files,
            create_folder(backend, client, parent, name).await,
        ),
        Command::ChooseFiles { target } => {
            action(Screen::Files, choose_files(backend, client, target).await)
        }
        Command::ChooseFolder { target } => {
            action(Screen::Files, choose_folder(backend, client, target).await)
        }
        Command::UploadDropped { .. } => action(
            Screen::Files,
            Err("native drop token was not resolved by the desktop host".into()),
        ),
        Command::DownloadFile { path, size, snapshot } => action(
            Screen::Files,
            choose_download(client.as_ref(), &path, size, snapshot.as_deref()).await,
        ),
        Command::DeleteFile(path) => action(
            Screen::Files,
            user_content_service::delete_file(backend.as_ref(), client.as_ref(), &path).await,
        ),
        Command::LoadFileDiff { from, to, prefix } => {
            ServiceEvent::FileDiffLoaded(load_file_diff(client.as_ref(), &from, &to, &prefix).await)
        }
        Command::SwitchWorkspace(_)
        | Command::AddNetwork
        | Command::UnlockAccount
        | Command::SecureAccount
        | Command::RevealRecovery
        | Command::CopyText(_) => action(
            Screen::Home,
            Err("this account action is handled by the desktop shell".into()),
        ),
        Command::LinkDevice
        | Command::PollLink
        | Command::ApproveLink { .. }
        | Command::CancelLink
        | Command::ResolveLinkChallenge { .. }
        | Command::GenerateLinkResponse { .. }
        | Command::StartPhoneEnrollment
        | Command::PollPhoneEnrollment
        | Command::ApprovePhoneEnrollment { .. }
        | Command::CancelPhoneEnrollment
        | Command::RemoveMember(_)
        | Command::UnbindNode(_)
        | Command::SetNodeLabel { .. }
        | Command::EnrollTouchId(_)
        | Command::DisableTouchId => unreachable!("account commands route before screen match"),
        Command::SaveDisplayName(_)
        | Command::SetDuckName(_)
        | Command::ChooseAvatar
        | Command::SaveProfile { .. } => unreachable!("profile commands route before screen match"),
    }
}

fn decode_ed25519_key(value: &str, field: &str) -> Result<Vec<u8>, String> {
    let bytes = decode_hex(value)?;
    if bytes.len() != 32 {
        return Err(format!("invalid {field}"));
    }
    Ok(bytes)
}

fn wire_bytes_hex(bytes: &[Value]) -> Option<String> {
    if bytes.is_empty() || bytes.len() > 65 {
        return None;
    }
    bytes
        .iter()
        .map(|value| value.as_u64().filter(|byte| *byte <= u8::MAX as u64))
        .collect::<Option<Vec<_>>>()
        .map(|bytes| {
            bytes
                .into_iter()
                .map(|byte| format!("{byte:02x}"))
                .collect()
        })
}

fn action(screen: Screen, result: Result<(), String>) -> ServiceEvent {
    ServiceEvent::ActionFinished { screen, result }
}

fn variant_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {key} reply"))
}

fn author_name(value: &Value) -> String {
    if value.as_str() == Some("system") {
        return "system".into();
    }
    if let Some(bytes) = value.get("user").and_then(Value::as_array) {
        let bytes = bytes
            .iter()
            .filter_map(Value::as_u64)
            .map(|value| value as u8)
            .collect::<Vec<_>>();
        if let Ok(name) = std::str::from_utf8(&bytes)
            && name.chars().all(|character| !character.is_control())
        {
            return name.to_string();
        }
        return bytes
            .iter()
            .take(4)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            + "…";
    }
    if let Some(agent) = value.get("agent") {
        let module = agent
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or("agent");
        let id = agent
            .get("agent_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return format!("{module}/{id}");
    }
    value
        .get("module")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) use crate::view_api::fresh_id;

fn clock_time(timestamp: u64) -> String {
    let seconds = timestamp % 86_400;
    format!("{:02}:{:02}", seconds / 3_600, seconds / 60 % 60)
}

/// A UTC calendar-date label ("Jul 16, 2026") for a unix-seconds timestamp,
/// used to head each day's messages with a divider. Same UTC basis as
/// `clock_time`. Uses Hinnant's exact days-from-civil algorithm — no date crate.
fn day_label(timestamp: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = (timestamp / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { year + 1 } else { year };
    format!("{} {}, {}", MONTHS[(month - 1) as usize], day, year)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("signer returned an invalid frame".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(|| "signer returned an invalid frame".to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_label_is_exact_across_epoch_and_leap_day() {
        assert_eq!(day_label(0), "Jan 1, 1970");
        // 1972-02-29 is day 789 since the epoch; a naive year*365 would miss it.
        assert_eq!(day_label(789 * 86_400), "Feb 29, 1972");
        // Any within-day second maps to the same civil date.
        assert_eq!(day_label(789 * 86_400 + 86_399), "Feb 29, 1972");
    }

    #[test]
    fn page_comment_mentions_share_the_strict_chat_mention_grammar() {
        let mentions = comment_mentions(&format!(
            "hello @user:{} and @agent:forge/reviewer @user:bad",
            "ab".repeat(32)
        ));
        assert_eq!(mentions.len(), 2);
        assert_eq!(
            mentions[0]
                .get("user")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(32)
        );
        assert_eq!(
            mentions[1]
                .get("agent")
                .and_then(|agent| agent.get("agent_id"))
                .and_then(Value::as_str),
            Some("reviewer")
        );
    }

    #[test]
    fn signed_frames_decode_strictly() {
        assert_eq!(decode_hex("00aF").unwrap(), vec![0, 175]);
        assert!(decode_hex("").is_err());
        assert!(decode_hex("abc").is_err());
        assert!(decode_hex("zz").is_err());
    }
}
