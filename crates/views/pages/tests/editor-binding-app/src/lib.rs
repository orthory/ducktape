// Compile the production adapter and reducer, not copies of their behavior.
#[path = "../../../src/document_ingress.rs"]
pub mod document_ingress;
#[path = "../../../src/editor.rs"]
pub mod editor;
#[path = "../../../src/editor_binding.rs"]
pub mod editor_binding;
#[path = "../../../src/editor_menu.rs"]
pub mod editor_menu;
pub mod fixture {
    use crate::document_ingress::DocumentSource;
    use ui_lang_guest::wire::{self, EditorCursor};
    pub fn large_source() -> DocumentSource {
        DocumentSource {
            reference: wire::encode(&wire::editor_document::EditorDocumentRef {
                document: "network-a/page-a/source-1".into(),
                reset: 1,
                text_revision: 0,
                revision: 0,
                cursor: EditorCursor::default(),
                byte_len: wire::editor_document::MAX_EDITOR_DOCUMENT_BYTES as u32,
            }),
        }
    }
}
ui_lang::include_app!("src/ui/app.ice");
ui_lang_guest::export_app!(
    PagesEditorFixture,
    "Pages editor binding",
    "Pages editor transaction regression fixture",
    ["pages"]
);
