pub mod comment;
pub mod frontmatter;
pub mod parser;
pub mod task;

pub use comment::{CommentError, CommentLatest, CommentV1};
pub use frontmatter::{FrontmatterError, FrontmatterLatest, FrontmatterV1};
pub use task::{TaskError, TaskLatest, TaskV1, TaskV1Status};
use uid::Identify;

use std::io::Read;

use serde::{Deserialize, Serialize};

/// Node is a trait that represents a node of a document.
///
/// All nodes also implement [`uid::Identify`] so consumers can ask any node
/// for its identity. Frontmatter holds the document's uid — `Document::uid()`
/// returns the frontmatter node's uid.
pub trait Node
where
    Self: Sized,
{
    fn try_match<R: Read>(document: &mut crate::parser::Parser<R>) -> anyhow::Result<Option<Self>>;
}

/// `Nodes` holds parsed nodes at their *latest* in-memory shape — older on-disk
/// versions are migrated forward by each node module's `try_parse_latest`. Variants
/// here are version-agnostic on purpose so consumers don't break when a new version of
/// a node type is added.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "@node_type")]
pub enum Nodes {
    Frontmatter(FrontmatterLatest),
    Comment(CommentLatest),
    Task(TaskLatest),
}

impl Identify for Nodes {
    fn uid(&self) -> uid::Uid {
        match self {
            Nodes::Frontmatter(s) => s.uid(),
            Nodes::Comment(s) => s.uid(),
            Nodes::Task(s) => s.uid(),
        }
    }
}

impl Nodes {
    pub fn try_parse_nodes<R: Read>(
        parser: &mut crate::parser::Parser<R>,
    ) -> anyhow::Result<Option<Self>> {
        if let Some(s) = frontmatter::try_parse_latest(parser)? {
            return Ok(Some(Nodes::Frontmatter(s)));
        }
        if let Some(s) = comment::try_parse_latest(parser)? {
            return Ok(Some(Nodes::Comment(s)));
        }
        if let Some(s) = task::try_parse_latest(parser)? {
            return Ok(Some(Nodes::Task(s)));
        }
        Ok(None)
    }
}
