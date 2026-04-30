pub mod v1;

pub use v1::{TaskError, TaskV1, TaskV1Status};

pub type TaskLatest = TaskV1;
