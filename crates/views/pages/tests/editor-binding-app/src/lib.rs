#[path = "../../../src/editor_indent.rs"]
pub mod indent;
#[path = "../../../src/editor_inline.rs"]
pub mod inline;
#[path = "../../../src/editor_markdown.rs"]
pub mod markdown;
#[path = "../../../src/editor_presentation.rs"]
pub mod presentation;
// Compile the production adapter and reducer, not copies of their behavior.
#[path = "../../../src/document_ingress.rs"]
pub mod document_ingress;
#[path = "../../../src/document_source.rs"]
pub mod document_source;
#[path = "../../../src/editor.rs"]
pub mod editor;
#[path = "../../../src/editor_binding.rs"]
pub mod editor_binding;
#[path = "../../../src/editor_menu.rs"]
pub mod editor_menu;
pub mod fixture {
    use crate::document_ingress::DocumentSource;
    use ui_lang_guest::wire;
    pub fn large_source() -> DocumentSource {
        DocumentSource {
            reference: wire::encode(&crate::document_source::DocumentIdentity {
                document: "network-a/page-a/source-1".into(),
                reset: 1,
            }),
        }
    }
}
include!("ui/view.rs");
ui_lang_guest::export_app!(
    PagesEditorFixture,
    "Pages editor binding",
    "Pages editor transaction regression fixture",
    ["pages"]
);
