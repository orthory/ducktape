pub mod v1;

pub use v1::{CommentError, CommentV1};

/// Always points at the newest version of the comment section.
/// Update when a new version is added; consumers can `use sections::comment::CommentLatest`.
pub type CommentLatest = CommentV1;
