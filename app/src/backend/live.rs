use super::*;
use ::chat;
use ::forge;

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
                        vec![
                            "chat".to_string(),
                            "pages".to_string(),
                            "inbox".to_string(),
                            "forge".to_string(),
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
                    Some(Ok(ModuleEvent::Lagged { module, cursor })) => {
                        state.cursors.insert(format!("module:{module}"), cursor);
                        live_resync(&module, -1)
                    }
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
async fn folded_update(rpc: &str, module: &str, op: ducktape_rpc::StreamOp) -> Option<LiveUpdate> {
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
            Ok(delta) => Some(LiveUpdate {
                kind: "pages".into(),
                status: format!("Live · block {height}"),
                height,
                module: "pages".into(),
                load_chat: false,
                load_pages: true,
                debounce: true,
                chat: ChatDelta::default(),
                pages: delta,
                bell: BellDelta::default(),
                forge: ForgeRefresh::default(),
            }),
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

/// One channel's row rebuilt from its canonical record (the dispatch-grade
/// point read) — the huddle roster length is not derivable from the op.
pub(crate) async fn load_channel_row(rpc: &str, channel_id: &str) -> Result<ChatChannel, String> {
    let rpc = rpc_client(rpc)?;
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::Channel {
                channel_id: channel_id.to_string(),
            },
        )
        .await?;
    let ChatReply::Channel(Some(channel)) = reply else {
        return Err("channel record was not found".into());
    };
    Ok(ChatChannel {
        id: channel.id,
        name: channel.name,
        archived: channel.archived,
        members_only: channel.post_policy == PostPolicy::MembersOnly,
        huddle_count: count_i64(channel.huddle.len()),
        head_seq: number_i64(channel.head_seq),
    })
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
        };
        let load_chat = planes == "chat" || planes == "both";
        let load_pages = planes == "pages" || planes == "both";
        if load_chat {
            let chat = load_chat_data(
                &rpc,
                (!channel_id.is_empty()).then_some(channel_id.as_str()),
            )
            .await?;
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
        if load_pages {
            let pages =
                load_pages_data(&rpc, (!page_id.is_empty()).then_some(page_id.as_str())).await?;
            refresh.pages_loaded = true;
            refresh.pages = pages.pages;
            refresh.blocks = pages.blocks;
            refresh.active_page = pages.active_page;
            refresh.active_page_title = pages.active_page_title;
            refresh.active_page_parent = pages.active_page_parent;
        }
        Ok(refresh)
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
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

pub fn keep_messages(
    loaded: bool,
    next: Vec<ChatMessage>,
    current: Vec<ChatMessage>,
) -> Vec<ChatMessage> {
    if loaded { next } else { current }
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

/// [`keep_roster`]'s loaded-gated half: a resync that did NOT load chat must
/// leave the roster alone rather than blank it.
pub fn keep_participants(
    loaded: bool,
    next: Vec<HuddleParticipant>,
    current: Vec<HuddleParticipant>,
) -> Vec<HuddleParticipant> {
    if loaded { next } else { current }
}

pub fn keep_pages(loaded: bool, next: Vec<PageItem>, current: Vec<PageItem>) -> Vec<PageItem> {
    if loaded { next } else { current }
}

pub fn keep_blocks(loaded: bool, next: Vec<PageBlock>, current: Vec<PageBlock>) -> Vec<PageBlock> {
    if loaded { next } else { current }
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
    .map_err(app_error)
}

pub async fn create_channel(
    rpc: String,
    password: String,
    name: String,
    members_only: bool,
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
                post_policy: match members_only {
                    true => PostPolicy::MembersOnly,
                    false => PostPolicy::Open,
                },
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

/// One peer of the DM directory. There is no `status`: presence has no source
/// anywhere in the product, and a dot that always reads "offline" is a lie.
///
/// `is_agent` is always false today — see [`load_dm_peers`].
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct DmPeer {
    pub key: String,
    pub name: String,
    pub initials: String,
    pub is_agent: bool,
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
            peers.push(DmPeer {
                initials: initials_of(&name),
                is_agent: false,
                key,
                name,
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

/// The channel list MINUS this viewer's DMs. A DM *is* an ordinary chat
/// channel, so it arrives in the same listing as the rooms and used to appear
/// twice over — once under CHANNELS wearing its derived id as a name, and once
/// under DIRECT wearing the peer's. The id is DERIVED, so this needs no
/// per-channel membership (which the list projection does not carry): a
/// channel is this viewer's DM exactly when its id is `dm_channel_id(me, peer)`
/// for some peer in the directory. A user-created channel cannot fake the id —
/// the module's namespace rule reserves it for the pair's own keys.
pub fn rooms_only(channels: Vec<ChatChannel>, peers: Vec<DmPeer>, me: String) -> Vec<ChatChannel> {
    if me.is_empty() {
        return channels;
    }
    let dm_ids: BTreeSet<String> = peers
        .iter()
        .map(|peer| dm_channel_id(me.clone(), peer.key.clone()))
        .collect();
    channels
        .into_iter()
        .filter(|channel| !dm_ids.contains(&channel.id))
        .collect()
}

/// Open the DM with one peer: resolve the deterministic channel when it
/// exists, else create it members-only and seat both keys, then load it.
///
/// NOT confidential. `MembersOnly` gates who may POST; every node replicates
/// the channel's plaintext, so a DM is a two-person room, not a private one.
/// Any copy on this surface that promises secrecy is a lie about the wire.
pub async fn open_dm(
    rpc: String,
    password: String,
    peer_key: String,
) -> Result<ChatData, AppError> {
    async {
        let peer = public_key(&peer_key, "peer public key")?;
        let me = local_user_key()
            .await
            .ok_or_else(|| "this device has no user key — a DM needs one".to_string())?;
        let channel_id = dm_channel_id(hex_encode(&me), hex_encode(&peer));
        let client = rpc_client(&rpc)?;
        let existing = load_chat_data(&client, Some(&channel_id)).await?;
        if existing.active_channel == channel_id {
            return Ok(existing);
        }
        signed_write(
            &client,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel_id.clone(),
                name: short_label(&hex_encode(&peer)),
                post_policy: PostPolicy::MembersOnly,
            }),
            password.clone(),
        )
        .await?;
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
            .await
            .map_err(committed_error)?;
        }
        load_chat_data(&client, Some(&channel_id))
            .await
            .map_err(committed_error)
    }
    .await
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
