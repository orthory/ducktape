use nat_traversal::{Msg, NatClient, NodeKey, run_coordinator};
use tokio::net::UdpSocket;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn two_clients_rendezvous_through_coordinator_and_send_directly() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock, nat_traversal::AuthPolicy::Open { require_pop: false }));

    let a_key = NodeKey([0xaa; 32]);
    let b_key = NodeKey([0xbb; 32]);
    let a = NatClient::bind(a_key, coord_addr).await.unwrap();
    let b = NatClient::bind(b_key, coord_addr).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();

    // A resolves B's reflexive address purely through the coordinator's
    // Lookup/LookupResponse rendezvous path — never touching B's socket
    // directly.
    let b_reflexive = timeout(Duration::from_secs(2), a.lookup(b_key))
        .await
        .expect("no timeout")
        .expect("lookup");

    // B learns A's reflexive from the PunchSync the coordinator fans out to
    // the other side of a Lookup — again, no direct access to A's socket.
    let a_reflexive = timeout(Duration::from_secs(2), b.recv_punch_sync())
        .await
        .expect("no timeout")
        .expect("recv PunchSync");

    // Only now, armed with what the coordinator resolved, does A send a
    // direct datagram to B's reflexive address.
    a.send_punch_to(b_reflexive).await.unwrap();

    // B accepts the Punch only if it actually arrives from the address the
    // rendezvous resolved for A.
    let got = timeout(Duration::from_secs(2), b.recv_punch_from(a_reflexive))
        .await
        .expect("no timeout")
        .expect("recv");
    assert_eq!(got, Msg::Punch { from: a_key });
}
