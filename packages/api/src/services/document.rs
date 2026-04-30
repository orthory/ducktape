use crate::utils::identifier::Identifier;
use std::{path::PathBuf, sync::Arc};

use doctree::{Entry, PersistedTree};
use driver::Stdfs;
use poem::{
    Result, handler,
    web::{Data, Json, Path},
};

pub struct DocumentService {
    tree: PersistedTree,
}

impl DocumentService {
    pub fn new(basedir: String) -> Self {
        let tree = PersistedTree::open(Stdfs, &PathBuf::from(basedir)).unwrap();
        DocumentService { tree }
    }
}

#[handler]
pub async fn get_documents(
    Path(document_path): Path<String>,
    Data(docsvc): Data<&Arc<DocumentService>>,
) -> Result<Json<Entry>> {
    let entry = docsvc
        .tree
        .get_entries(document_path)
        .map_err(poem::error::InternalServerError)?;

    Ok(Json(entry))
}

#[handler]
pub async fn create_document(
    Data(docsvc): Data<&Arc<DocumentService>>,
) -> Result<Json<Identifier>> {
    let path = docsvc
        .tree
        .create_document()
        .map_err(poem::error::InternalServerError)?;

    Ok(Json(Identifier::Document(path)))
}

#[handler]
pub async fn update_document(Data(_docsvc): Data<&Arc<DocumentService>>) -> poem::Result<String> {
    Ok("asdfasdf".to_string())
}
