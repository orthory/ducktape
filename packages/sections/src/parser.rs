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

/// Result of a successful command-group match.
///
/// `args` is `None` when the opening line is a bare command (e.g. `---`)
/// and `Some(_)` when the opener carries `{a;b;c}` arguments.
#[derive(Debug)]
pub struct MatchedGroup {
    pub args: Option<Vec<String>>,
    pub body: Vec<String>,
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

    /// Returns true when the peeked line opens a `command` group: either an
    /// exact match (`---`) or `command{...args}` form. Trailing whitespace
    /// or junk between the command and `{` is rejected — the close marker
    /// is strict, so the open marker has to be too.
    pub fn try_match_command(&mut self, command: &str) -> Result<bool, ParserError> {
        match self.linefeed.peek() {
            Some(Ok(line)) => Ok(line_opens_command(line, command)),
            Some(Err(e)) => Err(ParserError::IOError {
                kind: e.kind().to_string(),
                message: e.to_string(),
            }),
            _ => Ok(false),
        }
    }

    /// Advance one line and bump the position counter.
    fn advance(&mut self) -> Option<io::Result<String>> {
        let next = self.linefeed.next();
        if next.is_some() {
            self.current_line_pos += 1;
        }
        next
    }

    pub fn try_read_line(&mut self) -> Result<Option<String>, io::Error> {
        self.advance().transpose()
    }

    pub fn try_map_command_group(
        &mut self,
        command: &str,
    ) -> Result<Option<MatchedGroup>, ParserError> {
        if !self.try_match_command(command)? {
            return Ok(None);
        }

        let first_line = self
            .advance()
            .ok_or(ParserError::UnexpectedEOF)?
            .map_err(io_to_parser)?;
        let args = parse_args(&first_line);

        let mut body: Vec<String> = Vec::new();
        let mut closed = false;
        while let Some(next_line) = self.advance() {
            let next_line = next_line.map_err(io_to_parser)?;

            if next_line == command {
                closed = true;
                break;
            }

            body.push(next_line);
        }

        if !closed {
            return Err(ParserError::UnexpectedEOF);
        }

        Ok(Some(MatchedGroup { args, body }))
    }
}

fn line_opens_command(line: &str, command: &str) -> bool {
    if line == command {
        return true;
    }
    line.len() > command.len()
        && line.starts_with(command)
        && line.as_bytes()[command.len()] == b'{'
}

fn io_to_parser(e: io::Error) -> ParserError {
    ParserError::IOError {
        kind: e.kind().to_string(),
        message: e.to_string(),
    }
}

lazy_static! {
    // /command{a;b;c} — args are everything between the braces.
    static ref ARGS_REGEX: regex::Regex = regex::Regex::new(r"^/\S+\{(.*)\}$").unwrap();
}

pub fn parse_args(input: &str) -> Option<Vec<String>> {
    ARGS_REGEX.captures(input).map(|caps| {
        caps.get(1)
            .unwrap()
            .as_str()
            .split(';')
            .map(|s| s.trim().to_string())
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_trims_whitespace() {
        assert_eq!(
            parse_args("/cmd{a; b;  c }"),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
        );
    }

    #[test]
    fn parse_args_returns_none_for_bare_command() {
        assert_eq!(parse_args("---"), None);
        assert_eq!(parse_args("/comment"), None);
    }

    #[test]
    fn try_map_command_group_collects_body() {
        let input = "/command{a;b;c}\nfirst\nsecond\n/command\n";
        let mut p = Parser::new(input.as_bytes());

        let m = p
            .try_map_command_group("/command")
            .unwrap()
            .expect("expected match");
        assert_eq!(
            m.args,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
        assert_eq!(m.body, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn try_map_command_group_rejects_loose_open() {
        // Trailing junk on the opener must not match — close marker is strict.
        let input = "/comment trailing\nbody\n/comment\n";
        let mut p = Parser::new(input.as_bytes());
        assert!(p.try_map_command_group("/comment").unwrap().is_none());
    }

    #[test]
    fn try_map_command_group_unexpected_eof() {
        let input = "/comment{a;b;c}\nbody\n";
        let mut p = Parser::new(input.as_bytes());
        let err = p.try_map_command_group("/comment").unwrap_err();
        matches!(err, ParserError::UnexpectedEOF);
    }

    #[test]
    fn line_position_advances() {
        let input = "one\ntwo\nthree\n";
        let mut p = Parser::new(input.as_bytes());
        let _ = p.try_read_line().unwrap();
        let _ = p.try_read_line().unwrap();
        assert_eq!(p.current_line().0, 2);
    }
}
