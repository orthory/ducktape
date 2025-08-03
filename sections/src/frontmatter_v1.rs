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
                let misc: HashMap<String, String> = body
                    .into_iter()
                    .map(|line| {
                        let (k, v) = line.split_once(":").ok_or(FrontmatterError::InvalidData)?;
                        Ok((k.to_string(), v.to_string()))
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

                let timestamp = u64::from_str_radix(
                    misc.get("timestamp")
                        .ok_or(FrontmatterError::MissingData("timestamp"))?,
                    10,
                )
                .map_err(|_| FrontmatterError::Invariant("timestamp"))?;

                Ok(Some(FrontmatterV1 {
                    title: title,
                    author: author,
                    timestamp: timestamp,
                    misc: misc,
                }))
            },
        );

        Ok(result?)
    }
}
