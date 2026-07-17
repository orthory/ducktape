//! Capability-free Files column browser.
//!
//! Native filesystem access, dialogs, downloads, and platform drag APIs stay in
//! the host. This module only owns view state and emits typed effects.

use std::collections::BTreeMap;

use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, image, row, scrollable, stack, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{
    self, BODY, CAPTION, HEADING, LABEL, MONO, Palette, RADIUS_LG, RADIUS_SM, SANS, SANS_SEMIBOLD,
    TITLE,
};
use crate::ui;
use crate::view_api::{DropToken, Resource};

const COLUMN_WIDTH: f32 = 286.0;
const PREVIEW_WIDTH: f32 = 380.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub executable: bool,
    pub object: String,
    pub meta: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePreview {
    pub path: String,
    pub content: FilePreviewContent,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePreviewContent {
    Text(String),
    Image {
        bytes: Vec<u8>,
        width: u32,
        height: u32,
    },
    Pdf,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshot {
    pub id: String,
    pub message: String,
    pub height: u64,
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileListing {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub preview: Option<FilePreview>,
    pub read_only: bool,
    pub refreshing: bool,
    pub head: Option<String>,
    pub snapshot: Option<String>,
    pub history: Vec<FileSnapshot>,
    pub diff: Vec<FileDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryColumn {
    pub path: String,
    pub entries: Vec<FileEntry>,
    pub refreshing: bool,
    snapshot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub data: Resource<FileListing>,
    pub columns: Vec<DirectoryColumn>,
    pub selected: Option<FileEntry>,
    pub show_history: bool,
    pub show_new_folder: bool,
    pub new_folder_name: String,
    pub pending_delete: Option<FileEntry>,
    pub drop_active: bool,
    pub error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            columns: Vec::new(),
            selected: None,
            show_history: false,
            show_new_folder: false,
            new_folder_name: String::new(),
            pending_delete: None,
            drop_active: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    OpenEntry(String, FileKind),
    ClosePreview,
    Refresh,
    ToggleHistory,
    ToggleNewFolder,
    NewFolderNameChanged(String),
    CreateFolder,
    ChooseFiles,
    ChooseFolder,
    DropHovered(bool),
    FileDropped(DropToken),
    SelectSnapshot(Option<String>),
    Download(String, u64),
    RequestDelete(FileEntry),
    ConfirmDelete,
    CancelDelete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadDirectory {
        path: String,
    },
    LoadFile {
        path: String,
        snapshot: Option<String>,
    },
    CreateFolder {
        parent: String,
        name: String,
    },
    ChooseFiles {
        target: String,
    },
    ChooseFolder {
        target: String,
    },
    UploadDropped {
        target: String,
        token: DropToken,
    },
    LoadSnapshot {
        id: Option<String>,
        path: String,
    },
    Download {
        path: String,
        size: u64,
        snapshot: Option<String>,
    },
    Delete(String),
    CompareSnapshot {
        from: String,
        to: String,
        prefix: String,
    },
}

pub fn update(state: &mut State, message: Message) -> Option<Effect> {
    if !matches!(message, Message::DropHovered(_)) {
        state.error = None;
    }
    match message {
        Message::OpenEntry(path, FileKind::Directory) => {
            state.selected = None;
            if let Resource::Ready(listing) = &mut state.data {
                listing.preview = None;
            }
            Some(Effect::LoadDirectory { path })
        }
        Message::OpenEntry(path, FileKind::File | FileKind::Symlink) => {
            state.selected = find_entry(state, &path);
            Some(Effect::LoadFile {
                path,
                snapshot: snapshot(state),
            })
        }
        Message::ClosePreview => {
            state.selected = None;
            if let Resource::Ready(listing) = &mut state.data {
                listing.preview = None;
            }
            None
        }
        Message::Refresh => {
            // Show the "Refreshing…" strip while the in-place reload is in
            // flight: `data` stays Ready, so the column keeps rendering. The
            // fresh listing that `loaded` installs clears the flag.
            let path = listing_path(state).to_owned();
            if let Resource::Ready(listing) = &mut state.data {
                listing.refreshing = true;
            }
            if let Some(column) = state.columns.iter_mut().find(|column| column.path == path) {
                column.refreshing = true;
            }
            Some(Effect::LoadDirectory { path })
        }
        Message::ToggleHistory => {
            state.show_history = !state.show_history;
            None
        }
        Message::ToggleNewFolder => {
            if write_blocked(state) {
                return None;
            }
            state.show_new_folder = !state.show_new_folder;
            state.new_folder_name.clear();
            None
        }
        Message::NewFolderNameChanged(value) => {
            state.new_folder_name = value;
            None
        }
        Message::CreateFolder => {
            if write_blocked(state) {
                return None;
            }
            let name = nonempty(&state.new_folder_name)?;
            let parent = listing_path(state).to_owned();
            state.show_new_folder = false;
            state.new_folder_name.clear();
            Some(Effect::CreateFolder { parent, name })
        }
        Message::ChooseFiles => (!write_blocked(state)).then(|| Effect::ChooseFiles {
            target: listing_path(state).to_owned(),
        }),
        Message::ChooseFolder => (!write_blocked(state)).then(|| Effect::ChooseFolder {
            target: listing_path(state).to_owned(),
        }),
        Message::DropHovered(active) => {
            state.drop_active = active && !write_blocked(state);
            None
        }
        Message::FileDropped(token) => {
            state.drop_active = false;
            if write_blocked(state) {
                state.error = Some(match &state.data {
                    Resource::Ready(_) => {
                        "Switch to Live head before uploading dropped files.".into()
                    }
                    _ => "Files are unavailable; reconnect before uploading dropped files.".into(),
                });
                return None;
            }
            Some(Effect::UploadDropped {
                target: listing_path(state).to_owned(),
                token,
            })
        }
        Message::SelectSnapshot(id) => {
            state.columns.clear();
            state.selected = None;
            Some(Effect::LoadSnapshot {
                id,
                path: listing_path(state).to_owned(),
            })
        }
        Message::Download(path, size) => Some(Effect::Download {
            path,
            size,
            snapshot: snapshot(state),
        }),
        Message::RequestDelete(entry) => {
            if write_blocked(state) {
                return None;
            }
            state.pending_delete = Some(entry);
            None
        }
        Message::ConfirmDelete => {
            if write_blocked(state) {
                state.pending_delete = None;
                return None;
            }
            state
                .pending_delete
                .take()
                .map(|entry| Effect::Delete(entry.path))
        }
        Message::CancelDelete => {
            state.pending_delete = None;
            None
        }
    }
}

/// The diff a freshly loaded snapshot listing should auto-run against live head.
/// Mirrors the original app: selecting a past snapshot browses it *and* diffs it
/// against head into the pinned history section — no separate "Compare" button.
fn auto_diff(listing: &FileListing, snapshot_changed: bool) -> Option<Effect> {
    let snapshot = listing.snapshot.clone()?;
    let head = listing.head.clone()?;
    (snapshot_changed && snapshot != head).then(|| Effect::CompareSnapshot {
        from: snapshot,
        to: head,
        prefix: listing.path.clone(),
    })
}

pub fn loaded(state: &mut State, result: Result<Option<FileListing>, String>) -> Option<Effect> {
    match result {
        Ok(Some(listing)) => {
            let snapshot_changed = match &state.data {
                Resource::Ready(current) => current.snapshot != listing.snapshot,
                _ => false,
            };
            if snapshot_changed {
                state.columns.clear();
                state.selected = None;
            }
            let column = DirectoryColumn {
                path: listing.path.clone(),
                entries: listing.entries.clone(),
                refreshing: listing.refreshing,
                snapshot: listing.snapshot.clone(),
            };
            state.columns.retain(|candidate| {
                candidate.snapshot == listing.snapshot
                    && is_same_or_ancestor(&candidate.path, &listing.path)
                    && candidate.path != listing.path
            });
            state.columns.push(column);
            state.columns.sort_by_key(|column| path_depth(&column.path));
            let diff = auto_diff(&listing, snapshot_changed);
            state.data = Resource::Ready(listing);
            state.error = None;
            return diff;
        }
        Ok(None) => {
            state.columns.clear();
            state.selected = None;
            state.data = Resource::Empty;
        }
        // A fresh chain has no `/shared` yet — the files module answers
        // `path not found` for the default write root until the first write
        // creates it. That is an empty, writeable directory, not an error:
        // hard-erroring here bricked the whole tab on every new network
        // (mirrors the original app's DEFAULT_DIR && snapshot==null guard).
        // `contains`, not `ends_with`: the service layer wraps the module
        // error as `Module(files: path not found)` — a trailing paren the
        // module's raw string doesn't have.
        Err(error)
            if state.columns.is_empty()
                && error.contains("path not found")
                && snapshot(state).is_none() =>
        {
            let listing = FileListing {
                path: "/shared".into(),
                entries: Vec::new(),
                preview: None,
                read_only: false,
                refreshing: false,
                head: None,
                snapshot: None,
                history: Vec::new(),
                diff: Vec::new(),
            };
            state.columns.push(DirectoryColumn {
                path: listing.path.clone(),
                entries: Vec::new(),
                refreshing: false,
                snapshot: None,
            });
            state.data = Resource::Ready(listing);
            state.error = None;
        }
        Err(error) if state.columns.is_empty() => state.data = Resource::Error(error),
        Err(error) => state.error = Some(error),
    }
    None
}

pub fn preview_loaded(state: &mut State, result: Result<FilePreview, String>) {
    match result {
        Ok(preview) => {
            if let Resource::Ready(listing) = &mut state.data {
                listing.preview = Some(preview);
            }
        }
        Err(error) => state.error = Some(error),
    }
}

pub fn diff_loaded(state: &mut State, result: Result<Vec<FileDiff>, String>) {
    match result {
        Ok(diff) => {
            if let Resource::Ready(listing) = &mut state.data {
                listing.diff = diff;
            }
        }
        Err(error) => state.error = Some(error),
    }
}

pub fn listing_path(state: &State) -> &str {
    match &state.data {
        Resource::Ready(listing) => &listing.path,
        _ => "/shared",
    }
}

fn snapshot(state: &State) -> Option<String> {
    match &state.data {
        Resource::Ready(listing) => listing.snapshot.clone(),
        _ => None,
    }
}

fn write_blocked(state: &State) -> bool {
    !matches!(&state.data, Resource::Ready(listing) if !listing.read_only)
}

fn find_entry(state: &State, path: &str) -> Option<FileEntry> {
    state
        .columns
        .iter()
        .flat_map(|column| &column.entries)
        .chain(match &state.data {
            Resource::Ready(listing) => listing.entries.iter(),
            _ => [].iter(),
        })
        .find(|entry| entry.path == path)
        .cloned()
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn is_same_or_ancestor(candidate: &str, path: &str) -> bool {
    candidate == path
        || candidate == "/"
        || path
            .strip_prefix(candidate)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn path_depth(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

pub fn view(state: &State, p: Palette) -> Element<'_, Message> {
    let available = matches!(state.data, Resource::Ready(_));
    let (path, read_only) = match &state.data {
        Resource::Ready(listing) => (listing.path.as_str(), listing.read_only),
        _ => ("/shared", false),
    };
    let snapshot_badge: Element<'static, Message> = if read_only {
        snapshot_chip(p)
    } else {
        Space::new().width(0).into()
    };
    let header = container(
        row![
            text("Files")
                .font(SANS_SEMIBOLD)
                .size(HEADING)
                .color(p.filled),
            breadcrumb(path, p),
            snapshot_badge,
            Space::new().width(Length::Fill),
            outline_icon(
                "New folder",
                Icon::Modules,
                Message::ToggleNewFolder,
                available && !read_only,
                p,
            ),
            outline_enabled("Upload", Message::ChooseFiles, available && !read_only, p),
            outline_enabled(
                "Upload folder",
                Message::ChooseFolder,
                available && !read_only,
                p
            ),
            outline_enabled("Refresh", Message::Refresh, available, p),
            outline_icon(
                "History",
                Icon::Metrics,
                Message::ToggleHistory,
                available,
                p,
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .height(56)
    .padding([0, 20])
    .align_y(Alignment::Center)
    .style(move |_| bottom_border(p.paper, p.border_soft));

    let inside = match &state.data {
        Resource::Loading => center_state(
            "Loading files…",
            "Waiting for this node's filesystem.",
            Icon::Files,
            p,
        ),
        Resource::Empty => center_state("Empty directory", "Nothing here.", Icon::Files, p),
        // A missing node is not a read failure — the network just isn't entered
        // yet. Give it a calm center state, not the red error card.
        Resource::Error(error) if error.contains("enter a network") => center_state(
            "No node connected",
            "Enter a network to browse and upload files.",
            Icon::Files,
            p,
        ),
        Resource::Error(error) => error_state("Could not read folder", error, p),
        Resource::Ready(listing) => browser(state, listing, p),
    };
    let card = container(inside)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| card_style(p));
    let mut content = row![
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(18)
            .style(move |_| surface(p.sidebar))
    ];
    if state.show_history
        && let Resource::Ready(listing) = &state.data
    {
        content = content.push(history_panel(listing, p));
    }
    column![header, content].into()
}

fn browser<'a>(state: &'a State, listing: &'a FileListing, p: Palette) -> Element<'a, Message> {
    let selected_path = state.selected.as_ref().map(|entry| entry.path.as_str());
    let mut columns = row![].height(Length::Fill);
    if state.columns.is_empty() {
        columns = columns.push(directory_column(
            &DirectoryColumn {
                path: listing.path.clone(),
                entries: listing.entries.clone(),
                refreshing: listing.refreshing,
                snapshot: listing.snapshot.clone(),
            },
            selected_path,
            None,
            listing.read_only,
            p,
        ));
    } else {
        for (index, column) in state.columns.iter().enumerate() {
            if index > 0 {
                columns = columns.push(vertical_hairline(p.border_soft));
            }
            columns = columns.push(directory_column(
                column,
                selected_path,
                state.columns.get(index + 1).map(|next| next.path.as_str()),
                listing.read_only,
                p,
            ));
        }
    }
    if let Some(preview) = &listing.preview {
        columns = columns.push(vertical_hairline(p.border_soft));
        columns = columns.push(preview_panel(state, listing, preview, p));
    }

    let browser = scrollable(columns)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .width(Length::Fill)
        .height(Length::Fill);
    let mut body = column![].spacing(0);
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    if state.show_new_folder {
        body = body.push(
            container(
                row![
                    sem_input(
                        "Folder name",
                        &state.new_folder_name,
                        field(
                            "Folder name",
                            &state.new_folder_name,
                            Message::NewFolderNameChanged,
                            p
                        ),
                    ),
                    filled(
                        "Create folder",
                        Message::CreateFolder,
                        !state.new_folder_name.trim().is_empty(),
                        p,
                    )
                ]
                .spacing(8),
            )
            .padding([10, 16])
            .style(move |_| bottom_border(p.sunken, p.border_soft)),
        );
    }
    if let Some(entry) = &state.pending_delete {
        body = body.push(
            container(
                row![
                    text(format!(
                        "Delete {}? This removes the whole subtree.",
                        entry.name
                    ))
                    .font(SANS)
                    .size(BODY)
                    .color(p.danger),
                    Space::new().width(Length::Fill),
                    // Cancel left, destructive confirm right (danger triad).
                    outline("Cancel", Message::CancelDelete, p),
                    danger("Delete", Message::ConfirmDelete, p),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .style(move |_| bottom_border(p.danger_soft, p.danger_border)),
        );
    }
    let base: Element<'a, Message> = body.push(browser).into();
    if state.drop_active {
        stack![base, drop_overlay(listing_path(state), p)].into()
    } else {
        base
    }
}

fn directory_column(
    column: &DirectoryColumn,
    selected_path: Option<&str>,
    child_path: Option<&str>,
    read_only: bool,
    p: Palette,
) -> Element<'static, Message> {
    let mut entries = column![
        container(
            row![
                icons::view(Icon::Modules, 13.0, p.muted_3),
                text(path_label(&column.path))
                    .font(SANS_SEMIBOLD)
                    .size(BODY)
                    .color(p.ink)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .height(38)
        .padding([0, 12])
        .align_y(Alignment::Center)
        .style(move |_| bottom_border(p.sunken, p.border_soft))
    ];
    if column.refreshing {
        entries = entries.push(
            container(text("Refreshing…").font(SANS).size(CAPTION).color(p.muted_2))
                .padding([6, 16]),
        );
    }
    if column.entries.is_empty() {
        entries = entries.push(center_state(
            "Empty directory",
            "Nothing here.",
            Icon::Files,
            p,
        ));
    } else {
        for (index, entry) in column.entries.iter().enumerate() {
            if index > 0 {
                entries = entries.push(hairline(p.border_soft));
            }
            entries = entries.push(entry_row(
                entry,
                selected_path == Some(entry.path.as_str())
                    || child_path == Some(entry.path.as_str()),
                read_only,
                p,
            ));
        }
    }
    container(scrollable(entries))
        .width(COLUMN_WIDTH)
        .height(Length::Fill)
        .style(move |_| panel(p.paper, p.border_soft))
        .into()
}

fn entry_row(
    entry: &FileEntry,
    selected: bool,
    read_only: bool,
    p: Palette,
) -> Element<'static, Message> {
    let is_dir = entry.kind == FileKind::Directory;
    let is_symlink = entry.kind == FileKind::Symlink;
    let chevron: Element<'static, Message> = if is_dir {
        icons::view(Icon::ChevronRight, 14.0, p.muted_2).into()
    } else {
        Space::new().width(0).into()
    };
    // Symlinks read as a file with a "↪" tail; executables tag their size caption.
    let name = if is_symlink {
        format!("{} ↪", entry.name)
    } else {
        entry.name.clone()
    };
    let size_caption = if is_dir {
        String::new()
    } else if entry.executable {
        format!("{} · exec", human_bytes(entry.size))
    } else {
        human_bytes(entry.size)
    };
    let open = button(
        row![
            icon_tile(if is_dir { Icon::Modules } else { Icon::Files }, 28.0, p),
            text(name)
                .font(SANS)
                .size(BODY)
                .color(p.ink)
                .width(Length::Fill)
                .wrapping(Wrapping::None),
            text(size_caption).font(MONO).size(CAPTION).color(p.muted_2),
            chevron,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([11, 16])
    // Borderless row — the column owns the frame; a hairline divider separates
    // rows (S2: a per-row 4-side border doubles into a grid).
    .style(move |_, status| iced::widget::button::Style {
        background: (selected || matches!(status, iced::widget::button::Status::Hovered))
            .then_some(Background::Color(if selected {
                p.sidebar
            } else {
                p.hover
            })),
        text_color: p.ink,
        ..Default::default()
    })
    .on_press(Message::OpenEntry(entry.path.clone(), entry.kind))
    .width(Length::Fill);
    #[cfg(all(feature = "agent", debug_assertions))]
    let open = iced_agent_plugin::sem(iced_agent_plugin::Role::ListItem, entry.name.clone(), open);
    let actions: Element<'static, Message> = if is_dir {
        outline_enabled(
            "Delete",
            Message::RequestDelete(entry.clone()),
            !read_only,
            p,
        )
    } else {
        row![
            outline(
                "Download",
                Message::Download(entry.path.clone(), entry.size),
                p,
            ),
            outline_enabled(
                "Delete",
                Message::RequestDelete(entry.clone()),
                !read_only,
                p,
            ),
        ]
        .spacing(5)
        .into()
    };
    row![open, actions]
        .spacing(5)
        .align_y(Alignment::Center)
        .into()
}

fn preview_panel<'a>(
    state: &'a State,
    listing: &'a FileListing,
    preview: &'a FilePreview,
    p: Palette,
) -> Element<'a, Message> {
    let selected = state
        .selected
        .as_ref()
        .filter(|entry| entry.path == preview.path);
    // Only the entry's own actions are shown, and only when we have that entry —
    // no row of dead disabled buttons when `selected` is None (S3).
    let mut actions = row![].spacing(5).align_y(Alignment::Center);
    if let Some(entry) = selected {
        actions = actions.push(outline(
            "Download",
            Message::Download(entry.path.clone(), entry.size),
            p,
        ));
        actions = actions.push(outline_enabled(
            "Delete",
            Message::RequestDelete(entry.clone()),
            !listing.read_only,
            p,
        ));
    }
    actions = actions.push(icon_close(Message::ClosePreview, p));

    let mut body = column![
        container(
            row![
                text(path_label(&preview.path))
                    .font(SANS_SEMIBOLD)
                    .size(TITLE)
                    .color(p.ink),
                Space::new().width(Length::Fill),
                actions,
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        )
        .height(38)
        .padding([0, 14])
        .align_y(Alignment::Center)
        .style(move |_| bottom_border(p.sunken, p.border_soft)),
    ]
    .spacing(12);
    if let Some(entry) = selected {
        body = body.push(meta_block(entry, p));
    }
    body = body.push(text(&preview.detail).font(MONO).size(CAPTION).color(p.muted));
    body = body.push(preview_content(&preview.content, p));
    body.width(PREVIEW_WIDTH).height(Length::Fill).padding(14).into()
}

/// Authoritative metadata for the previewed entry: size, kind (+ exec), object
/// hash (selectable), then every wire-supplied meta pair (mime, …).
fn meta_block(entry: &FileEntry, p: Palette) -> Element<'_, Message> {
    let kind = match entry.kind {
        FileKind::Directory => "Directory",
        FileKind::File if entry.executable => "File · exec",
        FileKind::File => "File",
        FileKind::Symlink => "Symlink",
    };
    let mut rows = column![
        meta_row("Size", text(human_bytes(entry.size)).font(SANS).size(LABEL).color(p.muted_3).into(), p),
        meta_row("Kind", text(kind).font(SANS).size(LABEL).color(p.muted_3).into(), p),
    ]
    .spacing(4);
    if !entry.object.is_empty() {
        rows = rows.push(meta_row("Object", selectable(&entry.object, MONO, p.muted_3), p));
    }
    for (key, value) in &entry.meta {
        rows = rows.push(meta_row(
            key,
            text(value.clone()).font(SANS).size(LABEL).color(p.muted_3).into(),
            p,
        ));
    }
    rows.into()
}

fn meta_row<'a>(label: &'a str, value: Element<'a, Message>, p: Palette) -> Element<'a, Message> {
    row![
        container(
            text(label)
                .font(SANS_SEMIBOLD)
                .size(LABEL)
                .color(p.muted_2)
        )
        .width(64),
        value,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn preview_content(content: &FilePreviewContent, p: Palette) -> Element<'_, Message> {
    match content {
        FilePreviewContent::Text(content) => {
            scrollable(text(content).font(MONO).size(BODY).color(p.ink))
                .height(Length::Fill)
                .into()
        }
        FilePreviewContent::Image {
            bytes,
            width,
            height,
        } => container(
            column![
                image(iced::widget::image::Handle::from_bytes(bytes.clone()))
                    .content_fit(iced::ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill),
                text(format!("{width} × {height}"))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.muted),
            ]
            .spacing(8),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
        FilePreviewContent::Pdf => center_state(
            "PDF document",
            "PDFs are kept inert in preview. Download the file to open it.",
            Icon::Files,
            p,
        ),
        FilePreviewContent::Unsupported(reason) => {
            center_state("Preview unavailable", reason, Icon::Files, p)
        }
    }
}

fn history_panel(listing: &FileListing, p: Palette) -> Element<'static, Message> {
    let active = listing.snapshot.as_deref();
    let live_active = active.is_none();
    let head = listing
        .head
        .as_deref()
        .map(short)
        .unwrap_or_else(|| "empty".into());
    let header = container(
        row![
            icons::view(Icon::Metrics, 15.0, p.muted_3),
            text("History").font(SANS_SEMIBOLD).size(TITLE).color(p.ink),
            Space::new().width(Length::Fill),
            icon_close(Message::ToggleHistory, p),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([12, 14])
    .style(move |_| bottom_border(p.paper, p.border_soft));
    let live_head = button(
        row![
            text("Live head").font(SANS).size(BODY).color(p.ink),
            text(head).font(MONO).size(CAPTION).color(p.muted_2),
        ]
        .spacing(8),
    )
    .width(Length::Fill)
    .padding([9, 14])
    .style(move |_, _| history_button(live_active, p))
    .on_press(Message::SelectSnapshot(None));

    // Scrolling commit list.
    let mut list = column![];
    if listing.history.is_empty() {
        list = list.push(
            container(text("No commits yet.").font(SANS).size(BODY).color(p.muted_2)).padding(14),
        );
    } else {
        for snapshot in &listing.history {
            let selected = active == Some(snapshot.id.as_str());
            list = list.push(
                button(
                    column![
                        text(if snapshot.message.is_empty() {
                            "(no message)".into()
                        } else {
                            snapshot.message.clone()
                        })
                        .font(SANS)
                        .size(BODY)
                        .color(p.ink),
                        text(format!(
                            "h{} · {} · {}",
                            snapshot.height,
                            snapshot.time,
                            short(&snapshot.id)
                        ))
                        .font(MONO)
                        .size(CAPTION)
                        .color(p.muted_2),
                    ]
                    .spacing(3),
                )
                .width(Length::Fill)
                .padding([9, 14])
                .style(move |_, _| history_button(selected, p))
                .on_press(Message::SelectSnapshot(Some(snapshot.id.clone()))),
            );
        }
    }

    let panel_body = column![
        header,
        live_head,
        scrollable(list).height(Length::Fill),
        history_diff_section(listing, p),
    ];
    container(panel_body)
        .width(300)
        .height(Length::Fill)
        .style(move |_| panel(p.paper, p.border))
        .into()
}

/// Pinned bottom section: the auto-diff of the selected snapshot against head,
/// or an invitation to select one. Kind badges are colored by change.
fn history_diff_section(listing: &FileListing, p: Palette) -> Element<'static, Message> {
    let browsing_snapshot =
        listing.snapshot.is_some() && listing.snapshot.as_deref() != listing.head.as_deref();
    if !browsing_snapshot {
        return container(
            text("Select a snapshot to browse it and diff it against head.")
                .font(SANS)
                .size(LABEL)
                .color(p.muted_2),
        )
        .width(Length::Fill)
        .padding([12, 14])
        .style(move |_| bottom_border(p.sunken, p.border_soft))
        .into();
    }
    let mut rows = column![
        text("DIFF VS HEAD")
            .font(SANS_SEMIBOLD)
            .size(CAPTION)
            .color(p.muted_2),
    ]
    .spacing(6);
    if listing.diff.is_empty() {
        rows = rows.push(text("No changes.").font(SANS).size(LABEL).color(p.muted_2));
    } else {
        for change in &listing.diff {
            rows = rows.push(
                row![
                    text(change.kind.clone())
                        .font(SANS_SEMIBOLD)
                        .size(CAPTION)
                        .color(diff_tone(&change.kind, p)),
                    text(change.path.clone())
                        .font(MONO)
                        .size(LABEL)
                        .color(p.muted_3)
                        .width(Length::Fill)
                        .wrapping(Wrapping::WordOrGlyph),
                ]
                .spacing(8),
            );
        }
    }
    container(scrollable(rows).height(Length::Shrink))
        .width(Length::Fill)
        .max_height(240)
        .padding([12, 14])
        .style(move |_| bottom_border(p.sunken, p.border))
        .into()
}

fn diff_tone(kind: &str, p: Palette) -> Color {
    match kind {
        "added" => p.green,
        "removed" => p.danger,
        _ => p.amber,
    }
}

fn history_button(selected: bool, p: Palette) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: selected.then_some(Background::Color(p.sidebar)),
        text_color: p.ink,
        border: Border {
            color: if selected {
                theme::ACCENTS[0]
            } else {
                p.border_soft
            },
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn breadcrumb(path: &str, p: Palette) -> Element<'static, Message> {
    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    // A path, not a row of buttons: borderless text crumbs joined by ›, the
    // current (last) segment bold and non-clickable.
    let mut crumbs = row![].spacing(3).align_y(Alignment::Center);
    if segments.is_empty() {
        crumbs = crumbs.push(
            text("root")
                .font(SANS_SEMIBOLD)
                .size(theme::LABEL)
                .color(p.ink),
        );
        return crumbs.into();
    }
    crumbs = crumbs.push(crumb_link(
        "root",
        Message::OpenEntry("/".into(), FileKind::Directory),
        p,
    ));
    let mut current = String::new();
    let last = segments.len() - 1;
    for (index, segment) in segments.into_iter().enumerate() {
        current.push('/');
        current.push_str(segment);
        crumbs = crumbs.push(text("›").font(SANS).size(theme::LABEL).color(p.muted_2));
        if index == last {
            crumbs = crumbs.push(
                text(segment.to_string())
                    .font(SANS_SEMIBOLD)
                    .size(theme::LABEL)
                    .color(p.ink),
            );
        } else {
            crumbs = crumbs.push(crumb_link(
                segment.to_string(),
                Message::OpenEntry(current.clone(), FileKind::Directory),
                p,
            ));
        }
    }
    crumbs.into()
}

fn crumb_link(
    label: impl text::IntoFragment<'static>,
    message: Message,
    p: Palette,
) -> Element<'static, Message> {
    button(text(label).font(SANS).size(theme::LABEL))
        .padding([1, 3])
        .on_press(message)
        .style(move |_, status| iced::widget::button::Style {
            background: None,
            text_color: if matches!(status, iced::widget::button::Status::Hovered) {
                p.ink
            } else {
                p.muted
            },
            ..iced::widget::button::Style::default()
        })
        .into()
}

fn drop_overlay(target: &str, p: Palette) -> Element<'static, Message> {
    container(
        container(
            column![
                icons::view(Icon::Files, 28.0, theme::ACCENTS[0]),
                text("Drop to upload")
                    .font(SANS_SEMIBOLD)
                    .size(TITLE)
                    .color(p.ink),
                text(format!("Files will be copied to {target}"))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.muted),
            ]
            .spacing(9)
            .align_x(Alignment::Center),
        )
        .padding(24)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.paper)),
            border: Border {
                color: theme::ACCENTS[0],
                width: 2.0,
                radius: RADIUS_LG.into(),
            },
            shadow: Shadow {
                color: Color { a: 0.12, ..p.ink },
                offset: Vector::new(0.0, 4.0),
                blur_radius: 18.0,
            },
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(Color {
            a: 0.16,
            ..p.filled
        })),
        ..Default::default()
    })
    .into()
}

fn center_state<'a>(
    title: &'a str,
    detail: &'a str,
    icon: Icon,
    p: Palette,
) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    ui::empty_state::empty_state(Some(icon_tile(icon, 42.0, p)), title, detail, &t)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .into()
}

