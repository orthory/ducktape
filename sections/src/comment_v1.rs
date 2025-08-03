use document;
use serde::{Deserialize, Serialize};

const COMMAND: &str = "/comment";

#[derive(thiserror::Error, Debug)]
pub enum CommentError {
    #[error("empty arguments set provided")]
    EmptyArgument,

    #[error("invalid argument length {0}, expected {1}")]
    InvalidArgumentLength(usize, usize),

    #[error("invalid argument in comment declaration at argument position {0}: {1}")]
    InvalidArguments(usize, String),
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommentV1 {
    parent_id: u64,
    timestamp: u64,
    author: String,
    body: Vec<String>,
}

impl crate::Section for CommentV1 {
    fn try_match<R: std::io::Read>(
        document: &mut document::DocumentBuffer<R>,
    ) -> anyhow::Result<Option<Self>> {
        let result = document.try_map_command_group(COMMAND, |arguments, body| match arguments {
            None => Err(CommentError::EmptyArgument),
            Some(arguments) if arguments.len() != 3 => Err(CommentError::InvalidArgumentLength(
                arguments.len(),
                3 as usize,
            )),
            Some(arguments) => Ok(Some(CommentV1 {
                parent_id: u64::from_str_radix(arguments[1], 10)
                    .map_err(|e| CommentError::InvalidArguments(1, e.to_string()))?,
                timestamp: u64::from_str_radix(arguments[2], 10)
                    .map_err(|e| CommentError::InvalidArguments(2, e.to_string()))?,
                author: arguments[0].to_string(),
                body: body,
            })),
        });

        Ok(result?)
    }
}

#[cfg(test)]
mod tests {
    use document::DocumentBuffer;

    use super::*;
    use common::Section;

    const SAMPLE_COMMENT: &str = r#"
/comment{@author;12345;12345}
This is a sample comment.
Multiline xyz is also supported
/comment
"#;

    #[test]
    fn test_asdf() {
        let mut buffer = DocumentBuffer::new(SAMPLE_COMMENT.trim_start().as_bytes());
        let comment = CommentV1::try_match(&mut buffer);
        dbg!(comment);
    }
}
