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

/// init a workspace with the child's PATH pinned to `path_dir`, returning the
/// generated node.toml and the workspace dir — the seam that makes compute
/// detection hermetic: the probe only checks executability on PATH, it never
/// runs the binary.
fn init_with_path(home: &Path, name: &str, path_dir: &Path) -> (String, std::path::PathBuf) {
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(["node", "init", "--name", name, "--primary-coordinator", "none"])
        .env("DUCKTAPE_HOME", home)
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

/// fresh-workspace compute detection: the platform adapter's runtime on PATH
/// (a fake executable — podman on Linux, tart on macOS) makes a flagless init
/// write a LIVE `[sandbox]` table while granting no service at all; an empty
/// PATH keeps today's commented example.
#[test]
fn flagless_init_detects_the_platform_runtime_into_a_live_sandbox_table() {
    use std::os::unix::fs::PermissionsExt as _;

    // the probe wants the platform adapter AND its hard deps executable on
    // PATH, so the fake dir carries the full set — detection only writes a
    // table the boot probe would accept. Podman's deps are `pasta` (the netns
    // backend) plus `nft` + `nsenter` (the egress firewall the createRuntime
    // hook installs); see `SandboxBackend::probe`.
    let (runtime, fake_bins): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("tart", &["tart"])
    } else {
        ("podman", &["podman", "pasta", "nft", "nsenter"])
    };
    let bins = tempfile::tempdir().expect("fake bin dir");
    for bin in fake_bins {
        let fake = bins.path().join(bin);
        std::fs::write(&fake, "#!/bin/sh\nexit 0\n").expect("write fake runtime");
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))
            .expect("mark fake runtime executable");
    }

    let home = tempfile::tempdir().expect("tempdir");
    let (toml, workspace) = init_with_path(home.path(), "computed", bins.path());
    assert!(toml.contains("\n[sandbox]"), "live table written:\n{toml}");
    assert!(
        toml.contains(&format!("runtime = \"{runtime}\"")),
        "the platform adapter is chosen:\n{toml}"
    );
    // detection never opts the node into publishing capacity: the table says
    // only HOW a run would be isolated. Announcing needs a compute grant,
    // which init cannot mint (there is no daemon signaling yet), so a fresh
    // workspace carries no services.toml at all.
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
