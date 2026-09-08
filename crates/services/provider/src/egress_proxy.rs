//! the run's route OUT: an HTTP proxy on the host end of a guest tunnel.
//!
//! A sandboxed guest has no NIC; its reach is the tunnels the host binds for
//! it, each a loopback port the guest dials and the host answers. This is the
//! one that leads off the host. It is a plain HTTP proxy the guest's tools
//! find through `HTTP_PROXY`/`HTTPS_PROXY` — `CONNECT` for TLS, absolute-form
//! requests for plain http — so `git`, `cargo`, `curl` and a runtime's fetch
//! reach the network the way they would behind any office proxy, and
//! `NO_PROXY` keeps the guest's own loopback (its node lane, its broker) off
//! it.
//!
//! What it refuses is the way back in. A destination that resolves to this
//! host (loopback, the unspecified address) or to a link-local address (a
//! cloud metadata service) is never dialled: the node's listener is reachable
//! only through the run's cap-checked lane, and this must not be a second,
//! ungated door to it or to anything else the host serves on loopback.

use std::net::{IpAddr, Ipv4Addr};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

/// the variables every http client reads its proxy from, in both spellings
/// (curl and git read the lowercase ones, most runtimes the uppercase).
pub(crate) const PROXY_ENVS: [&str; 4] = ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"];
/// the one [`PROXY_ENVS`] entry the tunnel wiring reads the port back from.
pub(crate) const HTTPS_PROXY_ENV: &str = "HTTPS_PROXY";
/// the addresses a client must dial directly: the guest's own loopback, where
/// its node lane and credential broker answer.
const NO_PROXY: &str = "127.0.0.1,localhost";

/// the destinations this proxy will dial: a verdict over each resolved
/// address. A test admits its loopback upstream; production is [`off_host`].
type Admission = fn(IpAddr) -> bool;

/// the production policy: nothing on this host, nothing link-local.
pub(crate) fn off_host(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !(v4.is_loopback() || v4.is_unspecified() || v4.is_link_local() || v4.is_broadcast()),
        IpAddr::V6(v6) => {
            let mapped_v4_on_host = v6
                .to_ipv4_mapped()
                .is_some_and(|v4| !off_host(IpAddr::V4(v4)));
            !(v6.is_loopback() || v6.is_unspecified() || v6.is_unicast_link_local() || mapped_v4_on_host)
        }
    }
}

/// this run's egress proxy. Dropping it takes the proxy down with the run.
pub(crate) struct EgressProxy {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
    /// the guest learns the port from the env; a test dials it directly.
    #[cfg(test)]
    port: u16,
}

impl Drop for EgressProxy {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

impl EgressProxy {
    /// Bind this run's proxy and point `envs` at it, returning the proxy
    /// whose lifetime is the run's.
    pub(crate) async fn start(envs: &mut Vec<(String, String)>) -> Result<Self, String> {
        Self::start_with(envs, off_host).await
    }

    /// [`Self::start`] under an explicit destination policy.
    pub(crate) async fn start_with(
        envs: &mut Vec<(String, String)>,
        admit: Admission,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|e| format!("bind this run's egress proxy: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("read this run's egress proxy address: {e}"))?
            .port();
        let url = format!("http://{}:{port}", Ipv4Addr::LOCALHOST);
        for key in PROXY_ENVS {
            set_env(envs, key, &url);
        }
        set_env(envs, "NO_PROXY", NO_PROXY);
        set_env(envs, "no_proxy", NO_PROXY);
        let (shutdown, mut rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut rx => return,
                    accepted = listener.accept() => {
                        let Ok((client, _)) = accepted else { return };
                        tokio::spawn(serve(client, admit));
                    }
                }
            }
        });
        Ok(Self {
            shutdown: Some(shutdown),
            task,
            #[cfg(test)]
            port,
        })
    }

    #[cfg(test)]
    fn port(&self) -> u16 {
        self.port
    }
}

fn set_env(envs: &mut Vec<(String, String)>, key: &str, value: &str) {
    match envs.iter_mut().find(|(k, _)| k == key) {
        Some(entry) => entry.1 = value.to_string(),
        None => envs.push((key.to_string(), value.to_string())),
    }
}

