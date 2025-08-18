use serde::{Deserialize, Serialize};

use crate::rand::rand;

#[derive(Deserialize, Clone, Debug, Hash)]
pub enum DocumentID {
    Latest { id: [u8; 32] },
    Version { id: [u8; 32], commit_hash: [u8; 32] },
}

impl DocumentID {
    pub fn new() -> Self {
        Self::Latest { id: rand() }
    }
}

impl Serialize for DocumentID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            DocumentID::Latest { id } => serializer.collect_str(&format!("d/{:?}", id)),
            DocumentID::Version { id, commit_hash } => {
                serializer.collect_str(&format!("d/{:?}/{:?}", id, commit_hash))
            }
        }
    }
}

// impl<'de> Deserialize<'de> for DocumentID {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         // let k = deserializer.deserialize_char();
//     }
// }
