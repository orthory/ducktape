use crate::{Section, parser::Parser};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct FrontmatterV1 {
    // required fields
    pub title: String,
    pub author: String,

    // 0 means "not set" — writer fills these in on first persist.
    pub created_at: u64,
    pub updated_at: u64,

    // hashmap for extra fields (excludes the promoted keys above)
    pub misc: HashMap<String, String>,
}

impl Section for FrontmatterV1 {
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
            title,
            author,
            created_at,
            updated_at,
            misc,
        }))
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