/// one client connection: its first request head decides whether it is a
/// `CONNECT` tunnel or a plain-http relay; from then on bytes flow raw.
async fn serve(client: TcpStream, admit: Admission) {
    let mut reader = BufReader::new(client);
    let Some(head) = read_head(&mut reader).await else {
        return;
    };
    let leftover = reader.buffer().to_vec();
    let mut client = reader.into_inner();
    let Some((method, target, version)) = request_line(&head) else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return;
    };
    match method.as_str() {
        "CONNECT" => connect(client, leftover, &target, admit).await,
        _ => relay_http(client, leftover, &head, &method, &target, &version, admit).await,
    }
}

/// `CONNECT host:port`: dial, answer 200, then splice both ways.
async fn connect(mut client: TcpStream, leftover: Vec<u8>, target: &str, admit: Admission) {
    let Some((host, port)) = host_and_port(target, 443) else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return;
    };
    let Some(mut upstream) = dial(&host, port, admit).await else {
        let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await;
        return;
    };
    let established = client
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await;
    if established.is_err() {
        return;
    }
    if upstream.write_all(&leftover).await.is_err() {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

/// an absolute-form plain-http request: dial its origin, rewrite the request
/// line to origin-form, drop the proxy's own hop header, and relay.
async fn relay_http(
    mut client: TcpStream,
    leftover: Vec<u8>,
    head: &[String],
    method: &str,
    target: &str,
    version: &str,
    admit: Admission,
) {
    let Some(rest) = target.strip_prefix("http://") else {
        let _ = client
            .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nthis proxy relays http:// targets and CONNECT tunnels\r\n")
            .await;
        return;
    };
    let (authority, path) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, "/"),
    };
    let Some((host, port)) = host_and_port(authority, 80) else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return;
    };
    let Some(mut upstream) = dial(&host, port, admit).await else {
        let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await;
        return;
    };
    let mut rewritten = format!("{method} {path} {version}\r\n");
    for line in head.iter().skip(1) {
        let hop_by_hop = line
            .to_ascii_lowercase()
            .starts_with("proxy-connection:");
        if !hop_by_hop {
            rewritten.push_str(line);
            rewritten.push_str("\r\n");
        }
    }
    rewritten.push_str("\r\n");
    if upstream.write_all(rewritten.as_bytes()).await.is_err() {
        return;
    }
    if upstream.write_all(&leftover).await.is_err() {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

/// the request head as its lines, request line first, without the blank
/// terminator; `None` when the client closed before sending one.
async fn read_head(reader: &mut BufReader<TcpStream>) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await.ok()?;
        if read == 0 {
            return None;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return Some(lines);
        }
        lines.push(line.to_string());
    }
}

fn request_line(head: &[String]) -> Option<(String, String, String)> {
    let mut parts = head.first()?.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let version = parts.next().unwrap_or("HTTP/1.1").to_string();
    Some((method, target, version))
}

