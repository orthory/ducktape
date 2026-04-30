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
/// All sections must implement the `Section` trait. Identity (`uid::Identify`)
/// is implemented on the section types that need it — Comment, Task — but NOT
/// on Frontmatter, which is intrinsic to the parent document and shares the
/// document's uid.
pub trait Section
where
    Self: Sized,
{
    fn try_match<R: Read>(document: &mut crate::parser::Parser<R>) -> anyhow::Result<Option<Self>>;
}

/// `Sections` holds parsed sections at their *latest* in-memory shape — older on-disk
/// versions are migrated forward by each section module's `try_parse_latest`. Variants
/// here are version-agnostic on purpose so consumers don't break when a new version of
/// a section type is added.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "@section_type")]
pub enum Sections {
    Frontmatter(FrontmatterLatest),
    Comment(CommentLatest),
    Task(TaskLatest),
}

impl Sections {
    pub fn try_parse_sections<R: Read>(
        parser: &mut crate::parser::Parser<R>,
    ) -> anyhow::Result<Option<Self>> {
        if let Some(s) = frontmatter::try_parse_latest(parser)? {
            return Ok(Some(Sections::Frontmatter(s)));
        }
        if let Some(s) = comment::try_parse_latest(parser)? {
            return Ok(Some(Sections::Comment(s)));
        }
        if let Some(s) = task::try_parse_latest(parser)? {
            return Ok(Some(Sections::Task(s)));
        }
        Ok(None)
    }
}
