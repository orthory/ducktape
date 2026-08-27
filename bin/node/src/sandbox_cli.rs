//! `ducktape node sandbox` — is this node isolating provider runs, and if not,
//! why not.
//!
//! TWO ANSWERS THAT DRIFT APART. Whether a HOST can isolate a run is a property
//! of the machine, probed live. Whether a WORKSPACE will is a `[sandbox]` table
//! in its `node.toml`, written ONCE — when `node init` or `node join` probed the
//! host — and never revisited. Every way the machine moves afterwards leaves the
//! two disagreeing:
//!
//! - the hypervisor, the shim or e2fsprogs arrives after the workspace does;
//! - the workspace was seeded on a host that could not sandbox yet;
//! - the guest images move, and the table keeps naming where they used to be.
//!
//! The symptom is always the same and always late: every setup step reports
//! ready, and then the compute and agent daemons die at boot — `no [sandbox]
//! table in node.toml`, or `the microVM kernel image is missing`. The remedy was
//! hand-editing TOML under `~/.ducktape`. This verb is the remedy instead: it
//! asks both questions in one place and offers to write what it can fix.
//!
//! It PROPOSES and the operator approves, like `agent install` beside it.
//! Enabling this table is what makes a node start announcing compute to a
//! network, and repointing one changes which images every run boots — neither is
//! ours to do unasked.

use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use workspace_config::SandboxToml;

use crate::cli_args::SandboxArgs;

type SandboxResult = Result<(), Box<dyn std::error::Error>>;

const BUILDER: &str = "ops/build-guest-rootfs.sh";

/// What this workspace's table says, measured against the machine it is on.
enum WorkspaceSandbox {
    /// No table. The node announces no compute and refuses every run.
    Off,
    /// A table whose images are both present — this node can isolate a run.
    Ready(SandboxToml),
    /// A table naming an image that is not there. The node advertises the
    /// compute plane and still fails every run, which is the worst of the three.
    Stale {
        table: SandboxToml,
        missing: PathBuf,
    },
}

pub(crate) fn run(args: SandboxArgs) -> SandboxResult {
    let config = args.selector.config_path()?;
    let dir = config
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", config.display()))?;
    println!("sandbox ({})", config.display());

    // The host first: it is the question with a live answer, and the one whose
    // failure names its own fix.
    let (platform, backend) = workspace_config::platform_sandbox()?;
    let host = backend.probe_adapter();
    match &host {
        Ok(found) => println!(
            "  host       ok    {} — {}",
            platform.runtime,
            found.display()
        ),
        Err(why) => println!("  host       NO    {why}"),
    }

    match read_workspace(dir)? {
        WorkspaceSandbox::Ready(table) => {
            println!(
                "  workspace  on    runtime = \"{}\", kernel = {}",
                table.runtime,
                table.kernel.display()
            );
            Ok(())
        }
        WorkspaceSandbox::Off => {
            println!(
                "  workspace  OFF   no [sandbox] table — this node announces no \
                 compute and refuses every provider run"
            );
            offer(dir, &platform, host.is_ok(), args.yes)
        }
        WorkspaceSandbox::Stale { table, missing } => {
            println!(
                "  workspace  STALE runtime = \"{}\", but {} is not there",
                table.runtime,
                missing.display()
            );
            // Writing what is already written fixes nothing: when the table is
            // ALREADY the one this platform would write, the images are simply
            // not built, and the builder is the whole remedy.
            if table == platform {
                return Err(format!(
                    "{} is not built yet — build it:\n    {BUILDER}",
                    missing.display()
                )
                .into());
            }
            offer(dir, &platform, host.is_ok(), args.yes)
        }
    }
}

fn read_workspace(dir: &Path) -> Result<WorkspaceSandbox, String> {
    let Some(table) = workspace_config::default_plumbing(dir)?.sandbox else {
        return Ok(WorkspaceSandbox::Off);
    };
    let missing = [&table.kernel, &table.rootfs]
        .into_iter()
        .find(|image| !image.is_file())
        .cloned();
    Ok(match missing {
        Some(missing) => WorkspaceSandbox::Stale { table, missing },
        None => WorkspaceSandbox::Ready(table),
    })
}

/// Write the platform's table, once the operator says so.
///
/// A host that cannot isolate a run gets no offer at all — a table written for
/// one only moves the failure to the node's boot probe, which is where this verb
/// exists to stop sending people. Images that are not built yet do NOT block the
/// write: `init` and `join` write this table before the builder has ever run, so
/// refusing here would make the same table conditional on the order the operator
/// happened to do things in. It is called out instead.
fn offer(dir: &Path, platform: &SandboxToml, host_ok: bool, yes: bool) -> SandboxResult {
    if !host_ok {
        return Err(
            "this host cannot isolate a run yet — fix the reason above, then re-run".into(),
        );
    }

    println!("\n  enabling it writes:");
    println!("    runtime = \"{}\"", platform.runtime);
    println!("    kernel  = \"{}\"", platform.kernel.display());
    println!("    rootfs  = \"{}\"", platform.rootfs.display());
    if !approved(yes)? {
        return Ok(());
    }

    let mut plumbing = workspace_config::default_plumbing(dir)?;
    plumbing.sandbox = Some(platform.clone());
    let written = workspace_config::write_node_toml(dir, &plumbing)?;
    println!("\n  wrote [sandbox] into {}", written.display());

    let unbuilt = [&platform.kernel, &platform.rootfs]
        .into_iter()
        .find(|image| !image.is_file());
    match unbuilt {
        Some(unbuilt) => println!(
            "  {} is not built yet — build it:\n    {BUILDER}",
            unbuilt.display()
        ),
        None => println!("  restart the node to pick it up."),
    }
    Ok(())
}

