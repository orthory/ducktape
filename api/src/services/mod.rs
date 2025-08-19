pub mod comment;
pub mod document;
pub mod task;

pub use document::*;

pub struct Service {
    document: DocumentService,
}

impl Service {
    pub fn new(document_service: DocumentService) -> Self {
        Self {
            document: document_service,
        }
    }
}
