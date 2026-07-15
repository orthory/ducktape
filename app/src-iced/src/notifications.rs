//! Persisted in-app notification ring and native desktop presentation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::backend::private_fs;
use crate::transport::{NodeClient, ServerFrame};

pub const RECENT_CAP: usize = 50;
const MAX_STATE_BYTES: u64 = 256 * 1024;
const MAX_TEXT_CHARS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Mention,
    Reply,
    Huddle,
    Run,
    Forge,
    Governance,
}

impl Category {
    pub const fn fallback_screen(self) -> &'static str {
        match self {
            Self::Mention | Self::Reply | Self::Huddle => "chat",
            Self::Run => "agents",
            Self::Forge => "forge",
            Self::Governance => "governance",
        }
    }

    const fn group_label(self) -> &'static str {
        match self {
            Self::Mention => "Mentions",
            Self::Reply => "Replies",
            Self::Huddle => "Huddles",
            Self::Run => "Agent runs",
            Self::Forge => "Forge",
            Self::Governance => "Governance",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub category: Category,
    pub title: String,
    pub body: String,
    pub channel_id: Option<String>,
    pub message_id: Option<String>,
    pub at: u64,
}

impl Item {
    pub fn new(
        category: Category,
        title: impl Into<String>,
        body: impl Into<String>,
        channel_id: Option<String>,
        message_id: Option<String>,
    ) -> Self {
        Self {
            category,
            title: bounded_text(title.into()),
            body: bounded_text(body.into()),
            channel_id: channel_id.map(bounded_text),
            message_id: message_id.map(bounded_text),
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_millis() as u64),
        }
    }

    pub fn target(&self) -> Target {
        Target {
            screen: self.category.fallback_screen().into(),
            channel_id: self.channel_id.clone(),
            message_id: self.message_id.clone(),
        }
    }

    fn valid(&self) -> bool {
        !self.title.is_empty()
            && [&self.title, &self.body]
                .into_iter()
                .all(|value| value.chars().count() <= MAX_TEXT_CHARS)
            && [&self.channel_id, &self.message_id]
                .into_iter()
                .flatten()
                .all(|value| value.chars().count() <= MAX_TEXT_CHARS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub screen: String,
    pub channel_id: Option<String>,
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub key: String,
    pub label: String,
    pub indices: Vec<usize>,
}

impl Group {
    pub fn summary(&self, items: &[Item]) -> String {
        let messages = self.indices.iter().all(|index| {
            items
                .get(*index)
                .is_some_and(|item| matches!(item.category, Category::Mention | Category::Reply))
        });
        format!(
            "{} {}",
            self.indices.len(),
            if messages { "messages" } else { "updates" }
        )
    }
}

/// Stack newest-first items by channel, or by category when channel-less.
/// First-seen insertion order preserves the flat ring's newest ordering.
pub fn groups(items: &[Item], mut channel_name: impl FnMut(&str) -> String) -> Vec<Group> {
    let mut positions = BTreeMap::<String, usize>::new();
    let mut groups = Vec::<Group>::new();
    for (index, item) in items.iter().enumerate() {
        let key = item
            .channel_id
            .clone()
            .unwrap_or_else(|| format!("category:{:?}", item.category));
        if let Some(position) = positions.get(&key).copied() {
            groups[position].indices.push(index);
            continue;
        }
        let label = item.channel_id.as_deref().map_or_else(
            || item.category.group_label().to_owned(),
            |channel| {
                parse_forge_item_channel(channel).map_or_else(
                    || format!("#{}", channel_name(channel)),
                    |(repository, number)| format!("{repository} #{number}"),
                )
            },
        );
        positions.insert(key.clone(), groups.len());
        groups.push(Group {
            key,
            label,
            indices: vec![index],
        });
    }
    groups
}

pub fn parse_forge_item_channel(channel: &str) -> Option<(String, u64)> {
    let value = channel.strip_prefix("forge:")?;
    let (repository, number) = value.rsplit_once(':')?;
    let number = number.parse::<u64>().ok()?;
    (!repository.is_empty() && number > 0).then(|| (repository.to_owned(), number))
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Connected,
    Frame(ServerFrame),
    Disconnected,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub enabled: bool,
    pub mentions: bool,
    pub replies: bool,
    pub huddles: bool,
    pub runs: bool,
    pub forge: bool,
    pub governance: bool,
    pub muted_channels: Vec<String>,
    pub self_user_key: Option<String>,
    pub self_node_keys: Vec<String>,
    pub author_names: BTreeMap<String, String>,
    pub focused_channel: Option<String>,
    pub main_focused: bool,
}

#[derive(Debug, Clone)]
pub struct ReplyCandidate {
    item: Item,
    channel: String,
    root: u64,
    self_user_key: String,
}

#[derive(Debug, Clone)]
pub enum Matched {
    Item(Item),
    Reply(ReplyCandidate),
}

#[derive(Debug, Default)]
pub struct Matcher {
    huddles: BTreeMap<String, BTreeSet<String>>,
}

impl Matcher {
    pub fn handle(&mut self, frame: ServerFrame, config: &Config) -> Option<Matched> {
        let ServerFrame::Event { topic, op, .. } = frame else {
            return None;
        };
        let row = OpRow::decode(&op)?;
        let matched = match topic.as_str() {
            "module:chat" => self.chat(&row, config),
            "module:pages" => pages(&row, config).map(Matched::Item),
            "module:runs" => run(&row).map(Matched::Item),
            "module:forge" => forge(&row).map(Matched::Item),
            "module:governance" => governance(&row).map(Matched::Item),
            _ => None,
        }?;
        let item = match &matched {
            Matched::Item(item) => item,
            Matched::Reply(candidate) => &candidate.item,
        };
        should_present(item, config).then_some(matched)
    }

    fn chat(&mut self, row: &OpRow, config: &Config) -> Option<Matched> {
        let payload = row.payload.as_ref()?;
        if let Some(message) = payload.get("post_message") {
            return chat_message(message, row, config);
        }
        if let Some(join) = payload.get("join_huddle") {
            let channel = join.get("channel_id")?.as_str()?;
            let joiner = row.external_author()?;
            let roster = self.huddles.entry(channel.to_owned()).or_default();
            let was_empty = roster.is_empty();
            roster.insert(joiner.to_ascii_lowercase());
            let node = bytes_hex(join.get("node")?)?;
            if !was_empty || is_me(config, joiner) || is_me(config, &node) {
                return None;
            }
            return Some(Matched::Item(Item::new(
                Category::Huddle,
                format!("Huddle started in #{channel}"),
                format!("{} started a huddle", display_name(config, joiner)),
                Some(channel.into()),
                None,
            )));
        }
        if let Some(leave) = payload.get("leave_huddle") {
            let channel = leave.get("channel_id")?.as_str()?;
            let leaver = row.origin_id.as_deref()?;
            if let Some(roster) = self.huddles.get_mut(channel) {
                roster.remove(&leaver.to_ascii_lowercase());
            }
        } else if let Some(sweep) = payload.get("sweep_huddle") {
            let channel = sweep.get("channel_id")?.as_str()?;
            let user = bytes_hex(sweep.get("user")?)?;
            if let Some(roster) = self.huddles.get_mut(channel) {
                roster.remove(&user);
            }
        }
        None
    }
}

pub fn subscription(origin: String) -> iced::Subscription<StreamEvent> {
    iced::Subscription::run_with(origin, |origin: &String| stream(origin))
}

fn stream(origin: &str) -> impl iced::futures::Stream<Item = StreamEvent> + use<> {
    use iced::futures::SinkExt as _;

    let origin = origin.to_owned();
    iced::stream::channel(64, async move |mut output| {
        let Ok(client) = NodeClient::new(&origin) else {
            let _ = output.send(StreamEvent::Disconnected).await;
            return;
        };
        let topics = ["chat", "pages", "runs", "forge", "governance"]
            .into_iter()
            .map(|topic| format!("module:{topic}"))
            .collect();
        let Ok(mut source) = client.subscribe(topics, BTreeMap::new()) else {
            let _ = output.send(StreamEvent::Disconnected).await;
            return;
        };
        while let Some(event) = source.recv().await {
            let event = match event {
                crate::transport::StreamEvent::Connected => StreamEvent::Connected,
                crate::transport::StreamEvent::Frame(frame) => StreamEvent::Frame(frame),
                crate::transport::StreamEvent::Disconnected(_) => StreamEvent::Disconnected,
            };
            if output.send(event).await.is_err() {
                return;
            }
        }
    })
}

pub async fn resolve_reply(client: NodeClient, candidate: ReplyCandidate) -> Option<Item> {
    let reply = client
        .query(
            "chat",
            serde_json::json!({
                "messages_range": {
                    "channel_id": candidate.channel,
                    "from_seq": candidate.root,
                    "limit": 1
                }
            }),
        )
        .await
        .ok()?;
    let message = reply.get("messages")?.as_array()?.first()?;
    if message.get("seq")?.as_u64()? != candidate.root {
        return None;
    }
    let author = bytes_hex(message.get("head")?.get("author")?.get("user")?)?;
    author
        .eq_ignore_ascii_case(&candidate.self_user_key)
        .then_some(candidate.item)
}

#[derive(Debug)]
struct OpRow {
    origin_kind: String,
    origin_id: Option<String>,
    payload: Option<Value>,
}

impl OpRow {
    fn decode(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        object.get("height")?.as_u64()?;
        u32::try_from(object.get("seq")?.as_u64()?).ok()?;
        object.get("time")?.as_u64()?;
        let origin = object.get("origin")?.as_object()?;
        let origin_kind = origin.get("kind")?.as_str()?;
        if !matches!(origin_kind, "external" | "module" | "system") {
            return None;
        }
        let origin_id = match origin.get("id") {
            Some(id) => Some(id.as_str()?.to_owned()),
            None => None,
        };
        Some(Self {
            origin_kind: origin_kind.into(),
            origin_id,
            payload: object.get("payload").cloned(),
        })
    }

    fn external_author(&self) -> Option<&str> {
        (self.origin_kind == "external")
            .then_some(self.origin_id.as_deref())
            .flatten()
    }
}

fn chat_message(message: &Value, row: &OpRow, config: &Config) -> Option<Matched> {
    let author = row.external_author()?;
    if is_me(config, author) {
        return None;
    }
    let channel = message.get("channel_id")?.as_str()?;
    let message_id = message.get("message_id")?.as_str()?;
    let blocks = message.get("blocks")?.as_array()?;
    let name = display_name(config, author);
    if config.self_user_key.as_deref().is_some_and(|me| {
        mentions(blocks)
            .iter()
            .any(|mentioned| mentioned.eq_ignore_ascii_case(me))
    }) {
        return Some(Matched::Item(Item::new(
            Category::Mention,
            format!("{name} mentioned you in #{channel}"),
            blocks_preview(blocks),
            Some(channel.into()),
            Some(message_id.into()),
        )));
    }
    let root = message.get("thread")?.as_u64()?;
    Some(Matched::Reply(ReplyCandidate {
        item: Item::new(
            Category::Reply,
            format!("{name} replied to your thread in #{channel}"),
            blocks_preview(blocks),
            Some(channel.into()),
            Some(message_id.into()),
        ),
        channel: channel.into(),
        root,
        self_user_key: config.self_user_key.clone()?,
    }))
}

fn pages(row: &OpRow, config: &Config) -> Option<Item> {
    let author = row.external_author()?;
    if is_me(config, author) {
        return None;
    }
    let payload = row.payload.as_ref()?;
    let comment = payload
        .get("add_comment")
        .or_else(|| payload.get("edit_comment"))?;
    let me = config.self_user_key.as_deref()?;
    let mentioned = comment.get("mentions")?.as_array()?.iter().any(|mention| {
        mention
            .get("user")
            .and_then(bytes_hex)
            .is_some_and(|user| user.eq_ignore_ascii_case(me))
    });
    mentioned.then(|| {
        Item::new(
            Category::Mention,
            format!("{} mentioned you in Pages", display_name(config, author)),
            comment.get("text").and_then(Value::as_str).unwrap_or(""),
            None,
            None,
        )
    })
}

fn run(row: &OpRow) -> Option<Item> {
    if row.origin_kind != "module" || row.origin_id.as_deref() != Some("dispatch") {
        return None;
    }
    let payload = row.payload.as_ref()?.as_object()?;
    let dispatch = payload.get("dispatch_id")?.as_str()?;
    let outcome = payload.get("outcome")?.as_object()?;
    if outcome.contains_key("Ok") {
        Some(Item::new(
            Category::Run,
            "Agent run finished",
            format!("dispatch {}…", truncate(dispatch, 12)),
            None,
            None,
        ))
    } else {
        Some(Item::new(
            Category::Run,
            "Agent run failed",
            outcome.get("Err")?.as_str()?,
            None,
            None,
        ))
    }
}

fn forge(row: &OpRow) -> Option<Item> {
    let merged = row.payload.as_ref()?.get("merge_pr")?;
    let repo = match merged.get("repo")?.as_str()? {
        "" => "default",
        repo => repo,
    };
    let number = merged.get("number")?.as_u64()?;
    Some(Item::new(
        Category::Forge,
        format!("PR #{number} merged in {repo}"),
        "",
        None,
        None,
    ))
}

fn governance(row: &OpRow) -> Option<Item> {
    let payload = row.payload.as_ref()?;
    if let Some(proposal) = payload.get("propose") {
        let action = proposal.get("action")?;
        if action.get("add_validator").is_none() && action.get("add_resident").is_none() {
            return None;
        }
        return Some(Item::new(
            Category::Governance,
            "New admission proposal",
            format!("proposal {}", proposal.get("proposal_id")?.as_str()?),
            None,
            None,
        ));
    }
    let joiner = bytes_hex(payload.get("redeem")?.get("joiner")?)?;
    Some(Item::new(
        Category::Governance,
        "New member admitted",
        format!("{} joined via invite", short_hex(&joiner)),
        None,
        None,
    ))
}

fn should_present(item: &Item, config: &Config) -> bool {
    config.enabled
        && match item.category {
            Category::Mention => config.mentions,
            Category::Reply => config.replies,
            Category::Huddle => config.huddles,
            Category::Run => config.runs,
            Category::Forge => config.forge,
            Category::Governance => config.governance,
        }
        && !item
            .channel_id
            .as_ref()
            .is_some_and(|channel| config.muted_channels.iter().any(|muted| muted == channel))
        && !(config.main_focused
            && item
                .channel_id
                .as_ref()
                .is_some_and(|channel| config.focused_channel.as_ref() == Some(channel)))
}

fn is_me(config: &Config, key: &str) -> bool {
    config
        .self_node_keys
        .iter()
        .any(|mine| mine.eq_ignore_ascii_case(key))
}

fn display_name(config: &Config, key: &str) -> String {
    config
        .author_names
        .get(&key.to_ascii_lowercase())
        .cloned()
        .unwrap_or_else(|| short_hex(key))
}

fn mentions(blocks: &[Value]) -> Vec<String> {
    let mut users = Vec::new();
    for block in blocks {
        for spans in [block.get("paragraph"), block.get("quote")]
            .into_iter()
            .flatten()
        {
            let Some(spans) = spans.as_array() else {
                continue;
            };
            for span in spans {
                let Some(marks) = span.get("marks").and_then(Value::as_array) else {
                    continue;
                };
                for mark in marks {
                    if let Some(user) = mark
                        .get("mention")
                        .and_then(|mention| mention.get("user"))
                        .and_then(bytes_hex)
                    {
                        users.push(user);
                    }
                }
            }
        }
    }
    users
}

fn blocks_preview(blocks: &[Value]) -> String {
    let mut text = String::new();
    for block in blocks {
        let mut append = |value: Option<&Value>| {
            if let Some(spans) = value.and_then(Value::as_array) {
                for span in spans {
                    if let Some(value) = span.get("text").and_then(Value::as_str) {
                        text.push_str(value);
                    }
                }
            }
        };
        append(block.get("paragraph"));
        append(block.get("quote"));
        if let Some(code) = block
            .get("code")
            .and_then(|code| code.get("text"))
            .and_then(Value::as_str)
        {
            text.push_str(code);
        }
        text.push(' ');
        if text.chars().count() >= 140 {
            break;
        }
    }
    truncate(text.trim(), 140)
}

fn bytes_hex(value: &Value) -> Option<String> {
    use std::fmt::Write as _;
    let bytes = value.as_array()?;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hex, "{:02x}", u8::try_from(byte.as_u64()?).ok()?).ok()?;
    }
    Some(hex)
}

