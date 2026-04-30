use std::str::FromStr;

use crate::{Section, parser::Parser};
use serde::{Deserialize, Serialize};

const COMMAND: &str = "/task";

/// TaskV1
///
/// ```text
/// /task{@author;title;status(TaskV1Status);(start_at);(end_at);(assignees)...}
/// content...
/// /task
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
    pub title: String,
    pub body: Vec<String>,
    pub author: auth::User,
    pub assignees: Vec<auth::User>,
    pub start_at: u64,
    pub end_at: u64,
    pub status: TaskV1Status,
}

impl Section for TaskV1 {
    fn try_match<R: std::io::Read>(document: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let Some(matched) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        let args = matched.args.ok_or(TaskError::InvalidData)?;

        let author_raw = args.get(0).ok_or(TaskError::MissingData("author"))?;
        let title = args.get(1).ok_or(TaskError::MissingData("title"))?.clone();
        let status_raw = args.get(2).ok_or(TaskError::MissingData("status"))?;

        let author = auth::User::from_str(author_raw);
        let status = TaskV1Status::from_argument(status_raw)?;
        let start_at = parse_optional_u64(args.get(3), "start_at")?;
        let end_at = parse_optional_u64(args.get(4), "end_at")?;

        // Bounds-safe assignees slice — empty when args.len() <= 5.
        let assignees: Vec<auth::User> = args
            .get(5..)
            .unwrap_or(&[])
            .iter()
            .map(|a| auth::User::from_str(a.as_str()))
            .collect();

        Ok(Some(TaskV1 {
            title,
            author,
            assignees,
            start_at,
            end_at,
            status,
            body: matched.body,
        }))
    }
}

fn parse_optional_u64(value: Option<&String>, field: &'static str) -> Result<u64, TaskError> {
    match value {
        None => Ok(0),
        Some(s) if s.is_empty() => Ok(0),
        Some(s) => s
            .parse::<u64>()
            .map_err(|e| TaskError::InvalidArgument(field, e.to_string())),
    }
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
/task{@author;some title;InProgress(https://github.com);12345;12346;@orthory;@ever0de}
body line 1
body line 2
/task
";
        let mut buffer = Parser::new(input.as_bytes());
        let task = TaskV1::try_match(&mut buffer)
            .expect("parse ok")
            .expect("matched");

        assert_eq!(task.title, "some title");
        assert_eq!(task.start_at, 12345);
        assert_eq!(task.end_at, 12346);
        assert_eq!(task.assignees.len(), 2);
        assert_eq!(task.body.len(), 2);
        assert!(matches!(task.status, TaskV1Status::InProgress(Some(ref u)) if u == "https://github.com"));
    }

    #[test]
    fn minimal_task_does_not_panic() {
        // Only the three required args — used to panic at variables.split_at(5).
        let input = "\
/task{@author;title;Backlog}
body
/task
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
