//! Capability-free Files column browser.
//!
//! Native filesystem access, dialogs, downloads, and platform drag APIs stay in
//! the host. This module only owns view state and emits typed effects.

use iced::widget::{
    Button, Space, button, column, container, image, row, scrollable, stack, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_SM, SANS};
use crate::view_api::{DropToken, Resource};

const COLUMN_WIDTH: f32 = 286.0;
const PREVIEW_WIDTH: f32 = 520.0;

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
    pub drag_out_status: Option<String>,
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
            drag_out_status: None,
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
    RequestDragOut(String, u64),
    RequestDelete(FileEntry),
    ConfirmDelete,
    CancelDelete,
    CompareSnapshot(String),
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
    BeginDragOut {
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
        Message::Refresh => Some(Effect::LoadDirectory {
            path: listing_path(state).to_owned(),
        }),
        Message::ToggleHistory => {
            state.show_history = !state.show_history;
            None
        }
        Message::ToggleNewFolder => {
            if read_only(state) {
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
            if read_only(state) {
                return None;
            }
            let name = nonempty(&state.new_folder_name)?;
            let parent = listing_path(state).to_owned();
            state.show_new_folder = false;
            state.new_folder_name.clear();
            Some(Effect::CreateFolder { parent, name })
        }
        Message::ChooseFiles => (!read_only(state)).then(|| Effect::ChooseFiles {
            target: listing_path(state).to_owned(),
        }),
        Message::ChooseFolder => (!read_only(state)).then(|| Effect::ChooseFolder {
            target: listing_path(state).to_owned(),
        }),
        Message::DropHovered(active) => {
            state.drop_active = active && !read_only(state);
            None
        }
        Message::FileDropped(token) => {
            state.drop_active = false;
            if read_only(state) {
                state.error = Some("Switch to Live head before uploading dropped files.".into());
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
        Message::RequestDragOut(path, size) => {
            state.drag_out_status = None;
            Some(Effect::BeginDragOut {
                path,
                size,
                snapshot: snapshot(state),
            })
        }
        Message::RequestDelete(entry) => {
            if read_only(state) {
                return None;
            }
            state.pending_delete = Some(entry);
            None
        }
        Message::ConfirmDelete => state
            .pending_delete
            .take()
            .map(|entry| Effect::Delete(entry.path)),
        Message::CancelDelete => {
            state.pending_delete = None;
            None
        }
        Message::CompareSnapshot(from) => {
            let Resource::Ready(listing) = &state.data else {
                return None;
            };
            Some(Effect::CompareSnapshot {
                from,
                to: listing.head.clone()?,
                prefix: listing.path.clone(),
            })
        }
    }
}

pub fn loaded(state: &mut State, result: Result<Option<FileListing>, String>) {
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
            state.data = Resource::Ready(listing);
            state.error = None;
        }
        Ok(None) => {
            state.columns.clear();
            state.selected = None;
            state.data = Resource::Empty;
        }
        Err(error) if state.columns.is_empty() => state.data = Resource::Error(error),
        Err(error) => state.error = Some(error),
    }
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

pub fn drag_out_unavailable(state: &mut State, reason: String) {
    state.drag_out_status = Some(reason);
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

fn read_only(state: &State) -> bool {
    matches!(&state.data, Resource::Ready(listing) if listing.read_only)
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
    let (path, read_only) = match &state.data {
        Resource::Ready(listing) => (listing.path.as_str(), listing.read_only),
        _ => ("/shared", false),
    };
    let snapshot_badge: Element<'static, Message> = if read_only {
        text("snapshot").font(MONO).size(10).color(p.amber).into()
    } else {
        Space::new().width(0).into()
    };
    let header = container(
        row![
            text("Files").font(SANS).size(16).color(p.filled),
            breadcrumb(path, p),
            snapshot_badge,
            Space::new().width(Length::Fill),
            outline_enabled("New folder", Message::ToggleNewFolder, !read_only, p),
            outline_enabled("Upload", Message::ChooseFiles, !read_only, p),
            outline_enabled("Folder", Message::ChooseFolder, !read_only, p),
            outline("History", Message::ToggleHistory, p),
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
    if let Some(status) = &state.drag_out_status {
        body = body.push(notice_banner(status, p));
    }
    if state.show_new_folder {
        body = body.push(
            container(
                row![
                    field(
                        "Folder name",
                        &state.new_folder_name,
                        Message::NewFolderNameChanged,
                        p
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
                    .size(12)
                    .color(p.danger),
                    Space::new().width(Length::Fill),
                    outline("Cancel", Message::CancelDelete, p),
                    outline("Delete", Message::ConfirmDelete, p),
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
                    .font(SANS)
                    .size(12)
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
            container(text("Refreshing…").font(SANS).size(10.5).color(p.muted_2)).padding([6, 16]),
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
        for entry in &column.entries {
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
    let chevron: Element<'static, Message> = if is_dir {
        icons::view(Icon::ChevronRight, 14.0, p.muted_2).into()
    } else {
        Space::new().width(0).into()
    };
    let open = button(
        row![
            icon_tile(if is_dir { Icon::Modules } else { Icon::Files }, 28.0, p),
            text(entry.name.clone())
                .font(SANS)
                .size(13.5)
                .color(p.ink)
                .width(Length::Fill),
            text(if is_dir {
                String::new()
            } else {
                human_bytes(entry.size)
            })
            .font(MONO)
            .size(11)
            .color(p.muted_2),
            chevron,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([11, 16])
    .style(move |_, status| iced::widget::button::Style {
        background: (selected || matches!(status, iced::widget::button::Status::Hovered))
            .then_some(Background::Color(if selected {
                p.sidebar
            } else {
                p.hover
            })),
        text_color: p.ink,
        border: Border {
            color: p.border_soft,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .on_press(Message::OpenEntry(entry.path.clone(), entry.kind));
    let actions: Element<'static, Message> = if is_dir {
        outline_enabled(
            "Delete",
            Message::RequestDelete(entry.clone()),
            !read_only,
            p,
        )
        .into()
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
    row![open.width(Length::Fill), actions]
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
    let download = selected.map_or_else(
        || outline_enabled("Download", Message::ClosePreview, false, p),
        |entry| {
            outline(
                "Download",
                Message::Download(entry.path.clone(), entry.size),
                p,
            )
        },
    );
    let drag_out = selected.map_or_else(
        || outline_enabled("Drag out", Message::ClosePreview, false, p),
        |entry| {
            outline(
                "Drag out",
                Message::RequestDragOut(entry.path.clone(), entry.size),
                p,
            )
        },
    );
    let delete = selected.map_or_else(
        || outline_enabled("Delete", Message::ClosePreview, false, p),
        |entry| {
            outline_enabled(
                "Delete",
                Message::RequestDelete(entry.clone()),
                !listing.read_only,
                p,
            )
        },
    );
    column![
        container(
            row![
                text(path_label(&preview.path))
                    .font(SANS)
                    .size(13)
                    .color(p.ink),
                Space::new().width(Length::Fill),
                download,
                drag_out,
                delete,
                outline("Close", Message::ClosePreview, p),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        )
        .height(38)
        .padding([0, 14])
        .align_y(Alignment::Center)
        .style(move |_| bottom_border(p.sunken, p.border_soft)),
        text(&preview.detail).font(MONO).size(10.5).color(p.muted),
        preview_content(&preview.content, p),
    ]
    .width(PREVIEW_WIDTH)
    .height(Length::Fill)
    .spacing(12)
    .padding(14)
    .into()
}

fn preview_content(content: &FilePreviewContent, p: Palette) -> Element<'_, Message> {
    match content {
        FilePreviewContent::Text(content) => {
            scrollable(text(content).font(MONO).size(12.5).color(p.ink))
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
                    .size(10.5)
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
    let mut rows = column![
        container(
            row![
                icons::view(Icon::Metrics, 15.0, p.muted_3),
                text("History").font(SANS).size(13).color(p.ink),
                Space::new().width(Length::Fill),
                outline("Close", Message::ToggleHistory, p),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([12, 14])
        .style(move |_| bottom_border(p.paper, p.border_soft)),
        button(
            row![
                text("Live head").font(SANS).size(12).color(p.ink),
                text(head).font(MONO).size(10).color(p.muted_2),
            ]
            .spacing(8),
        )
        .width(Length::Fill)
        .padding([9, 14])
        .style(move |_, _| history_button(live_active, p))
        .on_press(Message::SelectSnapshot(None)),
    ];
    if listing.history.is_empty() {
        rows = rows.push(
            container(
                text("No commits yet.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.muted_2),
            )
            .padding(14),
        );
    } else {
        for snapshot in &listing.history {
            let selected = active == Some(snapshot.id.as_str());
            rows = rows.push(
                button(
                    column![
                        text(if snapshot.message.is_empty() {
                            "(no message)".into()
                        } else {
                            snapshot.message.clone()
                        })
                        .font(SANS)
                        .size(12)
                        .color(p.ink),
                        text(format!(
                            "h{} · {} · {}",
                            snapshot.height,
                            snapshot.time,
                            short(&snapshot.id)
                        ))
                        .font(MONO)
                        .size(10)
                        .color(p.muted_2),
                    ]
                    .spacing(3),
                )
                .width(Length::Fill)
                .padding([9, 14])
                .style(move |_, _| history_button(selected, p))
                .on_press(Message::SelectSnapshot(Some(snapshot.id.clone()))),
            );
            rows = rows.push(
                button(text("Compare with live head").font(SANS).size(10.5))
                    .width(Length::Fill)
                    .padding([4, 14])
                    .style(move |_, status| iced::widget::button::Style {
                        background: matches!(status, iced::widget::button::Status::Hovered)
                            .then_some(Background::Color(p.hover)),
                        text_color: p.muted,
                        ..Default::default()
                    })
                    .on_press(Message::CompareSnapshot(snapshot.id.clone())),
            );
        }
    }
    if !listing.diff.is_empty() {
        rows = rows.push(
            text("DIFF TO LIVE HEAD")
                .font(MONO)
                .size(9.5)
                .color(p.muted_2),
        );
        for change in &listing.diff {
            rows = rows.push(
                container(
                    column![
                        text(change.kind.clone())
                            .font(MONO)
                            .size(9.5)
                            .color(theme::ACCENTS[0]),
                        text(change.path.clone())
                            .font(MONO)
                            .size(10.5)
                            .color(p.ink_soft),
                    ]
                    .spacing(2),
                )
                .padding([6, 14]),
            );
        }
    }
    container(scrollable(rows))
        .width(300)
        .height(Length::Fill)
        .style(move |_| panel(p.paper, p.border))
        .into()
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
    let mut crumbs = row![outline(
        "root",
        Message::OpenEntry("/".into(), FileKind::Directory),
        p,
    )]
    .spacing(3)
    .align_y(Alignment::Center);
    let mut current = String::new();
    for segment in path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        current.push('/');
        current.push_str(segment);
        crumbs = crumbs.push(text("›").font(SANS).size(11).color(p.muted_2));
        crumbs = crumbs.push(outline(
            segment,
            Message::OpenEntry(current.clone(), FileKind::Directory),
            p,
        ));
    }
    crumbs.into()
}

fn drop_overlay(target: &str, p: Palette) -> Element<'static, Message> {
    container(
        container(
            column![
                icons::view(Icon::Files, 28.0, theme::ACCENTS[0]),
                text("Drop to upload").font(SANS).size(16).color(p.ink),
                text(format!("Files will be copied to {target}"))
                    .font(MONO)
                    .size(11)
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
    container(
        column![
            icon_tile(icon, 42.0, p),
            text(title).font(SANS).size(14).color(p.muted_3),
            text(detail).font(SANS).size(11.5).color(p.muted_2),
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

fn error_state<'a>(title: &'a str, detail: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        column![
            icon_tile(Icon::Settings, 42.0, p),
            text(title).font(SANS).size(14).color(p.ink),
            text(detail).font(MONO).size(11.5).color(p.red),
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
    text_input(placeholder, value)
        .on_input(on_input)
        .padding([8, 10])
        .size(12.5)
        .font(SANS)
        .style(move |_, status| iced::widget::text_input::Style {
            background: Background::Color(p.sunken),
            border: Border {
                color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    theme::ACCENTS[0]
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            icon: p.muted,
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
}

fn outline<'a>(label: impl ToString, message: Message, p: Palette) -> Button<'a, Message> {
    outline_enabled(label, message, true, p)
}

fn outline_enabled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(12))
        .padding([7, 10])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, iced::widget::button::Status::Hovered) {
                    p.hover
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
            border: Border {
                color: if enabled {
                    p.border_strong
                } else {
                    p.border_soft
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn filled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(12.5))
        .padding([8, 13])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(if enabled {
                if matches!(status, iced::widget::button::Status::Hovered) {
                    p.ink_soft
                } else {
                    p.filled
                }
            } else {
                p.border_soft
            })),
            text_color: if enabled { p.on_filled } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    if enabled {
        button.on_press(message)
    } else {
        button
    }
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
            color: Color {
                a: 0.05,
                ..Color::from_rgb8(40, 38, 34)
            },
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

fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(12).color(p.danger))
        .width(Length::Fill)
        .padding([10, 16])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.danger_soft)),
            border: Border {
                color: p.danger_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn notice_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(12).color(p.amber))
        .width(Length::Fill)
        .padding([10, 16])
        .style(move |_| bottom_border(p.sunken, p.border_soft))
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

    fn listing(path: &str, snapshot: Option<&str>) -> FileListing {
        FileListing {
            path: path.into(),
            entries: vec![FileEntry {
                path: format!("{}/child", path.trim_end_matches('/')),
                name: "child".into(),
                kind: FileKind::Directory,
                size: 0,
                executable: false,
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
    fn snapshot_is_downloadable_and_drag_contract_preserves_snapshot() {
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
        assert_eq!(
            update(
                &mut state,
                Message::RequestDragOut("/shared/design/logo.svg".into(), 42),
            ),
            Some(Effect::BeginDragOut {
                path: "/shared/design/logo.svg".into(),
                size: 42,
                snapshot: Some("snap-7".into()),
            })
        );
        drag_out_unavailable(&mut state, "Drag-out unavailable; use Download.".into());
        assert_eq!(
            state.drag_out_status.as_deref(),
            Some("Drag-out unavailable; use Download.")
        );
    }
}
