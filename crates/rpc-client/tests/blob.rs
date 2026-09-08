use std::io::{Read as _, Write as _};
use std::net::TcpListener;

use ducktape_rpc::Client;

fn response_server(response: &'static [u8]) -> (Client, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = Client::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        socket.write_all(response).unwrap();
        String::from_utf8(request).unwrap()
    });
    (client, server)
}

#[tokio::test]
async fn blob_get_uses_the_digest_and_accepts_the_exact_stream_limit() {
    let (client, server) = response_server(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n",
    );
    assert_eq!(client.get_blob(&[0xab; 32], 4).await.unwrap(), b"abcd");
    let request = server.join().unwrap();
    assert!(request.starts_with(&format!(
        "GET /v1/files/blob/{} HTTP/1.1\r\n",
        "ab".repeat(32)
    )));
}

#[tokio::test]
async fn blob_get_rejects_cumulative_chunks_over_the_limit() {
    let (client, server) = response_server(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n",
    );
    let error = client.get_blob(&[0; 32], 3).await.unwrap_err();
    assert!(error.to_string().contains("exceeds the client limit"));
    server.join().unwrap();
}

#[tokio::test]
async fn blob_get_rejects_declared_oversize_and_missing_blobs() {
    for (response, reason) in [
        (
            &b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n"[..],
            "exceeds the client limit",
        ),
        (
            &b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"[..],
            "404 Not Found",
        ),
    ] {
        let (client, server) = response_server(response);
        let error = client.get_blob(&[0; 32], 4).await.unwrap_err();
        assert!(error.to_string().contains(reason), "{error}");
        server.join().unwrap();
    }
}
