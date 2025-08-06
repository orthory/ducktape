use crate::{parser::Parser, sections::Section};
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
pub struct CommentV1 {
    parent_id: u64,
    timestamp: u64,
    author: String,
    body: Vec<String>,
}

impl Section for CommentV1 {
    fn try_match<R: std::io::Read>(document: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let Some((variables, body)) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        Ok(match variables {
            None => Err(CommentError::EmptyArgument),
            Some(variables) if variables.len() != 3 => Err(CommentError::InvalidArgumentLength(
                variables.join(";"),
                variables.len(),
                3 as usize,
            )),
            Some(variables) => Ok(Some(CommentV1 {
                author: variables[0].to_string(),
                parent_id: u64::from_str_radix(&variables[1], 10).map_err(|e| {
                    CommentError::InvalidArguments(variables.join(","), 1, e.to_string())
                })?,
                timestamp: u64::from_str_radix(&variables[2], 10).map_err(|e| {
                    CommentError::InvalidArguments(variables.join(","), 2, e.to_string())
                })?,
                body: body,
            })),
        }?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE_COMMENT: &str = r#"
/comment{@author;12345;12345}
This is a sample comment.
Multiline xyz is also supported
/comment
"#;

    #[test]
    fn test_asdf() {
        let mut buffer = Parser::new(SAMPLE_COMMENT.trim_start().as_bytes());
        let comment = CommentV1::try_match(&mut buffer);
        dbg!(comment);
    }
}
