use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let listen_addr = "0.0.0.0:21922".to_string();

    let base_path = std::env::current_dir()
        .unwrap()
        .join("examples".to_string())
        .join("sampledata".to_string())
        .to_string_lossy()
        .to_string();

    eprintln!("running service from {:?}", base_path);
    let document_service = api::services::document::DocumentService::new(base_path);
    api::http::server::create_server(listen_addr, Arc::new(document_service)).await;

    Ok(())
}
