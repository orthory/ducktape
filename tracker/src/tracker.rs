use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TrackerType {
    Github,
    External,
    Backlink,
}
