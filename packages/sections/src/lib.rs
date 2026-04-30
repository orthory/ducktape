pub mod comment;
pub mod frontmatter;
pub mod parser;
pub mod task;

pub use comment::{CommentError, CommentLatest, CommentV1};
pub use frontmatter::{FrontmatterError, FrontmatterLatest, FrontmatterV1};
pub use task::{TaskError, TaskLatest, TaskV1, TaskV1Status};

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
    FrontmatterV1(FrontmatterV1),
    CommentV1(CommentV1),
    TaskV1(TaskV1),
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
            FrontmatterV1 => FrontmatterV1,
            CommentV1 => CommentV1,
            TaskV1 => TaskV1,
        }
    }
}
