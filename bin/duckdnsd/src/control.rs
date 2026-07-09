use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::{SharedState, SnapshotStatus};

const CONTROL_TOKEN_FILE: &str = "control.token";
const MAX_CONTROL_FRAME: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ControlRequest {
    Register {
        workspace_id: String,
        ingress: SocketAddr,
        names: Vec<String>,
        lease_seconds: u64,
    },
    Clear {
        workspace_id: String,
    },
    Status,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ControlReply {
    Ok { status: SnapshotStatus },
    Error { error: String },
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    token: String,
    request: ControlRequest,
}

#[derive(Clone, Debug)]
pub struct ControlClient {
    address: SocketAddr,
    token: String,
}

impl ControlClient {
    pub fn new(address: SocketAddr, token: String) -> Result<Self, String> {
        if !address.ip().is_loopback() {
            return Err("duckdnsd: control address must be loopback".into());
        }
        validate_token(&token)?;
        Ok(Self { address, token })
    }

    pub fn from_token_file(address: SocketAddr, path: &Path) -> Result<Self, String> {
        let token = std::fs::read_to_string(path)
            .map_err(|error| format!("read DuckDNS control token {}: {error}", path.display()))?;
        Self::new(address, token.trim().to_owned())
    }

    pub async fn request(&self, request: ControlRequest) -> Result<SnapshotStatus, String> {
        let mut stream = TcpStream::connect(self.address)
            .await
            .map_err(|error| format!("connect DuckDNS control {}: {error}", self.address))?;
        let mut bytes = serde_json::to_vec(&Envelope {
            token: self.token.clone(),
            request,
        })
        .map_err(|error| format!("encode DuckDNS control request: {error}"))?;
        bytes.push(b'\n');
        stream
            .write_all(&bytes)
            .await
            .map_err(|error| format!("write DuckDNS control request: {error}"))?;

        let mut response = Vec::new();
        BufReader::new(stream)
            .take(MAX_CONTROL_FRAME)
            .read_until(b'\n', &mut response)
            .await
            .map_err(|error| format!("read DuckDNS control response: {error}"))?;
        match serde_json::from_slice::<ControlReply>(&response)
            .map_err(|error| format!("decode DuckDNS control response: {error}"))?
        {
            ControlReply::Ok { status } => Ok(status),
            ControlReply::Error { error } => Err(error),
        }
    }
}

pub fn control_token_path(state_dir: &Path) -> PathBuf {
    state_dir.join(CONTROL_TOKEN_FILE)
}

pub fn load_or_create_token(state_dir: &Path) -> io::Result<String> {
    std::fs::create_dir_all(state_dir)?;
    let path = control_token_path(state_dir);
    match std::fs::read_to_string(&path) {
        Ok(token) => {
            let token = token.trim().to_owned();
            validate_token(&token).map_err(io::Error::other)?;
            Ok(token)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let token = hex(&bytes);
            write_private_new(&path, format!("{token}\n").as_bytes())?;
            Ok(token)
        }
        Err(error) => Err(error),
    }
}

pub async fn run_control(
    listener: TcpListener,
    state: SharedState,
    expected_token: String,
) -> io::Result<()> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DuckDNS control listener must be loopback",
        ));
    }
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        let expected_token = expected_token.clone();
        tokio::spawn(async move {
            let _ = handle(stream, state, &expected_token).await;
        });
    }
}

async fn handle(stream: TcpStream, state: SharedState, expected_token: &str) -> io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut frame = Vec::new();
    let read = BufReader::new(reader)
        .take(MAX_CONTROL_FRAME + 1)
        .read_until(b'\n', &mut frame)
        .await?;
    let reply = if read == 0 || read as u64 > MAX_CONTROL_FRAME || !frame.ends_with(b"\n") {
        ControlReply::Error {
            error: "duckdnsd: invalid control frame".into(),
        }
    } else {
        match serde_json::from_slice::<Envelope>(&frame) {
            Ok(envelope) if token_matches(expected_token, &envelope.token) => {
                apply(&state, envelope.request)
            }
            Ok(_) => ControlReply::Error {
                error: "duckdnsd: control authentication failed".into(),
            },
            Err(error) => ControlReply::Error {
                error: format!("duckdnsd: malformed control request: {error}"),
            },
        }
    };
    let mut encoded = serde_json::to_vec(&reply).map_err(io::Error::other)?;
    encoded.push(b'\n');
    writer.write_all(&encoded).await
}

fn apply(state: &SharedState, request: ControlRequest) -> ControlReply {
    let result = match request {
        ControlRequest::Register {
            workspace_id,
            ingress,
            names,
            lease_seconds,
        } => state.replace(workspace_id, ingress, names, lease_seconds),
        ControlRequest::Clear { workspace_id } => state.clear(&workspace_id),
        ControlRequest::Status => Ok(()),
    };
    match result {
        Ok(()) => ControlReply::Ok {
            status: state.status(),
        },
        Err(error) => ControlReply::Error { error },
    }
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("duckdnsd: control token must be 64 lowercase hex digits".into());
    }
    Ok(())
}

fn token_matches(expected: &str, supplied: &str) -> bool {
    let mut difference = expected.len() ^ supplied.len();
    let maximum = expected.len().max(supplied.len());
    let expected = expected.as_bytes();
    let supplied = supplied.as_bytes();
    for index in 0..maximum {
        difference |= usize::from(*expected.get(index).unwrap_or(&0))
            ^ usize::from(*supplied.get(index).unwrap_or(&0));
    }
    difference == 0
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_private_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    use std::io::Write as _;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn authenticated_control_replaces_and_clears_active_workspace() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let token = "ab".repeat(32);
        let state = SharedState::default();
        tokio::spawn(run_control(listener, state.clone(), token.clone()));
        let client = ControlClient::new(address, token).unwrap();
        let status = client
            .request(ControlRequest::Register {
                workspace_id: "workspace-a".into(),
                ingress: "127.0.0.1:18080".parse().unwrap(),
                names: vec!["docs.team-a1b2c3d4.net.ducktape.quack".into()],
                lease_seconds: 30,
            })
            .await
            .unwrap();
        assert!(matches!(status, SnapshotStatus::Active { names: 1, .. }));
        client
            .request(ControlRequest::Clear {
                workspace_id: "workspace-a".into(),
            })
            .await
            .unwrap();
        assert_eq!(state.status(), SnapshotStatus::Inactive);
    }

    #[tokio::test]
    async fn wrong_token_cannot_mutate_state() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let state = SharedState::default();
        tokio::spawn(run_control(listener, state.clone(), "ab".repeat(32)));
        let client = ControlClient::new(address, "cd".repeat(32)).unwrap();
        assert!(client.request(ControlRequest::Status).await.is_err());
        assert_eq!(state.status(), SnapshotStatus::Inactive);
    }
}
