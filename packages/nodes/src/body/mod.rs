pub mod v1;

pub use v1::{BodyError, BodyV1};

use std::io::Read;

use crate::{Node, parser::Parser};

/// Always points at the newest in-memory shape of the body node. Update when a
/// new version is added; consumers can `use nodes::body::BodyLatest`.
pub type BodyLatest = BodyV1;

/// Try to parse a body node — coalesces consecutive non-command lines into one
/// block. Each version owns its detection logic. When V2 lands: add a branch
/// above for `BodyV2::try_match` and have `BodyV1` implement `Into<BodyV2>` (or
/// a `migrate_to_latest`) so the V1 branch becomes `Some(v1.into())`.
pub fn try_parse_latest<R: Read>(parser: &mut Parser<R>) -> anyhow::Result<Option<BodyLatest>> {
    if let Some(v1) = BodyV1::try_match(parser)? {
        return Ok(Some(v1));
    }
    Ok(None)
}
