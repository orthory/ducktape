//! `node init` without `--dir` materializes the workspace in the registry
//! (`$DUCKTAPE_HOME/workspaces/<chain-id>/`), where `node list` and the
//! `-n/--network <chain-id>` selector find it. pure-CLI: no node is booted,
//! no socket is bound — `DUCKTAPE_HOME` points every run at a temp registry.

use std::path::{Path, PathBuf};
use std::process::Command;

/// the checked-in `<id>.component.wasm` set — `node init` hashes a directory of
/// these into the descriptor, so a CLI test that founds a network needs one.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/kernel/host/tests/fixtures")
}

fn ducktape(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .arg("node")
        .args(args)
        .env("DUCKTAPE_HOME", home)
        .env("DUCKTAPE_MODULES_DIR", fixtures())
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

/// init a workspace with the child's PATH pinned to `path_dir`, returning the
/// generated node.toml and the workspace dir — the seam that makes compute
/// detection hermetic: the probe only checks executability on PATH, it never
/// runs the binary.
fn init_with_path(home: &Path, name: &str, path_dir: &Path) -> (String, std::path::PathBuf) {
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(["node", "init", "--name", name, "--primary-coordinator", "none"])
        .env("DUCKTAPE_HOME", home)
        .env("DUCKTAPE_MODULES_DIR", fixtures())
        .env("PATH", path_dir)
        .output()
        .expect("run ducktape");
    assert!(
        out.status.success(),
        "init failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let chain_id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let workspace = home.join("workspaces").join(&chain_id);
    let toml = std::fs::read_to_string(workspace.join("node.toml"))
        .expect("read generated node.toml");
    (toml, workspace)
}

/// fresh-workspace compute detection on a host that cannot run a microVM: the
/// `[sandbox]` table stays the commented example, and no service is granted.
///
/// A fake `firecracker` on PATH is deliberately NOT enough. Detection asks
/// `SandboxBackend::probe`, which opens `/dev/kvm` and stats the kernel and
/// rootfs images — a table naming a runtime this host cannot start would just
/// move the failure to the next boot, where nobody is standing. So the only
/// hermetic half of detection is this one: no usable microVM, no live table.
/// The agreement between the table init writes and the backend it probes is
/// pinned by `cli::sandbox_detection_tests`.
#[test]
fn init_on_a_host_without_a_microvm_keeps_the_commented_sandbox_table() {
    use std::os::unix::fs::PermissionsExt as _;

    // an executable named like the adapter, and its hard deps beside it: the
    // PATH question alone answers yes here, so a live table would prove
    // detection stopped at PATH instead of asking the host.
    let bins = tempfile::tempdir().expect("fake bin dir");
    for bin in ["firecracker", "mke2fs", "debugfs", "nft"] {
        let fake = bins.path().join(bin);
        std::fs::write(&fake, "#!/bin/sh\nexit 0\n").expect("write fake runtime");
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))
            .expect("mark fake runtime executable");
    }

    let home = tempfile::tempdir().expect("tempdir");
    let (toml, workspace) = init_with_path(home.path(), "computed", bins.path());
    assert!(
        toml.contains("#[sandbox]") && !toml.contains("\n[sandbox]"),
        "a fake runtime on PATH must not pass the host probe:\n{toml}"
    );
    // detection never opts the node into publishing capacity either way: the
    // table would say only HOW a run is isolated. Announcing needs a compute
    // grant, which init cannot mint (there is no daemon signaling yet), so a
    // fresh workspace carries no services.toml at all.
    assert!(
        !workspace.join("services.toml").exists(),
        "a fresh workspace grants no service"
    );

    let empty = tempfile::tempdir().expect("empty PATH dir");
    let home = tempfile::tempdir().expect("tempdir");
    let (toml, _) = init_with_path(home.path(), "bare", empty.path());
    assert!(
        toml.contains("#[sandbox]") && !toml.contains("\n[sandbox]"),
        "no runtime on PATH keeps the commented example:\n{toml}"
    );
}

/// Run any family, not just `node`.
fn ducktape_raw(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(args)
        .env("DUCKTAPE_HOME", home)
        .env("DUCKTAPE_MODULES_DIR", fixtures())
        .output()
        .expect("run ducktape")
}

