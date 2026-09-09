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

/// A source installation is acknowledged without copying document bytes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Installed {
    pub source: Vec<u8>,
    pub reset: u64,
    pub revision: u64,
    pub text_revision: u64,
    pub byte_len: u32,
    pub cursor: ui_lang_wire::EditorCursor,
}
impl Installed {
    pub fn matches(&self, reference: &ui_lang_wire::editor_document::EditorDocumentRef) -> bool {
        self.reset == reference.reset
            && self.revision == reference.revision
            && self.text_revision == reference.text_revision
            && self.byte_len == reference.byte_len
            && self.cursor == reference.cursor
    }
}
