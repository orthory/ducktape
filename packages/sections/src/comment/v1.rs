use crate::{Section, parser::Parser};
use serde::{Deserialize, Serialize};

const COMMAND: &str = "/comment";

#[derive(thiserror::Error, Debug)]
pub enum CommentError {
    #[error("CommentV1: empty arguments set provided")]
    EmptyArgument,

    #[error("CommentV1({0}) invalid argument length {1}, expected {2}")]
    InvalidArgumentLength(String, usize, usize),

    #[error(
        "CommentV1({0}): invalid argument in comment declaration at argument position {1}: {2}"
    )]
    InvalidArguments(String, usize, String),
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CommentV1 {
    parent_id: u64,
    timestamp: u64,
    author: String,
    body: Vec<String>,
}

impl Section for CommentV1 {
    fn try_match<R: std::io::Read>(document: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let Some(matched) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        let args = matched.args.ok_or(CommentError::EmptyArgument)?;
        let joined = args.join(";");
        let joined_comma = args.join(",");
        let len = args.len();
        let mut it = args.into_iter();
        let too_short = || CommentError::InvalidArgumentLength(joined.clone(), len, 3);

        let author = it.next().ok_or_else(too_short)?;
        let parent_id = it
            .next()
            .ok_or_else(too_short)?
            .parse::<u64>()
            .map_err(|e| CommentError::InvalidArguments(joined_comma.clone(), 1, e.to_string()))?;
        let timestamp = it
            .next()
            .ok_or_else(too_short)?
            .parse::<u64>()
            .map_err(|e| CommentError::InvalidArguments(joined_comma, 2, e.to_string()))?;
        if it.next().is_some() {
            return Err(CommentError::InvalidArgumentLength(joined, len, 3).into());
        }

        Ok(Some(CommentV1 {
            parent_id,
            timestamp,
            author,
            body: matched.body,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE_COMMENT: &str = r#"
/comment{@orthory;42;1700000000}
This is a sample comment.
Multiline xyz is also supported
/comment
"#;

    #[test]
    fn parses_comment() {
        let mut buffer = Parser::new(SAMPLE_COMMENT.trim_start().as_bytes());
        let comment = CommentV1::try_match(&mut buffer)
            .expect("parse ok")
            .expect("comment matched");
        assert_eq!(comment.author, "@orthory");
        assert_eq!(comment.parent_id, 42);
        assert_eq!(comment.timestamp, 1700000000);
        assert_eq!(comment.body.len(), 2);
    }

    #[test]
    fn rejects_wrong_arg_count() {
        let input = "/comment{@only;one}\nbody\n/comment\n";
        let mut buffer = Parser::new(input.as_bytes());
        assert!(CommentV1::try_match(&mut buffer).is_err());
    }
}
