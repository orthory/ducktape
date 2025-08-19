use std::{
    fs::File,
    io::Read,
    path::Path,
    time::{self, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{parser::Parser, sections::Sections};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Document {
    // global body buffer as vector of lines
    body: Vec<String>,

    // separate sections
    sections: Vec<Sections>,

    // misc data
    created_at: Option<u128>,
    updated_at: Option<u128>,
}

impl Default for Document {
    fn default() -> Self {
        let now = time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();
        Self {
            body: Default::default(),
            sections: Default::default(),
            created_at: Some(now),
            updated_at: Some(now),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DocumentInstanceError {
    #[error("DocumentInstance: io error")]
    IOError(String),

    #[error("DocumentInstance: error during parsing at line {1} (around {2}): {0}")]
    ParseError(anyhow::Error, usize, String),
}

impl Document {
    pub fn from_path(path: &Path) -> Result<Self, DocumentInstanceError> {
        let file = std::fs::File::options()
            .read(true)
            .open(&path)
            .map_err(|e| DocumentInstanceError::IOError(e.to_string()))?;

        Self::from_file(file)
    }

    pub fn from_file(file: File) -> Result<Self, DocumentInstanceError> {
        // create parser instance against the entire file,
        // and parse out all sections.
        let (body, sections) = try_instantiate_document(file)?;

        let timestamps = sections.iter().find_map(|section| match section {
            Sections::FrontmatterV1(fm) => Some((fm.created_at, fm.updated_at)),
            _ => None,
        });

        let (created_at, updated_at) = match timestamps {
            Some((c, u)) => (Some(c), Some(u)),
            None => (None, None),
        };

        Ok(Document {
            body,
            sections,
            created_at,
            updated_at,
        })
    }

    // todo: from_buffer?
}

fn try_instantiate_document<R: Read>(
    reader: R,
) -> Result<(Vec<String>, Vec<Sections>), DocumentInstanceError> {
    // create parser
    let mut parser = Parser::new(reader);

    let mut body: Vec<String> = Vec::new();
    let mut sections: Vec<Sections> = Vec::new();

    // loop over the buffer and parse out
    loop {
        match Sections::try_parse_sections(&mut parser) {
            Ok(Some(section)) => {
                sections.push(section);
            }
            Ok(None) => {
                // No section found, try matching body
                let next_line = parser.try_read_line().map_err(|e| {
                    let (line_pos, line) = parser.current_line();
                    DocumentInstanceError::ParseError(e.into(), line_pos, line)
                })?;
                match next_line {
                    Some(nl) => body.push(nl),
                    None => break, // EOF
                }
            }
            Err(e) => {
                let (line_pos, line) = parser.current_line();
                return Err(DocumentInstanceError::ParseError(e.into(), line_pos, line));
            }
        }
    }

    Ok((body, sections))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn try_instantiate_document_works() -> anyhow::Result<()> {
        let sample_document = r#"
---
title: test document
author: @orthory
created_at: 1234
updated_at: 1234
---

This is the general markdown block.
Multiline should also be supported,
and parsing shouldn't be deterred by
markdown directives, such as (-)

- List Item 1
- List Item 2
    - Liste Item 3

Let's test some commands

/comment{@orthory,1234;1234}
This is the comment block
/comment

How about some tasks?

/task{@author;some title;InProgress(https://github.com);12345;12345;@orthory;@ever0de;@lazka33;@0xF0D0;@jeffwoooo}
This is a sample comment.
Multiline xyz is also supported
/task

/task{@author;some title;InProgress(https://github.com);12345;12345;@orthory;@ever0de;@lazka33;@0xF0D0;@jeffwoooo}
This is a sample comment.
Multiline xyz is also supported
/task
        "#;

        let (body, sections) = try_instantiate_document(sample_document.as_bytes())?;
        dbg!(body, sections);
        Ok(())
    }
}
