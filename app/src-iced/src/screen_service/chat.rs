//! Chat wire and platform adapter.

use super::files::UploadEntry;
use super::*;

pub(super) async fn load_chat(
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
        .filter(|channel| !is_module_channel(&channel.id))
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

pub(super) async fn load_channel(
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

pub(super) async fn load_message_window(
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

pub(super) async fn load_thread(
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

pub(super) async fn load_chat_tags(
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

pub(super) async fn filter_chat_tag(
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

pub(super) async fn choose_chat_attachment(
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
    let entry = super::files::file_entry(name.clone(), source)?;
    let size = match &entry {
        UploadEntry::File { size, .. } => *size,
        UploadEntry::Directory { .. } => 0,
    };
    if size > 25 * 1024 * 1024 {
        return Err("attachment exceeds 25 MiB".into());
    }
    let target = format!("/shared/attachments/{}", fresh_id("upload"));
    super::files::upload_entries(backend, client, target.clone(), vec![entry], "attach").await?;
    let path = format!("{target}/{name}");
    Ok(format!(
        "{}[{name}](duck://files{path})",
        if is_image_name(&name) { "!" } else { "" }
    ))
}

pub(super) async fn download_chat_attachment(
    client: Option<&NodeClient>,
    path: &str,
) -> Result<(), String> {
    let client = client.ok_or_else(|| "enter a network to download an attachment".to_string())?;
    let entry = client
        .files_stat(path, None)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "attachment no longer exists".to_string())?;
    if entry.kind != "file" || entry.size > 25 * 1024 * 1024 {
        return Err("attachment is not a downloadable file".into());
    }
    super::files::choose_download(Some(client), path, entry.size, None).await
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

pub(super) async fn create_channel(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    name: String,
    policy: PostPolicy,
) -> Result<(), String> {
    let channel = crate::screens::chat::channel_id(&name);
    chat_write(
        backend,
        client,
        json!({
            "create_channel": {
                "channel_id": channel.clone(),
                "name": name,
                "post_policy": match policy {
                    PostPolicy::Open => "open",
                    PostPolicy::MembersOnly => "members_only",
                }
            }
        }),
    )
    .await?;
    // create_channel seeds NO members, so a members_only channel would lock its
    // own creator out — nobody, not even the creator, could post. Seed the
    // creator's own account key straight after (the same key posts are signed
    // with: identity_state().pubkey is the account key, not the node key).
    if policy == PostPolicy::MembersOnly
        && let Some(backend) = backend
        && let Some(pubkey) = backend.identity_state().await?.pubkey
    {
        let user = user_content_service::user_key_bytes(&pubkey)?;
        chat_write(
            Some(backend),
            client,
            json!({ "set_membership": { "channel_id": channel, "user": user, "member": true } }),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn send_message(
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

pub(super) async fn edit_message(
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

pub(super) fn comment_mentions(text: &str) -> Vec<Value> {
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

pub(super) async fn set_reaction(
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

pub(super) async fn set_membership(
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

pub(super) async fn set_huddle(
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

pub(super) async fn chat_write(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    payload: Value,
) -> Result<(), String> {
    user_content_service::chat_write(backend, client, payload).await
}

pub(super) fn parse_channel(value: &Value) -> Option<Channel> {
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

pub(super) fn parse_message(
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
        day: Some(day_label(created)),
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

/// Hide non-conversational lanes from the rail: module channels carry a ':' and
/// shared-terminal command channels are `term-<16 hex>` (mirrors the original
/// `isModuleChannel` in chat-client.ts).
fn is_module_channel(id: &str) -> bool {
    if id.contains(':') {
        return true;
    }
    id.strip_prefix("term-")
        .is_some_and(|rest| rest.len() == 16 && rest.bytes().all(|byte| byte.is_ascii_hexdigit()))
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
    fn identifiers_are_safe_and_bounded() {
        use crate::screens::chat::channel_id;
        assert_eq!(channel_id("  Release Planning  "), "release-planning");
        // No ASCII alphanumerics ⇒ empty id; the update layer refuses the create
        // rather than minting an unaddressable channel (matches channelIdOf).
        assert!(channel_id("한글").is_empty());
        assert!(channel_id(&"x".repeat(100)).len() <= 64);
    }

    #[test]
    fn module_and_terminal_lanes_are_hidden() {
        assert!(is_module_channel("agent:eddy"));
        assert!(is_module_channel("term-0123456789abcdef"));
        assert!(!is_module_channel("general"));
        assert!(!is_module_channel("term-short"));
    }
}
