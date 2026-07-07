use nat_traversal::{NatClient, NodeKey, run_coordinator};
use tokio::net::UdpSocket;

#[tokio::test]
async fn coordinator_answers_a_bind_request() {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(sock, nat_traversal::AuthPolicy::Open { require_pop: false }));

    let client = NatClient::bind(NodeKey([9u8; 32]), addr).await.unwrap();
    let reflexive = client.discover_reflexive().await.unwrap();
    // Wildcard bind vs observed loopback source: compare the port, not the IP.
    assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
}
