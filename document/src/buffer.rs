use std::{
    io::{self, BufRead, BufReader, Lines, Read},
    iter::Peekable,
};

use thiserror::Error;

use crate::variables::parse_variable;

#[derive(Error, Debug)]
pub enum DocumentBufferError {
    #[error("IO error during handling buffer")]
    IOError {
        kind: std::io::ErrorKind,
        message: String,
    },

    #[error("underlying group processor errored: {0}")]
    ProcessorError(String),

    #[error("reached EOF unexpectedly")]
    UnexpectedEOF,
}

// DocumentBuffer is a simple wrapper around a reader that provides a line-based interface.
// its main purpose is to hold Lines as a mutable buffer
pub struct DocumentBuffer<R>
where
    R: Read,
{
    linefeed: Peekable<Lines<BufReader<R>>>,
}

impl<R> DocumentBuffer<R>
where
    R: Read,
{
    pub fn new(source: R) -> Self {
        let linefeed = BufReader::new(source).lines().peekable();
        Self { linefeed }
    }

    pub fn try_match_command(&mut self, command: &str) -> Result<bool, DocumentBufferError> {
        match self.linefeed.peek() {
            // match if peeked line starts with this command
            Some(Ok(line)) => Ok(line.starts_with(command)),

            // handle any error while peeking
            Some(Err(e)) => Err(DocumentBufferError::IOError {
                kind: e.kind(),
                message: e.to_string(),
            }),

            // otherwise, return false,
            _ => Ok(false),
        }
    }

    pub fn try_read_line(&mut self) -> Result<Option<String>, io::Error> {
        self.linefeed.next().transpose()
    }

    pub fn try_map_command_group<ProcT, ProcE: std::error::Error>(
        &mut self,
        command: &str,
        processor: impl FnOnce(Option<Vec<&str>>, Vec<String>) -> Result<Option<ProcT>, ProcE>,
    ) -> Result<Option<ProcT>, DocumentBufferError> {
        // if next line doesn't contain any command, skip
        if !self.try_match_command(command)? {
            return Ok(None);
        }

        // otherwise try to read the command block
        // until the next occurrence, and provide as Vec<String>
        let first_line = self
            .linefeed
            .next()
            .ok_or(DocumentBufferError::UnexpectedEOF)?
            .map_err(|e| DocumentBufferError::IOError {
                kind: e.kind(),
                message: e.to_string(),
            })?;
        let variables = parse_variable(&first_line);
        let mut group_body: Vec<String> = Vec::new();

        // loop until we find another command block
        while let Some(next_line) = self.linefeed.next() {
            let next_line = next_line.map_err(|e| DocumentBufferError::IOError {
                kind: e.kind(),
                message: e.to_string(),
            })?;

            // found end of the command block, break
            if next_line == command {
                break;
            }

            group_body.push(next_line);
        }

        // run processor using the entire body
        match processor(variables, group_body) {
            Ok(Some(proc_result)) => Ok(Some(proc_result)),
            Ok(None) => Ok(None),
            Err(e) => Err(DocumentBufferError::ProcessorError(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STRING: &str = r#"
---
title: this is title
author: @orthory
date: 12345
---

# H1
## H2
### H3

/command[a,b,c]
command-body
command-body
/command
"#;

    #[test]
    fn test_read_line() {
        let mut buffer = DocumentBuffer::new(TEST_STRING.as_bytes());

        while let Ok(Some(line)) = buffer.try_read_line() {
            dbg!(line);
        }
    }

    #[test]
    fn test_try_map_command_group() {
        let test_string: &str = r#"
/command{a;b;c}
command-body
command-body
/command
        "#;
        let mut buffer = DocumentBuffer::new(test_string.trim_start().as_bytes());

        {
            #[derive(Debug)]
            struct SimpleCommandProcessor {
                arguments: Vec<String>,
                commands: Vec<String>,
            }

            #[derive(Error, Debug)]
            enum CommandError {
                #[error("")]
                InvalidCommand,
            }

            let simple_success = buffer
                .try_map_command_group::<SimpleCommandProcessor, CommandError>(
                    "/command",
                    |arguments, group_body| {
                        Ok(Some(SimpleCommandProcessor {
                            arguments: arguments.unwrap().iter().map(|a| a.to_string()).collect(),
                            commands: group_body.clone(),
                        }))
                    },
                );

            dbg!(simple_success);
        }
    }
}
