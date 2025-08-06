use std::str::FromStr;

use crate::{parser::Parser, sections::Section};
use auth;
use serde::{Deserialize, Serialize};

const COMMAND: &str = "/task";

/// TaskV1
///
/// ```
/// /task{@author;title;status(TaskV1Status);(start_at);(end_at);(assignees)...}
/// content...
/// /task
///

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
        let Some((variables, body)) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        let variables = variables.ok_or(TaskError::InvalidData)?;

        let author =
            auth::User::from_str(variables.get(0).ok_or(TaskError::MissingData("author"))?);
        let title = variables.get(1).ok_or(TaskError::MissingData("title"))?;
        let start_at = variables
            .get(3)
            .unwrap_or(&"0".to_string())
            .parse::<u64>()
            .map_err(|e| TaskError::InvalidArgument("start_at", e.to_string()))?;
        let end_at = variables
            .get(4)
            .unwrap_or(&"0".to_string())
            .parse::<u64>()
            .map_err(|e| TaskError::InvalidArgument("end_at", e.to_string()))?;
        let assignees: Vec<auth::User> = variables
            .split_at(5)
            .1
            .to_vec()
            .into_iter()
            .map(|assignee| auth::User::from_str(assignee.as_str()))
            .collect();

        // parse out status
        let status = variables.get(2).ok_or(TaskError::MissingData("status"))?;
        let status = TaskV1Status::from_argument(status)?;

        Ok(Some(TaskV1 {
            title: title.to_string(),
            author: author,
            assignees: assignees,
            start_at: start_at,
            end_at: end_at,
            status: status,

            body,
        }))
    }
}

/// TaskV1Status
///
/// {status}(tracker)
/// e.g. InProgress(https://github.com/newmetric/...)
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
    static ref STATUS_PATTERN: regex::Regex = regex::Regex::new(r"(\w+)(\([\w\W]*\))").unwrap();
}

impl FromStr for TaskV1Status {
    type Err = TaskError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_argument(s)
    }
}

impl TaskV1Status {
    pub fn from_argument(argument: &str) -> Result<Self, TaskError> {
        match STATUS_PATTERN.captures(argument) {
            // regex couldn't match with anything; return unknown
            None => Ok(Self::Unknown),

            // otherwise try to match
            Some(captures) => {
                // get the first signifier of TaskV1Status.
                // since we're already in the Some block this should rarely happen,
                // but just incase :)
                let Some(status) = captures.get(1) else {
                    return Err(TaskError::InvalidArgument(
                        "status",
                        format!("failed to parse status to TaskV1Status: {}", argument).to_string(),
                    ));
                };

                let tracker = match captures.get(2) {
                    None => None,

                    // strip out ( and ) at the end
                    Some(tracker_matched) => {
                        let tracker = tracker_matched.as_str();
                        let tracker = tracker
                            .strip_prefix("(")
                            .ok_or(TaskError::InvalidArgument(
                                "status",
                                "malformed tracker: expected ( at the beginning, found none"
                                    .to_string(),
                            ))?
                            .strip_suffix(")")
                            .ok_or(TaskError::InvalidArgument(
                                "status",
                                "malformed tracker: expected ) at the end, found none".to_string(),
                            ))?;

                        Some(tracker.to_string())
                    }
                };

                match status.as_str() {
                    "backlog" | "Backlog" => Ok(TaskV1Status::Backlog(tracker)),
                    "inprogress" | "InProgress" | "in-progress" => {
                        Ok(TaskV1Status::InProgress(tracker))
                    }
                    "inreivew" | "InReview" | "in-review" => Ok(TaskV1Status::InReview(tracker)),
                    "done" | "Done" => Ok(TaskV1Status::Done(tracker)),
                    "merged" | "Merged" => Ok(TaskV1Status::Merged(tracker)),
                    _ => Ok(TaskV1Status::Unknown),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE_COMMENT: &str = r#"
/task{@author;some title;InProgress(https://github.com);12345;12345;@orthory;@ever0de;@lazka33;@0xF0D0;@jeffwoooo}
This is a sample comment.
Multiline xyz is also supported
/task
"#;

    #[test]
    fn test_asdf() {
        let mut buffer = Parser::new(SAMPLE_COMMENT.trim_start().as_bytes());
        let comment = TaskV1::try_match(&mut buffer);
        dbg!(comment);
    }
}