fn error_state<'a>(title: &'a str, detail: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        column![
            icon_tile(Icon::Settings, 42.0, p),
            text(title).font(SANS_SEMIBOLD).size(TITLE).color(p.ink),
            // The failure detail is selectable so it can be pasted into a report.
            container(selectable(detail, MONO, p.red)).max_width(360),
            outline("Retry", Message::Refresh, p),
        ]
        .spacing(9)
        .align_x(Alignment::Center)
        .max_width(360),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .padding(24)
    .into()
}

fn icon_tile(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
    container(icons::view(icon, size * 0.48, p.muted_3))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.sunken)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .into()
}

fn field<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    let t = theme::ui_for(&p);
    ui::input::input(placeholder, value, &t).on_input(on_input)
}

/// Dev-only text-input tagging: wraps `input` in a `TextInput` semantic node
/// carrying `value`. Compiled out entirely unless the agent bridge is built.
#[cfg(all(feature = "agent", debug_assertions))]
fn sem_input<'a>(
    name: &'static str,
    value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced_agent_plugin::Sem::new(iced_agent_plugin::Role::TextInput, name, input)
        .value(value.to_string())
        .into()
}
#[cfg(not(all(feature = "agent", debug_assertions)))]
fn sem_input<'a>(
    _name: &'static str,
    _value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    input.into()
}

