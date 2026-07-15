//! Wire adapters for the native user screens.
//!
//! `screens::user` owns presentation and emits typed commands. This module is
//! the single place that translates those commands to the existing node API.

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

const FILE_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_INLINE_COMMIT_BYTES: u64 = 256 * 1024;
const MAX_FILE_CHANGES: usize = 4_096;
const MAX_FILE_PATH_BYTES: usize = 4_096;
const MAX_FILE_NAME_BYTES: usize = 255;

#[derive(Debug)]
enum UploadEntry {
    Directory {
        relative: String,
    },
    File {
        relative: String,
        source: PathBuf,
        size: u64,
        executable: bool,
    },
}

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
        Command::CreatePage { parent } => action(
            Screen::Pages,
            create_page(backend.as_ref(), client.as_ref(), parent).await,
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
        Command::BeginFileDragOut { path: _, size: _, snapshot: _ } => {
            ServiceEvent::FileDragOutUnavailable(
                "Native file drag-out is unavailable in this build; use Download instead.".into(),
            )
        }
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

async fn load_home(
    backend: Option<Backend>,
    active: Option<Workspace>,
    client: Option<NodeClient>,
) -> Result<Option<HomeData>, String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let (snapshot, identity, touch_id_available, touch_id_enrolled) = tokio::try_join!(
        backend.workspace_snapshot(),
        backend.identity_state(),
        backend.touch_id_available(),
        backend.touch_id_enrolled(),
    )?;
    let connected = match &client {
        Some(client) => client.status().await.is_ok(),
        None => false,
    };
    let mut account = match (connected, &client) {
        (true, Some(client)) => {
            load_identity_account(
                client,
                active.as_ref().map(|workspace| workspace.pubkey.as_str()),
                identity.pubkey.as_deref(),
            )
            .await?
        }
        _ => None,
    };
    if let (Some(current), Some(workspace), Some(client), Some(member_key)) = (
        account.as_ref(),
        active.as_ref(),
        client.as_ref(),
        identity.pubkey.as_deref(),
    ) {
        match account_service::complete_pending_bind(
            &backend, workspace, client, member_key, current,
        )
        .await
        {
            Ok(true) => {
                let node_key = decode_ed25519_key(&workspace.pubkey, "active node key")?;
                if let Some(bound) =
                    query_identity_account(client, json!({ "of_node": { "node_key": node_key } }))
                        .await?
                {
                    account = Some(bound);
                    profile_service::reconcile_best_effort(&backend, workspace, client).await;
                }
            }
            Ok(false) => {}
            Err(error) => tracing::debug!(
                target: "ducktape::account",
                event = "pending_link_bind_failed",
                reason = "post_link_bind_failed",
                detail = %error,
                "pending device link will retry while Home remains open"
            ),
        }
    }
    let account_name = account
        .as_ref()
        .map(|account| optional_account_text(account.get("display_name"), "display name", 64))
        .transpose()?
        .flatten()
        .unwrap_or_default();
    let account_id = account
        .as_ref()
        .map(|account| wire_key_hex(account.get("account_id"), 32, "account id"))
        .transpose()?;
    let account_avatar = account
        .as_ref()
        .map(|account| optional_account_text(account.get("avatar"), "account avatar", 512))
        .transpose()?
        .flatten();
    let account_bio = account
        .as_ref()
        .map(|account| optional_account_profile_text(account.get("bio"), "account bio", 280))
        .transpose()?
        .flatten();
    let duck_name = match (account_id.as_deref(), client.as_ref()) {
        (Some(account_id), Some(client)) if connected => {
            profile_service::duck_name(client, account_id).await?
        }
        _ => None,
    };
    let avatar_bytes = match (account_avatar.as_deref(), client.as_ref()) {
        (Some(path), Some(client)) if connected => {
            match profile_service::load_avatar_bytes(client, path).await {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    tracing::debug!(
                        target: "ducktape::account",
                        event = "profile_avatar_unavailable",
                        reason = "duckfs_avatar_read_failed",
                        detail = %error,
                        "the account avatar will use its initials fallback"
                    );
                    None
                }
            }
        }
        _ => None,
    };
    let profile = identity.pubkey.as_ref().map(|_| AccountProfile {
        display_name: account_name,
        account_id: account_id.clone().unwrap_or_default(),
        duck_name,
        avatar: account_avatar,
        avatar_bytes,
        bio: account_bio,
    });
    let custody = match (identity.pubkey.clone(), identity.state) {
        (Some(public_key), IdentityStatus::Plaintext) => Some(Custody {
            public_key,
            status: CustodyStatus::Plaintext,
        }),
        (Some(public_key), IdentityStatus::Locked) => Some(Custody {
            public_key,
            status: CustodyStatus::Locked,
        }),
        (Some(public_key), IdentityStatus::Unlocked) => Some(Custody {
            public_key,
            status: CustodyStatus::Unlocked,
        }),
        _ => None,
    };
    let (validators, residents) = match (&account, &client) {
        (Some(_), Some(client)) if connected => load_device_standings(client).await?,
        _ => (HashSet::new(), HashSet::new()),
    };
    let mut devices = match account.as_ref() {
        Some(account) => parse_devices(account, active.as_ref())?,
        None => active
            .as_ref()
            .map(|workspace| {
                vec![DeviceRow {
                    key: workspace.pubkey.clone(),
                    label: workspace.name.clone(),
                    standing: workspace_standing(workspace),
                    this_device: true,
                }]
            })
            .unwrap_or_default(),
    };
    for device in &mut devices {
        device.standing = if validators.contains(&device.key) {
            Standing::Validator
        } else if residents.contains(&device.key) {
            Standing::Resident
        } else {
            Standing::NoSeat
        };
    }
    let device_networks = match (account_id.as_deref(), active.as_ref()) {
        (Some(account_id), Some(active)) => {
            let known = snapshot
                .workspaces
                .iter()
                .map(|workspace| workspace.chain_id.clone())
                .collect();
            let live = CachedNetworkDevices {
                chain_id: active.chain_id.clone(),
                name: active.name.clone(),
                at_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
                rows: devices.iter().map(cached_device).collect(),
            };
            let cached = match backend
                .device_cache_update(account_id.to_string(), known, live.clone())
                .await
            {
                Ok(cached) => cached,
                Err(error) => {
                    tracing::debug!(
                        target: "ducktape::account",
                        event = "device_cache_unavailable",
                        reason = "local_state_write_failed",
                        detail = %error,
                        "inactive-network devices will not be shown"
                    );
                    vec![live]
                }
            };
            cached
                .into_iter()
                .map(|network| device_network(network, &active.chain_id))
                .collect()
        }
        _ => Vec::new(),
    };
    let member_keys = parse_member_keys(account.as_ref(), identity.pubkey.as_deref())?;
    let workspaces = snapshot
        .workspaces
        .into_iter()
        .map(|workspace| WorkspaceRow {
            active: active
                .as_ref()
                .is_some_and(|current| current.id == workspace.id),
            standing: if workspace.founder {
                Standing::Validator
            } else if workspace.member {
                Standing::Resident
            } else {
                Standing::NoSeat
            },
            id: workspace.id,
            name: workspace.name,
            network_id: workspace.chain_id,
        })
        .collect();
    Ok(Some(HomeData {
        profile,
        workspaces,
        devices,
        device_networks,
        member_keys,
        custody,
        touch_id_available,
        touch_id_enrolled,
        disconnected: !connected,
    }))
}

fn parse_member_keys(
    account: Option<&Value>,
    local_key: Option<&str>,
) -> Result<Vec<MemberKeyRow>, String> {
    let Some(account) = account else {
        return Ok(Vec::new());
    };
    let keys = account
        .get("member_keys")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid member-key list".to_string())?;
    if keys.is_empty() || keys.len() > 256 {
        return Err("node returned an invalid member-key list".into());
    }
    keys.iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            let kind = match object.get("kind").and_then(Value::as_str) {
                Some("ed25519") => AccountKeyKind::Ed25519,
                Some("p256") => AccountKeyKind::P256,
                Some("webauthn_p256") => AccountKeyKind::WebauthnP256,
                _ => return Err("node returned an unsupported member-key kind".into()),
            };
            let expected = match kind {
                AccountKeyKind::Ed25519 => &[32][..],
                AccountKeyKind::P256 | AccountKeyKind::WebauthnP256 => &[33, 65][..],
            };
            let bytes = object
                .get("pubkey")
                .and_then(Value::as_array)
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            if !expected.contains(&bytes.len()) {
                return Err("node returned a malformed member key".into());
            }
            let key = wire_bytes_hex(bytes)
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            let label = match object.get("label") {
                Some(Value::String(label))
                    if label.len() <= 64 && !label.chars().any(char::is_control) =>
                {
                    Some(label.clone())
                }
                Some(Value::Null) | None => None,
                _ => return Err("node returned an invalid member-key label".into()),
            };
            Ok(MemberKeyRow {
                this_device: local_key.is_some_and(|local| local.eq_ignore_ascii_case(&key)),
                key,
                kind,
                label,
            })
        })
        .collect()
}

