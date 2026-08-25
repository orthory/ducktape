use commonware_cryptography::{Signer as _, ed25519};
use nat_traversal::{ClientEvent, Msg, NatClient, NodeKey, SocketEvent, run_coordinator};
use tokio::net::UdpSocket;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn two_clients_rendezvous_through_coordinator_and_send_directly() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(
        nat_traversal::NatSocket::Owned(coord_sock),
        nat_traversal::AuthPolicy::Public,
    ));

    let a_signer = ed25519::PrivateKey::from_seed(1);
    let b_signer = ed25519::PrivateKey::from_seed(2);
    let mut a_key = [0; 32];
    let mut b_key = [0; 32];
    a_key.copy_from_slice(a_signer.public_key().as_ref());
    b_key.copy_from_slice(b_signer.public_key().as_ref());
    let a_key = NodeKey(a_key);
    let b_key = NodeKey(b_key);
    let a = NatClient::bind(a_key, vec![coord_addr], a_signer, None)
        .await
        .unwrap();
    let b = NatClient::bind(b_key, vec![coord_addr], b_signer, None)
        .await
        .unwrap();
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
    // Consumed through the dispatch API, as the reachability pump does.
    let a_reflexive = timeout(Duration::from_secs(2), async {
        loop {
            if let SocketEvent::Rendezvous(ClientEvent::PunchSync { peer_reflexive, .. }) =
                b.recv_socket_event().await?
            {
                return Ok::<_, std::io::Error>(peer_reflexive);
            }
        }
    })
    .await
    .expect("no timeout")
    .expect("recv PunchSync");

    // Only now, armed with what the coordinator resolved, does A send a
    // direct datagram to B's reflexive address.
    a.send_punch_to(b_reflexive).await.unwrap();

    // B accepts the Punch only if it actually arrives from the address the
    // rendezvous resolved for A.
    let got = timeout(Duration::from_secs(2), async {
        loop {
            if let SocketEvent::Rendezvous(ClientEvent::Punch { from, src }) =
                b.recv_socket_event().await?
                && src == a_reflexive
            {
                return Ok::<_, std::io::Error>(Msg::Punch { from });
            }
        }
    })
    .await
    .expect("no timeout")
    .expect("recv");
    assert_eq!(got, Msg::Punch { from: a_key });
}
