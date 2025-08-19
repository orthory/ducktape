use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let listen_addr = "0.0.0.0:21922".to_string();

    let base_path = ".".to_string();
    let document_service = api::services::document::DocumentService::new(base_path);
    api::http::server::create_server(listen_addr, Arc::new(document_service)).await;

    Ok(())
}
