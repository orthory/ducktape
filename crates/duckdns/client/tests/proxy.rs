use duckdns_client::{
    ProxyError, Publication, PublicationTarget, Publications, ServiceAnnouncement, ServiceIdentity,
    ServiceScope, proxy_to_publication,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn identity(service: &str) -> ServiceIdentity {
    ServiceIdentity {
        scope: ServiceScope::Network,
        service: service.into(),
    }
}

#[tokio::test]
async fn declared_service_proxies_streaming_bytes_in_both_directions() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = listener.local_addr().unwrap();
    let publications = Publications::new(vec![Publication {
        announcement: ServiceAnnouncement {
            scope: ServiceScope::Network,
            service: "docs".into(),
            default_homepage: false,
            allow_cross_site: false,
        },
        target: PublicationTarget::Loopback(target),
    }])
    .unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut first = [0u8; 5];
        socket.read_exact(&mut first).await.unwrap();
        assert_eq!(&first, b"hello");
        socket.write_all(b"world").await.unwrap();
        socket.shutdown().await.unwrap();
    });

    let (mut client, mut overlay) = tokio::io::duplex(64);
    let proxy = tokio::spawn(async move {
        proxy_to_publication(&identity("docs"), &publications, &mut overlay).await
    });
    client.write_all(b"hello").await.unwrap();
    client.shutdown().await.unwrap();
    let mut reply = Vec::new();
    client.read_to_end(&mut reply).await.unwrap();
    assert_eq!(reply, b"world");
    let (to_target, from_target) = proxy.await.unwrap().unwrap();
    assert_eq!((to_target, from_target), (5, 5));
    server.await.unwrap();
}

#[tokio::test]
async fn undeclared_identity_is_refused_before_any_dial() {
    let publications = Publications::default();
    let (_client, mut overlay) = tokio::io::duplex(16);
    assert!(matches!(
        proxy_to_publication(&identity("admin"), &publications, &mut overlay).await,
        Err(ProxyError::Unpublished)
    ));
}