/// Off a terminal there is nobody to ask, so it prints the command that does it
/// and writes nothing.
fn approved(yes: bool) -> Result<bool, String> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        println!("\n  not a terminal — enable it with:");
        println!("    ducktape node sandbox --yes");
        return Ok(false);
    }
    dialoguer::Confirm::new()
        .with_prompt("  enable this node's compute plane")
        .default(true)
        .interact()
        .map_err(|e| format!("prompt: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace whose `[sandbox]` table names `kernel`/`rootfs` under `dir`,
    /// creating the images only if `built`.
    fn workspace(built: bool) -> (tempfile::TempDir, SandboxToml) {
        let dir = tempfile::tempdir().unwrap();
        let table = SandboxToml {
            runtime: "firecracker".into(),
            kernel: dir.path().join("vmlinux"),
            rootfs: dir.path().join("rootfs.ext4"),
            cores: 0,
            mem_gb: 0,
        };
        if built {
            std::fs::write(&table.kernel, b"kernel").unwrap();
            std::fs::write(&table.rootfs, b"rootfs").unwrap();
        }
        let mut plumbing = workspace_config::default_plumbing(dir.path()).unwrap();
        plumbing.sandbox = Some(table.clone());
        workspace_config::write_node_toml(dir.path(), &plumbing).unwrap();
        (dir, table)
    }

    /// The three states this verb exists to tell apart. Only the third was ever
    /// hard: a table that IS there and IS wrong reports the same "ready" through
    /// every setup step as one that is right, and then kills the compute daemon
    /// at boot.
    #[test]
    fn a_table_that_names_images_it_does_not_have_is_not_enabled() {
        let (built, table) = workspace(true);
        assert!(
            matches!(read_workspace(built.path()).unwrap(), WorkspaceSandbox::Ready(found) if found == table),
            "images present: the node can isolate a run"
        );

        let (unbuilt, table) = workspace(false);
        let state = read_workspace(unbuilt.path()).unwrap();
        let WorkspaceSandbox::Stale { missing, .. } = state else {
            panic!("a table naming an absent kernel is stale, not ready");
        };
        assert_eq!(missing, table.kernel, "the refusal names the missing image");

        // ...and the same workspace with the table taken back out.
        let mut plumbing = workspace_config::default_plumbing(unbuilt.path()).unwrap();
        plumbing.sandbox = None;
        workspace_config::write_node_toml(unbuilt.path(), &plumbing).unwrap();
        assert!(matches!(
            read_workspace(unbuilt.path()).unwrap(),
            WorkspaceSandbox::Off
        ));
    }

    /// Approval is the whole point of the verb: a `no` must leave the file
    /// exactly as it found it. Non-interactive stdin is the standing `no` here —
    /// `--yes` is the only way a script gets a write.
    #[test]
    fn a_declined_offer_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let plumbing = workspace_config::default_plumbing(dir.path()).unwrap();
        workspace_config::write_node_toml(dir.path(), &plumbing).unwrap();
        let before = std::fs::read(dir.path().join("node.toml")).unwrap();

        let (platform, _) = workspace_config::platform_sandbox().unwrap();
        offer(dir.path(), &platform, true, false).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("node.toml")).unwrap(),
            before,
            "an unapproved offer must not touch node.toml"
        );

        // and a host that cannot isolate a run is never even offered one.
        assert!(offer(dir.path(), &platform, false, true).is_err());
        assert_eq!(std::fs::read(dir.path().join("node.toml")).unwrap(), before);
    }

    /// `--yes` writes the platform's table, and the workspace reads back as the
    /// state the daemons need. This is the reported bug's exact shape: a
    /// `make dev` workspace with no table, on a host that can sandbox fine.
    #[test]
    fn an_approved_offer_turns_the_workspace_on() {
        let dir = tempfile::tempdir().unwrap();
        let plumbing = workspace_config::default_plumbing(dir.path()).unwrap();
        workspace_config::write_node_toml(dir.path(), &plumbing).unwrap();
        assert!(matches!(
            read_workspace(dir.path()).unwrap(),
            WorkspaceSandbox::Off
        ));

        let (mut platform, _) = workspace_config::platform_sandbox().unwrap();
        // point at images that exist so the readback is Ready on any box,
        // built guest images or not.
        platform.kernel = dir.path().join("vmlinux");
        platform.rootfs = dir.path().join("rootfs.ext4");
        std::fs::write(&platform.kernel, b"kernel").unwrap();
        std::fs::write(&platform.rootfs, b"rootfs").unwrap();

        offer(dir.path(), &platform, true, true).unwrap();
        let state = read_workspace(dir.path()).unwrap();
        let WorkspaceSandbox::Ready(written) = state else {
            panic!("an approved offer leaves the workspace able to isolate a run");
        };
        assert_eq!(written, platform);
    }
}
