use std::collections::HashMap;

use document;
use serde::{Deserialize, Serialize};

const COMMAND: &str = "---";

#[derive(thiserror::Error, Debug)]
pub enum FrontmatterError {
    #[error("invalid frontmatter data")]
    InvalidData,

    #[error("required field missing: {0}")]
    MissingData(&'static str),

    #[error("invariant: {0}")]
    Invariant(&'static str),
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FrontmatterV1 {
    pub title: String,
    pub author: String,
    pub timestamp: u64,
    pub misc: HashMap<String, String>,
}

impl crate::Section for FrontmatterV1 {
    fn try_match<R: std::io::Read>(
        document: &mut document::DocumentBuffer<R>,
    ) -> anyhow::Result<Option<Self>> {
        let result = document.try_map_command_group::<FrontmatterV1, FrontmatterError>(
            COMMAND,
            |_, body| {
                let mut frontmatter = FrontmatterV1::default();

                body.into_iter()
                    .map(|line| line.split_once(":").ok_or(FrontmatterError::InvalidData))
                    .try_for_each(|Ok((key, value))| {
                        frontmatter.misc.insert(key.to_string(), value.to_string());
                        Ok(())
                    });

                // promote some key frontmatter-related fields
                frontmatter.title = frontmatter
                    .misc
                    .get("title")
                    .ok_or(FrontmatterError::MissingData("title"))?
                    .clone();

                frontmatter.author = frontmatter
                    .misc
                    .get("author")
                    .ok_or(FrontmatterError::MissingData("author"))?
                    .clone();

                frontmatter.timestamp = u64::from_str_radix(
                    frontmatter
                        .misc
                        .get("timestamp")
                        .ok_or(FrontmatterError::MissingData("timestamp"))?,
                    10,
                )
                .map_err(|e| FrontmatterError::MissingData("timestamp"))?;

                Ok(Some(frontmatter))
            },
        );

        Ok(result?)
    }
}
