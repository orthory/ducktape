pub mod v1;

pub use v1::{CommentError, CommentV1};

use std::io::Read;

use crate::{Node, parser::Parser};

/// Always points at the newest in-memory shape of the comment node. Update when a
/// new version is added; consumers can `use nodes::comment::CommentLatest`.
pub type CommentLatest = CommentV1;

/// Try to parse a comment node. Each version owns its own on-disk marker
/// (`/comment.v1`, `/comment.v2`, …) so dispatch is unambiguous — only one branch
/// can match a given block. Older versions are migrated forward to `CommentLatest`
/// before being returned. When V2 lands: add a branch above for `CommentV2::try_match`
/// and have `CommentV1` implement `Into<CommentV2>` (or a `migrate_to_latest`) so the
/// V1 branch becomes `Some(v1.into())`.
pub fn try_parse_latest<R: Read>(
    parser: &mut Parser<R>,
) -> anyhow::Result<Option<CommentLatest>> {
    if let Some(v1) = CommentV1::try_match(parser)? {
        return Ok(Some(v1));
    }
    Ok(None)
}
