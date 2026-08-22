//! driving one Firecracker microVM per run.
//!
//! Firecracker takes its whole configuration as a JSON file and boots straight
//! from it (`--config-file --no-api`), so this module does NOT speak the VMM's
//! HTTP API. That is a deliberate simplification over the obvious design and it
//! removes three failure modes: the wait for the API socket to appear (which
//! has no event seam and degrades into a spin), the `SUN_LEN` 108-byte cap that
//! bites any socket under a long workspace path, and a second HTTP client.
//!
//! What the API would buy is runtime control — pause, balloon, snapshot. The
//! spec puts snapshot restore in follow-on work, so none of it is needed yet.
//! Reach for `--api-sock` when a runtime command actually has a caller.
//!
//! [`boot_config`] is pure: the entire VM configuration is unit-testable
//! without a VMM, and the tests below are the guard on the parts that are
//! silently wrong rather than loudly wrong.

use std::path::{Path, PathBuf};

use crate::guest_manifest::{self, RunManifest};

/// how long a run's VM may live before it is killed regardless of progress.
/// A hung guest holds its whole memory footprint, so this is the backstop that
/// keeps one wedged run from costing the node a slot indefinitely.
pub const MAX_VM_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// what one run's VM needs. Sizes are hard: Firecracker gives the guest exactly
/// this many vcpus and this much memory, enforced by the hypervisor, with no
/// cgroup delegation to verify and no controller to check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmConfig {
    pub kernel: PathBuf,
    pub rootfs: PathBuf,
    /// the persistent per-agent cache volume (`CARGO_HOME`, `RUSTUP_HOME`,
    /// `target/`). Attached, never copied back — see the spec's *Build caches*.
    /// `None` for a run that does not get one.
    pub agent_volume: Option<PathBuf>,
    /// this run's READ-ONLY asset image: the context doc, the skills tree and
    /// any declared host PATH directories. Mounted at [`crate::guest_paths::GUEST_ASSETS`]
    /// and never read back.
    pub assets: PathBuf,
    /// this run's workspace image, built by [`crate::workspace_image`] and read
    /// back after the guest exits.
    pub workspace: PathBuf,
    pub vcpus: u32,
    pub mem_mib: u64,
    /// the host-side unix socket Firecracker bridges the guest's vsock onto.
    pub vsock_uds: PathBuf,
    /// the host tap device for this run, or `None` for a VM with no network
    /// device at all.
    pub tap: Option<String>,
}

/// one block device as both ends need to see it. The single source for the
/// drive list Firecracker is given AND the mountpoints the guest manifest
/// carries — the two must agree about which device is which, and deriving both
/// from one list is what makes that true by construction rather than by
/// two files happening to be edited together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestDrive {
    pub drive_id: &'static str,
    pub host_path: PathBuf,
    /// the name the guest kernel gives it. Firecracker enumerates virtio-block
    /// devices in the order they appear in the config, so this tracks position.
    pub device: &'static str,
    /// where the guest init mounts it, or `None` for the root device (already
    /// mounted by the kernel).
    pub mountpoint: Option<&'static str>,
    pub read_only: bool,
    pub is_root: bool,
}

/// where the guest sees the workspace. The manifest's `cwd` normally matches.
pub const WORKSPACE_MOUNTPOINT: &str = crate::guest_paths::GUEST_WORKSPACE;
/// where the guest sees the per-run read-only asset image.
pub const ASSETS_MOUNTPOINT: &str = crate::guest_paths::GUEST_ASSETS;
/// where the guest sees the persistent cache volume.
pub const AGENT_VOLUME_MOUNTPOINT: &str = crate::guest_paths::GUEST_AGENT_VOLUME;

/// the guest device names, in the order Firecracker enumerates them.
const DEVICE_ORDER: [&str; 4] = ["/dev/vda", "/dev/vdb", "/dev/vdc", "/dev/vdd"];

/// the drives for `cfg`, in attach order.
///
/// MOUNT ORDER IS LOAD-BEARING: the assets image must be mounted before the
/// workspace, because the workspace lands on the `workspace/` directory INSIDE
/// it. Reverse them and the workspace mount is immediately shadowed by the
/// assets mount, and the run sees an empty workspace.
pub fn guest_drives(cfg: &VmConfig) -> Vec<GuestDrive> {
    let mut drives = vec![GuestDrive {
        drive_id: "rootfs",
        host_path: cfg.rootfs.clone(),
        device: DEVICE_ORDER[0],
        // the kernel mounts the root device itself; the init must not remount it
        mountpoint: None,
        // SHARED across every concurrent run on this node. Writable would let
        // one buyer's run corrupt another's guest, so this is not a tuning knob.
        read_only: true,
        is_root: true,
    }];
    if let Some(volume) = &cfg.agent_volume {
        drives.push(GuestDrive {
            drive_id: "agent",
            host_path: volume.clone(),
            device: DEVICE_ORDER[drives.len()],
            mountpoint: Some(AGENT_VOLUME_MOUNTPOINT),
            read_only: false,
            is_root: false,
        });
    }
    drives.push(GuestDrive {
        drive_id: "assets",
        host_path: cfg.assets.clone(),
        device: DEVICE_ORDER[drives.len()],
        mountpoint: Some(ASSETS_MOUNTPOINT),
        // the run may not edit its own context doc or skills tree
        read_only: true,
        is_root: false,
    });
    drives.push(GuestDrive {
        drive_id: "workspace",
        host_path: cfg.workspace.clone(),
        device: DEVICE_ORDER[drives.len()],
        mountpoint: Some(WORKSPACE_MOUNTPOINT),
        read_only: false,
        is_root: false,
    });
    drives
}

