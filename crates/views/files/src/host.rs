//! The facts the host pushes, the readings folded off them, and the writes
//! that leave as intents — one per act the files browser offers.

use iced::futures::StreamExt;
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

/// One files-browser row, as the app's `files_ls` lists it.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct FsEntry {
    pub key: i64,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: i64,
    pub object: String,
}

/// One committed duckfs snapshot.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct FsSnapshot {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub height: i64,
    pub message: String,
}

/// One Added/Removed/Modified leaf between a snapshot and the head.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct FsDiffEntry {
    pub path: String,
    pub kind: String,
}

/// The screen's facts, as the app holds them. `listed` says the rows on hand
/// describe `path`; `writes` moves once per committed write, and the view
/// clears the name draft it consumed.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct FilesProps {
    pub path: String,
    pub listed: bool,
    pub entries: Vec<FsEntry>,
    pub directories: Vec<FsEntry>,
    pub connected: bool,
    pub loading: bool,
    pub preview_path: String,
    pub preview_entry: FsEntry,
    pub delete_target: String,
    pub diff_from: String,
    pub diff: Vec<FsDiffEntry>,
    pub history: Vec<FsSnapshot>,
    pub preview_truncated: bool,
    pub preview_binary: bool,
    pub preview_picture: bool,
    pub preview_width: i64,
    pub preview_height: i64,
    pub preview_text: String,
    pub dark: bool,
    pub write_refusal: String,
    pub writes: i64,
}

/// One item of the facts subscription: the facts, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct PropsItem {
    pub next: FilesProps,
    pub error: String,
}

/// The facts now, and again on every change the host sees — a
/// subscription, so a view restored from a snapshot asks again on its own.
pub fn props() -> iced::Subscription<PropsItem> {
    iced::Subscription::run(|| {
        host::subscribe("files.props", &[]).map(|answer| {
            let read = answer.and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())
            });
            match read {
                Ok(next) => PropsItem {
                    next,
                    error: String::new(),
                },
                Err(error) => PropsItem {
                    next: FilesProps::default(),
                    error,
                },
            }
        })
    })
}

/// `files.open_dir`, `files.open_file`, `files.arm_delete` — a duckfs path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    pub path: String,
}

/// `files.mkdir`, `files.new_file` — the new entry's name under the path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    pub name: String,
}

/// `files.show_diff` — the snapshot to diff against the head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
}

/// `files.save` — the edited body, written back to its path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub path: String,
    pub text: String,
}

/// `files.open_link` — a link the Markdown preview offered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
}

pub fn open_dir(path: &str) -> bool {
    notify("files.open_dir", &Path { path: path.into() })
}

pub fn open_file(path: &str) -> bool {
    notify("files.open_file", &Path { path: path.into() })
}

pub fn open_parent() -> bool {
    notify("files.open_parent", &())
}

pub fn make_dir(name: &str) -> bool {
    notify("files.mkdir", &Name { name: name.into() })
}

pub fn make_file(name: &str) -> bool {
    notify("files.new_file", &Name { name: name.into() })
}

pub fn arm_delete(path: &str) -> bool {
    notify("files.arm_delete", &Path { path: path.into() })
}

pub fn disarm_delete() -> bool {
    notify("files.disarm_delete", &())
}

pub fn delete_object() -> bool {
    notify("files.delete", &())
}

pub fn close_diff() -> bool {
    notify("files.close_diff", &())
}

pub fn show_diff(id: &str) -> bool {
    notify("files.show_diff", &Snapshot { id: id.into() })
}

pub fn save(path: &str, text: &str) -> bool {
    notify(
        "files.save",
        &Save {
            path: path.into(),
            text: text.into(),
        },
    )
}

pub fn open_link(url: &str) -> bool {
    notify("files.open_link", &Link { url: url.into() })
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}

/// The product icon set, as the bytes the wire carries.
pub fn icon(name: &str) -> Vec<u8> {
    design::icons::svg(name).as_bytes().to_vec()
}

pub fn no_fs_entry() -> FsEntry {
    FsEntry::default()
}

/// The draft as it stands, or nothing once a write consumed it.
pub fn keep_draft(consumed: bool, draft: &str) -> String {
    if consumed {
        String::new()
    } else {
        draft.into()
    }
}

// The desktop app's own readings (app/src/backend), repeated here because the
// view is its own crate: the wire carries the words, not the functions.

/// `12 files · 3 dirs` — the crumb bar's subtitle, and "" whenever the rows
/// on hand are not this path's: with the node down nobody asked, and
/// mid-navigation the rows still belong to the directory you left.
pub fn fs_counts_summary(connected: bool, listed: bool, entries: &[FsEntry]) -> String {
    if !connected || !listed || entries.is_empty() {
        return String::new();
    }
    let dirs = entries.iter().filter(|entry| entry.kind == "dir").count() as i64;
    let files = entries.len() as i64 - dirs;
    format!(
        "{} · {}",
        plural(files, "file", "files"),
        plural(dirs, "dir", "dirs")
    )
}

fn plural(count: i64, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

pub fn size_label(bytes: i64) -> String {
    const KB: i64 = 1_024;
    const MB: i64 = 1_024 * KB;
    const GB: i64 = 1_024 * MB;
    match bytes {
        size if size < KB => format!("{size} B"),
        size if size < MB => format!("{} KB", size / KB),
        size if size < GB => format!("{:.1} MB", size as f64 / MB as f64),
        size => format!("{:.1} GB", size as f64 / GB as f64),
    }
}

/// `h 84,912`; a snapshot without a height prints `h —`.
pub fn height_label(height: i64) -> String {
    if height < 0 {
        return "h —".into();
    }
    let digits = height.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("h {grouped}")
}

pub fn picture_caption(width: i64, height: i64) -> String {
    format!("{width} × {height}")
}

pub fn markdown_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}
