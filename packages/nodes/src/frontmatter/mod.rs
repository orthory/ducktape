pub mod v1;

pub use v1::{FrontmatterError, FrontmatterV1};

use std::io::Read;

use crate::{Node, parser::Parser};

pub type FrontmatterLatest = FrontmatterV1;

/// Frontmatter has no on-disk version marker (the fence is just `---`). When V2 lands
/// the convention can be a `version: N` key in the body — at which point this function
/// will need to parse the block once into a HashMap and dispatch by version, rather
/// than delegating to per-version `try_match`. See `comment::try_parse_latest` for the
/// pattern that applies once each version has its own marker.
pub fn try_parse_latest<R: Read>(
    parser: &mut Parser<R>,
) -> anyhow::Result<Option<FrontmatterLatest>> {
    if let Some(v1) = FrontmatterV1::try_match(parser)? {
        return Ok(Some(v1));
    }
    Ok(None)
}
