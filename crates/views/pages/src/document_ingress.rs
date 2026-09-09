//! Atomic Pages document bootstrap through the existing bounded editor transfer.
//! Only source metadata lives in Ice state. The subscription owns its partial
//! receiver, so restoring a guest restarts from Begin and preserves the last
//! installed Editor until the new stream completes. The consumer snapshots an
//! installed-source marker and subscribes only while desired != installed;
//! replaying an already completed source would overwrite later local edits.

use crate::document_source::DocumentIdentity;
use iced::futures::{StreamExt, future};
use ui_lang_guest::{host, wire};
use wire::editor_document::{EditorTransfer, EditorTransferReceiver};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DocumentSource {
    pub reference: Vec<u8>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentItem {
    pub notice: String,
    pub source: Vec<u8>,
    pub text: String,
    pub cursor: Vec<u8>,
    pub error: String,
}

pub fn empty_source() -> DocumentSource {
    DocumentSource::default()
}

pub fn document_source(source: DocumentSource) -> iced::Subscription<DocumentItem> {
    iced::Subscription::run_with(source, |source| {
        let source = source.clone();
        let expected = wire::decode::<DocumentIdentity>(&source.reference)
            .ok()
            .filter(|identity| !identity.document.is_empty() && identity.document.len() <= 1024);
        let incoming = host::subscribe("pages.document", &source.reference);
        incoming
            .scan(
                (
                    source,
                    expected,
                    None::<EditorTransferReceiver>,
                    Vec::new(),
                    false,
                ),
                |(source, expected, receiver, cursor, ended), answer| {
                    if *ended {
                        return future::ready(Some(None));
                    }
                    let result = (|| {
                        let expected = expected.as_ref().ok_or("Invalid document source")?;
                        let bytes = answer.map_err(|_| "Document transfer failed")?;
                        let transfer: EditorTransfer =
                            wire::decode(&bytes).map_err(|_| "Invalid document transfer")?;
                        if receiver.is_none() {
                            let EditorTransfer::Begin { id, target } = &transfer else {
                                return Err("Document transfer must start from the beginning");
                            };
                            if target.document != expected.document
                                || target.reset != expected.reset
                            {
                                return Err("Document source changed");
                            }
                            *cursor = wire::encode(&target.cursor);
                            *receiver = Some(
                                EditorTransferReceiver::new(id.clone(), target.clone())
                                    .map_err(|_| "Invalid document source")?,
                            );
                        }
                        receiver
                            .as_mut()
                            .expect("receiver established")
                            .receive(&transfer)
                            .map_err(|_| "Document transfer could not be completed")
                    })();
                    let item = match result {
                        Ok(None) => None,
                        Ok(Some(text)) => {
                            *ended = true;
                            Some(DocumentItem {
                                notice: crate::presentation::format_notice(&text),
                                source: source.reference.clone(),
                                text,
                                cursor: cursor.clone(),
                                error: String::new(),
                            })
                        }
                        Err(error) => {
                            *ended = true;
                            *receiver = None;
                            Some(DocumentItem {
                                notice: String::new(),
                                source: source.reference.clone(),
                                text: String::new(),
                                cursor: Vec::new(),
                                error: error.into(),
                            })
                        }
                    };
                    future::ready(Some(item))
                },
            )
            .filter_map(future::ready)
    })
}

/// A complete source installs text and its caret atomically. Ordinary accepted
/// edits never call this authoritative assignment.
pub fn document_editor(text: String, cursor: Vec<u8>) -> ui_lang_guest::Editor {
    let cursor = wire::decode(&cursor).expect("validated source cursor");
    let mut editor = ui_lang_guest::Editor::new(text);
    editor.move_to(cursor);
    editor
}
