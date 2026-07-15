//! Native global search palette.
//!
//! Chat and page hits come from the node's bounded derived-index views. Member
//! and file hits are filtered from the shell's already-loaded projections so a
//! palette query never creates a second source of truth.

use iced::widget::{
    Id, Space, button, column, container, operation, row, scrollable, stack, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::icons::{self, Icon};
use crate::theme::{self, Mode};
use crate::transport::NodeClient;

const RESULT_CAP: usize = 8;
const FILE_CATALOG_CAP: usize = 4_096;
const FILE_PAGE_SIZE: usize = 256;
const SEARCH_DELAY: std::time::Duration = std::time::Duration::from_millis(180);
const INPUT_ID: &str = "global-search-input";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberHit {
    pub account_id: Option<String>,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHit {
    pub path: String,
    pub name: String,
    pub directory: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    pub members: Vec<MemberHit>,
    pub files: Vec<FileHit>,
    pub client_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatHit {
    pub channel_id: String,
    pub seq: u64,
    pub message_id: String,
    pub author: String,
    pub height: u64,
    pub time: u64,
    pub text: String,
    pub deleted: bool,
    pub edited: bool,
    #[serde(default)]
    pub thread: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageHit {
    pub block_id: String,
    pub page_id: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub kind: String,
    pub text: String,
    pub height: u64,
    pub time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Results {
    pub chat: Vec<ChatHit>,
    pub pages: Vec<PageHit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Chat {
        channel_id: String,
        sequence: u64,
    },
    Page {
        page_id: String,
        block_id: String,
    },
    Member {
        account_id: Option<String>,
        key: String,
    },
    File {
        path: String,
        directory: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Open(Catalog),
    Close,
    QueryChanged(String),
    SearchFinished {
        generation: u64,
        result: Result<Results, String>,
    },
    FilesLoaded(Result<Vec<FileHit>, String>),
    MembersLoaded(Vec<MemberHit>),
    Select(Target),
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Focus,
    Search { generation: u64, query: String },
    Selected(Target),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub open: bool,
    pub query: String,
    pub catalog: Catalog,
    pub results: Results,
    pub searching: bool,
    pub error: Option<String>,
    generation: u64,
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Open(catalog) => {
            state.open = true;
            state.query.clear();
            state.catalog = catalog;
            state.results = Results::default();
            state.searching = false;
            state.error = None;
            state.generation = state.generation.wrapping_add(1);
            Some(Command::Focus)
        }
        Message::Close => {
            state.open = false;
            state.query.clear();
            state.results = Results::default();
            state.searching = false;
            state.error = None;
            state.generation = state.generation.wrapping_add(1);
            None
        }
        Message::QueryChanged(value) => {
            state.query = value;
            state.results = Results::default();
            state.error = None;
            state.generation = state.generation.wrapping_add(1);
            let query = state.query.trim().to_owned();
            state.searching = !query.is_empty();
            (!query.is_empty()).then_some(Command::Search {
                generation: state.generation,
                query,
            })
        }
        Message::SearchFinished { generation, result } if generation == state.generation => {
            state.searching = false;
            match result {
                Ok(results) => state.results = results,
                Err(error) => state.error = Some(error),
            }
            None
        }
        Message::SearchFinished { .. } => None,
        Message::FilesLoaded(Ok(files)) => {
            state.catalog.files = files;
            None
        }
        Message::FilesLoaded(Err(error)) => {
            state.error = Some(error);
            None
        }
        Message::MembersLoaded(members) => {
            state.catalog.members = members;
            None
        }
        Message::Select(target) => {
            state.open = false;
            Some(Command::Selected(target))
        }
        Message::Ignore => None,
    }
}

pub fn focus<Message>() -> iced::Task<Message> {
    operation::focus(Id::new(INPUT_ID))
}

pub async fn search(client: Option<NodeClient>, query: String) -> Result<Results, String> {
    tokio::time::sleep(SEARCH_DELAY).await;
    let Some(client) = client else {
        return Ok(Results::default());
    };
    let chat = client.view(
        "chat",
        json!({ "search": { "text": query, "limit": RESULT_CAP } }),
    );
    let pages = client.view(
        "pages",
        json!({ "search": { "text": query, "limit": RESULT_CAP } }),
    );
    let (chat, pages) = tokio::join!(chat, pages);
    // Indexes are deliberately independent. A node being upgraded may expose
    // one before the other; the available group still remains useful.
    let chat = chat
        .ok()
        .and_then(|reply| decode_hits::<ChatHit>(&reply).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|hit| !hit.channel_id.contains(':'))
        .take(RESULT_CAP)
        .collect();
    let pages = pages
        .ok()
        .and_then(|reply| decode_hits::<PageHit>(&reply).ok())
        .unwrap_or_default()
        .into_iter()
        .take(RESULT_CAP)
        .collect();
    Ok(Results { chat, pages })
}

pub async fn load_files(client: Option<NodeClient>) -> Result<Vec<FileHit>, String> {
    let Some(client) = client else {
        return Ok(Vec::new());
    };
    let mut files = Vec::new();
    let mut after: Option<String> = None;
    // The entry cap alone cannot bound this loop: empty pages carrying a
    // cycling `next` cursor (A,B,A,B,…) advance the cursor every iteration
    // while `files` stays empty, so a buggy/malicious node could spin it
    // forever. Cap the page count too — a full legitimate catalog needs at
    // most FILE_CATALOG_CAP / FILE_PAGE_SIZE pages plus a little slack.
    let mut pages_seen = 0usize;
    loop {
        let reply = client
            .query(
                "files",
                json!({
                    "find": {
                        "prefix": "/",
                        "snapshot": null,
                        "after": after.as_deref(),
                        "limit": FILE_PAGE_SIZE,
                    }
                }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let page = decode_file_page(&reply)?;
        files.extend(page.entries);
        if files.len() > FILE_CATALOG_CAP {
            return Err("file catalog exceeds the desktop search limit".into());
        }
        pages_seen += 1;
        if pages_seen > FILE_CATALOG_CAP / FILE_PAGE_SIZE + 4 {
            return Err("file catalog paginated past its limit".into());
        }
        match page.next {
            Some(next) if after.as_ref() != Some(&next) => after = Some(next),
            Some(_) => return Err("file catalog cursor did not advance".into()),
            None => break,
        }
    }
    Ok(files)
}

struct FilePage {
    entries: Vec<FileHit>,
    next: Option<String>,
}

fn decode_file_page(reply: &Value) -> Result<FilePage, String> {
    let page = reply
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("find"))
        .and_then(Value::as_object)
        .ok_or_else(|| "file catalog reply is invalid".to_string())?;
    let rows = page
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| "file catalog entries are invalid".to_string())?;
    if rows.len() > FILE_PAGE_SIZE {
        return Err("file catalog page exceeds its limit".into());
    }
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let object = row
            .as_object()
            .ok_or_else(|| "file catalog entry is invalid".to_string())?;
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| safe_file_path(path))
            .ok_or_else(|| "file catalog path is invalid".to_string())?;
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .filter(|kind| matches!(*kind, "dir" | "file" | "symlink"))
            .ok_or_else(|| "file catalog kind is invalid".to_string())?;
        entries.push(FileHit {
            path: path.to_owned(),
            name: path
                .rsplit('/')
                .find(|part| !part.is_empty())
                .unwrap_or("/")
                .to_owned(),
            directory: kind == "dir",
        });
    }
    let next = match page.get("next") {
        None | Some(Value::Null) => None,
        Some(Value::String(next)) if safe_file_path(next) => Some(next.clone()),
        Some(_) => return Err("file catalog cursor is invalid".into()),
    };
    Ok(FilePage { entries, next })
}

fn safe_file_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 4 * 1024
        && !path.chars().any(char::is_control)
        && !path.split('/').any(|segment| matches!(segment, "." | ".."))
}

fn decode_hits<T: for<'de> Deserialize<'de>>(reply: &Value) -> Result<Vec<T>, String> {
    let hits = reply
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("hits"))
        .ok_or_else(|| "search index reply is missing hits".to_string())?;
    let rows = hits
        .as_array()
        .ok_or_else(|| "search index hits are not an array".to_string())?;
    if rows.len() > 4_096 {
        return Err("search index returned too many hits".into());
    }
    rows.iter()
        .take(RESULT_CAP)
        .cloned()
        .map(|row| {
            serde_json::from_value(row).map_err(|_| "search index hit is invalid".to_string())
        })
        .collect()
}

pub fn view(state: &State, mode: Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let query = state.query.trim().to_ascii_lowercase();
    let members = (!query.is_empty() && !state.catalog.client_mode)
        .then(|| {
            state
                .catalog
                .members
                .iter()
                .filter(|member| {
                    member.name.to_ascii_lowercase().contains(&query)
                        || member.key.to_ascii_lowercase().contains(&query)
                })
                .take(RESULT_CAP)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let files = (!query.is_empty())
        .then(|| {
            state
                .catalog
                .files
                .iter()
                .filter(|file| file.path.to_ascii_lowercase().contains(&query))
                .take(RESULT_CAP)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut groups = column![].spacing(14);
    if state.query.trim().is_empty() {
        groups = groups.push(hint(
            if state.catalog.client_mode {
                "Type to search chat, pages, and files."
            } else {
                "Type to search chat, pages, members, and files."
            }
            .into(),
            p,
        ));
    } else {
        if !state.results.chat.is_empty() {
            let mut group = result_group("Chat", state.results.chat.len(), p);
            for hit in &state.results.chat {
                let edited = if hit.edited { " · edited" } else { "" };
                group = group.push(hit_button(
                    format!("#{} · {}{edited}", hit.channel_id, hit.author),
                    hit.text.clone(),
                    Target::Chat {
                        channel_id: hit.channel_id.clone(),
                        sequence: hit.seq,
                    },
                    p,
                ));
            }
            groups = groups.push(group);
        }
        if !state.results.pages.is_empty() {
            let mut group = result_group("Pages", state.results.pages.len(), p);
            for hit in &state.results.pages {
                group = group.push(hit_button(
                    format!("{} · {}", hit.page_id, hit.kind),
                    hit.text.clone(),
                    Target::Page {
                        page_id: hit.page_id.clone(),
                        block_id: hit.block_id.clone(),
                    },
                    p,
                ));
            }
            groups = groups.push(group);
        }
        if !members.is_empty() {
            let mut group = result_group("Members", members.len(), p);
            for hit in &members {
                group = group.push(hit_button(
                    short_key(&hit.key),
                    hit.name.clone(),
                    Target::Member {
                        account_id: hit.account_id.clone(),
                        key: hit.key.clone(),
                    },
                    p,
                ));
            }
            groups = groups.push(group);
        }
        if !files.is_empty() {
            let mut group = result_group("Files", files.len(), p);
            for hit in &files {
                group = group.push(hit_button(
                    hit.path.clone(),
                    hit.name.clone(),
                    Target::File {
                        path: hit.path.clone(),
                        directory: hit.directory,
                    },
                    p,
                ));
            }
            groups = groups.push(group);
        }
        let total =
            state.results.chat.len() + state.results.pages.len() + members.len() + files.len();
        if state.searching && state.results.chat.is_empty() && state.results.pages.is_empty() {
            groups = groups.push(hint("Searching…".into(), p));
        } else if !state.searching && total == 0 {
            groups = groups.push(hint(
                format!("Nothing matches “{}”.", state.query.trim()),
                p,
            ));
        }
        if let Some(error) = &state.error {
            groups = groups.push(text(error).size(11).color(p.danger));
        }
    }

    let input = row![
        icons::view(Icon::Search, 18.0, p.muted_2),
        text_input(
            if state.catalog.client_mode {
                "Search chat, pages, files…"
            } else {
                "Search chat, pages, members, files…"
            },
            &state.query,
        )
        .id(Id::new(INPUT_ID))
        .on_input(Message::QueryChanged)
        .padding(0)
        .size(15)
        .style(move |_, status| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: p.muted_2,
            placeholder: p.muted_2,
            value: p.ink,
            selection: if matches!(status, text_input::Status::Focused { .. }) {
                p.hover
            } else {
                p.hover
            },
        }),
        container(text("ESC").size(10).font(theme::MONO).color(p.muted_2))
            .padding([2, 6])
            .style(move |_| bordered(p.paper, p.border_soft, 4.0)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let panel = container(column![
        container(input)
            .padding([12, 16])
            .width(Length::Fill)
            .style(move |_| bottom_border(p.paper, p.border_soft)),
        scrollable(container(groups).padding(Padding {
            top: 8.0,
            right: 8.0,
            bottom: 12.0,
            left: 8.0,
        }))
        .height(Length::Shrink),
    ])
    .width(Length::Fixed(640.0))
    .max_width(640.0)
    .max_height(544.0)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.paper)),
        border: Border {
            color: p.border_strong,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.18,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..container::Style::default()
    });

    let dismiss = button(Space::new().width(Length::Fill).height(Length::Fill))
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_press(Message::Close)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(Color {
                a: 0.32,
                ..p.filled
            })),
            border: Border::default(),
            ..button::Style::default()
        });
    let guarded_panel = button(panel)
        .padding(0)
        .on_press(Message::Ignore)
        .style(|_, _| button::Style::default());
    stack![
        dismiss,
        container(column![
            Space::new().height(Length::FillPortion(12)),
            guarded_panel,
            Space::new().height(Length::FillPortion(20))
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn result_group<'a>(
    title: &'a str,
    count: usize,
    p: theme::Palette,
) -> iced::widget::Column<'a, Message> {
    column![
        text(format!("{title} · {count}").to_ascii_uppercase())
            .size(10)
            .font(theme::SANS_SEMIBOLD)
            .color(p.muted_2)
    ]
    .padding(Padding {
        top: 0.0,
        right: 2.0,
        bottom: 0.0,
        left: 10.0,
    })
    .spacing(2)
}

fn hit_button(
    meta: String,
    body: String,
    target: Target,
    p: theme::Palette,
) -> Element<'static, Message> {
    button(
        column![
            text(meta).size(10.5).color(p.muted_2),
            text(body).size(13).color(p.ink),
        ]
        .spacing(2),
    )
    .padding([8, 10])
    .width(Length::Fill)
    .on_press(Message::Select(target))
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(p.sunken)),
        text_color: p.ink,
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .into()
}

