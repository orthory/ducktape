//! `ducktape module …` — the operator verbs for a live code swap.
mod common;

use std::process::Command;

fn ducktape(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(args)
        .output()
        .expect("run ducktape");
    (
        out.status.success(),
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn status_against_no_node_says_the_node_is_not_running() {
    let ws = tempfile::tempdir().expect("tempdir");
    // a dev-shape node.toml with rpc_listen and no node behind it
    let cfg = ws.path().join("node.toml");
    std::fs::write(&cfg, common::minimal_dev_shape_toml(ws.path())).expect("write");
    let (ok, out) = ducktape(&["module", "status", "--config", cfg.to_str().unwrap()]);
    assert!(!ok, "{out}");
    assert!(out.contains("not running"), "{out}");
}
