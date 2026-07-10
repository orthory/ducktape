#![cfg(feature = "server")]

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use duckdnsd::{
    CaStore, ControlClient, ControlRequest, DnsHandler, LeafResolver, SharedState, SnapshotStatus,
    run_control, run_dns, run_https, tls_config,
};
use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{Name, RData, RecordType};
use rustls::pki_types::ServerName;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader,
};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_rustls::TlsConnector;

const FIRST: &str = "docs.team-a1b2c3d4.net.duck";
const SECOND: &str = "status.team-a1b2c3d4.net.duck";

fn dns_request(name: &str, kind: RecordType) -> Vec<u8> {
    let mut message = Message::new(17, MessageType::Query, OpCode::Query);
    message.add_query(Query::query(Name::from_str(name).unwrap(), kind));
    message.to_vec().unwrap()
}

async fn udp_query(address: SocketAddr, name: &str, kind: RecordType) -> Message {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    socket
        .send_to(&dns_request(name, kind), address)
        .await
        .unwrap();
    let mut response = [0u8; 2048];
    let (length, _) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut response))
        .await
        .unwrap()
        .unwrap();
    Message::from_vec(&response[..length]).unwrap()
}

async fn tcp_query(address: SocketAddr, name: &str, kind: RecordType) -> Message {
    let request = dns_request(name, kind);
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_u16(request.len() as u16).await.unwrap();
    stream.write_all(&request).await.unwrap();
    let length = stream.read_u16().await.unwrap();
    let mut response = vec![0; length as usize];
    stream.read_exact(&mut response).await.unwrap();
    Message::from_vec(&response).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_switches_the_authoritative_udp_and_tcp_namespace() {
    let state = SharedState::default();
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let dns_address = udp.local_addr().unwrap();
    let tcp = TcpListener::bind(dns_address).await.unwrap();
    let dns_task = tokio::spawn(run_dns(
        udp,
        tcp,
        DnsHandler::new(
            state.clone(),
            Some(Ipv4Addr::new(127, 77, 0, 1)),
            Some(Ipv6Addr::LOCALHOST),
        )
        .unwrap(),
    ));

    let control = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control_address = control.local_addr().unwrap();
    let token = "ab".repeat(32);
    let control_task = tokio::spawn(run_control(control, state.clone(), token.clone()));
    let client = ControlClient::new(control_address, token).unwrap();

    let status = client
        .request(ControlRequest::Register {
            workspace_id: "workspace-a".into(),
            ingress: "127.0.0.1:18080".parse().unwrap(),
            names: vec![FIRST.into()],
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert!(matches!(
        status,
        SnapshotStatus::Active {
            ref workspace_id,
            names: 1,
            ..
        } if workspace_id == "workspace-a"
    ));

    let a = udp_query(dns_address, FIRST, RecordType::A).await;
    assert_eq!(a.metadata.response_code, ResponseCode::NoError);
    assert_eq!(a.answers.len(), 1);
    assert_eq!(a.answers[0].ttl, 5);
    assert!(matches!(a.answers[0].data, RData::A(_)));
    let aaaa = udp_query(dns_address, FIRST, RecordType::AAAA).await;
    assert_eq!(aaaa.answers.len(), 1);
    assert!(matches!(aaaa.answers[0].data, RData::AAAA(_)));
    let tcp = tcp_query(dns_address, FIRST, RecordType::A).await;
    assert_eq!(tcp.metadata.response_code, ResponseCode::NoError);
    assert_eq!(tcp.answers.len(), 1);

    client
        .request(ControlRequest::Register {
            workspace_id: "workspace-b".into(),
            ingress: "127.0.0.1:18081".parse().unwrap(),
            names: vec![SECOND.into()],
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert_eq!(
        udp_query(dns_address, FIRST, RecordType::A)
            .await
            .metadata
            .response_code,
        ResponseCode::NXDomain
    );
    assert_eq!(
        udp_query(dns_address, SECOND, RecordType::A)
            .await
            .metadata
            .response_code,
        ResponseCode::NoError
    );

    assert!(
        client
            .request(ControlRequest::Clear {
                workspace_id: "workspace-a".into(),
            })
            .await
            .is_err(),
        "an old workspace cannot clear the newly active workspace"
    );
    assert_eq!(
        udp_query(dns_address, SECOND, RecordType::A)
            .await
            .metadata
            .response_code,
        ResponseCode::NoError
    );
    client
        .request(ControlRequest::Clear {
            workspace_id: "workspace-b".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        udp_query(dns_address, SECOND, RecordType::A)
            .await
            .metadata
            .response_code,
        ResponseCode::NXDomain
    );

    dns_task.abort();
    control_task.abort();
}

fn tls_connector(ca: &CaStore) -> TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.root_der()).unwrap();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    TlsConnector::from(Arc::new(config))
}

async fn tls_stream(
    connector: &TlsConnector,
    address: SocketAddr,
    hostname: &str,
) -> tokio_rustls::client::TlsStream<TcpStream> {
    let stream = TcpStream::connect(address).await.unwrap();
    connector
        .connect(ServerName::try_from(hostname.to_owned()).unwrap(), stream)
        .await
        .unwrap()
}

async fn read_http_response<R: AsyncBufRead + Unpin>(reader: &mut R) -> (u16, Vec<u8>) {
    let mut status_line = String::new();
    reader.read_line(&mut status_line).await.unwrap();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let mut content_length = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = Some(value.trim().parse::<usize>().unwrap());
        }
    }
    let mut body = vec![0; content_length.expect("response Content-Length")];
    reader.read_exact(&mut body).await.unwrap();
    (status, body)
}

async fn read_request_head(reader: &mut BufReader<TcpStream>) -> String {
    let mut head = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        assert!(
            !line.is_empty(),
            "request closed before its header terminator"
        );
        head.push_str(&line);
        if line == "\r\n" {
            return head;
        }
    }
}

async fn one_tls_request(
    connector: &TlsConnector,
    address: SocketAddr,
    hostname: &str,
) -> (u16, Vec<u8>) {
    let mut stream = tls_stream(connector, address, hostname).await;
    stream
        .write_all(
            format!("GET / HTTP/1.1\r\nHost: {hostname}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .unwrap();
    read_http_response(&mut BufReader::new(stream)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn https_uses_sni_trust_preserves_keep_alive_and_reports_404_502_503() {
    let directory = tempfile::tempdir().unwrap();
    let ca = CaStore::load_or_create(directory.path()).unwrap();
    let connector = tls_connector(&ca);
    let state = SharedState::default();

    let ingress = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ingress_address = ingress.local_addr().unwrap();
    let mock = tokio::spawn(async move {
        let (stream, _) = ingress.accept().await.unwrap();
        let mut stream = BufReader::new(stream);
        for (path, body, connection) in [("/one", "one", "keep-alive"), ("/two", "two", "close")] {
            let head = read_request_head(&mut stream).await;
            assert!(head.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
            assert!(head.contains(&format!("\r\nHost: {FIRST}\r\n")));
            stream
                .get_mut()
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: {connection}\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    state
        .replace(
            "workspace-a".into(),
            ingress_address,
            vec![FIRST.into()],
            30,
        )
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let https_address = listener.local_addr().unwrap();
    let https_task = tokio::spawn(run_https(
        listener,
        tls_config(LeafResolver::new(ca)),
        state.clone(),
    ));

    let stream = tls_stream(&connector, https_address, FIRST).await;
    let (read, mut write) = tokio::io::split(stream);
    let mut read = BufReader::new(read);
    for (path, expected) in [("/one", b"one".as_slice()), ("/two", b"two".as_slice())] {
        write
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: {FIRST}\r\nConnection: keep-alive\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        let (status, body) = read_http_response(&mut read).await;
        assert_eq!(status, 200);
        assert_eq!(body, expected);
    }
    mock.await.unwrap();

    // The helper rejects an unpublished SNI name before it attempts the
    // active workspace ingress. A dead ingress therefore still yields 404 for
    // the unknown name and 502 for the published one.
    let dead = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_address = dead.local_addr().unwrap();
    drop(dead);
    state
        .replace("workspace-a".into(), dead_address, vec![FIRST.into()], 30)
        .unwrap();
    let (status, _) = one_tls_request(&connector, https_address, SECOND).await;
    assert_eq!(status, 404);
    let (status, _) = one_tls_request(&connector, https_address, FIRST).await;
    assert_eq!(status, 502);

    state.clear("workspace-a").unwrap();
    let (status, _) = one_tls_request(&connector, https_address, FIRST).await;
    assert_eq!(status, 503);

    https_task.abort();
}