/// the `(device, mountpoint)` pairs for a run's manifest, derived from the same
/// [`guest_drives`] list the VM is configured from.
pub fn manifest_mounts(cfg: &VmConfig) -> Vec<(String, String)> {
    guest_drives(cfg)
        .into_iter()
        .filter_map(|drive| {
            let mountpoint = drive.mountpoint?;
            Some((drive.device.to_string(), mountpoint.to_string()))
        })
        .collect()
}

/// the kernel command line, profiled rather than copied.
///
/// The i8042 group and `quiet` are worth ~840 ms together on this host, and
/// that saving is flat across every run shape — cold boot went from 1285 ms to
/// 452 ms, 2.84×.
///
/// `panic=1` so a guest panic REBOOTS rather than sitting at the kernel prompt
/// burning the run's whole idle timeout while holding all of its memory.
///
/// NEVER add `acpi=off`. It measures as a 69 ms win and is a correctness bug:
/// Firecracker enumerates vCPUs through ACPI, so the guest comes up with ONE
/// processor no matter what `vcpu_count` says — `vcpu_count=4` reports "Total
/// of 4 processors activated" with ACPI and "Total of 1" without. A node would
/// sell four cores and deliver one, silently. The test below is the guard.
pub fn boot_args(manifest_token: &str) -> String {
    format!(
        "console=ttyS0 reboot=k panic=1 pci=off quiet loglevel=1 \
         i8042.noaux i8042.nokbd i8042.nomux i8042.nopnp i8042.dumbkbd \
         init=/duck-guest-init {}={manifest_token}",
        guest_manifest::CMDLINE_KEY
    )
}

/// the complete VM configuration Firecracker boots from. Pure.
pub fn boot_config(cfg: &VmConfig, manifest: &RunManifest) -> serde_json::Value {
    let drives: Vec<serde_json::Value> = guest_drives(cfg)
        .into_iter()
        .map(|drive| {
            serde_json::json!({
                "drive_id": drive.drive_id,
                "path_on_host": drive.host_path,
                "is_read_only": drive.read_only,
                "is_root_device": drive.is_root,
            })
        })
        .collect();

    let mut config = serde_json::json!({
        "boot-source": {
            "kernel_image_path": cfg.kernel,
            "boot_args": boot_args(&guest_manifest::encode(manifest)),
        },
        "drives": drives,
        "machine-config": {
            "vcpu_count": cfg.vcpus,
            "mem_size_mib": cfg.mem_mib,
            // SMT off: a run is sold N cores and must not be able to observe
            // another tenant's sibling thread.
            "smt": false,
        },
        "vsock": {
            "guest_cid": 3,
            "uds_path": cfg.vsock_uds,
        },
    });

    // No tap means NO network device — not an unconfigured one. A VM with an
    // interface it cannot route through behaves differently from one with no
    // interface at all, and "offline" has to mean the second.
    if let Some(tap) = &cfg.tap {
        config["network-interfaces"] = serde_json::json!([{
            "iface_id": "eth0",
            "host_dev_name": tap,
        }]);
    }
    config
}

