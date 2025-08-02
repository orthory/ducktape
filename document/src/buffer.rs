use std::{
    io::{self, BufRead, BufReader, Lines, Read},
    iter::Peekable,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocumentBufferError {
    #[error("IO error during handling buffer")]
    IOError {
        kind: std::io::ErrorKind,
        message: String,
    },
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
/command
"#;

    #[test]
    fn test_read_line() {
        let mut buffer = DocumentBuffer::new(TEST_STRING.as_bytes());

        while let Some(line) = buffer.try_read_line() {
            dbg!(line.unwrap());
        }
    }
}
