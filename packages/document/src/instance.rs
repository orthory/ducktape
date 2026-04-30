use std::{io::Read, path::Path};

use serde::{Deserialize, Serialize};

use sections::{Sections, parser::Parser};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Document {
    // global body buffer as vector of lines
    pub(crate) body: Vec<String>,

    // separate sections
    pub(crate) sections: Vec<Sections>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            body: Default::default(),
            sections: Default::default(),
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

        Self::from_reader(file)
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Self, DocumentInstanceError> {
        let (body, sections) = try_instantiate_document(reader)?;

        Ok(Document { body, sections })
    }
}

fn try_instantiate_document<R: Read>(
    reader: R,
) -> Result<(Vec<String>, Vec<Sections>), DocumentInstanceError> {
    // create parser
    let mut parser = Parser::new(reader);

    // create holders for body and section
    // note that body is really just a vector of string, carrying each line
    // sections are enum defined by Sections
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

/comment.v1{@orthory;1234;1234}
This is the comment block
/comment.v1

How about some tasks?

/task.v1{@author;some title;InProgress(https://github.com);12345;12345;@orthory;@ever0de;@lazka33;@0xF0D0;@jeffwoooo}
This is a sample comment.
Multiline xyz is also supported
/task.v1

/task.v1{@author;some title;InProgress(https://github.com);12345;12345;@orthory;@ever0de;@lazka33;@0xF0D0;@jeffwoooo}
This is a sample comment.
Multiline xyz is also supported
/task.v1
        "#;

        let (body, sections) = try_instantiate_document(sample_document.as_bytes())?;
        dbg!(body, sections);
        Ok(())
    }

    #[test]
    pub fn read_and_structured_match_parsed_sections() -> anyhow::Result<()> {
        let sample_document = r#"
---
title: t
author: @a
created_at: 1
updated_at: 1
---

/comment.v1{@orthory;1234;1234}
hello
/comment.v1
"#;

        let doc = Document::from_reader(sample_document.as_bytes())?;

        let bulk = doc.sections();
        let streamed: Vec<_> = doc.sections_iter().collect();

        assert_eq!(bulk.len(), 2);
        assert_eq!(streamed.len(), bulk.len());
        assert!(matches!(bulk[0], Sections::Frontmatter(_)));
        assert!(matches!(bulk[1], Sections::Comment(_)));

        Ok(())
    }
}