async fn load_chat(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    active: Option<&str>,
) -> Result<Option<ChatData>, String> {
    let client = client.ok_or_else(|| "enter a network to load Chat".to_string())?;
    let self_key = match backend {
        Some(backend) => backend.identity_state().await?.pubkey,
        None => None,
    };
    let reply = client
        .query("chat", json!("channels"))
        .await
        .map_err(|error| error.to_string())?;
    let channels = variant_array(&reply, "channels")?
        .iter()
        .filter_map(parse_channel)
        .filter(|channel| !channel.id.contains(':'))
        .collect::<Vec<_>>();
    if channels.is_empty() {
        return Ok(None);
    }
    let content = if let Some(channel) = active
        .and_then(|active| channels.iter().find(|channel| channel.id == active))
        .or_else(|| channels.iter().find(|channel| !channel.archived))
        .or_else(|| channels.first())
    {
        load_channel(backend, Some(client), &channel.id).await?
    } else {
        ChannelContent {
            messages: Vec::new(),
            members: Vec::new(),
        }
    };
    Ok(Some(ChatData {
        channels,
        messages: content.messages,
        thread: None,
        members: content.members,
        tags: Vec::new(),
        hits: Vec::new(),
        history_window: None,
        self_key,
    }))
}

async fn load_channel(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    channel: &str,
) -> Result<ChannelContent, String> {
    let client = client.ok_or_else(|| "enter a network to load Chat".to_string())?;
    let self_key = match backend {
        Some(backend) => backend.identity_state().await?.pubkey,
        None => None,
    };
    let (reply, members) = tokio::try_join!(
        client.query(
            "chat",
            json!({ "messages_latest": { "channel_id": channel, "limit": 256 } }),
        ),
        client.query("chat", json!({ "members": { "channel_id": channel } })),
    )
    .map_err(|error| error.to_string())?;
    let messages = variant_array(&reply, "messages")?
        .iter()
        .filter_map(|value| parse_message(value, self_key.as_deref(), false))
        .collect();
    let members = variant_array(&members, "members")?
        .iter()
        .filter_map(Value::as_array)
        .filter_map(|bytes| wire_bytes_hex(bytes))
        .collect();
    Ok(ChannelContent { messages, members })
}

