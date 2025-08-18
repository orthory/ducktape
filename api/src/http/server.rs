use std::sync::Arc;

use poem::{
    EndpointExt, Result, Route, Server, get, handler, listener::TcpListener, middleware::AddData,
};
use serde::Serialize;

use crate::services::{DocumentService, Service};

pub fn create_server<Document, Header, Comment, Task>(
    addr: String,
    service: Service<Document, Header, Comment, Task>,
) where
    Document: Serialize + 'static,
    Header: Serialize + 'static,
    Comment: Serialize + 'static,
    Task: Serialize + 'static,
{
    let app = Route::new()
        .at("path", get(create_document))
        .with(AddData::new(Arc::new(service)));

    Server::new(TcpListener::bind(addr)).run(app);
}

#[handler]
pub fn create_document(svc: Arc) -> Result<()> {
    Ok(())
}
