use std::str::FromStr;

use crate::{Node, parser::Parser};
use serde::{Deserialize, Serialize};
use uid::{Identify, Uid};

const COMMAND: &str = "/task.v1";

/// TaskV1
///
/// ```text
/// /task.v1{@author;title;status(TaskV1Status);(start_at);(end_at);(assignees)...}
/// content...
/// /task.v1
/// ```
#[derive(thiserror::Error, Debug, Deserialize, Serialize)]
pub enum TaskError {
    #[error("invalid task data")]
    InvalidData,

    #[error("required field missing: {0}")]
    MissingData(&'static str),

    #[error("invalid field {0}: {1}")]
    InvalidArgument(&'static str, String),

    #[error("invariant: {0}")]
    Invariant(String),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TaskV1 {
    // Stable identity. v1 on-disk format doesn't carry uids; the parser mints
    // a fresh one at parse time. v2 will carry it in args.
    pub uid: Uid,
    pub title: String,
    pub body: String,
    pub author: auth::User,
    pub assignees: Vec<auth::User>,
    pub start_at: u64,
    pub end_at: u64,
    pub status: TaskV1Status,
}

impl Identify for TaskV1 {
    fn uid(&self) -> Uid {
        self.uid
    }
}

impl Node for TaskV1 {
    fn try_match<R: std::io::Read>(document: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let Some(matched) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        let mut it = matched.args.ok_or(TaskError::InvalidData)?.into_iter();

        let author = auth::User::from_str(&it.next().ok_or(TaskError::MissingData("author"))?);
        let title = it.next().ok_or(TaskError::MissingData("title"))?;
        let status =
            TaskV1Status::from_argument(&it.next().ok_or(TaskError::MissingData("status"))?)?;
        let start_at = match it.next() {
            Some(s) => parse_u64(&s, "start_at")?,
            None => 0,
        };
        let end_at = match it.next() {
            Some(s) => parse_u64(&s, "end_at")?,
            None => 0,
        };
        let assignees: Vec<auth::User> = it.map(|a| auth::User::from_str(&a)).collect();

        Ok(Some(TaskV1 {
            // v1 markdown doesn't carry a uid; mint a fresh one at parse time.
            // v2 will read it from args.
            uid: uid::new(),
            title,
            author,
            assignees,
            start_at,
            end_at,
            status,
            body: matched.body.join("\n"),
        }))
    }

    // `/task.v1{author;title;status;start_at;end_at;assignee...}\n<body>\n/task.v1`.
    // Arg order mirrors the parser. start_at/end_at are always emitted (even 0),
    // assignees are appended one-per-arg. author/assignees render through
    // `auth::User`'s Display; status through `TaskV1Status::render`. Bare `;`
    // separators (parser trims args). uid is not rendered, body is verbatim.
    fn render(&self) -> String {
        let mut args = format!(
            "{};{};{};{};{}",
            self.author,
            self.title,
            self.status.render(),
            self.start_at,
            self.end_at,
        );
        for assignee in &self.assignees {
            args.push(';');
            args.push_str(&assignee.to_string());
        }

        format!("/task.v1{{{}}}\n{}\n/task.v1", args, self.body)
    }
}

fn parse_u64(s: &str, field: &'static str) -> Result<u64, TaskError> {
    if s.is_empty() {
        return Ok(0);
    }
    s.parse::<u64>()
        .map_err(|e| TaskError::InvalidArgument(field, e.to_string()))
}

/// TaskV1Status
///
/// `{status}` or `{status}(tracker)` — tracker URL is optional.
/// e.g. `InProgress(https://github.com/newmetric/...)` or `Backlog`.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TaskV1Status {
    Backlog(Option<String>),
    InProgress(Option<String>),
    InReview(Option<String>),
    Done(Option<String>),
    Merged(Option<String>),
    Unknown,
}

lazy_static::lazy_static! {
    // status name (group 1), optional `(tracker)` (group 2 captures inner text).
    static ref STATUS_PATTERN: regex::Regex =
        regex::Regex::new(r"^([\w-]+)(?:\(([^)]*)\))?$").unwrap();
}

impl FromStr for TaskV1Status {
    type Err = TaskError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_argument(s)
    }
}

impl TaskV1Status {
    /// Render to the `Status` or `Status(tracker)` form `from_argument` parses.
    /// Names are the canonical capitalization (`InProgress`, not `in-progress`);
    /// `from_argument` lowercases before matching, so they round-trip. Note the
    /// parse is lossy for unrecognized input — any unknown string becomes
    /// `Unknown`, which renders as `"Unknown"`; byte-idempotence still holds
    /// (`Unknown` re-parses to `Unknown`), but the original text isn't recovered.
    pub fn render(&self) -> String {
        let (name, tracker) = match self {
            TaskV1Status::Backlog(t) => ("Backlog", t),
            TaskV1Status::InProgress(t) => ("InProgress", t),
            TaskV1Status::InReview(t) => ("InReview", t),
            TaskV1Status::Done(t) => ("Done", t),
            TaskV1Status::Merged(t) => ("Merged", t),
            TaskV1Status::Unknown => return "Unknown".to_string(),
        };
        match tracker {
            Some(url) => format!("{}({})", name, url),
            None => name.to_string(),
        }
    }