/// `host:port`, `[v6]:port`, or a bare host at `default_port`.
fn host_and_port(authority: &str, default_port: u16) -> Option<(String, u16)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None => default_port,
        };
        return Some((host.to_string(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => Some((host.to_string(), port.parse().ok()?)),
        _ if !authority.is_empty() => Some((authority.to_string(), default_port)),
        _ => None,
    }
}

/// resolve `host` and connect to the first admitted address. A destination
/// with no admitted address is refused — reported once per connection, by
/// reason only: the host name is the agent's, not the operator's to spread.
async fn dial(host: &str, port: u16, admit: Admission) -> Option<TcpStream> {
    let addrs = tokio::net::lookup_host((host, port)).await.ok()?;
    let mut refused = false;
    for addr in addrs {
        if !admit(addr.ip()) {
            refused = true;
            continue;
        }
        if let Ok(stream) = TcpStream::connect(addr).await {
            return Some(stream);
        }
    }
    if refused {
        tracing::debug!(
            target: "ducktape::sandbox",
            reason = "egress_destination_refused",
            "this run's egress proxy refused a destination on or beside this host"
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    fn admit_all(_: IpAddr) -> bool {
        true
    }

    /// a loopback service that echoes every byte it reads.
    async fn echo_upstream() -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut read, mut write) = socket.split();
                    let _ = tokio::io::copy(&mut read, &mut write).await;
                });
            }
        });
        port
    }

    /// a loopback http origin that answers with the request head it saw.
    async fn head_echo_upstream() -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut reader = BufReader::new(socket);
                    let head = read_head(&mut reader).await.unwrap_or_default().join("\n");
                    let mut socket = reader.into_inner();
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{head}",
                        head.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        port
    }

    async fn status_line(stream: &mut BufReader<TcpStream>) -> String {
        let mut line = String::new();
        stream.read_line(&mut line).await.unwrap();
        line.trim_end().to_string()
    }

    #[tokio::test]
    async fn a_connect_request_becomes_a_raw_tunnel() {
        let upstream = echo_upstream().await;
        let mut envs = Vec::new();
        let proxy = EgressProxy::start_with(&mut envs, admit_all).await.unwrap();
        let client = TcpStream::connect((Ipv4Addr::LOCALHOST, proxy.port()))
            .await
            .unwrap();
        let mut client = BufReader::new(client);
        client
            .get_mut()
            .write_all(
                format!("CONNECT 127.0.0.1:{upstream} HTTP/1.1\r\nHost: 127.0.0.1:{upstream}\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        assert_eq!(status_line(&mut client).await, "HTTP/1.1 200 Connection Established");
        read_head(&mut client).await.expect("the established head ends");
        client.get_mut().write_all(b"quack").await.unwrap();
        let mut echoed = [0u8; 5];
        client.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, b"quack");
    }

    #[tokio::test]
    async fn a_plain_http_request_reaches_its_origin_in_origin_form() {
        let upstream = head_echo_upstream().await;
        let mut envs = Vec::new();
        let proxy = EgressProxy::start_with(&mut envs, admit_all).await.unwrap();
        let client = TcpStream::connect((Ipv4Addr::LOCALHOST, proxy.port()))
            .await
            .unwrap();
        let mut client = BufReader::new(client);
        client
            .get_mut()
            .write_all(
                format!(
                    "GET http://127.0.0.1:{upstream}/x?y=1 HTTP/1.1\r\nHost: 127.0.0.1:{upstream}\r\nProxy-Connection: keep-alive\r\nAccept: */*\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        assert_eq!(status_line(&mut client).await, "HTTP/1.1 200 OK");
        read_head(&mut client).await.expect("the reply head ends");
        let mut body = String::new();
        client.read_to_string(&mut body).await.unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines[0], "GET /x?y=1 HTTP/1.1", "{body}");
        assert!(lines.contains(&"Accept: */*"), "{body}");
        assert!(
            !body.to_ascii_lowercase().contains("proxy-connection"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn the_host_itself_is_refused_as_a_destination() {
        let upstream = echo_upstream().await;
        let mut envs = Vec::new();
        let proxy = EgressProxy::start(&mut envs).await.unwrap();
        let client = TcpStream::connect((Ipv4Addr::LOCALHOST, proxy.port()))
            .await
            .unwrap();
        let mut client = BufReader::new(client);
        client
            .get_mut()
            .write_all(format!("CONNECT 127.0.0.1:{upstream} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        assert_eq!(status_line(&mut client).await, "HTTP/1.1 403 Forbidden");
    }

    #[test]
    fn the_policy_refuses_this_host_and_link_local_and_admits_the_network() {
        let refused = ["127.0.0.1", "127.9.9.9", "0.0.0.0", "169.254.169.254", "::1", "::", "fe80::1", "::ffff:127.0.0.1"];
        for ip in refused {
            assert!(!off_host(ip.parse().unwrap()), "{ip}");
        }
        let admitted = ["8.8.8.8", "140.82.112.3", "10.0.0.7", "192.168.1.20", "2606:4700::1111"];
        for ip in admitted {
            assert!(off_host(ip.parse().unwrap()), "{ip}");
        }
    }

    #[tokio::test]
    async fn the_guest_env_points_every_client_at_the_proxy_and_keeps_loopback_direct() {
        let mut envs = vec![("HTTP_PROXY".to_string(), "http://stale:1".to_string())];
        let proxy = EgressProxy::start(&mut envs).await.unwrap();
        let url = format!("http://127.0.0.1:{}", proxy.port());
        for key in PROXY_ENVS {
            let value = envs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
            assert_eq!(value, Some(url.as_str()), "{key}");
        }
        for key in ["NO_PROXY", "no_proxy"] {
            let value = envs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
            assert_eq!(value, Some(NO_PROXY), "{key}");
        }
        assert_eq!(envs.iter().filter(|(k, _)| k == "HTTP_PROXY").count(), 1, "replaced, not duplicated");
    }
}
