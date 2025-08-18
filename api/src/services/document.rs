use std::sync::Arc;

use poem::{handler, web::Data};

pub struct DocumentService {
    doctree: doctree::Tree<document::Document>,
}

#[handler]
pub async fn create_document(
    Data(docsvc): Data<&Arc<DocumentService>>,
) -> poem::Result<identifier::Identifier> {
    docsvc.doctree.create_document(d);
}

#[handler]
pub async fn update_document(Data(doctree): Data<&Arc<DocumentService>>) -> poem::Result<String> {}

// pub trait DocumentService
// where
//     Self: Send + Sync,
// {
//     type Header: Serialize;
//     type Document: Serialize;

//     fn create_document(&mut self, id: String) -> Result<Self::Document>;
//     fn update_document(&mut self, id: String) -> Result<Self::Document>;

//     fn get_document_headers(&self, path: String) -> Result<Vec<Self::Header>>;
//     fn get_document_header(&self, path: String) -> Result<Self::Header>;
//     fn get_document(&self, id: String) -> Result<Self::Document>;
// }