/// ONE registered workspace means there is nothing to disambiguate, and every
/// family must agree about that — `node run` and `node status` always did,
/// while `service`, `user account-init` and `user cred` each demanded
/// `-n/--network` on a machine with exactly one network on it. That is the
/// difference between `ducktape service list` and
/// `ducktape service list -n 'mynet#d0cdf950'` for every command in a session.
///
/// Driven as a REAL subprocess against a real registry, because the bug this
/// pins is not in the ladder: it is a family resolving the registry's
/// `(chain-id, node.toml-path)` pair and using the PATH where a DIRECTORY was
/// meant. A unit test of the ladder cannot see that; `service list` answering
/// `read ".../node.toml/services.toml": Not a directory` can.
#[test]
fn one_registered_workspace_needs_no_selector_in_any_family() {
    let home = tempfile::tempdir().expect("tempdir");
    let chain_id = init(home.path(), "solonet");

    // `service` reads grants off disk and renders an unreachable node calmly,
    // so this answers with no node running at all.
    let out = ducktape_raw(home.path(), &["service", "list"]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "service list must infer the lone workspace:\n{stderr}"
    );
    assert!(
        !stderr.contains("node.toml/"),
        "the workspace DIR was resolved, not its node.toml path: {stderr}"
    );
    assert!(
        stdout.contains("none enabled") || stdout.contains("KIND"),
        "service list rendered nothing: {stdout:?} {stderr:?}"
    );

    // and the KIND filter every other service verb takes is accepted here too.
    let out = ducktape_raw(home.path(), &["service", "status", "compute"]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        !stderr.contains("unexpected argument"),
        "`service status <kind>` must parse: {stderr}"
    );
    assert!(
        stderr.contains("ducktape service run compute"),
        "an absent kind says how to start it: {stderr}"
    );

    // a SECOND network removes the inference — and the refusal names both, so
    // the reader can pick.
    let second = init(home.path(), "othernet");
    let out = ducktape_raw(home.path(), &["service", "list"]);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "two workspaces must not be guessed at");
    assert!(
        stderr.contains(&chain_id) && stderr.contains(&second),
        "an ambiguous registry names its candidates: {stderr}"
    );
    assert!(stderr.contains("-n"), "and the flag that picks one: {stderr}");
}

/// `--modules <dir>` is how a founder pins its genesis wasm set: every
/// component in the directory is hashed INTO the descriptor and copied into
/// `<workspace>/modules`, the bundle the node seeds its blobstore from at boot.
/// The copy and the hash must be the same bytes, or the node refuses its own
/// workspace on the next start.
#[test]
fn init_writes_module_hashes_and_the_bundle() {
    use sha2::Digest as _;
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().join("ws");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(["node", "init", "--name", "bundled", "--primary-coordinator", "none", "--dir"])
        .arg(&ws)
        .args(["--listen", "127.0.0.1:0", "--advertised", "127.0.0.1:1", "--modules"])
        .arg(fixtures())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let d = workspace_config::NetworkDescriptor::load(&ws.join("network.toml")).unwrap();
    let ids: Vec<&str> = d.modules.iter().map(|m| m.id.as_str()).collect();
    let mut want = topology::TOPOLOGY.wasm_ids(topology::PRODUCTION);
    want.sort_unstable();
    assert_eq!(ids, want);
    for m in &d.modules {
        let component = ws.join("modules").join(format!("{}.component.wasm", m.id));
        let bytes = std::fs::read(&component).expect("the bundle carries every hashed component");
        assert_eq!(workspace_config::hex_bytes(&sha2::Sha256::digest(&bytes)), m.code_hash);
    }
}

/// a bundle missing a component is named by the file the operator has to go
/// look for — not by a hash mismatch three boots later.
#[test]
fn init_names_the_missing_component() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = tmp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(["node", "init", "--name", "x", "--primary-coordinator", "none", "--dir"])
        .arg(tmp.path().join("ws"))
        .args(["--listen", "127.0.0.1:0", "--advertised", "127.0.0.1:1", "--modules"])
        .arg(&empty)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("acl.component.wasm"));
}
