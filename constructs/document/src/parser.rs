use serde::{Deserialize, Serialize};
use std::{
    io::{self, BufRead, BufReader, Lines, Read},
    iter::Peekable,
};

use lazy_static::lazy_static;
use thiserror::Error;

#[derive(Deserialize, Serialize, Error, Debug)]
pub enum ParserError {
    #[error("IO error during handling buffer")]
    IOError { kind: String, message: String },

    #[error("reached EOF unexpectedly")]
    UnexpectedEOF,
}

// DocumentBuffer is a simple wrapper around a reader that provides a line-based interface.
// its main purpose is to hold Lines as a mutable buffer
pub struct Parser<R>
where
    R: Read,
{
    linefeed: Peekable<Lines<BufReader<R>>>,
    current_line_pos: usize,
}

impl<R> Parser<R>
where
    R: Read,
{
    pub fn new(source: R) -> Self {
        let linefeed = BufReader::new(source).lines().peekable();
        Self {
            linefeed,
            current_line_pos: 0,
        }
    }

    pub fn current_line(&mut self) -> (usize, String) {
        let peeked = match self.linefeed.peek() {
            Some(Ok(r)) => r.to_string(),
            Some(Err(_)) | None => "failed to peek line for error :(".to_string(),
        };

        (self.current_line_pos, peeked)
    }

    pub fn try_match_command(&mut self, command: &str) -> Result<bool, ParserError> {
        match self.linefeed.peek() {
            // match if peeked line starts with this command
            Some(Ok(line)) => Ok(line.starts_with(command)),

            // handle any error while peeking
            Some(Err(e)) => Err(ParserError::IOError {
                kind: e.kind().to_string(),
                message: e.to_string(),
            }),

            // otherwise, return false,
            _ => Ok(false),
        }
    }

    pub fn try_read_line(&mut self) -> Result<Option<String>, io::Error> {
        self.current_line_pos = self.current_line_pos + 1;
        self.linefeed.next().transpose()
    }

    pub fn try_map_command_group(
        &mut self,
        command: &str,
    ) -> Result<Option<(Option<Vec<String>>, Vec<String>)>, ParserError> {
        // if next line doesn't contain any command, skip
        if !self.try_match_command(command)? {
            return Ok(None);
        }

        // otherwise try to read the command block
        // until the next occurrence, and provide as Vec<String>
        let first_line = self
            .linefeed
            .next()
            .ok_or(ParserError::UnexpectedEOF)?
            .map_err(|e| ParserError::IOError {
                kind: e.kind().to_string(),
                message: e.to_string(),
            })?;
        let variables = parse(first_line);
        let mut group_body: Vec<String> = Vec::new();

        // boolean value indicating whether section parsing has successfully
        // finished. this is so that we can filter out unclosed sections,
        // i.e. we saw the opening /comment but not the ending
        let mut is_matching_command_found = false;

        // loop until we find another command block
        while let Some(next_line) = self.linefeed.next() {
            let next_line = next_line.map_err(|e| ParserError::IOError {
                kind: e.kind().to_string(),
                message: e.to_string(),
            })?;

            // found end of the command block, break
            if next_line == command {
                is_matching_command_found = true;
                break;
            }

            group_body.push(next_line);
        }

        // if matching closure isn't found, we saw an unexpected EOF
        if !is_matching_command_found {
            return Err(ParserError::UnexpectedEOF);
        }

        Ok(Some((variables, group_body)))
    }
}

lazy_static! {
    static ref PATTERN_REGEX: regex::Regex = regex::Regex::new(r"^\/[\w\W]+\{([\w\W]+)}$").unwrap();
}

pub fn parse(input: String) -> Option<Vec<String>> {
    match PATTERN_REGEX.captures(&input) {
        None => None,
        Some(captures) => Some(
            captures
                .get(1)
                .unwrap()
                .as_str()
                .split(";")
                .map(|f| f.to_string())
                .collect(),
        ),
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
        let mut buffer = Parser::new(TEST_STRING.as_bytes());

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
        let buffer = Parser::new(test_string.trim_start().as_bytes());

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

            // let simple_success = buffer.try_map_command_group::<SimpleCommandProcessor, ()>(
            //     "/command",
            //     |arguments, group_body| {
            //         Ok(Some(SimpleCommandProcessor {
            //             arguments: arguments.unwrap().iter().map(|a| a.to_string()).collect(),
            //             commands: group_body.clone(),
            //         }))
            //     },
            // );

            // dbg!(simple_success);
        }
    }
}
