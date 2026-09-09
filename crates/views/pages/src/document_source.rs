//! Stable app-owned document identity. A transfer Begin supplies the changing
//! full reference; edits never turn this source marker into a replacement.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocumentIdentity {
    pub document: String,
    pub reset: u64,
}

/// Small read-only navigation emitted after the matching editor interaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Navigation {
    pub link: String,
    pub comment_line: Option<u32>,
}

/// The document bytes remain in the canonical host editor, never in an intent.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Accepted {
    pub source: Vec<u8>,
    pub reference: Vec<u8>,
    pub navigation: Vec<u8>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommentMark {
    pub line: i64,
    pub count: i64,
}
