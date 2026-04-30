pub mod v1;

pub use v1::{FrontmatterError, FrontmatterV1};

use std::io::Read;

use crate::{Section, parser::Parser};

pub type FrontmatterLatest = FrontmatterV1;

/// See `comment::try_parse_latest` for the migration pattern; same shape applies here.
pub fn try_parse_latest<R: Read>(
    parser: &mut Parser<R>,
) -> anyhow::Result<Option<FrontmatterLatest>> {
    if let Some(v1) = FrontmatterV1::try_match(parser)? {
        return Ok(Some(v1));
    }
    Ok(None)
}
