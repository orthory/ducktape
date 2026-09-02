//! The `duck://` URI protocol (v1) — one grammar, one module table, one
//! place to add a module.
//!
//! ```text
//! duck-uri  = "duck://" authority path [ "@" rev ] [ "#" fragment ]
//! authority = module | gateway-host        ; a dot means gateway plane
//! ```
//!
//! [`classify_duck_link`] is the module table: every surface that opens or
//! embeds a link (the reader's markdown, the open plane in
//! `handlers/chat.ice`) classifies through it and nowhere else. A malformed
//! or unknown ref is [`DuckKind::Unknown`] — never an error here; the caller
//! decides what "nothing to open" looks like.

pub(crate) use crate::DuckKind;

/// One classified link. Only the fields its `kind` names are meaningful;
/// the rest are empty / zero.
#[derive(Clone, Debug, PartialEq)]
pub struct DuckLink {
    pub kind: DuckKind,
    /// `forge_*`: the repository name.
    pub repo: String,
    /// `forge_item`: the item number (≥ 1).
    pub number: i64,
    /// `channel_message`: the message seq (≥ 1); `forge_item`: the Discussion
    /// message seq, or 0.
    pub seq: i64,
    /// `page`: the page id.
    pub page: String,
    /// `channel` / `channel_message`: the channel id.
    pub channel: String,
    /// `files`: the absolute duckfs path; `forge_blob`: the repo-relative path.
    pub path: String,
    /// `forge_blob`: the `@rev`, or "" for the head.
    pub rev: String,
}

impl DuckLink {
    fn unknown() -> Self {
        Self {
            kind: DuckKind::Unknown,
            repo: String::new(),
            number: 0,
            seq: 0,
            page: String::new(),
            channel: String::new(),
            path: String::new(),
            rev: String::new(),
        }
    }

    fn of(kind: DuckKind) -> Self {
        Self {
            kind,
            ..Self::unknown()
        }
    }
}

/// Classify one link. Web links (`http(s)://`) are [`DuckKind::Web`];
/// everything that is not a well-formed module-plane duck URI is
/// [`DuckKind::Unknown`].
pub fn classify_duck_link(url: String) -> DuckLink {
    let web = url.starts_with("http://") || url.starts_with("https://");
    if web {
        return DuckLink::of(DuckKind::Web);
    }
    let Some(rest) = url.strip_prefix("duck://") else {
        return DuckLink::unknown();
    };
    let (authority, tail) = match rest.split_once('/') {
        Some((authority, tail)) => (authority, format!("/{tail}")),
        None => (rest, String::new()),
    };
    let gateway_plane = authority.contains('.');
    if gateway_plane || authority.is_empty() {
        return DuckLink::unknown();
    }
    let (body, fragment) = tail.split_once('#').unwrap_or((tail.as_str(), ""));
    let (path, rev) = body.split_once('@').unwrap_or((body, ""));
    let Some(segments) = clean_segments(path) else {
        return DuckLink::unknown();
    };
    match authority {
        "page" => classify_page(&segments, rev, fragment),
        "files" => classify_files(path, &segments, rev, fragment),
        "forge" => classify_forge(&segments, rev, fragment),
        "channel" => classify_channel(&segments, rev, fragment),
        _ => DuckLink::unknown(),
    }
}

/// The path's segments, or `None` when any is empty (`//`), `.` or `..`.
/// A bare `/` or empty path is an empty list.
fn clean_segments(path: &str) -> Option<Vec<&str>> {
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    let segments: Vec<&str> = trimmed.split('/').collect();
    let clean = segments
        .iter()
        .all(|segment| !segment.is_empty() && *segment != "." && *segment != "..");
    clean.then_some(segments)
}

/// A 1-based decimal, or `None` (empty, zero, signs, anything else).
fn positive(digits: &str) -> Option<i64> {
    let decimal = !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit());
    if !decimal {
        return None;
    }
    digits.parse::<i64>().ok().filter(|number| *number > 0)
}

fn classify_page(segments: &[&str], rev: &str, fragment: &str) -> DuckLink {
    let [id] = segments else {
        return DuckLink::unknown();
    };
    let plain = rev.is_empty() && fragment.is_empty();
    if !plain {
        return DuckLink::unknown();
    }
    DuckLink {
        page: (*id).to_owned(),
        ..DuckLink::of(DuckKind::Page)
    }
}

/// Confined to exactly `/shared/attachments/<dir>/<name>` — classify is the
/// only guard between a crafted ref and a client read at another path.
fn classify_files(path: &str, segments: &[&str], rev: &str, fragment: &str) -> DuckLink {
    let ["shared", "attachments", _dir, _name] = segments else {
        return DuckLink::unknown();
    };
    let plain = rev.is_empty() && fragment.is_empty();
    if !plain {
        return DuckLink::unknown();
    }
    DuckLink {
        path: path.to_owned(),
        ..DuckLink::of(DuckKind::Files)
    }
}