/// write `config` where Firecracker can read it, and return the path.
pub fn write_boot_config(dir: &Path, config: &serde_json::Value) -> Result<PathBuf, String> {
    let path = dir.join("firecracker.json");
    let body = serde_json::to_vec_pretty(config).map_err(|e| format!("encode vm config: {e}"))?;
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VmConfig {
        VmConfig {
            kernel: "/srv/guest/vmlinux".into(),
            rootfs: "/srv/guest/rootfs.ext4".into(),
            agent_volume: Some("/srv/agents/a1/cache.ext4".into()),
            assets: "/run/ducktape/run7/assets.ext4".into(),
            workspace: "/run/ducktape/run7/ws.ext4".into(),
            vcpus: 4,
            mem_mib: 8192,
            vsock_uds: "/run/ducktape/run7/v.sock".into(),
            tap: Some("dtap7".into()),
        }
    }

    fn manifest() -> RunManifest {
        RunManifest {
            argv: vec!["/usr/bin/claude".into(), "-p".into()],
            env: vec![("HOME".into(), "/root".into())],
            cwd: WORKSPACE_MOUNTPOINT.into(),
            mounts: manifest_mounts(&cfg()),
            tunnel_ports: vec![8931],
        }
    }

    /// `acpi=off` measures as a 69 ms win and silently drops every vCPU but the
    /// boot one, because Firecracker enumerates them through ACPI. A node would
    /// sell four cores and deliver one. This is a prohibition, so it is a test.
    #[test]
    fn the_boot_args_never_disable_acpi() {
        let args = boot_args("token");
        assert!(
            !args.contains("acpi=off"),
            "acpi=off drops all but the boot vCPU: {args}"
        );
    }

    /// A guest panic must reboot, not park at the prompt holding the run's
    /// whole memory footprint until the idle timeout.
    #[test]
    fn a_panicking_guest_reboots_rather_than_parking() {
        let args = boot_args("token");
        assert!(args.contains("panic=1"), "{args}");
        // `reboot=k` is what makes the guest's RESTART reach the VMM at all —
        // Firecracker has no ACPI power button.
        assert!(args.contains("reboot=k"), "{args}");
    }

    /// The rootfs is shared by every concurrent run on the node, and the asset
    /// image holds inputs the run may read but must not edit. Everything the
    /// run legitimately writes is writable.
    #[test]
    fn only_the_devices_a_run_may_write_are_writable() {
        let drives = guest_drives(&cfg());
        let by_id = |id: &str| {
            drives
                .iter()
                .find(|drive| drive.drive_id == id)
                .unwrap_or_else(|| panic!("no {id} drive"))
        };
        assert!(by_id("rootfs").is_root && by_id("rootfs").read_only);
        assert!(by_id("assets").read_only, "the context doc is an input");
        assert!(!by_id("workspace").read_only, "the run's output goes here");
        assert!(!by_id("agent").read_only, "the build cache is written");
    }

    /// The device names shift with how many drives a run gets. The manifest's
    /// mountpoints must shift with them, or the guest mounts the workspace at
    /// another device's mountpoint and the run's output goes nowhere.
    #[test]
    fn the_device_names_track_how_many_drives_the_run_got() {
        assert_eq!(
            manifest_mounts(&cfg()),
            vec![
                ("/dev/vdb".to_string(), AGENT_VOLUME_MOUNTPOINT.to_string()),
                ("/dev/vdc".to_string(), ASSETS_MOUNTPOINT.to_string()),
                ("/dev/vdd".to_string(), WORKSPACE_MOUNTPOINT.to_string()),
            ]
        );

        let without_cache = VmConfig {
            agent_volume: None,
            ..cfg()
        };
        assert_eq!(
            manifest_mounts(&without_cache),
            vec![
                ("/dev/vdb".to_string(), ASSETS_MOUNTPOINT.to_string()),
                ("/dev/vdc".to_string(), WORKSPACE_MOUNTPOINT.to_string()),
            ]
        );
    }

    /// The workspace image is mounted ON TOP OF a directory inside the asset
    /// image. Mount them the other way round and the assets mount shadows the
    /// workspace, so the run starts in an empty directory and its output is
    /// written somewhere nobody reads back.
    #[test]
    fn the_assets_are_mounted_before_the_workspace_that_sits_inside_them() {
        let mounts = manifest_mounts(&cfg());
        let assets = mounts
            .iter()
            .position(|(_, at)| at == ASSETS_MOUNTPOINT)
            .expect("assets");
        let workspace = mounts
            .iter()
            .position(|(_, at)| at == WORKSPACE_MOUNTPOINT)
            .expect("workspace");
        assert!(assets < workspace, "{mounts:?}");
        assert!(
            WORKSPACE_MOUNTPOINT.starts_with(ASSETS_MOUNTPOINT),
            "the ordering only matters because of this nesting"
        );
    }

    /// The root device is mounted by the kernel; a manifest that also listed it
    /// would have the init remount the shared rootfs read-write.
    #[test]
    fn the_root_device_is_never_handed_to_the_init_to_mount() {
        for mount in manifest_mounts(&cfg()) {
            assert_ne!(mount.0, "/dev/vda", "the init must not mount the rootfs");
        }
    }

    /// "Offline" has to mean no interface, not an interface that cannot route.
    #[test]
    fn a_run_without_a_tap_gets_no_network_device_at_all() {
        let offline = VmConfig {
            tap: None,
            ..cfg()
        };
        let config = boot_config(&offline, &manifest());
        assert!(config.get("network-interfaces").is_none(), "{config}");

        let online = boot_config(&cfg(), &manifest());
        assert_eq!(online["network-interfaces"][0]["host_dev_name"], "dtap7");
    }

    /// The whole point of the seam: the size the buyer paid for is the size the
    /// hypervisor enforces, with no cgroup in between.
    #[test]
    fn the_machine_config_carries_the_runs_exact_size() {
        let config = boot_config(&cfg(), &manifest());
        assert_eq!(config["machine-config"]["vcpu_count"], 4);
        assert_eq!(config["machine-config"]["mem_size_mib"], 8192);
        assert_eq!(config["machine-config"]["smt"], false);
    }

    /// The manifest reaches the guest on the command line and nowhere else, so
    /// it has to survive being one whitespace-delimited token.
    #[test]
    fn the_manifest_rides_the_cmdline_as_one_token_and_parses_back() {
        let config = boot_config(&cfg(), &manifest());
        let args = config["boot-source"]["boot_args"]
            .as_str()
            .expect("boot args");
        assert_eq!(
            guest_manifest::from_cmdline(args).expect("round trip"),
            manifest()
        );
    }
}
