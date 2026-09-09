//! Atomic Pages document bootstrap through the existing bounded editor transfer.
//! Only source metadata lives in Ice state. The subscription owns its partial
//! receiver, so restoring a guest restarts from Begin and preserves the last
//! installed Editor until the new stream completes. The consumer snapshots an
//! installed-source marker and subscribes only while desired != installed;
//! replaying an already completed source would overwrite later local edits.

use iced::futures::{StreamExt, future};
use ui_lang_guest::{host, wire};
use wire::editor_document::{EditorDocumentRef, EditorTransfer, EditorTransferReceiver};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DocumentSource {
    pub reference: Vec<u8>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentItem {
    pub source: Vec<u8>,
    pub text: String,
    pub error: String,
}

pub fn empty_source() -> DocumentSource {
    DocumentSource::default()
}

pub fn document_source(source: DocumentSource) -> iced::Subscription<DocumentItem> {
    iced::Subscription::run_with(source, |source| {
        let source = source.clone();
        let expected = wire::decode::<EditorDocumentRef>(&source.reference)
            .ok()
            .filter(|reference| reference.validate().is_ok());
        let incoming = host::subscribe("pages.document", &source.reference);
        incoming
            .scan(
                (source, expected, None::<EditorTransferReceiver>, false),
                |(source, expected, receiver, ended), answer| {
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
                            if target != expected {
                                return Err("Document source changed");
                            }
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
                                source: source.reference.clone(),
                                text,
                                error: String::new(),
                            })
                        }
                        Err(error) => {
                            *ended = true;
                            *receiver = None;
                            Some(DocumentItem {
                                source: source.reference.clone(),
                                text: String::new(),
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
