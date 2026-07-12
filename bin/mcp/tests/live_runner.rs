//! the FULL-STACK live proof: a real runner CLI, driving the real
//! `ducktape-mcp` binary, against a real node, doing a real gated write that
//! really lands in consensus.
//!
//! `#[ignore]` by design. it shells out to `claude`, which needs a logged-in
//! CLI, network, and money — none of which belong in a gate that must be green
//! on every machine. but the thing it proves is the one thing no other test in
//! this crate can: that the capability spec's argv actually makes a RUNNER
//! spawn our server and let the model call it. everything else here proves our
//! binary speaks MCP correctly to a client we wrote ourselves.
//!
//! run it deliberately:
//!
//! ```text
//! cargo test -p mcp-bin --test live_runner -- --ignored --nocapture
//! ```
//!
//! keep the argv below in lockstep with `capability-host`'s claude spec — the
//! test is worthless if it proves a different command line than the one
//! production runs.

mod support;

use std::process::{Command, Stdio};

use serde_json::json;
use support::{AGENT_ID, Harness};

/// exactly the tool args `crates/system/capability-host/specs/claude.toml`
/// splices in after `-p`. if you change them there, change them here.
const MCP_CONFIG: &str = r#"{"mcpServers":{"ducktape":{"command":"ducktape-mcp"}}}"#;

#[test]
#[ignore = "drives the real `claude` CLI: needs auth, network, and budget"]
fn a_real_claude_run_writes_through_the_tool_plane_into_consensus() {
    let h = Harness::start(&["tasks.create"]);

    // the binary under test must be resolvable by BARE NAME, exactly as the
    // provisioner arranges it (path_entries() puts its dir on the run's PATH).
    // if this is wrong the runner silently starts no server and the model
    // reports the tool "unavailable" — a failure mode that cost me an hour, so
    // it is asserted rather than assumed.
    let bin = std::path::Path::new(env!("CARGO_BIN_EXE_ducktape-mcp"));
    let bin_dir = bin.parent().expect("the test binary has a directory");
    assert!(bin.exists(), "ducktape-mcp was not built at {bin:?}");
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let prompt = "Use the ducktape MCP tools. First call ducktape_whoami. Then call \
                  ducktape_task_create with the title: live-proof. Then reply with the word \
                  DONE and nothing else.";

    let out = Command::new("claude")
        .args([
            "-p",
            // --- the spec's [tools] args, spliced after args[0] ---
            "--mcp-config",
            MCP_CONFIG,
            "--allowedTools",
            "mcp__ducktape",
            // --- the spec's own base args ---
            "--output-format",
            "json",
            "--permission-mode",
            "acceptEdits",
        ])
        // the run identity the provisioner injects, and nothing else.
        .env("PATH", &path)
        .env("DUCKTAPE_NODE", h.node_url())
        .env("DUCKTAPE_RUN_AGENT", AGENT_ID)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write as _;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(prompt.as_bytes())?;
            child.wait_with_output()
        })
        .expect("run the claude CLI");

    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!("claude said: {stdout}");

    // the ONLY assertion that matters: ask the NODE what it holds. the model's
    // prose is not evidence — a model will happily claim it called a tool it
    // never reached (it did, repeatedly, while this was being built). consensus
    // is the oracle.
    let reply = h.query("tasks", json!("list"));
    let tasks = reply["tasks"].as_array().expect("a task list");
    assert_eq!(
        tasks.len(),
        1,
        "a real claude run, through the real spec argv, must have written exactly one task \
         through the MCP tool plane — the node holds: {reply}"
    );
    assert_eq!(tasks[0]["title"], "live-proof");
    assert_eq!(tasks[0]["status"], "open");
}
