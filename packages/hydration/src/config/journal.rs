use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// High-water mark — number of ops per Journal before it rotates into
    /// `to_be_flushed` and a fresh journal is opened.
    pub hwm: usize,
}