fn outline<'a>(label: impl ToString, message: Message, p: Palette) -> Element<'a, Message> {
    outline_enabled(label, message, true, p)
}

fn outline_enabled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    let label = label.to_string();
    let button = ui::button::button(label.clone(), &t)
        .variant(ui::button::ButtonVariant::Outline)
        .size(ui::button::ButtonSize::Small)
        .disabled(!enabled)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

/// `outline_enabled` with a leading icon — a labelled header action.
fn outline_icon<'a>(
    label: &'static str,
    icon: Icon,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    let ink = if enabled { p.ink_soft } else { p.muted_2 };
    let content = row![
        icons::view(icon, 13.0, ink),
        text(label).font(SANS).size(LABEL).color(ink),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let button = ui::button::Button::new(content, &t)
        .variant(ui::button::ButtonVariant::Outline)
        .size(ui::button::ButtonSize::Small)
        .disabled(!enabled)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

/// Borderless icon-only close (X) for panel headers.
fn icon_close<'a>(message: Message, p: Palette) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    let button = ui::button::Button::new(icons::view(Icon::Close, 14.0, p.muted), &t)
        .variant(ui::button::ButtonVariant::Ghost)
        .size(ui::button::ButtonSize::Icon)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, "Close", button).into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

/// The read-only snapshot badge — a warning-toned status pill.
fn snapshot_chip<'a>(p: Palette) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    ui::badge::badge("snapshot", ui::badge::BadgeVariant::Warning, &t).into()
}

