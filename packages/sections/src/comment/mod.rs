pub mod v1;

pub use v1::{CommentError, CommentV1};

use std::io::Read;

use crate::{Section, parser::Parser};

/// Always points at the newest in-memory shape of the comment section. Update when a
/// new version is added; consumers can `use sections::comment::CommentLatest`.
pub type CommentLatest = CommentV1;

/// Try to parse a comment section, attempting versions newest-first. Older versions are
/// migrated forward to `CommentLatest` before being returned. When `V2` lands, add
/// `if let Some(v) = CommentV2::try_match(parser)? { return Ok(Some(v)); }` *above* the
/// V1 branch and have V1 implement `Into<CommentV2>` (or a dedicated `migrate_to_latest`).
pub fn try_parse_latest<R: Read>(
    parser: &mut Parser<R>,
) -> anyhow::Result<Option<CommentLatest>> {
    if let Some(v1) = CommentV1::try_match(parser)? {
        return Ok(Some(v1));
    }
    Ok(None)
}
