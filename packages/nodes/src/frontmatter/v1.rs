use crate::{Node, parser::Parser};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uid::{Identify, Uid};

const COMMAND: &str = "---";

#[cfg(test)]
const PROMOTED_KEYS: &[&str] = &["title", "author", "created_at", "updated_at"];

#[derive(thiserror::Error, Debug)]
pub enum FrontmatterError {
    #[error("invalid frontmatter data")]
    InvalidData,

    #[error("required field missing: {0}")]
    MissingData(&'static str),

    #[error("invariant: {0}")]
    Invariant(String),
}

// Frontmatter holds the document's identity — Document::uid() returns this
// node's uid. There's only ever one Frontmatter per Document, so this is also
// the document's uid. v1 on-disk format doesn't carry uids; the parser mints
// a fresh one at parse time. v2 will read it from the frontmatter body.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct FrontmatterV1 {
    // Document-level uid (frontmatter's uid IS the document's uid).
    pub uid: Uid,

    // required fields
    pub title: String,
    pub author: String,

    // 0 means "not set" — writer fills these in on first persist.
    pub created_at: u64,
    pub updated_at: u64,

    // hashmap for extra fields (excludes the promoted keys above)
    pub misc: HashMap<String, String>,
}

impl Identify for FrontmatterV1 {
    fn uid(&self) -> Uid {
        self.uid
    }
}

impl Node for FrontmatterV1 {
    fn try_match<R: std::io::Read>(document: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let Some(matched) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        let mut fields: HashMap<String, String> = matched
            .body
            .into_iter()
            .map(|line| {
                let (k, v) = line.split_once(":").ok_or(FrontmatterError::InvalidData)?;
                Ok((k.to_string(), v.trim_start().to_string()))
            })
            .collect::<Result<HashMap<String, String>, FrontmatterError>>()?;

        let title = fields
            .remove("title")
            .ok_or(FrontmatterError::MissingData("title"))?;
        let author = fields
            .remove("author")
            .ok_or(FrontmatterError::MissingData("author"))?;
        let created_at = parse_optional_u64(fields.remove("created_at").as_deref(), "created_at")?;
        let updated_at = parse_optional_u64(fields.remove("updated_at").as_deref(), "updated_at")?;

        // Anything left is misc — guaranteed to not collide with promoted keys.
        let misc = fields;

        Ok(Some(FrontmatterV1 {
            // v1 markdown doesn't carry a uid; mint a fresh one at parse time.
            // v2 will read it from the frontmatter body.
            uid: uid::new(),
            title,
            author,
            created_at,
            updated_at,
            misc,
        }))
    }

    // `---\n<key: value lines>\n---`. Promoted keys come first in a fixed order,
    // then misc keys sorted lexically — HashMap iteration order is not stable,
    // so sorting is what makes the render deterministic. uid is not rendered
    // (v1 mints it at parse time and it never appears on disk). No trailing
    // newline after the closing `---`; the orchestrator joins nodes with '\n'.
    fn render(&self) -> String {
        let mut out = String::from("---\n");
        out.push_str(&format!("title: {}\n", self.title));
        out.push_str(&format!("author: {}\n", self.author));
        out.push_str(&format!("created_at: {}\n", self.created_at));
        out.push_str(&format!("updated_at: {}\n", self.updated_at));

        let mut keys: Vec<&String> = self.misc.keys().collect();
        keys.sort();
        for k in keys {
            out.push_str(&format!("{}: {}\n", k, self.misc[k]));
        }

        out.push_str("---");
        out
    }
}

fn parse_optional_u64(value: Option<&str>, field: &'static str) -> Result<u64, FrontmatterError> {
    match value {
        None => Ok(0),
        Some(s) => s
            .parse::<u64>()
            .map_err(|e| FrontmatterError::Invariant(format!("{}: {}", field, e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_and_misc_without_duplicating() {
        let input = "\
---
title: my title
author: @orthory
created_at: 1700000000
updated_at: 1700000001
extra1: 111
extra2: 222
---
";
        let mut buffer = Parser::new(input.as_bytes());
        let fm = FrontmatterV1::try_match(&mut buffer)
            .expect("parse ok")
            .expect("matched");

        assert_eq!(fm.title, "my title");
        assert_eq!(fm.author, "@orthory");
        assert_eq!(fm.created_at, 1700000000);
        assert_eq!(fm.updated_at, 1700000001);

        // promoted keys must NOT also live in misc.
        for promoted in PROMOTED_KEYS {
            assert!(
                !fm.misc.contains_key(*promoted),
                "misc unexpectedly contains promoted key {}",
                promoted
            );
        }
        assert_eq!(fm.misc.get("extra1").map(String::as_str), Some("111"));
        assert_eq!(fm.misc.get("extra2").map(String::as_str), Some("222"));
    }

    #[test]
    fn missing_timestamps_default_to_zero_not_now() {
        let input = "\
---
title: t
author: a
---
";
        let mut buffer = Parser::new(input.as_bytes());
        let fm = FrontmatterV1::try_match(&mut buffer)
            .expect("parse ok")
            .expect("matched");
        assert_eq!(fm.created_at, 0);
        assert_eq!(fm.updated_at, 0);
    }

    #[test]
    fn render_round_trips_and_sorts_misc() {
        let input = "\
---
title: my title
author: @orthory
created_at: 100
updated_at: 200
zeta: z
alpha: a
---
";
        let mut p = Parser::new(input.as_bytes());
        let fm = FrontmatterV1::try_match(&mut p)
            .expect("parse ok")
            .expect("matched");
        let rendered = fm.render();

        // misc keys are emitted in sorted order (alpha before zeta) — the
        // determinism that makes the storage form HashMap-iteration-independent.
        assert!(rendered.contains("alpha: a\nzeta: z"));

        let mut p2 = Parser::new(rendered.as_bytes());
        let fm2 = FrontmatterV1::try_match(&mut p2)
            .expect("reparse ok")
            .expect("rematched");
        assert_eq!(fm2.title, fm.title);
        assert_eq!(fm2.author, fm.author);
        assert_eq!(fm2.created_at, fm.created_at);
        assert_eq!(fm2.updated_at, fm.updated_at);
        assert_eq!(fm2.misc, fm.misc);
    }

    #[test]
    fn missing_required_fields_errors() {
        let input = "\
---
author: a
---
";
        let mut buffer = Parser::new(input.as_bytes());
        assert!(FrontmatterV1::try_match(&mut buffer).is_err());
    }
}