    pub fn from_argument(argument: &str) -> Result<Self, TaskError> {
        let Some(captures) = STATUS_PATTERN.captures(argument) else {
            return Ok(Self::Unknown);
        };

        let status = captures
            .get(1)
            .ok_or_else(|| {
                TaskError::InvalidArgument(
                    "status",
                    format!("failed to parse status: {}", argument),
                )
            })?
            .as_str();
        let tracker = captures.get(2).map(|m| m.as_str().to_string());

        Ok(match status.to_ascii_lowercase().as_str() {
            "backlog" => Self::Backlog(tracker),
            "inprogress" | "in-progress" => Self::InProgress(tracker),
            "inreview" | "in-review" => Self::InReview(tracker),
            "done" => Self::Done(tracker),
            "merged" => Self::Merged(tracker),
            _ => Self::Unknown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_task() {
        let input = "\
/task.v1{@author;some title;InProgress(https://github.com);12345;12346;@orthory;@ever0de}
body line 1
body line 2
/task.v1
";
        let mut buffer = Parser::new(input.as_bytes());
        let task = TaskV1::try_match(&mut buffer)
            .expect("parse ok")
            .expect("matched");

        assert_eq!(task.title, "some title");
        assert_eq!(task.start_at, 12345);
        assert_eq!(task.end_at, 12346);
        assert_eq!(task.assignees.len(), 2);
        assert_eq!(task.body, "body line 1\nbody line 2");
        assert!(matches!(task.status, TaskV1Status::InProgress(Some(ref u)) if u == "https://github.com"));
    }

    #[test]
    fn render_round_trips_through_parse() {
        let input = "\
/task.v1{@author;some title;InProgress(https://github.com);12345;12346;@orthory;@ever0de}
body line 1
body line 2
/task.v1
";
        let mut p = Parser::new(input.as_bytes());
        let t = TaskV1::try_match(&mut p).expect("parse ok").expect("matched");
        let rendered = t.render();

        let mut p2 = Parser::new(rendered.as_bytes());
        let t2 = TaskV1::try_match(&mut p2)
            .expect("reparse ok")
            .expect("rematched");
        assert_eq!(t2.title, t.title);
        assert_eq!(t2.author.to_string(), t.author.to_string());
        assert_eq!(t2.start_at, t.start_at);
        assert_eq!(t2.end_at, t.end_at);
        assert_eq!(t2.body, t.body);
        assert_eq!(t2.status.render(), t.status.render());
        let a1: Vec<String> = t.assignees.iter().map(|u| u.to_string()).collect();
        let a2: Vec<String> = t2.assignees.iter().map(|u| u.to_string()).collect();
        assert_eq!(a1, a2);
    }

    #[test]
    fn minimal_task_does_not_panic() {
        // Only the three required args — used to panic at variables.split_at(5).
        let input = "\
/task.v1{@author;title;Backlog}
body
/task.v1
";
        let mut buffer = Parser::new(input.as_bytes());
        let task = TaskV1::try_match(&mut buffer)
            .expect("parse ok")
            .expect("matched");

        assert_eq!(task.start_at, 0);
        assert_eq!(task.end_at, 0);
        assert!(task.assignees.is_empty());
        assert!(matches!(task.status, TaskV1Status::Backlog(None)));
    }

    #[test]
    fn bare_status_without_tracker_parses() {
        assert!(matches!(
            TaskV1Status::from_argument("Done").unwrap(),
            TaskV1Status::Done(None)
        ));
        assert!(matches!(
            TaskV1Status::from_argument("in-review").unwrap(),
            TaskV1Status::InReview(None)
        ));
    }

    #[test]
    fn status_with_tracker_extracts_inner() {
        assert!(matches!(
            TaskV1Status::from_argument("Merged(https://example.com/pr/1)").unwrap(),
            TaskV1Status::Merged(Some(ref u)) if u == "https://example.com/pr/1"
        ));
    }
}