fn short_hex(value: &str) -> String {
    if value.len() <= 12 {
        value.into()
    } else {
        format!("{}…{}", &value[..6], &value[value.len() - 4..])
    }
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Stored {
    unread: u32,
    recent: Vec<Item>,
}

#[derive(Debug, Default)]
pub struct State {
    pub unread: u32,
    pub recent: Vec<Item>,
    pub open: bool,
    pub expanded: Option<String>,
    path: Option<PathBuf>,
}

impl State {
    pub fn load_default() -> Self {
        let path = state_path();
        let stored = path.as_deref().map(load).unwrap_or_default();
        Self {
            unread: stored.unread,
            recent: stored.recent,
            open: false,
            expanded: None,
            path,
        }
    }

    pub fn push(&mut self, item: Item) {
        if !item.valid() {
            tracing::warn!(
                target: "ducktape::notify",
                reason = "invalid_item",
                "discarded an invalid desktop notification"
            );
            return;
        }
        self.recent.insert(0, item);
        self.recent.truncate(RECENT_CAP);
        self.unread = self.unread.saturating_add(1);
        self.save();
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.expanded = None;
        if self.open && self.unread != 0 {
            self.unread = 0;
            self.save();
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle_group(&mut self, key: String) {
        self.expanded = (self.expanded.as_deref() != Some(&key)).then_some(key);
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Err(error) = save(
            path,
            &Stored {
                unread: self.unread,
                recent: self.recent.clone(),
            },
        ) {
            tracing::warn!(
                target: "ducktape::notify",
                reason = "state_write_failed",
                error = %error,
                "could not persist notification state"
            );
        }
    }
}

pub async fn present(item: Item) -> Option<Target> {
    let target = item.target();
    tokio::task::spawn_blocking(move || present_blocking(&item).then_some(target))
        .await
        .ok()
        .flatten()
}

fn present_blocking(item: &Item) -> bool {
    let mut clicked = false;
    let shown = notify_rust::Notification::new()
        .appname("Ducktape")
        .summary(&item.title)
        .body(&item.body)
        .action("default", "Open Ducktape")
        .show();
    match shown {
        Ok(handle) => handle.wait_for_action(|action| {
            clicked = action == "default";
        }),
        Err(error) => tracing::warn!(
            target: "ducktape::notify",
            reason = "native_present_failed",
            error = %error,
            "could not show native notification"
        ),
    }
    clicked
}

fn state_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape/notify/state.json"))
}

fn load(path: &Path) -> Stored {
    let Ok(Some(file)) = private_fs::open_private_read(path) else {
        return Stored::default();
    };
    if file
        .metadata()
        .map_or(true, |metadata| metadata.len() > MAX_STATE_BYTES)
    {
        return Stored::default();
    }
    let mut bytes = Vec::new();
    if file
        .take(MAX_STATE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_STATE_BYTES
    {
        return Stored::default();
    }
    let Ok(mut stored) = serde_json::from_slice::<Stored>(&bytes) else {
        return Stored::default();
    };
    stored.recent.retain(Item::valid);
    stored.recent.truncate(RECENT_CAP);
    stored
}

fn save(path: &Path, stored: &Stored) -> Result<(), String> {
    let bytes = serde_json::to_vec(stored).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err("notification state exceeds its size limit".into());
    }
    private_fs::write_atomic(path, &bytes)
}

fn bounded_text(value: String) -> String {
    value.chars().take(MAX_TEXT_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn frame(topic: &str, payload: Value) -> ServerFrame {
        ServerFrame::Event {
            topic: topic.into(),
            cursor: "1:0".into(),
            op: serde_json::json!({
                "height": 1,
                "seq": 0,
                "time": 1,
                "origin": { "kind": "external", "id": "aabb" },
                "payload": payload,
            }),
        }
    }

    #[test]
    fn ring_is_bounded_and_open_marks_seen() {
        let mut state = State::default();
        for index in 0..60 {
            state.push(Item::new(
                Category::Mention,
                format!("message {index}"),
                "body",
                Some("general".into()),
                None,
            ));
        }
        assert_eq!(state.recent.len(), RECENT_CAP);
        assert_eq!(state.unread, 60);
        state.toggle();
        assert!(state.open);
        assert_eq!(state.unread, 0);
    }

    #[test]
    fn groups_stack_channels_and_route_hidden_forge_items() {
        let items = vec![
            Item::new(
                Category::Mention,
                "new",
                "body",
                Some("general".into()),
                None,
            ),
            Item::new(Category::Reply, "old", "body", Some("general".into()), None),
            Item::new(Category::Run, "run", "body", None, None),
            Item::new(
                Category::Forge,
                "review",
                "body",
                Some("forge:team:repo:17".into()),
                None,
            ),
        ];
        let grouped = groups(&items, |channel| {
            if channel == "general" {
                "General"
            } else {
                channel
            }
            .to_owned()
        });
        assert_eq!(grouped[0].label, "#General");
        assert_eq!(grouped[0].indices, vec![0, 1]);
        assert_eq!(grouped[0].summary(&items), "2 messages");
        assert_eq!(grouped[1].label, "Agent runs");
        assert_eq!(grouped[2].label, "team:repo #17");
        assert_eq!(
            parse_forge_item_channel("forge:team:repo:17"),
            Some(("team:repo".into(), 17))
        );
        assert_eq!(parse_forge_item_channel("forge::0"), None);
    }

    #[test]
    fn malformed_or_oversized_state_fails_closed() {
        let root = std::env::temp_dir().join(format!(
            "ducktape-notify-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("state.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&path, b"not json").unwrap();
        assert!(load(&path).recent.is_empty());
        fs::write(&path, vec![b'x'; MAX_STATE_BYTES as usize + 1]).unwrap();
        assert!(load(&path).recent.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn persisted_notification_content_is_user_private() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = std::env::temp_dir().join(format!(
            "ducktape-notify-private-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("notify/state.json");
        save(
            &path,
            &Stored {
                unread: 1,
                recent: vec![Item::new(Category::Mention, "private", "body", None, None)],
            },
        )
        .unwrap();
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn matcher_filters_mentions_by_identity_focus_and_mutes() {
        let event = frame(
            "module:chat",
            serde_json::json!({
                "post_message": {
                    "channel_id": "general",
                    "message_id": "m1",
                    "blocks": [{"paragraph": [{"text": "hello", "marks": [{"mention": {"user": [1, 2]}}]}]}],
                    "thread": null
                }
            }),
        );
        let mut matcher = Matcher::default();
        let mut config = Config {
            enabled: true,
            mentions: true,
            self_user_key: Some("0102".into()),
            ..Config::default()
        };
        assert!(matches!(
            matcher.handle(event.clone(), &config),
            Some(Matched::Item(Item {
                category: Category::Mention,
                ..
            }))
        ));
        config.main_focused = true;
        config.focused_channel = Some("general".into());
        assert!(matcher.handle(event.clone(), &config).is_none());
        config.main_focused = false;
        config.muted_channels.push("general".into());
        assert!(matcher.handle(event, &config).is_none());
    }

    #[test]
    fn huddle_notifies_only_for_the_first_remote_join() {
        let event = frame(
            "module:chat",
            serde_json::json!({"join_huddle": {"channel_id": "general", "node": [3, 4]}}),
        );
        let config = Config {
            enabled: true,
            huddles: true,
            ..Config::default()
        };
        let mut matcher = Matcher::default();
        assert!(matches!(
            matcher.handle(event.clone(), &config),
            Some(Matched::Item(Item {
                category: Category::Huddle,
                ..
            }))
        ));
        assert!(matcher.handle(event, &config).is_none());
    }
}