fn classify_forge(segments: &[&str], rev: &str, fragment: &str) -> DuckLink {
    match segments {
        [repo] => {
            let plain = rev.is_empty() && fragment.is_empty();
            if !plain {
                return DuckLink::unknown();
            }
            DuckLink {
                repo: (*repo).to_owned(),
                ..DuckLink::of(DuckKind::ForgeRepo)
            }
        }
        [repo, "blob", file @ ..] => {
            // The node browses a pinned revision by exact oid only (a branch
            // name is not an address — it moves), so the protocol says the
            // same: `@rev` is 40 lowercase hex or absent (the head).
            let oid = rev.is_empty()
                || (rev.len() == 40
                    && rev
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
            let named = !file.is_empty() && fragment.is_empty() && oid;
            if !named {
                return DuckLink::unknown();
            }
            DuckLink {
                repo: (*repo).to_owned(),
                path: file.join("/"),
                rev: rev.to_owned(),
                ..DuckLink::of(DuckKind::ForgeBlob)
            }
        }
        [repo, number] => {
            let Some(number) = positive(number) else {
                return DuckLink::unknown();
            };
            let seq = match fragment.is_empty() {
                true => Some(0),
                false => positive(fragment),
            };
            let Some(seq) = seq.filter(|_| rev.is_empty()) else {
                return DuckLink::unknown();
            };
            DuckLink {
                repo: (*repo).to_owned(),
                number,
                seq,
                ..DuckLink::of(DuckKind::ForgeItem)
            }
        }
        _ => DuckLink::unknown(),
    }
}

fn classify_channel(segments: &[&str], rev: &str, fragment: &str) -> DuckLink {
    let [id] = segments else {
        return DuckLink::unknown();
    };
    if !rev.is_empty() {
        return DuckLink::unknown();
    }
    let channel = (*id).to_owned();
    if fragment.is_empty() {
        return DuckLink {
            channel,
            ..DuckLink::of(DuckKind::Channel)
        };
    }
    let Some(seq) = positive(fragment) else {
        return DuckLink::unknown();
    };
    DuckLink {
        channel,
        seq,
        ..DuckLink::of(DuckKind::ChannelMessage)
    }
}

/// Echo lanes: the open plane hands a classified link's field to an EXISTING
/// navigation handler through a run continuation (`run every duck_echo_str(x)
/// -> forge_open_repo _`), the one way an Ice handler reaches another.
pub async fn duck_echo_str(value: String) -> Result<String, AppError> {
    Ok(value)
}

pub async fn duck_echo_i64(value: i64) -> Result<i64, AppError> {
    Ok(value)
}

/// Which second step a forge deep link still owes once its repo is open.
pub fn forge_focus_kind(number: i64, path: String) -> crate::ForgeFocus {
    let item = number > 0;
    let blob = !path.is_empty();
    match (item, blob) {
        (true, _) => crate::ForgeFocus::Item,
        (false, true) => crate::ForgeFocus::Blob,
        (false, false) => crate::ForgeFocus::Idle,
    }
}

/// The Discussion note a `#seq` deep link landed on, if it is in the loaded
/// discussion — drawn once above the list by the forge item page.
pub fn linked_note(discussion: Vec<super::ChatMessage>, focus: i64) -> Option<super::ChatMessage> {
    if focus <= 0 {
        return None;
    }
    discussion.into_iter().find(|note| note.seq == focus)
}

use super::AppError;

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(url: &str) -> DuckKind {
        classify_duck_link(url.into()).kind
    }

    /// The module table, row by row — the cases the protocol plan pinned for
    /// the TS `classifyDuckRef`, carried to the Rust table verbatim.
    #[test]
    fn the_module_table_classifies_every_row_and_refuses_the_rest() {
        let page = classify_duck_link("duck://page/pg-1".into());
        assert_eq!((page.kind, page.page.as_str()), (DuckKind::Page, "pg-1"));
        assert_eq!(kind("duck://page/a/b"), DuckKind::Unknown);
        assert_eq!(kind("duck://page/"), DuckKind::Unknown);

        let file = classify_duck_link("duck://files/shared/attachments/u1/doc.pdf".into());
        assert_eq!(
            (file.kind, file.path.as_str()),
            (DuckKind::Files, "/shared/attachments/u1/doc.pdf")
        );
        assert_eq!(kind("duck://files/shared/skills/x.md"), DuckKind::Unknown);
        assert_eq!(kind("duck://files/shared/attachments/a/b/c"), DuckKind::Unknown);
        assert_eq!(kind("duck://files/shared/attachments/../etc/pw"), DuckKind::Unknown);
        assert_eq!(kind("duck://files/shared/attachments/u1/a.png"), DuckKind::Files);

        let repo = classify_duck_link("duck://forge/ducktape".into());
        assert_eq!((repo.kind, repo.repo.as_str()), (DuckKind::ForgeRepo, "ducktape"));
        let item = classify_duck_link("duck://forge/ducktape/58".into());
        assert_eq!((item.kind, item.number, item.seq), (DuckKind::ForgeItem, 58, 0));
        let anchored = classify_duck_link("duck://forge/ducktape/58#12".into());
        assert_eq!((anchored.kind, anchored.number, anchored.seq), (DuckKind::ForgeItem, 58, 12));
        assert_eq!(kind("duck://forge/ducktape#12"), DuckKind::Unknown);
        assert_eq!(kind("duck://forge/ducktape/0"), DuckKind::Unknown);
        assert_eq!(kind("duck://forge/ducktape/58#0"), DuckKind::Unknown);
        assert_eq!(kind("duck://forge/ducktape/-1"), DuckKind::Unknown);

        let channel = classify_duck_link("duck://channel/general".into());
        assert_eq!((channel.kind, channel.channel.as_str()), (DuckKind::Channel, "general"));
        let hidden = classify_duck_link("duck://channel/forge:ducktape:58".into());
        assert_eq!((hidden.kind, hidden.channel.as_str()), (DuckKind::Channel, "forge:ducktape:58"));
        let message = classify_duck_link("duck://channel/general#42".into());
        assert_eq!((message.kind, message.seq), (DuckKind::ChannelMessage, 42));
        assert_eq!(kind("duck://channel/general#0"), DuckKind::Unknown);
        assert_eq!(kind("duck://channel/"), DuckKind::Unknown);

        assert_eq!(kind("duck://memory/notes/a.md"), DuckKind::Unknown, "reserved");
        assert_eq!(kind("duck://team.duck/index.html"), DuckKind::Unknown, "gateway plane");
        assert_eq!(kind("duck://net.duck"), DuckKind::Unknown, "gateway plane");
        assert_eq!(kind("duck://"), DuckKind::Unknown);
        assert_eq!(kind("mailto:a@b"), DuckKind::Unknown);
        assert_eq!(kind("./img/a.png"), DuckKind::Unknown, "a relative path is the caller's to resolve");
        assert_eq!(kind("https://example.com/a.png"), DuckKind::Web);
        assert_eq!(kind("http://example.com"), DuckKind::Web);
    }

    /// The forge blob row: `/<repo>/blob/<path>[@<rev>]`.
    #[test]
    fn a_forge_blob_names_a_committed_file_at_a_revision_or_the_head() {
        let head = classify_duck_link("duck://forge/ducktape/blob/docs/logo.png".into());
        assert_eq!(
            (head.kind, head.repo.as_str(), head.path.as_str(), head.rev.as_str()),
            (DuckKind::ForgeBlob, "ducktape", "docs/logo.png", "")
        );
        assert_eq!(
            kind("duck://forge/ducktape/blob/README.md@main"),
            DuckKind::Unknown,
            "a branch name moves — a rev is an exact oid"
        );
        assert_eq!(
            kind("duck://forge/ducktape/blob/a.png@ABCDEF0000000000000000000000000000000000"),
            DuckKind::Unknown,
            "lowercase hex"
        );
        let oid = classify_duck_link(
            "duck://forge/ducktape/blob/a/b.png@1111111111111111111111111111111111111111".into(),
        );
        assert_eq!(oid.path, "a/b.png");
        assert_eq!(oid.rev.len(), 40);
        assert_eq!(kind("duck://forge/ducktape/blob"), DuckKind::Unknown, "no file");
        assert_eq!(kind("duck://forge/ducktape/blob/"), DuckKind::Unknown, "no file");
        assert_eq!(kind("duck://forge/ducktape/blob/../x"), DuckKind::Unknown, "no dot-segments");
        assert_eq!(kind("duck://forge/ducktape/blob/a.png#L3"), DuckKind::Unknown, "no fragment yet");
    }

    #[test]
    fn the_second_forge_step_is_the_item_else_the_blob_else_nothing() {
        assert_eq!(forge_focus_kind(0, String::new()), crate::ForgeFocus::Idle);
        assert_eq!(forge_focus_kind(7, String::new()), crate::ForgeFocus::Item);
        assert_eq!(forge_focus_kind(0, "a.png".into()), crate::ForgeFocus::Blob);
        assert_eq!(forge_focus_kind(7, "a.png".into()), crate::ForgeFocus::Item, "a number wins");
    }
}