/// A read-only, selectable text field — errors, hashes and ids the user copies.
fn selectable<'a>(value: &str, font: iced::Font, color: Color) -> Element<'a, Message> {
    text_input("", value)
        .font(font)
        .size(LABEL)
        .padding(0)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: color,
            placeholder: color,
            value: color,
            selection: theme::ACCENTS[0],
        })
        .into()
}

/// Destructive confirm button — the danger triad, never a neutral outline.
fn danger<'a>(label: impl ToString, message: Message, p: Palette) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    let label = label.to_string();
    let button = ui::button::button(label.clone(), &t)
        .variant(ui::button::ButtonVariant::Destructive)
        .size(ui::button::ButtonSize::Small)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button).into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn filled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    let label = label.to_string();
    let button = ui::button::button(label.clone(), &t)
        .size(ui::button::ButtonSize::Small)
        .disabled(!enabled)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn card_style(p: Palette) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(p.paper)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            // Mode-aware: light mode keeps the warm graphite, dark mode uses
            // black instead of inheriting a brown wash.
            color: Color { a: 0.05, ..p.shadow },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ..Default::default()
    }
}

fn surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn panel(color: Color, border: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: border,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn bottom_border(bg: Color, border: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// A 1px filled rule — the honest divider (a Border paints all four sides).
fn hairline(color: Color) -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(1))
        .width(Length::Fill)
        .style(move |_| surface(color))
        .into()
}

