use std::sync::Arc;

use poem::{EndpointExt, Route, Server, get, handler, listener::TcpListener, middleware::AddData};

use crate::services::{DocumentService, get_documents};

pub async fn create_server(addr: String, document_service: Arc<DocumentService>) {
    let app = Route::new()
        .at("/settings", get(todo))
        .at("/new", get(todo))
        .at("/comments", get(todo))
        .at("/tasks", get(todo))
        .at("/t", get(todo))
        // documents
        .at("/documents/*", get(get_documents))
        .at("/d/*", get(get_documents))
        .with(AddData::new(document_service));

    Server::new(TcpListener::bind(addr)).run(app).await.unwrap();
}

#[handler]
async fn todo() -> String {
    "todo".to_string()
}
