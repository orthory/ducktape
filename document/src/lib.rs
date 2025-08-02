mod buffer;
mod variables;

pub use buffer::{DocumentBuffer, DocumentBufferError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("buffer error")]
    BufferError(#[from] DocumentBufferError),
}
