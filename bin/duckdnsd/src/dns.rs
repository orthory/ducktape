use std::io;
use std::iter;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use hickory_proto::op::{Header, HeaderCounts, Metadata, ResponseCode};
use hickory_proto::rr::rdata::{A, AAAA};
use hickory_proto::rr::{DNSClass, RData, Record, RecordType};
use hickory_server::Server;
use hickory_server::net::runtime::Time;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use hickory_server::zone_handler::MessageResponseBuilder;
use tokio::net::{TcpListener, UdpSocket};

use crate::SharedState;

const DNS_TTL: u32 = 5;

#[derive(Clone)]
pub struct DnsHandler {
    state: SharedState,
    ipv4: Option<Ipv4Addr>,
    ipv6: Option<Ipv6Addr>,
}

impl DnsHandler {
    pub fn new(
        state: SharedState,
        ipv4: Option<Ipv4Addr>,
        ipv6: Option<Ipv6Addr>,
    ) -> Result<Self, String> {
        if ipv4.is_none() && ipv6.is_none() {
            return Err("duckdnsd: DNS needs at least one HTTPS loopback address".into());
        }
        if ipv4.is_some_and(|address| !address.is_loopback())
            || ipv6.is_some_and(|address| !address.is_loopback())
        {
            return Err("duckdnsd: DNS answers must be loopback addresses".into());
        }
        Ok(Self { state, ipv4, ipv6 })
    }
}

#[async_trait::async_trait]
impl RequestHandler for DnsHandler {
    async fn handle_request<R: ResponseHandler, T: Time>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        let mut metadata = Metadata::response_from_request(&request.metadata);
        metadata.authoritative = true;
        let mut answers = Vec::new();
        let response_code = match request.request_info() {
            Ok(info) => {
                let hostname = info.query.name().to_string();
                let hostname = hostname.trim_end_matches('.');
                if !inside_zone(hostname) {
                    ResponseCode::Refused
                } else if !self.state.resolves(hostname) {
                    ResponseCode::NXDomain
                } else if info.query.query_class() != DNSClass::IN {
                    ResponseCode::Refused
                } else {
                    let name = info.query.original().name().clone();
                    match info.query.query_type() {
                        RecordType::A | RecordType::ANY => {
                            if let Some(address) = self.ipv4 {
                                answers.push(Record::from_rdata(
                                    name.clone(),
                                    DNS_TTL,
                                    RData::A(A(address)),
                                ));
                            }
                        }
                        _ => {}
                    }
                    match info.query.query_type() {
                        RecordType::AAAA | RecordType::ANY => {
                            if let Some(address) = self.ipv6 {
                                answers.push(Record::from_rdata(
                                    name,
                                    DNS_TTL,
                                    RData::AAAA(AAAA(address)),
                                ));
                            }
                        }
                        _ => {}
                    }
                    ResponseCode::NoError
                }
            }
            Err(_) => ResponseCode::FormErr,
        };
        metadata.response_code = response_code;
        let fallback = Header {
            metadata,
            counts: HeaderCounts::default(),
        };
        let response = MessageResponseBuilder::new(&request.queries, request.edns.as_ref()).build(
            metadata,
            answers.iter(),
            iter::empty(),
            iter::empty(),
            iter::empty(),
        );
        response_handle
            .send_response(response)
            .await
            .unwrap_or_else(|_| fallback.into())
    }
}

pub async fn run_dns(udp: UdpSocket, tcp: TcpListener, handler: DnsHandler) -> io::Result<()> {
    if !udp.local_addr()?.ip().is_loopback() || !tcp.local_addr()?.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DuckDNS DNS listeners must be loopback",
        ));
    }
    let mut server = Server::new(handler);
    server.register_socket(udp);
    server.register_listener(tcp, Duration::from_secs(10), 32);
    server.block_until_done().await.map_err(io::Error::other)
}

fn inside_zone(hostname: &str) -> bool {
    hostname == duckdns_core::DUCKDNS_ZONE
        || hostname.ends_with(&format!(".{}", duckdns_core::DUCKDNS_ZONE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::op::{Message, MessageType, OpCode, Query};
    use hickory_proto::rr::Name;
    use std::str::FromStr as _;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    fn request(name: &str, kind: RecordType) -> Vec<u8> {
        let mut message = Message::new(7, MessageType::Query, OpCode::Query);
        message.metadata.recursion_desired = true;
        message.add_query(Query::query(Name::from_str(name).unwrap(), kind));
        message.to_vec().unwrap()
    }

    async fn query(address: std::net::SocketAddr, name: &str, kind: RecordType) -> Message {
        let bytes = request(name, kind);
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        socket.send_to(&bytes, address).await.unwrap();
        let mut response = [0u8; 2048];
        let (length, _) = timeout(Duration::from_secs(2), socket.recv_from(&mut response))
            .await
            .unwrap()
            .unwrap();
        Message::from_vec(&response[..length]).unwrap()
    }

    async fn query_tcp(address: std::net::SocketAddr, name: &str, kind: RecordType) -> Message {
        let bytes = request(name, kind);
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_u16(bytes.len() as u16).await.unwrap();
        stream.write_all(&bytes).await.unwrap();
        let length = timeout(Duration::from_secs(2), stream.read_u16())
            .await
            .unwrap()
            .unwrap();
        let mut response = vec![0; length as usize];
        stream.read_exact(&mut response).await.unwrap();
        Message::from_vec(&response).unwrap()
    }

    #[tokio::test]
    async fn authoritative_dns_answers_only_active_published_names() {
        let state = SharedState::default();
        state
            .replace(
                "workspace-a".into(),
                "127.0.0.1:18080".parse().unwrap(),
                vec!["docs.team-a1b2c3d4.net.ducktape.quack".into()],
                30,
            )
            .unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = udp.local_addr().unwrap();
        let tcp = TcpListener::bind(address).await.unwrap();
        let task = tokio::spawn(run_dns(
            udp,
            tcp,
            DnsHandler::new(state.clone(), Some(Ipv4Addr::new(127, 77, 0, 1)), None).unwrap(),
        ));

        let answer = query(
            address,
            "docs.team-a1b2c3d4.net.ducktape.quack.",
            RecordType::A,
        )
        .await;
        assert_eq!(answer.metadata.response_code, ResponseCode::NoError);
        assert!(answer.metadata.authoritative);
        assert_eq!(answer.answers.len(), 1);
        assert_eq!(answer.answers[0].ttl, DNS_TTL);

        let tcp_answer = query_tcp(
            address,
            "docs.team-a1b2c3d4.net.ducktape.quack.",
            RecordType::A,
        )
        .await;
        assert_eq!(tcp_answer.metadata.response_code, ResponseCode::NoError);
        assert_eq!(tcp_answer.answers.len(), 1);

        let unknown = query(
            address,
            "unknown.team-a1b2c3d4.net.ducktape.quack.",
            RecordType::A,
        )
        .await;
        assert_eq!(unknown.metadata.response_code, ResponseCode::NXDomain);

        state.clear("workspace-a").unwrap();
        let inactive = query(
            address,
            "docs.team-a1b2c3d4.net.ducktape.quack.",
            RecordType::A,
        )
        .await;
        assert_eq!(inactive.metadata.response_code, ResponseCode::NXDomain);
        task.abort();
    }
}
