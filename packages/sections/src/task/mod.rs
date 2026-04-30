pub mod v1;

pub use v1::{TaskError, TaskV1, TaskV1Status};

use std::io::Read;

use crate::{Section, parser::Parser};

pub type TaskLatest = TaskV1;

/// On-disk marker is version-suffixed (`/task.v1`, `/task.v2`, …) so dispatch is
/// unambiguous. See `comment::try_parse_latest` for the full migration pattern.
pub fn try_parse_latest<R: Read>(
    parser: &mut Parser<R>,
) -> anyhow::Result<Option<TaskLatest>> {
    if let Some(v1) = TaskV1::try_match(parser)? {
        return Ok(Some(v1));
    }
    Ok(None)
}