async fn load_message_window(
    client: Option<&NodeClient>,
    channel: &str,
    sequence: u64,
    backend: Option<&Backend>,
) -> Result<Vec<ChatMessage>, String> {
    let client = client.ok_or_else(|| "enter a network to load Chat".to_string())?;
    let self_key = match backend {
        Some(backend) => backend.identity_state().await?.pubkey,
        None => None,
    };
    let reply = client
        .query(
            "chat",
            json!({ "messages_around": { "channel_id": channel, "seq": sequence, "limit": 256 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(variant_array(&reply, "messages")?
        .iter()
        .filter_map(|value| parse_message(value, self_key.as_deref(), false))
        .collect())
}

async fn load_thread(
    client: Option<&NodeClient>,
    channel: &str,
    root: u64,
    backend: Option<&Backend>,
) -> Result<ChatThread, String> {
    let client = client.ok_or_else(|| "enter a network to load Chat".to_string())?;
    let self_key = match backend {
        Some(backend) => backend.identity_state().await?.pubkey,
        None => None,
    };
    let reply = client
        .query(
            "chat",
            json!({ "thread": { "channel_id": channel, "root_seq": root, "from": 0, "limit": 256 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let thread = reply
        .get("thread")
        .filter(|value| !value.is_null())
        .ok_or_else(|| "thread was not found".to_string())?;
    let root = parse_message(
        thread
            .get("root")
            .ok_or_else(|| "node returned no thread root".to_string())?,
        self_key.as_deref(),
        true,
    )
    .ok_or_else(|| "node returned an invalid thread root".to_string())?;
    let replies = thread
        .get("replies")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned invalid thread replies".to_string())?
        .iter()
        .filter_map(|value| parse_message(value, self_key.as_deref(), true))
        .collect();
    Ok(ChatThread { root, replies })
}

async fn load_chat_tags(
    client: Option<&NodeClient>,
    channel: &str,
) -> Result<Vec<ChatTag>, String> {
    let reply = client
        .ok_or_else(|| "enter a network to load Chat".to_string())?
        .view(
            "chat",
            json!({ "tags": { "channelId": channel, "limit": 256 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(variant_array(&reply, "tags")?
        .iter()
        .filter_map(|value| {
            Some(ChatTag {
                label: value.get("tag")?.as_str()?.to_string(),
                count: value.get("count")?.as_u64()?.try_into().ok()?,
            })
        })
        .collect())
}

async fn filter_chat_tag(
    client: Option<&NodeClient>,
    channel: &str,
    tag: &str,
) -> Result<Vec<ChatHit>, String> {
    let tag = tag.trim().trim_start_matches('#');
    if tag.is_empty() || tag.len() > 128 {
        return Err("chat tag is invalid".into());
    }
    let reply = client
        .ok_or_else(|| "enter a network to load Chat".to_string())?
        .view(
            "chat",
            json!({ "tagSearch": { "tag": tag, "channelId": channel, "limit": 256 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(variant_array(&reply, "hits")?
        .iter()
        .filter_map(|value| {
            Some(ChatHit {
                channel: value.get("channelId")?.as_str()?.to_string(),
                sequence: value.get("seq")?.as_u64()?,
                author: value.get("author")?.as_str()?.to_string(),
                text: value.get("text")?.as_str()?.to_string(),
            })
        })
        .collect())
}

async fn load_pages(
    backend: Option<&Backend>,
    client: Option<NodeClient>,
    active: Option<&str>,
    open_tabs: Vec<String>,
) -> Result<Option<PagesData>, String> {
    let client = client.ok_or_else(|| "enter a network to load Pages".to_string())?;
    let reply = client
        .query("pages", json!("list_pages"))
        .await
        .map_err(|error| error.to_string())?;
    let pages = variant_array(&reply, "page_list")?
        .iter()
        .filter_map(parse_page_meta)
        .collect::<Vec<_>>();
    if pages.is_empty() {
        Ok(None)
    } else {
        let document = match active.filter(|active| pages.iter().any(|page| page.id == *active)) {
            Some(active) => {
                let mut document = load_page(Some(client.clone()), active).await?;
                document.ancestry = page_ancestry(&pages, active);
                document.self_key = match backend {
                    Some(backend) => backend.identity_state().await?.pubkey,
                    None => None,
                };
                Some(document)
            }
            None => None,
        };
        Ok(Some(PagesData {
            open_tabs: open_tabs
                .into_iter()
                .filter(|tab| pages.iter().any(|page| &page.id == tab))
                .collect(),
            pages,
            document,
        }))
    }
}

async fn load_page(client: Option<NodeClient>, page: &str) -> Result<PageDocument, String> {
    let client = client.ok_or_else(|| "enter a network to load Pages".to_string())?;
    let reply = client
        .query("pages", json!({ "get_page": { "page_id": page } }))
        .await
        .map_err(|error| error.to_string())?;
    let wire = reply
        .get("page")
        .and_then(Value::as_array)
        .ok_or_else(|| "page was not found".to_string())?;
    let root = wire
        .first()
        .ok_or_else(|| "page contains no root block".to_string())?;
    let title = root
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("Untitled")
        .to_string();
    let mut parents = std::collections::HashMap::new();
    for value in wire {
        if let (Some(id), Some(parent)) = (
            value.get("id").and_then(Value::as_str),
            value.get("parent").and_then(Value::as_str),
        ) {
            parents.insert(id, parent);
        }
    }
    let blocks = wire
        .iter()
        .skip(1)
        .filter_map(|value| parse_page_block(value, &parents))
        .collect::<Vec<_>>();
    let targets = std::iter::once(page.to_string())
        .chain(blocks.iter().take(511).map(|block| block.id.clone()))
        .collect::<Vec<_>>();
    let comments = client
        .query(
            "pages",
            json!({ "threads_for_targets": { "targets": targets } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let comment_threads = parse_page_comments(&comments)?;
    let page_comments = comment_threads
        .iter()
        .filter(|thread| thread.target == page)
        .count();
    Ok(PageDocument {
        id: page.to_string(),
        title,
        ancestry: Vec::new(),
        blocks,
        page_comments,
        comment_threads,
        presence: Vec::new(),
        self_key: None,
    })
}

async fn load_page_with_ancestry(
    backend: Option<&Backend>,
    client: Option<NodeClient>,
    page: &str,
) -> Result<PageDocument, String> {
    let client = client.ok_or_else(|| "enter a network to load Pages".to_string())?;
    let mut document = load_page(Some(client.clone()), page).await?;
    document.self_key = match backend {
        Some(backend) => backend.identity_state().await?.pubkey,
        None => None,
    };
    let reply = client
        .query("pages", json!("list_pages"))
        .await
        .map_err(|error| error.to_string())?;
    let pages = variant_array(&reply, "page_list")?
        .iter()
        .filter_map(parse_page_meta)
        .collect::<Vec<_>>();
    document.ancestry = page_ancestry(&pages, page);
    Ok(document)
}

fn page_ancestry(pages: &[PageMeta], page: &str) -> Vec<PageMeta> {
    let mut ancestry = Vec::new();
    let mut cursor = pages
        .iter()
        .find(|candidate| candidate.id == page)
        .and_then(|candidate| candidate.parent.as_deref());
    while let Some(parent) = cursor {
        let Some(meta) = pages.iter().find(|candidate| candidate.id == parent) else {
            break;
        };
        ancestry.push(meta.clone());
        cursor = meta.parent.as_deref();
        if ancestry.len() >= pages.len() {
            return Vec::new();
        }
    }
    ancestry.reverse();
    ancestry
}

async fn load_files(
    client: Option<NodeClient>,
    path: String,
    snapshot: Option<String>,
) -> Result<Option<FileListing>, String> {
    let client = client.ok_or_else(|| "enter a network to load Files".to_string())?;
    let (entries, refs, history) = tokio::try_join!(
        client.files_ls(&path, snapshot.as_deref()),
        client.files_refs(),
        client.files_history(64),
    )
    .map_err(|error| error.to_string())?;
    let entries = entries
        .into_iter()
        .filter_map(|entry| {
            let kind = match entry.kind.as_str() {
                "dir" => FileKind::Directory,
                "file" => FileKind::File,
                "symlink" => FileKind::Symlink,
                _ => return None,
            };
            let name = entry
                .path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            Some(ScreenFileEntry {
                path: entry.path,
                name,
                kind,
                size: entry.size,
                executable: entry.exec,
            })
        })
        .collect();
    let history = history
        .into_iter()
        .map(|entry| ScreenFileSnapshot {
            id: entry.id,
            message: entry.message,
            height: entry.height,
            time: clock_time(entry.consensus_time),
        })
        .collect();
    Ok(Some(FileListing {
        path,
        entries,
        preview: None,
        read_only: snapshot.is_some(),
        refreshing: false,
        head: refs.head,
        snapshot,
        history,
        diff: Vec::new(),
    }))
}

async fn load_file(
    client: Option<NodeClient>,
    path: String,
    snapshot: Option<String>,
) -> Result<FilePreview, String> {
    let client = client.ok_or_else(|| "enter a network to load Files".to_string())?;
    let (bytes, complete) = client
        .files_preview(&path, snapshot.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let detail = if complete {
        format!("{} bytes", bytes.len())
    } else {
        format!("{} byte preview · file continues", bytes.len())
    };
    Ok(FilePreview {
        path,
        content: classify_file_preview(bytes, complete),
        detail,
    })
}

fn classify_file_preview(bytes: Vec<u8>, complete: bool) -> FilePreviewContent {
    if bytes
        .windows(5)
        .take(1_024)
        .any(|header| header == b"%PDF-")
    {
        return FilePreviewContent::Pdf;
    }
    if let Ok(format) = image::guess_format(&bytes)
        && matches!(
            format,
            image::ImageFormat::Png
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Gif
                | image::ImageFormat::WebP
        )
    {
        if !complete {
            return FilePreviewContent::Unsupported(
                "Image exceeds the 1 MiB encoded preview limit. Download it to open safely.".into(),
            );
        }
        let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes));
        reader.set_format(format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4_096);
        limits.max_image_height = Some(4_096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        return match reader.into_dimensions() {
            Ok((width, height)) => FilePreviewContent::Image {
                bytes,
                width,
                height,
            },
            Err(_) => FilePreviewContent::Unsupported(
                "Image is invalid or exceeds the 4096 × 4096 preview limit.".into(),
            ),
        };
    }
    match String::from_utf8(bytes) {
        Ok(text)
            if !text.chars().any(|character| {
                character.is_control() && !matches!(character, '\n' | '\r' | '\t')
            }) =>
        {
            FilePreviewContent::Text(text)
        }
        _ => FilePreviewContent::Unsupported(
            "This binary file type is not rendered by the desktop preview.".into(),
        ),
    }
}

async fn choose_download(
    client: Option<&NodeClient>,
    path: &str,
    size: u64,
    snapshot: Option<&str>,
) -> Result<(), String> {
    let name = path
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("download");
    let Some(destination) = rfd::AsyncFileDialog::new()
        .set_file_name(name)
        .save_file()
        .await
    else {
        return Ok(());
    };
    user_content_service::download_file(client, path, snapshot, size, destination.path()).await
}

async fn load_file_diff(
    client: Option<&NodeClient>,
    from: &str,
    to: &str,
    prefix: &str,
) -> Result<Vec<ScreenFileDiff>, String> {
    let diff = user_content_service::file_diff(client, from, to, prefix)
        .await?
        .iter()
        .map(parse_file_diff)
        .collect::<Result<Vec<_>, String>>()?;
    Ok(diff)
}

fn parse_file_diff(value: &Value) -> Result<ScreenFileDiff, String> {
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| path.starts_with('/') && path.len() <= 4_096)
        .ok_or_else(|| "node returned an invalid files diff path".to_string())?;
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .filter(|kind| matches!(*kind, "added" | "removed" | "modified"))
        .ok_or_else(|| "node returned an invalid files diff kind".to_string())?;
    Ok(ScreenFileDiff {
        path: path.to_string(),
        kind: kind.to_string(),
    })
}

async fn create_folder(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    parent: String,
    name: String,
) -> Result<(), String> {
    let client = client.ok_or_else(|| "enter a network to use Files".to_string())?;
    if name.is_empty()
        || name.len() > 255
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '\0'))
    {
        return Err("folder name is invalid".into());
    }
    let path = if parent == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", parent.trim_end_matches('/'))
    };
    let head = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    let body = json!({
        "base_snapshot": head,
        "message": format!("create {name}"),
        "changes": [{ "mkdir": { "path": path } }]
    });
    commit_files(backend, &client, body).await
}

async fn choose_files(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
) -> Result<(), String> {
    let Some(handles) = rfd::AsyncFileDialog::new().pick_files().await else {
        return Ok(());
    };
    let paths = handles
        .into_iter()
        .map(|handle| handle.path().to_owned())
        .collect::<Vec<_>>();
    let entries = tokio::task::spawn_blocking(move || selected_files(paths))
        .await
        .map_err(|_| "native file selection task failed".to_string())??;
    upload_entries(backend, client, target, entries, "upload files").await
}

async fn choose_chat_attachment(
    backend: Option<Backend>,
    client: Option<NodeClient>,
) -> Result<String, String> {
    let Some(handle) = rfd::AsyncFileDialog::new().pick_file().await else {
        return Ok(String::new());
    };
    let source = handle.path().to_owned();
    let raw_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let name = sanitize_attachment_name(raw_name);
    let entry = file_entry(name.clone(), source)?;
    let size = match &entry {
        UploadEntry::File { size, .. } => *size,
        UploadEntry::Directory { .. } => 0,
    };
    if size > 25 * 1024 * 1024 {
        return Err("attachment exceeds 25 MiB".into());
    }
    let target = format!("/shared/attachments/{}", fresh_id("upload"));
    upload_entries(backend, client, target.clone(), vec![entry], "attach").await?;
    let path = format!("{target}/{name}");
    Ok(format!(
        "{}[{name}](duck://files{path})",
        if is_image_name(&name) { "!" } else { "" }
    ))
}

async fn download_chat_attachment(client: Option<&NodeClient>, path: &str) -> Result<(), String> {
    let client = client.ok_or_else(|| "enter a network to download an attachment".to_string())?;
    let entry = client
        .files_stat(path, None)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "attachment no longer exists".to_string())?;
    if entry.kind != "file" || entry.size > 25 * 1024 * 1024 {
        return Err("attachment is not a downloadable file".into());
    }
    choose_download(Some(client), path, entry.size, None).await
}

fn sanitize_attachment_name(raw: &str) -> String {
    let mut name = String::new();
    let mut last_dash = false;
    for character in raw.nfc() {
        let bidi = matches!(
            character,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        );
        if character.is_control() || bidi {
            continue;
        }
        if character.is_whitespace()
            || matches!(character, '/' | '\\' | '[' | ']' | '*' | '(' | ')')
        {
            if !name.is_empty() && !last_dash {
                name.push('-');
                last_dash = true;
            }
            continue;
        }
        name.push(character);
        last_dash = character == '-';
    }
    let mut name = name
        .trim_matches(|character| matches!(character, '.' | '-'))
        .to_string();
    if name.is_empty() {
        name = "file".into();
    }
    while name.len() > MAX_FILE_NAME_BYTES {
        name.pop();
    }
    if name.is_empty() { "file".into() } else { name }
}

fn is_image_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif"
        )
    })
}

async fn choose_folder(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
) -> Result<(), String> {
    let Some(handle) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(());
    };
    let path = handle.path().to_owned();
    let entries = tokio::task::spawn_blocking(move || selected_folder(&path))
        .await
        .map_err(|_| "native folder selection task failed".to_string())??;
    upload_entries(backend, client, target, entries, "upload folder").await
}

async fn upload_dropped(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
    source: PathBuf,
) -> Result<(), String> {
    let entries = tokio::task::spawn_blocking(move || dropped_entries(source))
        .await
        .map_err(|_| "native dropped-file inspection task failed".to_string())??;
    upload_entries(backend, client, target, entries, "drop files").await
}

fn dropped_entries(source: PathBuf) -> Result<Vec<UploadEntry>, String> {
    let metadata = std::fs::symlink_metadata(&source)
        .map_err(|error| format!("could not inspect dropped path: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("symbolic links cannot be imported".into());
    }
    if metadata.is_dir() {
        selected_folder(&source)
    } else if metadata.is_file() {
        selected_files(vec![source])
    } else {
        Err("the dropped path is not a regular file or folder".into())
    }
}

fn selected_files(paths: Vec<PathBuf>) -> Result<Vec<UploadEntry>, String> {
    if paths.len() > MAX_FILE_CHANGES {
        return Err("file selection exceeds the 4096-item limit".into());
    }
    paths
        .into_iter()
        .map(|source| {
            let relative = source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "a selected file name is not valid UTF-8".to_string())?;
            validate_relative_path(relative)?;
            file_entry(relative.to_owned(), source)
        })
        .collect()
}

fn selected_folder(root: &Path) -> Result<Vec<UploadEntry>, String> {
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "the selected folder name is not valid UTF-8".to_string())?
        .to_owned();
    validate_relative_path(&root_name)?;
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|error| format!("could not inspect selected folder: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("the selected item is not a regular folder".into());
    }

    let mut entries = vec![UploadEntry::Directory {
        relative: root_name.clone(),
    }];
    walk_folder(root, &root_name, &mut entries)?;
    Ok(entries)
}

fn walk_folder(root: &Path, relative: &str, entries: &mut Vec<UploadEntry>) -> Result<(), String> {
    let mut children = std::fs::read_dir(root)
        .map_err(|error| format!("could not read selected folder: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("could not read selected folder: {error}"))?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        if entries.len() >= MAX_FILE_CHANGES {
            return Err("folder selection exceeds the 4096-item limit".into());
        }
        let name = child
            .file_name()
            .into_string()
            .map_err(|_| "a selected path is not valid UTF-8".to_string())?;
        let child_relative = format!("{relative}/{name}");
        validate_relative_path(&child_relative)?;
        let metadata = child
            .path()
            .symlink_metadata()
            .map_err(|error| format!("could not inspect selected path: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "symbolic links cannot be imported: {child_relative}"
            ));
        }
        if metadata.is_dir() {
            entries.push(UploadEntry::Directory {
                relative: child_relative.clone(),
            });
            walk_folder(&child.path(), &child_relative, entries)?;
        } else if metadata.is_file() {
            entries.push(file_entry(child_relative, child.path())?);
        } else {
            return Err(format!("unsupported filesystem entry: {child_relative}"));
        }
    }
    Ok(())
}

fn file_entry(relative: String, source: PathBuf) -> Result<UploadEntry, String> {
    let metadata = std::fs::symlink_metadata(&source)
        .map_err(|error| format!("could not inspect selected file: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("selected path is not a regular file: {relative}"));
    }
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    Ok(UploadEntry::File {
        relative,
        source,
        size: metadata.len(),
        executable,
    })
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.starts_with('/') || path.len() > MAX_FILE_PATH_BYTES {
        return Err("selected path is invalid or too long".into());
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.len() > MAX_FILE_NAME_BYTES
            || segment.contains(['\\', '\0'])
        {
            return Err(format!("selected path contains an invalid segment: {path}"));
        }
    }
    Ok(())
}

fn upload_path(target: &str, relative: &str) -> Result<String, String> {
    if !target.starts_with('/') || target.contains('\0') {
        return Err("file upload target must be an absolute DuckFS path".into());
    }
    if target
        .split('/')
        .skip(1)
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            segment == "."
                || segment == ".."
                || segment.len() > MAX_FILE_NAME_BYTES
                || segment.contains('\\')
        })
    {
        return Err("file upload target contains an invalid path segment".into());
    }
    let target = target.trim_end_matches('/');
    let path = if target.is_empty() {
        format!("/{relative}")
    } else {
        format!("{target}/{relative}")
    };
    if path.len() > MAX_FILE_PATH_BYTES {
        return Err(format!("file upload target is too long: {path}"));
    }
    Ok(path)
}

async fn upload_entries(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
    entries: Vec<UploadEntry>,
    label: &str,
) -> Result<(), String> {
    let client = client.ok_or_else(|| "enter a network to use Files".to_string())?;
    if entries.is_empty() || entries.len() > MAX_FILE_CHANGES {
        return Err("file import requires between 1 and 4096 items".into());
    }
    let mut seen = HashSet::with_capacity(entries.len());
    let mut changes = Vec::with_capacity(entries.len());
    let mut inline_bytes = 0_u64;
    for entry in entries {
        match entry {
            UploadEntry::Directory { relative } => {
                let path = upload_path(&target, &relative)?;
                if !seen.insert(path.clone()) {
                    return Err(format!("file selection contains a duplicate path: {path}"));
                }
                changes.push(json!({ "mkdir": { "path": path } }));
            }
            UploadEntry::File {
                relative,
                source,
                size,
                executable,
            } => {
                let path = upload_path(&target, &relative)?;
                if !seen.insert(path.clone()) {
                    return Err(format!("file selection contains a duplicate path: {path}"));
                }
                let content = if size > 0
                    && inline_bytes.saturating_add(size) <= MAX_INLINE_COMMIT_BYTES
                {
                    let bytes = tokio::fs::read(&source)
                        .await
                        .map_err(|error| format!("could not read {relative}: {error}"))?;
                    if bytes.len() as u64 != size {
                        return Err(format!("selected file changed while importing: {relative}"));
                    }
                    inline_bytes += size;
                    json!({
                        "inline": {
                            "b64": base64::engine::general_purpose::STANDARD.encode(bytes)
                        }
                    })
                } else {
                    let chunks = stage_file(&client, &source, size, &relative).await?;
                    json!({ "chunks": { "size": size, "chunks": chunks } })
                };
                changes.push(json!({
                    "put": {
                        "path": path,
                        "exec": executable,
                        "meta": {},
                        "content": content
                    }
                }));
            }
        }
    }
    let head = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    let body = json!({
        "base_snapshot": head,
        "message": format!("{label} to {}", if target.is_empty() { "/" } else { &target }),
        "changes": changes
    });
    commit_files(backend, &client, body).await
}

async fn stage_file(
    client: &NodeClient,
    source: &Path,
    expected_size: u64,
    relative: &str,
) -> Result<Vec<String>, String> {
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    let mut file = tokio::fs::File::open(source)
        .await
        .map_err(|error| format!("could not read {relative}: {error}"))?;
    let mut remaining = expected_size;
    let mut chunks = Vec::new();
    while remaining > 0 {
        let wanted = remaining.min(FILE_CHUNK_BYTES as u64) as usize;
        let mut chunk = vec![0_u8; wanted];
        file.read_exact(&mut chunk).await.map_err(|error| {
            format!("selected file changed while importing {relative}: {error}")
        })?;
        chunks.push(
            client
                .put_blob(chunk)
                .await
                .map_err(|error| error.to_string())?,
        );
        remaining -= wanted as u64;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .await
        .map_err(|error| format!("could not finish reading {relative}: {error}"))?
        != 0
    {
        return Err(format!("selected file changed while importing: {relative}"));
    }
    Ok(chunks)
}

async fn commit_files(
    backend: Option<Backend>,
    client: &NodeClient,
    body: Value,
) -> Result<(), String> {
    user_content_service::submit_signed(
        backend.as_ref(),
        Some(client),
        crate::backend::ContentTarget::Files,
        json!({ "commit": body }),
    )
    .await
}

async fn load_identity_account(
    client: &NodeClient,
    node_key: Option<&str>,
    member_key: Option<&str>,
) -> Result<Option<Value>, String> {
    if let Some(node_key) = node_key {
        let node_key = decode_ed25519_key(node_key, "active node key")?;
        if let Some(account) =
            query_identity_account(client, json!({ "of_node": { "node_key": node_key } })).await?
        {
            return Ok(Some(account));
        }
    }
    let Some(member_key) = member_key else {
        return Ok(None);
    };
    let member_key = decode_ed25519_key(member_key, "local member key")?;
    query_identity_account(client, json!({ "of_member": { "member_key": member_key } })).await
}

async fn query_identity_account(
    client: &NodeClient,
    query: Value,
) -> Result<Option<Value>, String> {
    let reply = client
        .query("identity", query)
        .await
        .map_err(|error| error.to_string())?;
    match reply.get("account") {
        Some(Value::Null) | None => Ok(None),
        Some(account @ Value::Object(_)) => Ok(Some(account.clone())),
        Some(_) => Err("node returned an invalid identity account".into()),
    }
}

fn parse_devices(account: &Value, active: Option<&Workspace>) -> Result<Vec<DeviceRow>, String> {
    let nodes = account
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid bound-node list".to_string())?;
    if nodes.len() > 256 {
        return Err("node returned too many bound nodes".into());
    }
    nodes
        .iter()
        .map(|node| {
            let node = node
                .as_object()
                .ok_or_else(|| "node returned a malformed bound node".to_string())?;
            let key = wire_key_hex(node.get("node_key"), 32, "bound node key")?;
            let label = optional_account_text(node.get("label"), "node label", 64)?
                .unwrap_or_else(|| "Device".into());
            Ok(DeviceRow {
                this_device: active
                    .is_some_and(|workspace| workspace.pubkey.eq_ignore_ascii_case(&key)),
                label,
                standing: Standing::NoSeat,
                key,
            })
        })
        .collect()
}

async fn load_device_standings(
    client: &NodeClient,
) -> Result<(HashSet<String>, HashSet<String>), String> {
    let (validators, residents) = tokio::try_join!(
        load_standing_keys(client, "validators"),
        load_standing_keys(client, "residents"),
    )?;
    Ok((validators, residents))
}

async fn load_standing_keys(client: &NodeClient, variant: &str) -> Result<HashSet<String>, String> {
    let reply = client
        .query("valset", Value::String(variant.into()))
        .await
        .map_err(|error| error.to_string())?;
    let rows = reply
        .get(variant)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {variant} list"))?;
    if rows.len() > 512 {
        return Err(format!("node returned too many {variant}"));
    }
    rows.iter()
        .map(|row| wire_key_hex(Some(row), 32, variant))
        .collect()
}

fn workspace_standing(workspace: &Workspace) -> Standing {
    if workspace.founder {
        Standing::Validator
    } else if workspace.member {
        Standing::Resident
    } else {
        Standing::NoSeat
    }
}

fn cached_device(device: &DeviceRow) -> CachedDeviceRow {
    CachedDeviceRow {
        node_key: device.key.clone(),
        label: (device.label != "Device").then(|| device.label.clone()),
        standing: match device.standing {
            Standing::Validator => DeviceStanding::Validator,
            Standing::Resident => DeviceStanding::Resident,
            Standing::NoSeat => DeviceStanding::NoSeat,
        },
        this_device: device.this_device,
    }
}

fn device_network(network: CachedNetworkDevices, active_chain: &str) -> DeviceNetworkGroup {
    DeviceNetworkGroup {
        active: network.chain_id == active_chain,
        network_id: network.chain_id,
        name: network.name,
        at_ms: network.at_ms,
        devices: network
            .rows
            .into_iter()
            .map(|device| DeviceRow {
                key: device.node_key,
                label: device.label.unwrap_or_else(|| "Device".into()),
                standing: match device.standing {
                    DeviceStanding::Validator => Standing::Validator,
                    DeviceStanding::Resident => Standing::Resident,
                    DeviceStanding::NoSeat => Standing::NoSeat,
                },
                this_device: device.this_device,
            })
            .collect(),
    }
}

fn optional_account_text(
    value: Option<&Value>,
    field: &str,
    max_bytes: usize,
) -> Result<Option<String>, String> {
    match value {
        Some(Value::String(value))
            if value.len() <= max_bytes && !value.chars().any(char::is_control) =>
        {
            Ok(Some(value.clone()))
        }
        Some(Value::Null) | None => Ok(None),
        _ => Err(format!("node returned an invalid {field}")),
    }
}

fn optional_account_profile_text(
    value: Option<&Value>,
    field: &str,
    max_bytes: usize,
) -> Result<Option<String>, String> {
    match value {
        Some(Value::String(value)) if !value.is_empty() && value.len() <= max_bytes => {
            Ok(Some(value.clone()))
        }
        Some(Value::Null) | None => Ok(None),
        _ => Err(format!("node returned an invalid {field}")),
    }
}

fn wire_key_hex(value: Option<&Value>, len: usize, field: &str) -> Result<String, String> {
    let bytes = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {field}"))?;
    if bytes.len() != len {
        return Err(format!("node returned an invalid {field}"));
    }
    wire_bytes_hex(bytes).ok_or_else(|| format!("node returned an invalid {field}"))
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

async fn create_channel(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    name: String,
    policy: PostPolicy,
) -> Result<(), String> {
    chat_write(
        backend,
        client,
        json!({
            "create_channel": {
                "channel_id": slug(&name),
                "name": name,
                "post_policy": match policy {
                    PostPolicy::Open => "open",
                    PostPolicy::MembersOnly => "members_only",
                }
            }
        }),
    )
    .await
}

async fn send_message(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    channel: String,
    body: String,
    thread: Option<u64>,
) -> Result<(), String> {
    if body.is_empty() || body.len() > 16 * 1024 {
        return Err("message must be between 1 and 16384 bytes".into());
    }
    chat_write(
        backend,
        client,
        json!({
            "post_message": {
                "channel_id": channel,
                "message_id": fresh_id("message"),
                "blocks": chat_blocks_wire(&body),
                "thread": thread,
                "as_agent": null
            }
        }),
    )
    .await
}

async fn edit_message(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    channel: String,
    sequence: u64,
    base_revision: u64,
    body: String,
) -> Result<(), String> {
    if body.is_empty() || body.len() > 16 * 1024 {
        return Err("message must be between 1 and 16384 bytes".into());
    }
    chat_write(
        backend,
        client,
        json!({
            "edit_message": {
                "channel_id": channel,
                "seq": sequence,
                "blocks": chat_blocks_wire(&body),
                "base_rev": base_revision
            }
        }),
    )
    .await
}

fn chat_blocks_wire(body: &str) -> Vec<Value> {
    body.split("\n\n")
        .map(|chunk| {
            let chunk = chunk.trim_end_matches('\n');
            if chunk.trim() == "---" {
                return json!("divider");
            }
            if let Some(code) = chunk.strip_prefix("```") {
                let (lang, text) = code.split_once('\n').map_or((None, code), |(lang, text)| {
                    (
                        some_nonempty(lang.trim()),
                        text.strip_suffix("```").unwrap_or(text),
                    )
                });
                return json!({ "code": { "lang": lang, "text": text } });
            }
            let (kind, text) = if chunk.lines().all(|line| line.starts_with("> ")) {
                (
                    "quote",
                    chunk
                        .lines()
                        .map(|line| line.trim_start_matches("> "))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            } else {
                ("paragraph", chunk.to_string())
            };
            if kind == "quote" {
                json!({ "quote": markdown_spans(&text) })
            } else {
                json!({ "paragraph": markdown_spans(&text) })
            }
        })
        .collect()
}

fn markdown_spans(text: &str) -> Vec<Value> {
    let mut spans = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let next = ["**", "*"]
            .into_iter()
            .filter_map(|marker| rest.find(marker).map(|at| (at, marker)))
            .min_by_key(|(at, _)| *at);
        let Some((at, marker)) = next else {
            push_wire_spans(&mut spans, rest, None);
            break;
        };
        if at > 0 {
            push_wire_spans(&mut spans, &rest[..at], None);
        }
        let after = &rest[at + marker.len()..];
        if let Some(end) = after.find(marker) {
            push_wire_spans(
                &mut spans,
                &after[..end],
                Some(if marker == "**" { "bold" } else { "italic" }),
            );
            rest = &after[end + marker.len()..];
        } else {
            push_wire_spans(&mut spans, marker, None);
            rest = after;
        }
    }
    spans
}

fn push_wire_spans(output: &mut Vec<Value>, text: &str, base_mark: Option<&str>) {
    let mut rest = text;
    while !rest.is_empty() {
        let link = rest.find('[').and_then(|open| {
            let after = &rest[open + 1..];
            let label_end = after.find("](")?;
            let url_start = open + 1 + label_end + 2;
            let close = rest[url_start..].find(')')? + url_start;
            let url = &rest[url_start..close];
            matches!(Url::parse(url).ok()?.scheme(), "http" | "https").then_some((
                open,
                open + 1 + label_end,
                url_start,
                close,
            ))
        });
        let mention = find_wire_mention(rest);
        match (link, mention) {
            (Some(link), Some(mention)) if mention.0 < link.0 => {
                push_plain_wire(output, &rest[..mention.0], base_mark);
                output.push(json!({
                    "text": mention.1,
                    "marks": wire_marks(base_mark, Some(mention.2))
                }));
                rest = &rest[mention.3..];
            }
            (Some((open, label_end, url_start, close)), _) => {
                push_plain_wire(output, &rest[..open], base_mark);
                output.push(json!({
                    "text": &rest[open + 1..label_end],
                    "marks": wire_marks(base_mark, Some(json!({ "link": &rest[url_start..close] })))
                }));
                rest = &rest[close + 1..];
            }
            (None, Some((start, label, mark, end))) => {
                push_plain_wire(output, &rest[..start], base_mark);
                output.push(json!({
                    "text": label,
                    "marks": wire_marks(base_mark, Some(mark))
                }));
                rest = &rest[end..];
            }
            (None, None) => {
                push_plain_wire(output, rest, base_mark);
                break;
            }
        }
    }
}

fn push_plain_wire(output: &mut Vec<Value>, text: &str, base_mark: Option<&str>) {
    if !text.is_empty() {
        output.push(json!({ "text": text, "marks": wire_marks(base_mark, None) }));
    }
}

fn wire_marks(base_mark: Option<&str>, extra: Option<Value>) -> Vec<Value> {
    base_mark
        .map(Value::from)
        .into_iter()
        .chain(extra)
        .collect()
}

fn find_wire_mention(text: &str) -> Option<(usize, String, Value, usize)> {
    for (start, _) in text.match_indices('@') {
        if start > 0 && !text[..start].chars().last()?.is_whitespace() {
            continue;
        }
        let tail = &text[start..];
        if let Some(hex) = tail.strip_prefix("@user:").and_then(|tail| tail.get(..64))
            && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            let end = start + "@user:".len() + 64;
            return Some((
                start,
                format!("@{}…", &hex[..8]),
                json!({ "mention": { "user": decode_hex(hex).ok()? } }),
                end,
            ));
        }
        if let Some(token) = tail.strip_prefix("@agent:") {
            let end = token.find(char::is_whitespace).unwrap_or(token.len());
            let token = &token[..end];
            let (module, id) = token.split_once('/')?;
            if safe_ref_segment(module) && safe_ref_segment(id) {
                return Some((
                    start,
                    format!("@{id}"),
                    json!({ "mention": { "agent": { "module": module, "agent_id": id } } }),
                    start + "@agent:".len() + end,
                ));
            }
        }
    }
    None
}

fn comment_mentions(text: &str) -> Vec<Value> {
    let mut mentions = Vec::new();
    let mut rest = text;
    while mentions.len() < 64 {
        let Some((_, _, mark, end)) = find_wire_mention(rest) else {
            break;
        };
        if let Some(mention) = mark.get("mention")
            && !mentions.contains(mention)
        {
            mentions.push(mention.clone());
        }
        rest = &rest[end..];
    }
    mentions
}

fn some_nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

async fn set_reaction(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    channel: String,
    sequence: u64,
    emoji: String,
    remove: bool,
) -> Result<(), String> {
    let emoji = user_content_service::validate_emoji(&emoji)?;
    chat_write(
        backend,
        client,
        if remove {
            json!({ "remove_reaction": { "channel_id": channel, "seq": sequence, "emoji": emoji } })
        } else {
            json!({ "add_reaction": { "channel_id": channel, "seq": sequence, "emoji": emoji } })
        },
    )
    .await
}

async fn set_membership(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    channel: String,
    key: String,
    member: bool,
) -> Result<(), String> {
    let user = user_content_service::user_key_bytes(&key)?;
    chat_write(
        backend,
        client,
        json!({ "set_membership": { "channel_id": channel, "user": user, "member": member } }),
    )
    .await
}

async fn set_huddle(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    channel: String,
    joined: bool,
) -> Result<(), String> {
    let payload = if joined {
        let status = client
            .ok_or_else(|| "enter a network to join a huddle".to_string())?
            .status()
            .await
            .map_err(|error| error.to_string())?;
        let node = user_content_service::user_key_bytes(
            status
                .public_key
                .as_deref()
                .ok_or_else(|| "this node has no huddle identity".to_string())?,
        )?;
        json!({ "join_huddle": { "channel_id": channel, "node": node } })
    } else {
        json!({ "leave_huddle": { "channel_id": channel } })
    };
    chat_write(backend, client, payload).await
}

async fn create_page(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    parent: Option<String>,
) -> Result<(), String> {
    pages_write(
        backend,
        client,
        json!({
            "create_page": {
                "page_id": fresh_id("page"),
                "title": "Untitled",
                "parent": parent
            }
        }),
    )
    .await
}

async fn chat_write(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    payload: Value,
) -> Result<(), String> {
    user_content_service::chat_write(backend, client, payload).await
}

async fn apply_slash(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    block: String,
    kind: BlockKind,
    text: String,
) -> Result<(), String> {
    pages_write(
        backend,
        client,
        json!({ "update_text": { "block_id": &block, "text": text } }),
    )
    .await?;
    pages_write(
        backend,
        client,
        json!({ "set_kind": { "block_id": block, "kind": block_kind_wire(kind) } }),
    )
    .await
}

async fn pages_write(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    payload: Value,
) -> Result<(), String> {
    user_content_service::pages_write(backend, client, payload).await
}

async fn split_page_block(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    left: PageBlock,
    right: PageBlock,
    thread_moves: Vec<ThreadMove>,
) -> Result<(), String> {
    // Keep the original full block authoritative until the right half and all
    // of its exact comment anchors have landed. Any failure leaves a visible
    // duplicate, never silently discarded text.
    pages_write(
        backend,
        client,
        json!({
            "insert_block": {
                "parent": right.parent,
                "after": left.id,
                "block": {
                    "id": right.id,
                    "kind": block_kind_wire(right.kind),
                    "text": right.text,
                    "marks": marks_wire(&right.marks)
                }
            }
        }),
    )
    .await?;
    move_page_threads(backend, client, thread_moves).await?;
    pages_write(
        backend,
        client,
        json!({
            "update_text": {
                "block_id": left.id,
                "text": left.text,
                "marks": marks_wire(&left.marks)
            }
        }),
    )
    .await
}

async fn merge_page_block(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    destination: PageBlock,
    source: PageBlock,
    thread_moves: Vec<ThreadMove>,
) -> Result<(), String> {
    // The source remains the fallback copy until its comments and children
    // have moved. Removing it earlier would turn a rejected move into loss.
    pages_write(
        backend,
        client,
        json!({
            "update_text": {
                "block_id": destination.id,
                "text": destination.text,
                "marks": marks_wire(&destination.marks)
            }
        }),
    )
    .await?;
    move_page_threads(backend, client, thread_moves).await?;
    let mut after = destination.children.last().cloned();
    for child in &source.children {
        pages_write(
            backend,
            client,
            json!({
                "move_block": {
                    "block_id": child,
                    "parent": destination.id,
                    "after": after
                }
            }),
        )
        .await?;
        after = Some(child.clone());
    }
    pages_write(
        backend,
        client,
        json!({ "remove_block": { "block_id": source.id } }),
    )
    .await
}

async fn move_page_threads(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    moves: Vec<ThreadMove>,
) -> Result<(), String> {
    for movement in moves {
        pages_write(
            backend,
            client,
            json!({
                "move_comment_thread": {
                    "thread_id": movement.thread,
                    "target": movement.target,
                    "anchor": movement.anchor.map(anchor_wire)
                }
            }),
        )
        .await?;
    }
    Ok(())
}

fn anchor_wire(anchor: RelativeAnchor) -> Value {
    json!({ "start": anchor.start, "end": anchor.end })
}

fn marks_wire(marks: &[SpanMark]) -> Vec<Value> {
    marks
        .iter()
        .map(|mark| {
            json!({
                "start": mark.start,
                "end": mark.end,
                "kind": inline_mark_wire(mark.kind)
            })
        })
        .collect()
}

async fn paste_page_blocks(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    parent: String,
    mut after: Option<String>,
    blocks: Vec<(BlockKind, String, bool)>,
) -> Result<(), String> {
    if blocks.is_empty() || blocks.len() > 60 {
        return Err("page paste must contain between 1 and 60 blocks".into());
    }
    for (kind, text, checked) in blocks {
        let id = fresh_id("block");
        pages_write(
            backend,
            client,
            json!({
                "insert_block": {
                    "parent": parent,
                    "after": after,
                    "block": { "id": id, "kind": block_kind_wire(kind), "text": text }
                }
            }),
        )
        .await?;
        if checked {
            pages_write(
                backend,
                client,
                json!({ "set_checked": { "block_id": id, "checked": true } }),
            )
            .await?;
        }
        after = Some(id);
    }
    Ok(())
}

const fn block_kind_wire(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph => "paragraph",
        BlockKind::Heading1 => "heading1",
        BlockKind::Heading2 => "heading2",
        BlockKind::Heading3 => "heading3",
        BlockKind::Bulleted => "bulleted",
        BlockKind::Numbered => "numbered",
        BlockKind::Todo => "todo",
        BlockKind::Toggle => "toggle",
        BlockKind::Quote => "quote",
        BlockKind::Code => "code",
        BlockKind::Callout => "callout",
        BlockKind::Divider => "divider",
    }
}

const fn inline_mark_wire(kind: InlineMark) -> &'static str {
    match kind {
        InlineMark::Bold => "bold",
        InlineMark::Italic => "italic",
        InlineMark::Underline => "underline",
        InlineMark::Strikethrough => "strikethrough",
        InlineMark::Code => "code",
    }
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

fn parse_channel(value: &Value) -> Option<Channel> {
    Some(Channel {
        id: value.get("id")?.as_str()?.to_string(),
        name: value.get("name")?.as_str()?.to_string(),
        archived: value
            .get("archived")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        policy: match value.get("post_policy").and_then(Value::as_str) {
            Some("members_only") => PostPolicy::MembersOnly,
            _ => PostPolicy::Open,
        },
        owner: value
            .get("owner")
            .filter(|owner| !owner.is_null())
            .and_then(Value::as_array)
            .and_then(|bytes| wire_bytes_hex(bytes)),
        huddle: value
            .get("huddle")
            .and_then(Value::as_array)
            .map(|members| {
                members
                    .iter()
                    .filter_map(|member| {
                        Some(HuddleMember {
                            user: wire_bytes_hex(member.get("user")?.as_array()?)?,
                            node: wire_bytes_hex(member.get("node")?.as_array()?)?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_message(
    value: &Value,
    self_key: Option<&str>,
    include_reply: bool,
) -> Option<ChatMessage> {
    let head = value.get("head")?;
    if !include_reply && head.get("thread").is_some_and(|thread| !thread.is_null()) {
        return None;
    }
    let created = head
        .get("created_at")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    Some(ChatMessage {
        sequence: value.get("seq")?.as_u64()?,
        message_id: head.get("message_id")?.as_str()?.to_string(),
        revision: head.get("rev").and_then(Value::as_u64).unwrap_or_default(),
        author: author_name(head.get("author")?),
        body: if head
            .get("deleted")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "Message deleted".into()
        } else {
            blocks_text(head.get("blocks")?)
        },
        time: clock_time(created),
        day: None,
        replies: head
            .get("reply_count")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        reactions: value
            .get("reactions")
            .and_then(Value::as_array)
            .map(|reactions| {
                reactions
                    .iter()
                    .filter_map(|reaction| {
                        let reactors = reaction.get("reactors")?.as_array()?;
                        Some(Reaction {
                            emoji: reaction.get("emoji")?.as_str()?.to_string(),
                            count: reactors.len(),
                            self_reacted: self_key.is_some_and(|self_key| {
                                reactors.iter().any(|author| {
                                    author
                                        .get("user")
                                        .and_then(Value::as_array)
                                        .and_then(|bytes| wire_bytes_hex(bytes))
                                        .is_some_and(|key| key.eq_ignore_ascii_case(self_key))
                                })
                            }),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        author_key: head
            .get("author")
            .and_then(|author| author.get("user"))
            .and_then(Value::as_array)
            .and_then(|bytes| wire_bytes_hex(bytes)),
        edited: head
            .get("edited_at")
            .is_some_and(|edited| !edited.is_null()),
        rich: parse_chat_rich(head.get("blocks")?),
    })
}

fn parse_chat_rich(value: &Value) -> Vec<ChatSpan> {
    let Some(blocks) = value.as_array() else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for (block_index, block) in blocks.iter().enumerate() {
        if block_index > 0 {
            output.push(chat_span("\n", false, false, None));
        }
        if block.as_str() == Some("divider") {
            output.push(chat_span("———", false, false, None));
            continue;
        }
        if let Some(code) = block.get("code") {
            output.push(chat_span(
                code.get("text").and_then(Value::as_str).unwrap_or_default(),
                false,
                false,
                None,
            ));
            continue;
        }
        let (spans, quoted) = if let Some(spans) = block.get("paragraph").and_then(Value::as_array)
        {
            (spans, false)
        } else if let Some(spans) = block.get("quote").and_then(Value::as_array) {
            (spans, true)
        } else {
            continue;
        };
        if quoted {
            output.push(chat_span("> ", false, false, None));
        }
        for span in spans {
            let text = span.get("text").and_then(Value::as_str).unwrap_or_default();
            let marks = span.get("marks").and_then(Value::as_array);
            let bold =
                marks.is_some_and(|marks| marks.iter().any(|mark| mark.as_str() == Some("bold")));
            let italic =
                marks.is_some_and(|marks| marks.iter().any(|mark| mark.as_str() == Some("italic")));
            let link = marks.and_then(|marks| marks.iter().find_map(parse_chat_mark_link));
            if let Some(link) = link {
                output.push(chat_span(text, bold, italic, Some(link)));
            } else {
                output.extend(split_chat_refs(text, bold, italic));
            }
        }
    }
    output
}

fn parse_chat_mark_link(mark: &Value) -> Option<ChatLink> {
    if let Some(link) = mark.get("link").and_then(Value::as_str)
        && matches!(Url::parse(link).ok()?.scheme(), "http" | "https")
    {
        return Some(ChatLink::External(link.to_string()));
    }
    let mention = mark.get("mention")?;
    if let Some(user) = mention
        .get("user")
        .and_then(Value::as_array)
        .and_then(|bytes| wire_bytes_hex(bytes))
    {
        return Some(ChatLink::User(user));
    }
    let agent = mention.get("agent")?;
    Some(ChatLink::Agent {
        module: agent.get("module")?.as_str()?.to_string(),
        id: agent.get("agent_id")?.as_str()?.to_string(),
    })
}

fn split_chat_refs(text: &str, bold: bool, italic: bool) -> Vec<ChatSpan> {
    let mut output = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let embed = open > 0 && rest.as_bytes()[open - 1] == b'!';
        let literal_end = if embed { open - 1 } else { open };
        let after_open = &rest[open + 1..];
        let Some(label_end) = after_open.find("](") else {
            break;
        };
        let url_start = open + 1 + label_end + 2;
        let Some(close) = rest[url_start..].find(')') else {
            break;
        };
        let close = url_start + close;
        let label = &rest[open + 1..open + 1 + label_end];
        let url = &rest[url_start..close];
        let Some(link) = classify_chat_ref(url, label, embed) else {
            let through = open + 1;
            output.extend(split_chat_tags(&rest[..through], bold, italic));
            rest = &rest[through..];
            continue;
        };
        output.extend(split_chat_tags(&rest[..literal_end], bold, italic));
        output.push(chat_span(label, bold, italic, Some(link)));
        rest = &rest[close + 1..];
    }
    output.extend(split_chat_tags(rest, bold, italic));
    output
}

fn classify_chat_ref(url: &str, label: &str, _embed: bool) -> Option<ChatLink> {
    if let Some(id) = url.strip_prefix("duck://page/")
        && safe_ref_segment(id)
    {
        return Some(ChatLink::Page(id.to_string()));
    }
    if let Some(path) = url.strip_prefix("duck://files")
        && safe_attachment_path(path)
    {
        return Some(ChatLink::File {
            path: path.to_string(),
            name: label.to_string(),
        });
    }
    if let Some(value) = url.strip_prefix("duck://channel/") {
        let (id, sequence) = value
            .split_once('#')
            .map_or((value, None), |(id, sequence)| {
                (id, sequence.parse::<u64>().ok())
            });
        if safe_ref_segment(id) {
            return Some(ChatLink::Channel {
                id: id.to_string(),
                sequence,
            });
        }
    }
    if let Some(value) = url.strip_prefix("duck://forge/") {
        let (repository, number) = value
            .rsplit_once('/')
            .map_or((value, None), |(repo, number)| {
                (repo, number.parse::<u64>().ok())
            });
        if safe_ref_segment(repository) {
            return Some(ChatLink::Forge {
                repository: repository.to_string(),
                number,
            });
        }
    }
    None
}

fn split_chat_tags(text: &str, bold: bool, italic: bool) -> Vec<ChatSpan> {
    let mut output = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'#' && (index == 0 || bytes[index - 1].is_ascii_whitespace()) {
            let mut end = index + 1;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'-' | b'_'))
            {
                end += 1;
            }
            if end > index + 1 {
                if start < index {
                    output.push(chat_span(&text[start..index], bold, italic, None));
                }
                output.push(chat_span(
                    &text[index..end],
                    bold,
                    italic,
                    Some(ChatLink::Tag(text[index + 1..end].to_string())),
                ));
                start = end;
                index = end;
                continue;
            }
        }
        index += 1;
    }
    if start < text.len() {
        output.push(chat_span(&text[start..], bold, italic, None));
    }
    output
}

fn chat_span(text: &str, bold: bool, italic: bool, link: Option<ChatLink>) -> ChatSpan {
    ChatSpan {
        text: text.to_string(),
        bold,
        italic,
        link,
    }
}

fn safe_ref_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && !value.contains(['/', '\\', '\0'])
        && value != "."
        && value != ".."
}

fn safe_attachment_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/shared/attachments/") else {
        return false;
    };
    let segments = rest.split('/').collect::<Vec<_>>();
    segments.len() >= 2 && segments.iter().all(|segment| safe_ref_segment(segment))
}

fn parse_page_meta(value: &Value) -> Option<PageMeta> {
    Some(PageMeta {
        id: value.get("id")?.as_str()?.to_string(),
        title: value.get("title")?.as_str()?.to_string(),
        parent: value
            .get("parent")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn parse_page_block(
    value: &Value,
    parents: &std::collections::HashMap<&str, &str>,
) -> Option<PageBlock> {
    let id = value.get("id")?.as_str()?;
    let mut depth = 0;
    let mut cursor = id;
    while let Some(parent) = parents.get(cursor) {
        depth += 1;
        cursor = parent;
        if depth > parents.len() {
            return None;
        }
    }
    Some(PageBlock {
        id: id.to_string(),
        kind: match value.get("kind")?.as_str()? {
            "heading1" => BlockKind::Heading1,
            "heading2" => BlockKind::Heading2,
            "heading3" => BlockKind::Heading3,
            "bulleted" => BlockKind::Bulleted,
            "numbered" => BlockKind::Numbered,
            "todo" => BlockKind::Todo,
            "toggle" => BlockKind::Toggle,
            "quote" => BlockKind::Quote,
            "code" => BlockKind::Code,
            "callout" => BlockKind::Callout,
            "divider" => BlockKind::Divider,
            _ => BlockKind::Paragraph,
        },
        text: value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        depth: depth.saturating_sub(1),
        checked: value
            .get("checked")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        parent: value.get("parent")?.as_str()?.to_string(),
        children: value
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                children
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        marks: value
            .get("marks")
            .and_then(Value::as_array)
            .map(|marks| marks.iter().filter_map(parse_span_mark).collect())
            .unwrap_or_default(),
    })
}

fn parse_span_mark(value: &Value) -> Option<SpanMark> {
    Some(SpanMark {
        start: value.get("start")?.as_u64()?.try_into().ok()?,
        end: value.get("end")?.as_u64()?.try_into().ok()?,
        kind: match value.get("kind")?.as_str()? {
            "bold" => InlineMark::Bold,
            "italic" => InlineMark::Italic,
            "underline" => InlineMark::Underline,
            "strikethrough" => InlineMark::Strikethrough,
            "code" => InlineMark::Code,
            _ => return None,
        },
    })
}

fn parse_page_comments(value: &Value) -> Result<Vec<PageCommentThread>, String> {
    let groups = variant_array(value, "comment_threads")?;
    if groups.len() > 512 {
        return Err("page comments exceed the desktop safety limit".into());
    }
    let mut output = Vec::new();
    for group in groups {
        let target = group
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| "node returned an invalid page comment target".to_string())?;
        let threads = group
            .get("threads")
            .and_then(Value::as_array)
            .ok_or_else(|| "node returned invalid page comment threads".to_string())?;
        for view in threads.iter().take(512usize.saturating_sub(output.len())) {
            let thread = view
                .get("thread")
                .ok_or_else(|| "node returned an invalid page comment thread".to_string())?;
            let comments = view
                .get("comments")
                .and_then(Value::as_array)
                .ok_or_else(|| "node returned invalid page comments".to_string())?;
            output.push(PageCommentThread {
                id: thread
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "node returned an invalid page comment id".to_string())?
                    .to_string(),
                target: target.to_string(),
                anchor: match thread.get("anchor") {
                    Some(Value::Object(anchor)) => {
                        let start = anchor
                            .get("start")
                            .and_then(Value::as_u64)
                            .and_then(|value| value.try_into().ok())
                            .ok_or_else(|| "node returned an invalid comment anchor".to_string())?;
                        let end = anchor
                            .get("end")
                            .and_then(Value::as_u64)
                            .and_then(|value| value.try_into().ok())
                            .ok_or_else(|| "node returned an invalid comment anchor".to_string())?;
                        if start >= end {
                            return Err("node returned an invalid comment anchor".into());
                        }
                        Some(RelativeAnchor { start, end })
                    }
                    Some(Value::Null) | None => None,
                    _ => return Err("node returned an invalid comment anchor".into()),
                },
                resolved: thread
                    .get("resolved")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                comments: comments
                    .iter()
                    .take(512)
                    .map(|comment| {
                        Ok(PageComment {
                            id: comment
                                .get("id")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    "node returned an invalid page comment id".to_string()
                                })?
                                .to_string(),
                            author: author_name(comment.get("author").ok_or_else(|| {
                                "node returned an invalid page comment author".to_string()
                            })?),
                            author_key: comment
                                .get("author")
                                .and_then(|author| author.get("user"))
                                .and_then(Value::as_array)
                                .filter(|bytes| bytes.len() == 32)
                                .and_then(|bytes| wire_bytes_hex(bytes)),
                            text: comment
                                .get("text")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    "node returned invalid page comment text".to_string()
                                })?
                                .to_string(),
                            deleted: comment
                                .get("deleted")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            edited: comment
                                .get("edited_at")
                                .is_some_and(|value| !value.is_null()),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            });
        }
    }
    Ok(output)
}

fn blocks_text(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| {
            if block.as_str() == Some("divider") {
                return Some("———".to_string());
            }
            if let Some(code) = block
                .get("code")
                .and_then(|code| code.get("text"))
                .and_then(Value::as_str)
            {
                return Some(code.to_string());
            }
            for key in ["paragraph", "quote"] {
                if let Some(spans) = block.get(key).and_then(Value::as_array) {
                    let text = spans
                        .iter()
                        .filter_map(|span| span.get("text").and_then(Value::as_str))
                        .collect::<String>();
                    return Some(if key == "quote" {
                        format!("> {text}")
                    } else {
                        text
                    });
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join("\n")
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

fn slug(name: &str) -> String {
    let mut slug = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        fresh_id("channel")
    } else {
        slug.truncate(64);
        slug
    }
}

fn fresh_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}-{nanos:x}")
}

fn clock_time(timestamp: u64) -> String {
    let seconds = timestamp % 86_400;
    format!("{:02}:{:02}", seconds / 3_600, seconds / 60 % 60)
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
    fn chat_wire_is_flattened_without_leaking_module_channels() {
        let channel = parse_channel(&json!({
            "id": "general",
            "name": "General",
            "post_policy": "members_only"
        }))
        .unwrap();
        assert_eq!(channel.policy, PostPolicy::MembersOnly);
        let message = parse_message(
            &json!({
                "seq": 7,
                "reactions": [],
                "head": {
                    "message_id": "m-7",
                    "rev": 0,
                    "author": { "user": [101, 100, 100, 121] },
                    "blocks": [{ "paragraph": [{ "text": "hello", "marks": [] }] }],
                    "created_at": 3723,
                    "deleted": false,
                    "thread": null,
                    "reply_count": 2
                }
            }),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            (message.author.as_str(), message.body.as_str()),
            ("eddy", "hello")
        );
        assert_eq!(message.time, "01:02");
    }

    #[test]
    fn chat_wire_keeps_huddle_and_reaction_identity() {
        let channel = parse_channel(&json!({
            "id": "general",
            "name": "General",
            "huddle": [{ "user": [1, 2], "node": [3, 4] }]
        }))
        .unwrap();
        assert_eq!(channel.huddle[0].user, "0102");
        assert_eq!(channel.huddle[0].node, "0304");

        let message = parse_message(
            &json!({
                "seq": 7,
                "reactions": [{
                    "emoji": "👍",
                    "reactors": [{ "user": [1, 2] }, { "user": [9, 9] }]
                }],
                "head": {
                    "message_id": "m-7",
                    "rev": 1,
                    "author": { "user": [1, 2] },
                    "blocks": [{ "paragraph": [{ "text": "hello", "marks": [] }] }],
                    "created_at": 0,
                    "deleted": false,
                    "thread": null,
                    "reply_count": 0
                }
            }),
            Some("0102"),
            false,
        )
        .unwrap();
        assert_eq!(message.reactions[0].count, 2);
        assert!(message.reactions[0].self_reacted);
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
    fn page_comment_anchor_and_marks_keep_exact_utf16_wire_offsets() {
        let comments = json!({
            "comment_threads": [{
                "target": "b1",
                "threads": [{
                    "thread": {
                        "id": "t1",
                        "anchor": { "start": 2, "end": 5 },
                        "resolved": false
                    },
                    "comments": []
                }]
            }]
        });
        let parsed = parse_page_comments(&comments).unwrap();
        assert_eq!(parsed[0].anchor, Some(RelativeAnchor { start: 2, end: 5 }));
        assert_eq!(
            marks_wire(&[SpanMark {
                start: 1,
                end: 3,
                kind: InlineMark::Bold
            }]),
            vec![json!({ "start": 1, "end": 3, "kind": "bold" })],
        );
        assert_eq!(
            anchor_wire(RelativeAnchor { start: 2, end: 5 }),
            json!({ "start": 2, "end": 5 }),
        );
    }

    #[test]
    fn files_diff_wire_is_flat_and_closed() {
        assert_eq!(
            parse_file_diff(&json!({ "path": "/shared/new", "kind": "added" })).unwrap(),
            ScreenFileDiff {
                path: "/shared/new".into(),
                kind: "added".into(),
            }
        );
        assert!(parse_file_diff(&json!({ "path": "relative", "kind": "added" })).is_err());
        assert!(parse_file_diff(&json!({ "path": "/shared/new", "kind": "renamed" })).is_err());
    }

    #[test]
    fn file_preview_classifier_keeps_binary_content_inert_and_bounded() {
        assert_eq!(
            classify_file_preview(b"hello\nworld".to_vec(), true),
            FilePreviewContent::Text("hello\nworld".into())
        );
        assert_eq!(
            classify_file_preview(b"%PDF-1.7\n".to_vec(), true),
            FilePreviewContent::Pdf
        );
        assert!(matches!(
            classify_file_preview(vec![0, 159, 146, 150], true),
            FilePreviewContent::Unsupported(_)
        ));
        let png = base64::engine::general_purpose::STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        assert!(matches!(
            classify_file_preview(png.clone(), true),
            FilePreviewContent::Image {
                width: 1,
                height: 1,
                ..
            }
        ));
        assert!(matches!(
            classify_file_preview(png, false),
            FilePreviewContent::Unsupported(_)
        ));
    }

    #[test]
    fn every_page_block_kind_matches_the_module_wire() {
        let kinds = [
            (BlockKind::Paragraph, "paragraph"),
            (BlockKind::Heading1, "heading1"),
            (BlockKind::Heading2, "heading2"),
            (BlockKind::Heading3, "heading3"),
            (BlockKind::Bulleted, "bulleted"),
            (BlockKind::Numbered, "numbered"),
            (BlockKind::Todo, "todo"),
            (BlockKind::Toggle, "toggle"),
            (BlockKind::Quote, "quote"),
            (BlockKind::Code, "code"),
            (BlockKind::Callout, "callout"),
            (BlockKind::Divider, "divider"),
        ];
        for (kind, wire) in kinds {
            assert_eq!(block_kind_wire(kind), wire);
        }
    }

    #[test]
    fn page_ancestry_is_root_first_and_rejects_cycles() {
        let pages = vec![
            PageMeta {
                id: "root".into(),
                title: "Root".into(),
                parent: None,
            },
            PageMeta {
                id: "child".into(),
                title: "Child".into(),
                parent: Some("root".into()),
            },
            PageMeta {
                id: "leaf".into(),
                title: "Leaf".into(),
                parent: Some("child".into()),
            },
        ];
        assert_eq!(
            page_ancestry(&pages, "leaf")
                .iter()
                .map(|page| page.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root", "child"]
        );
        assert!(
            page_ancestry(
                &[PageMeta {
                    id: "loop".into(),
                    title: "Loop".into(),
                    parent: Some("loop".into()),
                }],
                "loop"
            )
            .is_empty()
        );
    }

    #[test]
    fn identifiers_are_safe_and_bounded() {
        assert_eq!(slug("  Release Planning  "), "release-planning");
        assert!(slug("한글").starts_with("channel-"));
        assert!(slug(&"x".repeat(100)).len() <= 64);
    }

    #[test]
    fn signed_frames_decode_strictly() {
        assert_eq!(decode_hex("00aF").unwrap(), vec![0, 175]);
        assert!(decode_hex("").is_err());
        assert!(decode_hex("abc").is_err());
        assert!(decode_hex("zz").is_err());
    }

    #[test]
    fn upload_paths_are_normalized_and_cannot_escape_duckfs() {
        assert_eq!(
            upload_path("/shared", "Project/readme.md").unwrap(),
            "/shared/Project/readme.md"
        );
        assert_eq!(upload_path("/", "readme.md").unwrap(), "/readme.md");
        assert!(upload_path("shared", "readme.md").is_err());
        assert!(upload_path("/shared/../private", "readme.md").is_err());
        assert!(validate_relative_path("../private/key").is_err());
        assert!(validate_relative_path("Project\\key").is_err());
        assert!(validate_relative_path("Project/readme.md").is_ok());
    }

    #[test]
    fn dropped_files_and_folders_use_the_picker_validation_path() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("design.txt");
        std::fs::write(&file, b"design").unwrap();
        let file_entries = dropped_entries(file).unwrap();
        assert!(matches!(
            file_entries.as_slice(),
            [UploadEntry::File { relative, size: 6, .. }] if relative == "design.txt"
        ));

        let folder = root.path().join("Project");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("README.md"), b"readme").unwrap();
        let folder_entries = dropped_entries(folder).unwrap();
        assert!(matches!(
            folder_entries.first(),
            Some(UploadEntry::Directory { relative }) if relative == "Project"
        ));
        assert!(matches!(
            folder_entries.get(1),
            Some(UploadEntry::File { relative, .. }) if relative == "Project/README.md"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dropped_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("real.txt");
        let link = root.path().join("link.txt");
        std::fs::write(&file, b"real").unwrap();
        symlink(file, &link).unwrap();
        assert!(
            dropped_entries(link)
                .unwrap_err()
                .contains("symbolic links")
        );
    }

    #[tokio::test]
    async fn drag_out_contract_fails_closed_when_the_host_has_no_platform_adapter() {
        let event = execute(
            None,
            None,
            None,
            Command::BeginFileDragOut {
                path: "/shared/design.svg".into(),
                size: 42,
                snapshot: Some("snapshot-7".into()),
            },
        )
        .await;
        assert!(matches!(
            event,
            ServiceEvent::FileDragOutUnavailable(reason)
                if reason.contains("unavailable") && reason.contains("Download")
        ));
    }

    #[test]
    fn identity_wire_keys_reject_malformed_bytes() {
        assert_eq!(
            wire_bytes_hex(&[json!(0), json!(15), json!(255)]).unwrap(),
            "000fff"
        );
        assert!(wire_bytes_hex(&[]).is_none());
        assert!(wire_bytes_hex(&[json!(256)]).is_none());
        assert!(wire_bytes_hex(&[json!("1")]).is_none());
    }
}
