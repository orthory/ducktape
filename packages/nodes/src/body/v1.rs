use crate::{Node, parser::Parser};
use serde::{Deserialize, Serialize};
use uid::{Identify, Uid};

#[derive(thiserror::Error, Debug)]
pub enum BodyError {}

/// Free-form prose between structured nodes — paragraphs, lists, anything
/// that isn't a recognized command. Consecutive non-command lines coalesce
/// into a single `BodyV1` so a paragraph stays intact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BodyV1 {
    // Body lines have no on-disk uid (they're just text). Parser mints one
    // at parse time so every node carries a stable identity.
    uid: Uid,
    pub lines: Vec<String>,
}

impl Identify for BodyV1 {
    fn uid(&self) -> Uid {
        self.uid
    }
}

impl Node for BodyV1 {
    fn try_match<R: std::io::Read>(parser: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let mut lines = Vec::new();

        loop {
            match parser.peek_line()? {
                None => break, // EOF
                Some(line) if looks_like_command(&line) => break,
                Some(_) => {
                    // safe to advance — we just peeked a body line
                    let line = parser
                        .try_read_line()?
                        .expect("peek_line returned Some, so try_read_line must too");
                    lines.push(line);
                }
            }
        }

        if lines.is_empty() {
            Ok(None)
        } else {
            Ok(Some(BodyV1 {
                uid: uid::new(),
                lines,
            }))
        }
    }
}

/// Body boundary heuristic: a line is a command opener if it's the literal
/// frontmatter delimiter (`---`) or starts with `/` (the prefix every other
/// node command uses). Everything else is prose.
fn looks_like_command(line: &str) -> bool {
    line == "---" || line.starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_consecutive_prose_lines() {
        let input = "first line\nsecond line\nthird line\n";
        let mut p = Parser::new(input.as_bytes());
        let body = BodyV1::try_match(&mut p)
            .expect("parse ok")
            .expect("body matched");
        assert_eq!(body.lines, vec!["first line", "second line", "third line"]);
    }

    #[test]
    fn stops_at_frontmatter_delimiter() {
        let input = "prose\n---\nrest\n";
        let mut p = Parser::new(input.as_bytes());
        let body = BodyV1::try_match(&mut p)
            .expect("parse ok")
            .expect("body matched");
        assert_eq!(body.lines, vec!["prose"]);
        // `---` should still be at the head of the parser
        assert_eq!(p.peek_line().unwrap(), Some("---".to_string()));
    }

    #[test]
    fn stops_at_command_opener() {
        let input = "prose\n/comment.v1{@a;1;1}\nbody\n/comment.v1\n";
        let mut p = Parser::new(input.as_bytes());
        let body = BodyV1::try_match(&mut p)
            .expect("parse ok")
            .expect("body matched");
        assert_eq!(body.lines, vec!["prose"]);
    }

    #[test]
    fn returns_none_when_starting_on_command() {
        let input = "---\nfrontmatter\n---\n";
        let mut p = Parser::new(input.as_bytes());
        assert!(BodyV1::try_match(&mut p).expect("parse ok").is_none());
    }

    #[test]
    fn returns_none_at_eof() {
        let input = "";
        let mut p = Parser::new(input.as_bytes());
        assert!(BodyV1::try_match(&mut p).expect("parse ok").is_none());
    }

    #[test]
    fn coalesces_paragraph_with_blank_lines() {
        let input = "paragraph one\n\nstill body — blank line is content\n";
        let mut p = Parser::new(input.as_bytes());
        let body = BodyV1::try_match(&mut p)
            .expect("parse ok")
            .expect("body matched");
        assert_eq!(body.lines.len(), 3);
        assert_eq!(body.lines[1], "");
    }
}
