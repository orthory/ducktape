use std::{collections::HashMap, io::Read, path::Path};
use nodes::{Nodes, parser::Parser};
use uid::Identify;

#[derive(Debug)]
pub struct Document {
    // uid is _always_ assigned upon a successful hydration; equals the
    // frontmatter node's uid
    pub(crate) uid: uid::Uid,

    // a hashmap based linked list (forward only) for reconstructing in-memoery
    // construction back to rendered document
    pub(crate) nodes_map: HashMap<
        uid::Uid,
        (/*current_node*/nodes::Nodes, /*next*/Option<uid::Uid>)
    >
}

impl Identify for Document {
    // uid is the document id (= frontmatter's uid)
    fn uid(&self) -> uid::Uid {
        self.uid
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Errors {
    #[error("DocumentInstance: io error")]
    IOError(String),

    #[error("DocumentInstance: error during parsing at line {1} (around {2}): {0}")]
    ParseError(anyhow::Error, usize, String),

    #[error("document is empty")]
    EmptyDocument,

    #[error("frontmatter is empty")]
    EmptyFrontmatter,

    #[error("invalid frontmatter")]
    InvalidFrontmatter
}

impl Document {
    pub fn from_path(path: &Path) -> Result<Self, Errors> {
        let file = std::fs::File::options()
            .read(true)
            .open(&path)
            .map_err(|e| Errors::IOError(e.to_string()))?;

        Self::from_reader(file)
    }
    
    pub fn from_reader<R: Read>(
        reader: R
    ) -> Result<Self, Errors> {
        // try parsing from reader
        let nodes = try_instantiate_document(reader)?;

        // nodes must not be empty; if it is, we are fed an empty document
        if nodes.is_empty() {
            return Err(Errors::EmptyDocument);
        };

        // first node must be frontmatter
        let Nodes::Frontmatter(frontmatter) = &nodes[0] else {
            return Err(Errors::EmptyFrontmatter);
        };

        // uid must be present on frontmatter (as it IS the document's uid)
        let uid = frontmatter.uid();
        if uid.is_nil() {
            return Err(Errors::InvalidFrontmatter);
        };

        // iterate over constructed nodes vector,
        // construct a singly linked list in a hashmap
        let nodes_map = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (
                node.uid(),
                (node.clone(), nodes.get(i + 1).map(|next| next.uid()))
            ))
            .collect::<HashMap<_, _>>();

        // good to go
        Ok(Document {
            uid,
            nodes_map,
        })
    }
}

fn try_instantiate_document<R: Read>(
    reader: R,
) -> Result<Vec<Nodes>, Errors> {
    let mut parser = Parser::new(reader);
    let mut nodes: Vec<Nodes> = Vec::new();

    loop {
        match Nodes::try_parse_nodes(&mut parser) {
            Ok(Some(node)) => nodes.push(node),
            Ok(None) => break,
            Err(e) => {
                let (line_pos, line) = parser.current_line();
                return Err(Errors::ParseError(e.into(), line_pos, line));
            }
        }
    }

    Ok(nodes)
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

        let nodes = try_instantiate_document(sample_document.as_bytes())?;
        dbg!(nodes);
        Ok(())
    }

    #[test]
    pub fn read_and_structured_match_parsed_nodes() -> anyhow::Result<()> {
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

        let doc = Document::from_reader(sample_document.trim_start().as_bytes())?;
        let streamed: Vec<_> = doc.nodes_iter().collect();

        // frontmatter → (any body between) → comment
        assert!(streamed.len() >= 2);
        assert!(matches!(streamed[0], Nodes::Frontmatter(_)));
        assert!(matches!(streamed.last().unwrap(), Nodes::Comment(_)));

        Ok(())
    }
}
