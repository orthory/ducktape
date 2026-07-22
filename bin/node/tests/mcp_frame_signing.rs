//! The MCP child gets no frame signer. It can only send a typed Runs message to
//! the host-owned, run-scoped action endpoint provisioned for this invocation.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const RUN_ID: &str = "saga-7:0";
const AGENT_ID: &str = "quackbot";
const TOKEN: &str = "abababababababababababababababababababababababababababababababab";

struct Captured {
    path: String,
    token: Option<String>,
    message: runs::RunsMsg,
}

fn capture_action(tool: &str, arguments: Value) -> (Value, Captured) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind scoped action stub");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let address = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let stub = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let (path, headers, body) = read_request(&mut stream);
                    let request: Value = serde_json::from_slice(&body).expect("json request");
                    let message =
                        serde_json::from_value(request["message"].clone()).expect("typed RunsMsg");
                    tx.send(Captured {
                        path,
                        token: headers
                            .into_iter()
                            .find(|(name, _)| name == "x-ducktape-run-action")
                            .map(|(_, value)| value),
                        message,
                    })
                    .unwrap();
                    write_json(&mut stream, 200, &json!({"message":"ok"}));
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "MCP never called the action endpoint"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accept action request: {error}"),
            }
        }
    });

    let result = call_tool(
        command(Some(format!("http://{address}/v1/run-action"))),
        tool,
        arguments,
    );
    let captured = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("captured action");
    stub.join().expect("action stub");
    (result, captured)
}

fn command(action_url: Option<String>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ducktape"));
    command
        .arg("mcp")
        .env("DUCKTAPE_RUN_AGENT", AGENT_ID)
        .env("DUCKTAPE_RUN_ID", RUN_ID)
        .env_remove("DUCKTAPE_RUN_SESSION_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(url) = action_url {
        command
            .env("DUCKTAPE_RUN_ACTION_URL", url)
            .env("DUCKTAPE_RUN_ACTION_TOKEN", TOKEN);
    } else {
        command
            .env_remove("DUCKTAPE_RUN_ACTION_URL")
            .env_remove("DUCKTAPE_RUN_ACTION_TOKEN");
    }
    command
}

fn call_tool(mut command: Command, tool: &str, arguments: Value) -> Value {
    let mut child = command.spawn().expect("spawn MCP");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = BufReader::new(child.stdout.take().expect("stdout"));
    for request in [
        json!({"jsonrpc":"2.0", "id":0, "method":"initialize", "params":{}}),
        json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
        json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"tools/call",
            "params":{"name":tool, "arguments":arguments},
        }),
    ] {
        writeln!(stdin, "{request}").expect("write MCP request");
    }
    drop(stdin);
    let responses: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(&line.expect("response line")).expect("response json"))
        .collect();
    child.wait().expect("MCP exits");
    responses
        .into_iter()
        .find(|response| response["id"] == 1)
        .expect("tool response")["result"]
        .clone()
}

#[test]
fn task_write_uses_the_exact_run_scoped_endpoint() {
    let (result, captured) = capture_action("ducktape_task_create", json!({"title":"prove it"}));
    assert_ne!(result["isError"], true, "{result}");
    assert_eq!(captured.path, "/v1/run-action");
    assert_eq!(captured.token.as_deref(), Some(TOKEN));
    match captured.message {
        runs::RunsMsg::AgentAction { run_id, action } => {
            assert_eq!(run_id, RUN_ID);
            match action {
                agent::AgentAction::CreateTask { title, .. } => assert_eq!(title, "prove it"),
                other => panic!("expected CreateTask, got {other:?}"),
            }
        }
        other => panic!("expected AgentAction, got {other:?}"),
    }
}

#[test]
fn peer_call_exposes_no_caller_identity_or_authority_input() {
    let (result, captured) = capture_action(
        "ducktape_delegate",
        json!({
            "request_id":"review-1",
            "agent_id":"reviewer",
            "instruction":"Review this change",
            "skills":["/shared/skills/review"],
        }),
    );
    assert_ne!(result["isError"], true, "{result}");
    match captured.message {
        runs::RunsMsg::DelegateRun {
            run_id,
            request_id,
            request,
        } => {
            assert_eq!(run_id, RUN_ID);
            assert_eq!(request_id, "review-1");
            assert_eq!(request.agent_id, "reviewer");
            assert_eq!(request.instruction, "Review this change");
            assert_eq!(request.skills, ["/shared/skills/review"]);
        }
        other => panic!("expected DelegateRun, got {other:?}"),
    }
}

#[test]
fn a_run_without_a_scoped_endpoint_refuses_to_write() {
    let result = call_tool(
        command(None),
        "ducktape_task_create",
        json!({"title":"nope"}),
    );
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("no scoped action endpoint")
    );
}

fn read_request(stream: &mut TcpStream) -> (String, Vec<(String, String)>, Vec<u8>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut start = String::new();
    reader.read_line(&mut start).expect("request line");
    let path = start.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut headers = Vec::new();
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("header");
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.to_ascii_lowercase();
            let value = value.trim().to_string();
            if name == "content-length" {
                length = value.parse().unwrap_or(0);
            }
            headers.push((name, value));
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("request body");
    (path, headers, body)
}

fn write_json(stream: &mut TcpStream, status: u16, value: &Value) {
    let body = value.to_string();
    write!(
        stream,
        "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write response");
}
