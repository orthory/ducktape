//! Bounded live-run log state and semantic JSONL rendering.

use std::collections::BTreeMap;

const MAX_ACTIVITY_ENTRIES: usize = 120;
const MAX_RAW_ENTRY_CHARS: usize = 256_000;
const MAX_ACTIVITY_ROWS: usize = 500;
const MAX_FIELD_LINES: usize = 120;
const MAX_ROW_CHARS: usize = 4_000;
const MAX_FIELD_CHARS: usize = 128_000;
const FIELD_HEAD_LINES: usize = 20;
const FIELD_TAIL_LINES: usize = MAX_FIELD_LINES - FIELD_HEAD_LINES - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogEntry {
    Line { stream: RunStream, text: String },
    Gap(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunLog {
    pub entries: Vec<RunLogEntry>,
    pub dropped: u64,
    pub last_cursor: u64,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SemanticLogKind {
    Message,
    Command,
    Output,
    Status,
    Exit,
    File,
    Tool,
    Text,
    Gap,
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SemanticLogRow {
    pub(super) kind: SemanticLogKind,
    pub(super) stream: Option<RunStream>,
    pub(super) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogEvent {
    Connected,
    Line {
        dispatch_id: String,
        cursor: u64,
        stream: RunStream,
        text: String,
    },
    Lagged {
        dispatch_id: String,
        cursor: u64,
    },
    Unavailable {
        dispatch_id: Option<String>,
        reason: String,
    },
    Disconnected(String),
}

pub(super) fn apply_event(
    logs: &mut BTreeMap<String, RunLog>,
    expanded: &[String],
    event: RunLogEvent,
) {
    match event {
        RunLogEvent::Connected => {
            for dispatch_id in expanded {
                logs.entry(dispatch_id.clone()).or_default().unavailable = false;
            }
        }
        RunLogEvent::Line {
            dispatch_id,
            cursor,
            stream,
            text,
        } => {
            let log = logs.entry(dispatch_id).or_default();
            if cursor <= log.last_cursor {
                return;
            }
            log.last_cursor = cursor;
            let entry = if text.len() > MAX_RAW_ENTRY_CHARS {
                RunLogEntry::Gap(format!(
                    "provider event omitted: {} characters over limit",
                    text.len() - MAX_RAW_ENTRY_CHARS
                ))
            } else {
                RunLogEntry::Line { stream, text }
            };
            append_run_log(log, entry);
        }
        RunLogEvent::Lagged {
            dispatch_id,
            cursor,
        } => {
            let log = logs.entry(dispatch_id).or_default();
            if cursor > log.last_cursor {
                log.last_cursor = cursor;
                append_run_log(
                    log,
                    RunLogEntry::Gap(format!(
                        "output gap: older lines were dropped before cursor {cursor}"
                    )),
                );
            }
        }
        RunLogEvent::Unavailable {
            dispatch_id,
            reason: _,
        } => match dispatch_id {
            Some(dispatch_id) => {
                logs.entry(dispatch_id).or_default().unavailable = true;
            }
            None => {
                for dispatch_id in expanded {
                    logs.entry(dispatch_id.clone()).or_default().unavailable = true;
                }
            }
        },
        RunLogEvent::Disconnected(_) => {}
    }
}

fn append_run_log(log: &mut RunLog, entry: RunLogEntry) {
    if log.entries.len() == MAX_ACTIVITY_ENTRIES {
        log.entries.remove(0);
        log.dropped = log.dropped.saturating_add(1);
    }
    log.entries.push(entry);
}

/// Flatten the rendered log into copyable plain text — one line per visible
/// row, matching what the pane shows. The operator's whole reason to open the
/// pane is to lift a command, stack trace, or error line out of it.
pub(super) fn flatten_for_copy(log: &RunLog) -> String {
    semantic_log_rows(&log.entries)
        .iter()
        .map(|row| row.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn semantic_log_rows(entries: &[RunLogEntry]) -> Vec<SemanticLogRow> {
    let mut rows = Vec::new();
    let mut started = BTreeMap::<String, usize>::new();
    let first = entries.len().saturating_sub(MAX_ACTIVITY_ENTRIES);
    for entry in &entries[first..] {
        match entry {
            RunLogEntry::Gap(text) => push_log_row(
                &mut rows,
                SemanticLogRow {
                    kind: SemanticLogKind::Gap,
                    stream: None,
                    text: normalize_log_text(text),
                },
            ),
            RunLogEntry::Line { stream, text } => {
                if text.trim().is_empty() {
                    push_log_row(
                        &mut rows,
                        SemanticLogRow {
                            kind: SemanticLogKind::Blank,
                            stream: Some(*stream),
                            text: String::new(),
                        },
                    );
                } else if !parse_json_log_event(text, *stream, &mut rows, &mut started) {
                    push_log_text(&mut rows, SemanticLogKind::Text, *stream, text);
                }
            }
        }
    }
    if rows.len() <= MAX_ACTIVITY_ROWS {
        return rows;
    }
    let omitted = rows.len() - (MAX_ACTIVITY_ROWS - 1);
    let mut tail = rows.split_off(rows.len() - (MAX_ACTIVITY_ROWS - 1));
    tail.insert(
        0,
        SemanticLogRow {
            kind: SemanticLogKind::Gap,
            stream: None,
            text: format!("live log tail: {omitted} rendered rows omitted"),
        },
    );
    tail
}

fn parse_json_log_event(
    line: &str,
    stream: RunStream,
    rows: &mut Vec<SemanticLogRow>,
    started: &mut BTreeMap<String, usize>,
) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
        return false;
    };
    let Some(record) = value.as_object() else {
        if let Some(text) = value.as_str() {
            push_log_text(rows, SemanticLogKind::Text, stream, text);
        } else {
            push_log_status(rows, stream, "event: JSON value".into());
        }
        return true;
    };
    let event_type = json_string(record, "type");
    match event_type {
        Some("thread.started") => push_log_status(
            rows,
            stream,
            json_string(record, "thread_id").map_or_else(
                || "thread started".into(),
                |thread| format!("thread started: {thread}"),
            ),
        ),
        Some("turn.started") => push_log_status(rows, stream, "turn started".into()),
        Some("turn.completed") => push_log_status(rows, stream, "turn completed".into()),
        Some("item.started") | Some("item.completed") => {
            let completed = event_type == Some("item.completed");
            let Some(item) = record.get("item").and_then(serde_json::Value::as_object) else {
                push_log_status(
                    rows,
                    stream,
                    if completed {
                        "item completed".into()
                    } else {
                        "item started".into()
                    },
                );
                return true;
            };
            let key = log_item_key(item);
            let paired = if completed {
                key.as_ref().is_some_and(|key| take_started(started, key))
            } else {
                if let Some(key) = key {
                    *started.entry(key).or_default() += 1;
                }
                false
            };
            push_log_item(rows, stream, item, !completed || !paired, completed);
        }
        _ => {
            if let Some(message) = json_string(record, "message")
                .or_else(|| json_string(record, "text"))
                .or_else(|| json_string(record, "error"))
            {
                push_log_text(rows, SemanticLogKind::Text, stream, message);
            } else {
                push_log_status(
                    rows,
                    stream,
                    event_type.map_or_else(
                        || "event: JSON object".into(),
                        |kind| format!("event: {kind}"),
                    ),
                );
            }
        }
    }
    true
}

fn push_log_item(
    rows: &mut Vec<SemanticLogRow>,
    stream: RunStream,
    item: &serde_json::Map<String, serde_json::Value>,
    primary: bool,
    details: bool,
) {
    let item_type = json_string(item, "type");
    if primary {
        let label = match item_type {
            Some("agent_message") => {
                json_string(item, "text").map(|text| (SemanticLogKind::Message, text.to_owned()))
            }
            Some("command_execution") => json_string(item, "command")
                .map(|command| (SemanticLogKind::Command, command.to_owned())),
            Some("file_change") => {
                let files = changed_file_labels(item.get("changes"));
                Some((
                    SemanticLogKind::File,
                    if files == "file changes" {
                        files
                    } else {
                        format!("files: {files}")
                    },
                ))
            }
            Some("mcp_tool_call") => {
                let server = json_string(item, "server");
                let tool = json_string(item, "tool").or_else(|| json_string(item, "name"));
                let name = match (server, tool) {
                    (Some(server), Some(tool)) => format!("{server}/{tool}"),
                    (Some(name), None) | (None, Some(name)) => name.to_owned(),
                    (None, None) => "call".into(),
                };
                Some((SemanticLogKind::Tool, format!("MCP tool: {name}")))
            }
            Some(kind) => Some((SemanticLogKind::Text, format!("item: {kind}"))),
            None => None,
        };
        if let Some((kind, text)) = label {
            if matches!(kind, SemanticLogKind::Message | SemanticLogKind::Output) {
                push_log_text(rows, kind, stream, &text);
            } else {
                push_log_row(
                    rows,
                    SemanticLogRow {
                        kind,
                        stream: Some(stream),
                        text,
                    },
                );
            }
        }
    }
    if !details {
        return;
    }
    if item_type == Some("command_execution") {
        if let Some(output) =
            json_string(item, "aggregated_output").or_else(|| json_string(item, "output"))
        {
            push_log_text(rows, SemanticLogKind::Output, stream, output);
        }
    } else if item_type == Some("mcp_tool_call")
        && let Some(output) = json_string(item, "result").or_else(|| json_string(item, "error"))
    {
        push_log_text(rows, SemanticLogKind::Output, stream, output);
    }
    if let Some(status) = json_string(item, "status") {
        push_log_status(rows, stream, format!("status: {status}"));
    }
    if let Some(exit) = item.get("exit_code").and_then(|value| match value {
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::String(value) => Some(value.clone()),
        _ => None,
    }) {
        push_log_row(
            rows,
            SemanticLogRow {
                kind: SemanticLogKind::Exit,
                stream: Some(stream),
                text: format!("exit: {exit}"),
            },
        );
    }
}

fn log_item_key(item: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let kind = json_string(item, "type")?;
    if let Some(id) = json_string(item, "id") {
        return Some(format!("{kind}:id:{id}"));
    }
    let primary = match kind {
        "agent_message" => json_string(item, "text")?.to_owned(),
        "command_execution" => json_string(item, "command")?.to_owned(),
        "file_change" => changed_file_labels(item.get("changes")),
        "mcp_tool_call" => {
            let server = json_string(item, "server");
            let tool = json_string(item, "tool").or_else(|| json_string(item, "name"));
            match (server, tool) {
                (Some(server), Some(tool)) => format!("{server}/{tool}"),
                (Some(name), None) | (None, Some(name)) => name.to_owned(),
                (None, None) => "call".into(),
            }
        }
        _ => kind.to_owned(),
    };
    Some(format!("{kind}:value:{primary}"))
}

fn take_started(started: &mut BTreeMap<String, usize>, key: &str) -> bool {
    let Some(count) = started.get_mut(key) else {
        return false;
    };
    *count -= 1;
    if *count == 0 {
        started.remove(key);
    }
    true
}

fn changed_file_labels(value: Option<&serde_json::Value>) -> String {
    let files = value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|change| {
            change.as_str().map(str::to_owned).unwrap_or_else(|| {
                change
                    .as_object()
                    .and_then(|change| {
                        json_string(change, "path")
                            .or_else(|| json_string(change, "file"))
                            .or_else(|| json_string(change, "filename"))
                    })
                    .unwrap_or("changed file")
                    .to_owned()
            })
        })
        .collect::<Vec<_>>();
    if files.is_empty() {
        "file changes".into()
    } else {
        files.join(", ")
    }
}

fn json_string<'a>(
    record: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    record.get(key).and_then(serde_json::Value::as_str)
}

fn push_log_status(rows: &mut Vec<SemanticLogRow>, stream: RunStream, text: String) {
    push_log_row(
        rows,
        SemanticLogRow {
            kind: SemanticLogKind::Status,
            stream: Some(stream),
            text,
        },
    );
}

fn push_log_text(
    rows: &mut Vec<SemanticLogRow>,
    kind: SemanticLogKind,
    stream: RunStream,
    text: &str,
) {
    let mut text = normalize_log_text(&clip_log_field(text));
    while text.ends_with('\n') {
        text.pop();
    }
    if text.is_empty() {
        return;
    }
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let visible = if lines.len() > MAX_FIELD_LINES {
        let omitted = lines.len() - FIELD_HEAD_LINES - FIELD_TAIL_LINES;
        let mut visible = lines[..FIELD_HEAD_LINES].to_vec();
        visible.push(format!("… {omitted} lines omitted …"));
        visible.extend_from_slice(&lines[lines.len() - FIELD_TAIL_LINES..]);
        visible
    } else {
        lines
    };
    for line in visible {
        let gap = line.starts_with('…') && line.ends_with("omitted …");
        push_log_row(
            rows,
            SemanticLogRow {
                kind: if gap { SemanticLogKind::Gap } else { kind },
                stream: Some(stream),
                text: line,
            },
        );
    }
}

fn push_log_row(rows: &mut Vec<SemanticLogRow>, mut row: SemanticLogRow) {
    row.text = clip_log_row(&row.text);
    if row.text.trim().is_empty() {
        row.kind = SemanticLogKind::Blank;
        row.text.clear();
    }
    if row.kind == SemanticLogKind::Blank
        && rows
            .last()
            .is_some_and(|row| row.kind == SemanticLogKind::Blank)
    {
        return;
    }
    rows.push(row);
}

fn normalize_log_text(text: &str) -> String {
    let normalized = text
        .replace("\\r\\n", "\n")
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut output = String::with_capacity(normalized.len());
    let mut newlines = 0;
    for character in normalized.chars() {
        if character == '\n' {
            newlines += 1;
            if newlines <= 2 {
                output.push(character);
            }
        } else {
            newlines = 0;
            output.push(character);
        }
    }
    output
}

fn clip_log_field(text: &str) -> String {
    clip_log_text(text, MAX_FIELD_CHARS)
}

fn clip_log_row(text: &str) -> String {
    clip_log_text(text, MAX_ROW_CHARS)
}

fn clip_log_text(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let omitted = count - limit;
    let marker = format!(" … {omitted} characters omitted … ");
    let available = limit.saturating_sub(marker.chars().count()).max(2);
    let head = available / 3;
    let tail = available - head;
    let start = text.chars().take(head).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}{marker}{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_log_deduplicates_cursors_and_keeps_an_honest_tail() {
        let dispatch_id = "ab".repeat(32);
        let expanded = vec![dispatch_id.clone()];
        let mut logs = BTreeMap::new();
        for cursor in 1..=150 {
            apply_event(
                &mut logs,
                &expanded,
                RunLogEvent::Line {
                    dispatch_id: dispatch_id.clone(),
                    cursor,
                    stream: RunStream::Stdout,
                    text: format!("line {cursor}"),
                },
            );
        }
        apply_event(
            &mut logs,
            &expanded,
            RunLogEvent::Line {
                dispatch_id: dispatch_id.clone(),
                cursor: 150,
                stream: RunStream::Stderr,
                text: "duplicate".into(),
            },
        );
        let log = &logs[&dispatch_id];
        assert_eq!(log.entries.len(), MAX_ACTIVITY_ENTRIES);
        assert_eq!(log.dropped, 30);
        assert_eq!(log.last_cursor, 150);
        assert!(
            !log.entries.iter().any(
                |entry| matches!(entry, RunLogEntry::Line { text, .. } if text == "duplicate")
            )
        );
    }

    #[test]
    fn live_log_prettifies_jsonl_and_pairs_started_items() {
        let json_line = |value: serde_json::Value| RunLogEntry::Line {
            stream: RunStream::Stdout,
            text: value.to_string(),
        };
        let rows = semantic_log_rows(&[
            json_line(serde_json::json!({
                "type": "thread.started",
                "thread_id": "thread-123"
            })),
            json_line(serde_json::json!({ "type": "turn.started" })),
            json_line(serde_json::json!({
                "type": "item.started",
                "item": { "type": "command_execution", "command": "cargo test -p app" }
            })),
            json_line(serde_json::json!({
                "type": "item.completed",
                "item": {
                    "type": "command_execution",
                    "command": "cargo test -p app",
                    "aggregated_output": "running tests\n\n\nfinished\n",
                    "exit_code": 0,
                    "status": "completed"
                }
            })),
            json_line(serde_json::json!({
                "type": "item.started",
                "item": { "type": "agent_message", "text": "all tests passed" }
            })),
            json_line(serde_json::json!({
                "type": "item.completed",
                "item": { "type": "agent_message", "text": "all tests passed" }
            })),
            json_line(serde_json::json!({ "type": "turn.completed" })),
        ]);
        let summary = rows
            .iter()
            .map(|row| (row.kind, row.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            vec![
                (SemanticLogKind::Status, "thread started: thread-123"),
                (SemanticLogKind::Status, "turn started"),
                (SemanticLogKind::Command, "cargo test -p app"),
                (SemanticLogKind::Output, "running tests"),
                (SemanticLogKind::Blank, ""),
                (SemanticLogKind::Output, "finished"),
                (SemanticLogKind::Status, "status: completed"),
                (SemanticLogKind::Exit, "exit: 0"),
                (SemanticLogKind::Message, "all tests passed"),
                (SemanticLogKind::Status, "turn completed"),
            ]
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.kind == SemanticLogKind::Command)
                .count(),
            1
        );
        assert!(!rows.iter().any(|row| row.text.contains("item.completed")));
    }

    #[test]
    fn live_log_semantic_rows_bound_untrusted_multiline_output() {
        let output = (0..10_000)
            .map(|line| format!("output line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rows = semantic_log_rows(&[RunLogEntry::Line {
            stream: RunStream::Stdout,
            text: serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "cmd-1",
                    "type": "command_execution",
                    "command": "generate output",
                    "aggregated_output": output,
                    "exit_code": 101,
                    "status": "failed"
                }
            })
            .to_string(),
        }]);
        assert!(rows.len() <= MAX_ACTIVITY_ROWS);
        assert!(
            rows.iter()
                .all(|row| row.text.chars().count() <= MAX_ROW_CHARS)
        );
        assert!(rows.iter().any(|row| row.text.contains("lines omitted")));
        assert!(rows.iter().any(|row| row.text == "output line 9999"));
        assert_eq!(rows.last().map(|row| row.text.as_str()), Some("exit: 101"));
    }
}
