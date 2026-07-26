//! In-process embedding proof: no CARGO_BIN_EXE, no child process.

mod harness; // for try_request (raw HTTP against the embedded server)

use std::net::{SocketAddr, TcpStream};

fn loopback0() -> SocketAddr {
    "127.0.0.1:0".parse().expect("addr")
}

fn boot_auto(storage: &std::path::Path) -> simnode::SimHandle {
    simnode::boot(
        storage,
        loopback0(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .expect("boot embedded sim")
}

#[test]
fn embedded_boot_serves_and_commits() {
    let storage = tempfile::tempdir().expect("storage");
    let sim = boot_auto(storage.path());
    let port = sim.addr().port();

    let (status, reply) = harness::try_request(port, "GET", "/v1/status", None).expect("status");
    assert_eq!(status, 200, "status: {reply}");

    // Auto-mode submit commits inline; query reads it back — the whole
    // round-trip in one process.
    let (status, reply) = harness::try_request(
        port,
        "POST",
        "/v1/submit",
        Some(&serde_json::json!({
            "target": "chat",
            "payload": { "create_channel": {
                "channel_id": "embed", "name": "embed", "post_policy": "open" } }
        })),
    )
    .expect("submit");
    assert_eq!(status, 200, "submit: {reply}");

    // the canonical read is a point read (the read-model cutover moved every
    // list-all into the index guests), so the round-trip names the channel it
    // just committed.
    let (status, reply) = harness::try_request(
        port,
        "POST",
        "/v1/query",
        Some(&serde_json::json!({
            "target": "chat", "query": { "channel": { "channel_id": "embed" } }
        })),
    )
    .expect("query");
    assert_eq!(status, 200, "query: {reply}");
    assert_eq!(
        reply["channel"]["name"], "embed",
        "committed channel visible: {reply}"
    );

    sim.shutdown();
}

#[test]
fn two_embedded_instances_are_independent() {
    let (dir_a, dir_b) = (
        tempfile::tempdir().expect("a"),
        tempfile::tempdir().expect("b"),
    );
    let a = boot_auto(dir_a.path());
    let b = boot_auto(dir_b.path());
    assert_ne!(a.addr(), b.addr());

    let (status, _) = harness::try_request(
        a.addr().port(),
        "POST",
        "/v1/submit",
        Some(&serde_json::json!({ "target": "chat", "payload": { "create_channel": {
            "channel_id": "only-a", "name": "only-a", "post_policy": "open" } } })),
    )
    .expect("submit a");
    assert_eq!(status, 200);

    let state_a = a.state().expect("state a");
    let state_b = b.state().expect("state b");
    assert_ne!(
        state_a["height"], state_b["height"],
        "a committed, b did not: {state_a} vs {state_b}"
    );

    a.shutdown();
    b.shutdown();
}

#[test]
fn shutdown_frees_the_port() {
    let storage = tempfile::tempdir().expect("storage");
    let sim = boot_auto(storage.path());
    let addr = sim.addr();
    sim.shutdown();
    assert!(
        TcpStream::connect(addr).is_err(),
        "port must refuse connections after shutdown"
    );
}

#[test]
fn held_mode_step_via_handle() {
    let storage = tempfile::tempdir().expect("storage");
    let sim = simnode::boot(storage.path(), loopback0(), simnode::SimOpts::default())
        .expect("boot held-mode sim");
    // Held mode: a submit parks until step. Submit from a thread (its HTTP
    // reply hangs until the step), then step via the handle.
    let port = sim.addr().port();
    let submitter = std::thread::spawn(move || {
        harness::try_request(
            port,
            "POST",
            "/v1/submit",
            Some(&serde_json::json!({ "target": "chat", "payload": { "create_channel": {
                "channel_id": "held", "name": "held", "post_policy": "open" } } })),
        )
    });
    // Poll sim state until the op is parked, then commit it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let state = sim.state().expect("state");
        if state["held"].as_u64().unwrap_or(0) > 0 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "op never parked: {state}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let stepped = sim.step().expect("step");
    let (status, _) = submitter.join().expect("join").expect("submit reply");
    assert_eq!(status, 200, "held submit resolves after step: {stepped}");
    sim.shutdown();
}