/// A 1px full-height rule separating adjacent same-colored columns (S10).
fn vertical_hairline(color: Color) -> Element<'static, Message> {
    container(Space::new().width(1).height(Length::Fill))
        .width(1)
        .height(Length::Fill)
        .style(move |_| surface(color))
        .into()
}

fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    // Selectable so the failure can be copied into a bug report.
    ui::alert::alert(
        selectable(copy, SANS, p.danger),
        ui::alert::AlertVariant::Destructive,
        &t,
    )
    .into()
}

fn path_label(path: &str) -> String {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("root")
        .to_owned()
}

fn human_bytes(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}

fn short(value: &str) -> String {
    if value.chars().count() <= 18 {
        value.to_owned()
    } else {
        format!(
            "{}…{}",
            value.chars().take(10).collect::<String>(),
            value
                .chars()
                .rev()
                .take(6)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_chain_missing_shared_root_is_empty_and_writeable() {
        let mut state = State::default();
        loaded(&mut state, Err("Module(files: path not found)".into()));
        let Resource::Ready(listing) = &state.data else {
            panic!("expected a synthesized listing, got {:?}", state.data);
        };
        assert_eq!(listing.path, "/shared");
        assert!(listing.entries.is_empty());
        assert!(!listing.read_only, "the synthesized root must accept writes");
        assert_eq!(state.error, None);
    }

    #[test]
    fn missing_path_below_a_loaded_root_stays_an_error() {
        let mut state = State::default();
        loaded(&mut state, Ok(Some(listing("/shared", None))));
        loaded(&mut state, Err("Module(files: path not found)".into()));
        assert!(state.error.is_some(), "a real miss must surface, not vanish");
    }

    #[test]
    fn selecting_a_snapshot_auto_diffs_it_against_head() {
        let mut state = State::default();
        loaded(&mut state, Ok(Some(listing("/shared", None))));
        assert_eq!(
            update(&mut state, Message::SelectSnapshot(Some("snap-1".into()))),
            Some(Effect::LoadSnapshot {
                id: Some("snap-1".into()),
                path: "/shared".into(),
            })
        );
        // When the snapshot listing lands it auto-runs the diff against head —
        // no per-row "Compare" button.
        let effect = loaded(&mut state, Ok(Some(listing("/shared", Some("snap-1")))));
        assert_eq!(
            effect,
            Some(Effect::CompareSnapshot {
                from: "snap-1".into(),
                to: "head".into(),
                prefix: "/shared".into(),
            })
        );
    }

    #[test]
    fn returning_to_live_head_runs_no_diff() {
        let mut state = State::default();
        loaded(&mut state, Ok(Some(listing("/shared", Some("snap-1")))));
        assert_eq!(loaded(&mut state, Ok(Some(listing("/shared", None)))), None);
    }

    #[test]
    fn refresh_marks_the_current_column_refreshing_then_clears_it() {
        let mut state = State::default();
        loaded(&mut state, Ok(Some(listing("/shared", None))));
        assert_eq!(
            update(&mut state, Message::Refresh),
            Some(Effect::LoadDirectory {
                path: "/shared".into(),
            })
        );
        assert!(matches!(&state.data, Resource::Ready(listing) if listing.refreshing));
        assert!(
            state
                .columns
                .iter()
                .any(|column| column.path == "/shared" && column.refreshing)
        );
        // The fresh listing installed by `loaded` clears the strip.
        loaded(&mut state, Ok(Some(listing("/shared", None))));
        assert!(matches!(&state.data, Resource::Ready(listing) if !listing.refreshing));
    }

    fn listing(path: &str, snapshot: Option<&str>) -> FileListing {
        FileListing {
            path: path.into(),
            entries: vec![FileEntry {
                path: format!("{}/child", path.trim_end_matches('/')),
                name: "child".into(),
                kind: FileKind::Directory,
                size: 0,
                executable: false,
                object: String::new(),
                meta: BTreeMap::new(),
            }],
            preview: None,
            read_only: snapshot.is_some(),
            refreshing: false,
            head: Some("head".into()),
            snapshot: snapshot.map(str::to_owned),
            history: Vec::new(),
            diff: Vec::new(),
        }
    }

    fn assert_writes_blocked(data: Resource<FileListing>) {
        let entry = FileEntry {
            path: "/shared/child".into(),
            name: "child".into(),
            kind: FileKind::Directory,
            size: 0,
            executable: false,
            object: String::new(),
            meta: BTreeMap::new(),
        };
        let token = crate::view_api::test_drop_token();
        let mut state = State {
            data,
            new_folder_name: "draft".into(),
            ..State::default()
        };

        assert_eq!(update(&mut state, Message::ToggleNewFolder), None);
        assert!(!state.show_new_folder);
        assert_eq!(update(&mut state, Message::CreateFolder), None);
        assert_eq!(update(&mut state, Message::ChooseFiles), None);
        assert_eq!(update(&mut state, Message::ChooseFolder), None);
        assert_eq!(update(&mut state, Message::DropHovered(true)), None);
        assert!(!state.drop_active);
        assert_eq!(update(&mut state, Message::FileDropped(token)), None);
        assert!(!state.drop_active);
        assert_eq!(
            update(&mut state, Message::RequestDelete(entry.clone())),
            None
        );
        assert!(state.pending_delete.is_none());

        state.pending_delete = Some(entry);
        assert_eq!(update(&mut state, Message::ConfirmDelete), None);
        assert!(state.pending_delete.is_none());
    }

    #[test]
    fn unavailable_and_read_only_states_reject_all_writes() {
        for data in [
            Resource::Loading,
            Resource::Empty,
            Resource::Error("offline".into()),
            Resource::Ready(listing("/shared", Some("snap-7"))),
        ] {
            assert_writes_blocked(data);
        }
    }

    #[test]
    fn directory_loads_accumulate_parent_columns() {
        let mut state = State::default();
        loaded(&mut state, Ok(Some(listing("/shared", None))));
        assert_eq!(
            update(
                &mut state,
                Message::OpenEntry("/shared/child".into(), FileKind::Directory),
            ),
            Some(Effect::LoadDirectory {
                path: "/shared/child".into(),
            })
        );
        loaded(&mut state, Ok(Some(listing("/shared/child", None))));
        assert_eq!(
            state
                .columns
                .iter()
                .map(|column| column.path.as_str())
                .collect::<Vec<_>>(),
            vec!["/shared", "/shared/child"]
        );
    }

    #[test]
    fn live_drop_targets_current_directory_but_snapshot_rejects_it() {
        let token = crate::view_api::test_drop_token();
        let mut state = State::default();
        loaded(&mut state, Ok(Some(listing("/shared/design", None))));
        assert_eq!(
            update(&mut state, Message::FileDropped(token)),
            Some(Effect::UploadDropped {
                target: "/shared/design".into(),
                token,
            })
        );

        loaded(
            &mut state,
            Ok(Some(listing("/shared/design", Some("snap-7")))),
        );
        state.drop_active = true;
        assert_eq!(update(&mut state, Message::FileDropped(token)), None);
        assert!(!state.drop_active);
        assert!(state.error.as_deref().unwrap().contains("Live head"));
    }

    #[test]
    fn snapshot_download_preserves_snapshot() {
        let mut state = State::default();
        loaded(
            &mut state,
            Ok(Some(listing("/shared/design", Some("snap-7")))),
        );
        assert_eq!(
            update(
                &mut state,
                Message::Download("/shared/design/logo.svg".into(), 42),
            ),
            Some(Effect::Download {
                path: "/shared/design/logo.svg".into(),
                size: 42,
                snapshot: Some("snap-7".into()),
            })
        );
    }
}