fn hint(label: String, p: theme::Palette) -> Element<'static, Message> {
    container(text(label).size(12.5).color(p.muted_2))
        .padding([4, 10])
        .into()
}

fn short_key(key: &str) -> String {
    if key.chars().count() <= 16 {
        key.to_owned()
    } else {
        let start = key.chars().take(8).collect::<String>();
        let end = key.chars().rev().take(6).collect::<Vec<_>>();
        format!("{start}…{}", end.into_iter().rev().collect::<String>())
    }
}

fn bordered(background: Color, border: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        },
        ..container::Style::default()
    }
}

fn bottom_border(background: Color, border: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 0.0,
            radius: 0.0.into(),
        },
        shadow: Shadow {
            color: border,
            offset: Vector::new(0.0, 1.0),
            blur_radius: 0.0,
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_search_results_do_not_replace_the_current_query() {
        let mut state = State::default();
        update(&mut state, Message::Open(Catalog::default()));
        let first = match update(&mut state, Message::QueryChanged("one".into())) {
            Some(Command::Search { generation, .. }) => generation,
            command => panic!("unexpected command: {command:?}"),
        };
        let second = match update(&mut state, Message::QueryChanged("two".into())) {
            Some(Command::Search { generation, .. }) => generation,
            command => panic!("unexpected command: {command:?}"),
        };
        update(
            &mut state,
            Message::SearchFinished {
                generation: first,
                result: Ok(Results {
                    chat: vec![chat_hit("old")],
                    pages: vec![],
                }),
            },
        );
        assert!(state.results.chat.is_empty());
        update(
            &mut state,
            Message::SearchFinished {
                generation: second,
                result: Ok(Results {
                    chat: vec![chat_hit("new")],
                    pages: vec![],
                }),
            },
        );
        assert_eq!(state.results.chat[0].text, "new");
    }

    #[test]
    fn strict_decoder_rejects_extra_fields_and_caps_results() {
        let malformed = json!({ "hits": [{
            "channelId": "general", "seq": 1, "messageId": "m1",
            "author": "user:a", "height": 1, "time": 1, "text": "x",
            "deleted": false, "edited": false, "surprise": true
        }] });
        assert!(decode_hits::<ChatHit>(&malformed).is_err());

        let rows = (0..20)
            .map(|index| serde_json::to_value(chat_hit(&index.to_string())).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            decode_hits::<ChatHit>(&json!({ "hits": rows }))
                .unwrap()
                .len(),
            RESULT_CAP
        );
    }

    #[test]
    fn module_channels_never_become_chat_targets() {
        let rows = vec![
            chat_hit("normal"),
            ChatHit {
                channel_id: "forge:repo:1".into(),
                ..chat_hit("hidden")
            },
        ];
        let visible = rows
            .into_iter()
            .filter(|hit| !hit.channel_id.contains(':'))
            .collect::<Vec<_>>();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].text, "normal");
    }

    #[test]
    fn file_catalog_pages_are_bounded_and_paths_cannot_escape() {
        let page = decode_file_page(&json!({
            "find": {
                "entries": [{ "path": "/shared/notes.txt", "kind": "file" }],
                "next": "/shared/notes.txt"
            }
        }))
        .unwrap();
        assert_eq!(page.entries[0].name, "notes.txt");
        assert!(!page.entries[0].directory);
        assert_eq!(page.next.as_deref(), Some("/shared/notes.txt"));
        assert!(!safe_file_path("/shared/../secret"));
        assert!(!safe_file_path("relative"));
    }

    fn chat_hit(text: &str) -> ChatHit {
        ChatHit {
            channel_id: "general".into(),
            seq: 1,
            message_id: "m1".into(),
            author: "user:a".into(),
            height: 1,
            time: 1,
            text: text.into(),
            deleted: false,
            edited: false,
            thread: None,
            tags: vec![],
        }
    }
}
