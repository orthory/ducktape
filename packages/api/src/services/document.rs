use crate::utils::identifier::Identifier;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use doctree::{Entry, Stdfs, Tree};
use poem::{
    Result, handler,
    web::{Data, Json, Path},
};

pub struct DocumentService {
    doctree: RwLock<Tree>,
}

impl DocumentService {
    pub fn new(basedir: String) -> Self {
        let tree = Tree::new(&PathBuf::from(basedir), Stdfs).unwrap();
        DocumentService {
            doctree: RwLock::new(tree),
        }
    }
}

#[handler]
pub async fn get_documents(
    Path(document_path): Path<String>,
    Data(docsvc): Data<&Arc<DocumentService>>,
) -> Result<Json<Entry>> {
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

    let (next_tree, path) = doctree
        .create_document()
        .map_err(poem::error::InternalServerError)?;
    *doctree = next_tree;

    Ok(Json(Identifier::Document(path)))
}

#[handler]
pub async fn update_document(Data(_doctree): Data<&Arc<DocumentService>>) -> poem::Result<String> {
    Ok("asdfasdf".to_string())
}
