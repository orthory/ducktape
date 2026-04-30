use crate::utils::identifier::Identifier;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use doctree::Entry;
use document::Document;
use poem::{
    Result, handler,
    web::{Data, Json, Path},
};

pub struct DocumentService {
    doctree: RwLock<doctree::Tree<document::Document>>,
}

impl DocumentService {
    pub fn new(basedir: String) -> Self {
        let next_doctree = doctree::Tree::<document::Document>::new(
            &PathBuf::from(basedir),
            |f| document::Document::from_reader(f).map_err(anyhow::Error::from),
            |l| doctree::stdfs::load(l),
            |w| doctree::stdfs::write(w),
        )
        .unwrap();

        DocumentService {
            doctree: RwLock::new(next_doctree),
        }
    }
}

#[handler]
pub async fn get_documents(
    Path(document_path): Path<String>,
    Data(docsvc): Data<&Arc<DocumentService>>,
) -> Result<Json<Entry<Document>>> {
    let doctree = docsvc.doctree.read().unwrap();
    let result = doctree
        .get_entries(document_path)
        .map_err(poem::error::InternalServerError)?;

    Ok(Json(result.clone()))
}

#[handler]
pub async fn create_document(
    Data(docsvc): Data<&Arc<DocumentService>>,
) -> Result<Json<Identifier>> {
    let mut doctree = docsvc.doctree.write().unwrap();

    // create a temporary document
    let next_document_path = doctree
        .create_document()
        .map_err(poem::error::InternalServerError)?;
    let identifier = Identifier::Document(next_document_path);

    Ok(Json(identifier))
}

#[handler]
pub async fn update_document(Data(doctree): Data<&Arc<DocumentService>>) -> poem::Result<String> {
    Ok("asdfasdf".to_string())
}

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
