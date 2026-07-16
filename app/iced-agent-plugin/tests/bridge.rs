//! Integration test: boot the bridge with a fabricated snapshot, connect over
//! TCP, and exercise the read tools + error paths. Injection itself needs the
//! live event loop and is covered by the app-level e2e.

use std::time::Duration;

use iced_agent_plugin::collect::WindowSnapshot;
use iced_agent_plugin::logs::ring_layer;
use iced_agent_plugin::protocol::{Rect, Role, SemNode};
use iced_agent_plugin::AgentHandle;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

fn fabricated_main() -> WindowSnapshot {
    let root = SemNode {
        r#ref: "@1".into(),
        role: Role::Window,
        name: "main".into(),
        value: None,
        bounds: Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        },
        disabled: false,
        focused: false,
        children: vec![SemNode {
            r#ref: "@2".into(),
            role: Role::Button,
            name: "Forge".into(),
            value: None,
            bounds: Rect {
                x: 10.0,
                y: 20.0,
                width: 80.0,
                height: 30.0,
            },
            disabled: false,
            focused: false,
            children: Vec::new(),
        }],
    };
    WindowSnapshot::from_root(root)
}

/// One request line in, one response line out.
async fn roundtrip(addr: std::net::SocketAddr, line: &str) -> serde_json::Value {
    let stream = TcpStream::connect(addr).await.expect("connect");
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    writer.write_all(line.as_bytes()).await.expect("write");
    writer.write_all(b"\n").await.expect("newline");
    let reply = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
        .await
        .expect("reply in time")
        .expect("read")
        .expect("a line");
    serde_json::from_str(&reply).expect("valid json response")
}

#[tokio::test]
async fn bridge_serves_tree_find_and_errors() {
    let (_layer, logs) = ring_layer();
    let handle = AgentHandle::boot(&format!("iced-agent-test-{}-tree", std::process::id()), logs);
    *handle.snapshot_slot().lock().unwrap() = vec![fabricated_main()];
    let addr = handle.local_addr();

    // tree returns the window root with the Forge child.
    let tree = roundtrip(addr, r#"{"id":1,"cmd":{"cmd":"tree","window":"main"}}"#).await;
    assert_eq!(tree["ok"], true);
    assert_eq!(tree["result"]["name"], "main");
    assert_eq!(tree["result"]["children"][0]["name"], "Forge");
    assert_eq!(tree["result"]["children"][0]["ref"], "@2");

    // find by role+name returns the @ref.
    let found = roundtrip(
        addr,
        r#"{"id":2,"cmd":{"cmd":"find","window":"main","role":"button","name":"forge","text":null}}"#,
    )
    .await;
    assert_eq!(found["ok"], true);
    let matches = found["result"]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["ref"], "@2");

    // malformed JSON is a structured error, not a dropped connection.
    let bad = roundtrip(addr, "{ this is not json").await;
    assert_eq!(bad["ok"], false);
    assert!(bad["error"].as_str().unwrap().contains("bad json"));

    // clicking an unknown ref errors and names the ref.
    let click = roundtrip(
        addr,
        r#"{"id":3,"cmd":{"cmd":"click","target":{"ref":"@999","x":null,"y":null}}}"#,
    )
    .await;
    assert_eq!(click["ok"], false);
    assert!(click["error"].as_str().unwrap().contains("@999"));

    // windows lists the fabricated main window.
    let windows = roundtrip(addr, r#"{"id":4,"cmd":{"cmd":"windows"}}"#).await;
    assert_eq!(windows["ok"], true);
    assert_eq!(windows["result"]["windows"][0]["name"], "main");
}

#[tokio::test]
async fn bridge_serves_state_and_expect() {
    let (_layer, logs) = ring_layer();
    let handle = AgentHandle::boot(&format!("iced-agent-test-{}-state", std::process::id()), logs);
    *handle.state_slot().lock().unwrap() = serde_json::json!({
        "screen": "home",
        "search_open": true,
    });
    *handle.snapshot_slot().lock().unwrap() = vec![fabricated_main()];
    let addr = handle.local_addr();

    // dot-path state query.
    let state = roundtrip(addr, r#"{"id":1,"cmd":{"cmd":"state","path":"screen"}}"#).await;
    assert_eq!(state["result"], "home");

    // expect a StatePath condition.
    let expect = roundtrip(
        addr,
        r#"{"id":2,"cmd":{"cmd":"expect","cond":{"state_path":{"path":"search_open","equals":true}}}}"#,
    )
    .await;
    assert_eq!(expect["result"]["pass"], true);

    // expect a node condition against the snapshot.
    let node = roundtrip(
        addr,
        r#"{"id":3,"cmd":{"cmd":"expect","cond":{"node":{"role":"button","name":"Forge","exists":true}}}}"#,
    )
    .await;
    assert_eq!(node["result"]["pass"], true);
}
