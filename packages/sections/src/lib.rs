mod comment_v1;
mod frontmatter_v1;
mod task_v1;

pub mod parser;

pub use comment_v1::*;
pub use frontmatter_v1::*;
pub use task_v1::*;

// common
use std::io::Read;

use serde::{Deserialize, Serialize};

/// Section is a trait that represents a section of a document.
///
/// All sections must implement the `Section` trait.
pub trait Section
where
    Self: Sized,
{
    fn try_match<R: Read>(document: &mut crate::parser::Parser<R>) -> anyhow::Result<Option<Self>>;
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "@section_type")]
pub enum Sections {
    FrontmatterV1(crate::frontmatter_v1::FrontmatterV1),
    CommentV1(crate::comment_v1::CommentV1),
    TaskV1(crate::task_v1::TaskV1),
}

macro_rules! try_all_sections {
    ($parser:expr, $($variant:ident => $type:ty),* $(,)?) => {
        $(
            if let Some(section) = <$type>::try_match($parser)? {
                return Ok(Some(Sections::$variant(section)));
            }
        )*
        Ok(None)
    };
}

impl Sections {
    pub fn try_parse_sections<R: Read>(
        parser: &mut crate::parser::Parser<R>,
    ) -> anyhow::Result<Option<Self>> {
        try_all_sections! {
            parser,
            FrontmatterV1 => crate::frontmatter_v1::FrontmatterV1,
            CommentV1 => crate::comment_v1::CommentV1,
            TaskV1 => crate::task_v1::TaskV1,
        }
    }
}
