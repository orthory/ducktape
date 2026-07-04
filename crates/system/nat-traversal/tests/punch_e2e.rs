use nat_traversal::{Msg, NatClient, NodeKey, run_coordinator};
use tokio::net::UdpSocket;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn two_clients_rendezvous_and_send_directly() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock));

    let a = NatClient::bind(NodeKey([0xaa; 32]), coord_addr).await.unwrap();
    let b = NatClient::bind(NodeKey([0xbb; 32]), coord_addr).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();

    // A resolves B via the coordinator and sends a direct datagram to B's
    // reflexive (== B's loopback addr here). B receives it.
    let b_addr = b.local_addr().await.unwrap();
    a.send_punch_to(b_addr).await.unwrap();

    let got = timeout(Duration::from_secs(2), b.recv_punch())
        .await
        .expect("no timeout")
        .expect("recv");
    assert_eq!(got, Msg::Punch { from: NodeKey([0xaa; 32]) });
}
