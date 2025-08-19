use crate::{parser::Parser, sections::Section};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const COMMAND: &str = "---";

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
    pub created_at: u128,
    pub updated_at: u128,

    // hashmap for extra fields
    pub misc: HashMap<String, String>,
}

impl Section for FrontmatterV1 {
    fn try_match<R: std::io::Read>(document: &mut Parser<R>) -> anyhow::Result<Option<Self>> {
        let Some((_, body)) = document.try_map_command_group(COMMAND)? else {
            return Ok(None);
        };

        let misc: HashMap<String, String> = body
            .into_iter()
            .map(|line| {
                let (k, v) = line.split_once(":").ok_or(FrontmatterError::InvalidData)?;
                Ok((k.to_string(), v.trim_start().to_string()))
            })
            .collect::<Result<HashMap<String, String>, FrontmatterError>>()?;

        // promote some key frontmatter-related fields
        let title = misc
            .get("title")
            .ok_or(FrontmatterError::MissingData("title"))?
            .clone();

        let author = misc
            .get("author")
            .ok_or(FrontmatterError::MissingData("author"))?
            .clone();

        let now = utils::time::now_string();

        let created_at = u128::from_str_radix(misc.get("created_at").unwrap_or_else(|| &now), 10)
            .map_err(|e| {
            FrontmatterError::Invariant(format!("created_at: {}", e.to_string()))
        })?;

        let updated_at = u128::from_str_radix(misc.get("updated_at").unwrap_or_else(|| &now), 10)
            .map_err(|e| {
            FrontmatterError::Invariant(format!("updated_at: {}", e.to_string()))
        })?;

        Ok(Some(FrontmatterV1 {
            title: title,
            author: author,
            created_at: created_at,
            updated_at: updated_at,
            misc: misc,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE_FRONTMATTER: &str = r#"
---
title: asdfasdfasd
author: @orthory
date: 21421
extra1: 111
extra2: 222
---
"#;

    #[test]
    fn test_asdf() {
        let mut buffer = crate::parser::Parser::new(SAMPLE_FRONTMATTER.trim_start().as_bytes());
        let comment = FrontmatterV1::try_match(&mut buffer);
        dbg!(comment);
    }
}
