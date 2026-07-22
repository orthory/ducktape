//! `node init` without `--dir` materializes the workspace in the registry
//! (`$DUCKTAPE_HOME/workspaces/<chain-id>/`), where `node list` and the
//! `-n/--network <chain-id>` selector find it. pure-CLI: no node is booted,
//! no socket is bound — `DUCKTAPE_HOME` points every run at a temp registry.

use std::path::Path;
use std::process::Command;

fn ducktape(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .arg("node")
        .args(args)
        .env("DUCKTAPE_HOME", home)
        .output()
        .expect("run ducktape")
}

fn init(home: &Path, name: &str) -> String {
    // hermetic: no ambient coordinator baked into the workspace config.
    let out = ducktape(home, &["init", "--name", name, "--primary-coordinator", "none"]);
    assert!(
        out.status.success(),
        "init failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn init_defaults_into_the_registry_and_n_selects_it() {
    let home = tempfile::tempdir().expect("tempdir");
    let chain_id = init(home.path(), "regnet");
    assert!(chain_id.starts_with("regnet#"), "chain id: {chain_id:?}");

    // the workspace landed under the registry, named by the chain id.
    let dir = home.path().join("workspaces").join(&chain_id);
    for file in ["node.toml", "network.toml", "identity.key"] {
        assert!(dir.join(file).is_file(), "missing {file} in {dir:?}");
    }

    // `node list` enumerates it.
    let out = ducktape(home.path(), &["list"]);
    assert!(out.status.success());
    let listing = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        listing.contains(&format!("{chain_id}\t{}", dir.join("node.toml").display())),
        "list output: {listing:?}"
    );

    // the run selector resolves through the registry: a bogus chain id is a
    // loud miss (proof the `-n` path scanned the registry, without booting).
    let out = ducktape(home.path(), &["run", "-n", "no-such-net"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains("no workspace"), "run -n stderr: {err:?}");
}

#[test]
fn same_name_founds_two_distinct_registry_workspaces() {
    let home = tempfile::tempdir().expect("tempdir");
    let first = init(home.path(), "kitchen");
    let second = init(home.path(), "kitchen");
    // the chain-id salt keeps same-named networks in separate workspaces.
    assert_ne!(first, second);
    let out = ducktape(home.path(), &["list"]);
    let listing = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(listing.contains(&first) && listing.contains(&second), "list: {listing:?}");
}
