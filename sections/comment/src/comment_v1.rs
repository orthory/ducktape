use std::io::Read;

use serde::{Deserialize, Serialize};

use document;
use lazy_static::lazy_static;
use thiserror::Error;

const PATTERN: &str = "/comment";
const ARGUMENT_DELIMITER: &str = ";";

lazy_static! {
    static ref PATTERN_REGEX: regex::Regex = regex::Regex::new(r"^/(\w+)\{([^}]*)\}$").unwrap();
}

#[derive(Error, Debug)]
pub enum CommentError {
    #[error("invalid arguments formation in the comment command")]
    InvalidArguments,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommentV1 {
    parent_id: u64,
    timestamp: u64,
    author: String,
    body: String,
}

impl CommentV1 {
    pub fn try_match<R: Read>(
        document: &mut document::DocumentBuffer<R>,
    ) -> anyhow::Result<Option<Self>> {
        let match_result = document.try_match_command(PATTERN)?;
        if !match_result {
            return Ok(None);
        }

        // extract the first line and parse the arguments
        let Some(line) = document.try_read_line()? else {
            return Ok(None);
        };

        let capture = PATTERN_REGEX
            .captures(&line)
            .ok_or(CommentError::InvalidArguments)?;

        let author = capture.get(1).ok_or(CommentError::InvalidArguments)?;
        let parent_id = capture.get(2).ok_or(CommentError::InvalidArguments)?;
        let timestamp = capture.get(3).ok_or(CommentError::InvalidArguments)?;

        Ok(Some(Self {
            parent_id: parent_id.as_str().parse()?,
            timestamp: timestamp.as_str().parse()?,
            author: author.as_str().to_string(),
            body: String::new(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use document::DocumentBuffer;

    use super::*;

    const SAMPLE_COMMENT: &str = r#"
/comment{@author;12345;12345}
This is a sample comment.
/comment
"#;

    #[test]
    fn test_asdf() {
        let mut buffer = DocumentBuffer::new(SAMPLE_COMMENT.trim_start().as_bytes());
        let comment = CommentV1::try_match(&mut buffer);
        dbg!(comment);
    }
}
