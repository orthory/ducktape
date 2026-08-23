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

use crate::guest_manifest::GuestMount;

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
    /// this run's manifest device — argv, env, cwd, mounts and tunnel ports,
    /// written by [`crate::guest_manifest::encode`]. A device rather than the
    /// kernel command line: see that module.
    pub manifest: PathBuf,
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
const DEVICE_ORDER: [&str; 5] = ["/dev/vda", "/dev/vdb", "/dev/vdc", "/dev/vdd", "/dev/vde"];

/// the drives for `cfg`, in attach order.
///
/// TWO ORDERINGS ARE LOAD-BEARING here.
///
/// The manifest device is attached SECOND, straight after the root device, so
/// it is always [`crate::guest_manifest::MANIFEST_DEVICE`]. The guest reads it before
/// it knows anything else, so its name may not move when a run gains or loses
/// an agent volume.
///
/// The assets image must be mounted before the workspace, because the workspace
/// lands on the `workspace/` directory INSIDE it. Reverse them and the
/// workspace mount is immediately shadowed by the assets mount, and the run
/// sees an empty workspace.
pub fn guest_drives(cfg: &VmConfig) -> Vec<GuestDrive> {
    let mut drives = vec![
        GuestDrive {
            drive_id: "rootfs",
            host_path: cfg.rootfs.clone(),
            device: DEVICE_ORDER[0],
            // the kernel mounts the root device itself; the init must not remount it
            mountpoint: None,
            // SHARED across every concurrent run on this node. Writable would let
            // one buyer's run corrupt another's guest, so this is not a tuning knob.
            read_only: true,
            is_root: true,
        },
        GuestDrive {
            drive_id: "manifest",
            host_path: cfg.manifest.clone(),
            device: DEVICE_ORDER[1],
            // read RAW, never mounted: the manifest is what says what to mount
            mountpoint: None,
            read_only: true,
            is_root: false,
        },
    ];
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

/// the mounts for a run's manifest, derived from the same [`guest_drives`] list
/// the VM is configured from — including each drive's read-only bit, which the
/// guest cannot infer and which Firecracker enforces at the device.
pub fn manifest_mounts(cfg: &VmConfig) -> Vec<GuestMount> {
    guest_drives(cfg)
        .into_iter()
        .filter_map(|drive| {
            Some(GuestMount {
                device: drive.device.to_string(),
                at: drive.mountpoint?.to_string(),
                read_only: drive.read_only,
            })
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
///
/// FIXED, and short: nothing per-run rides here. The run's manifest has its own
/// device precisely because a cmdline is capped near 2 KiB.
pub fn boot_args() -> String {
    "console=ttyS0 reboot=k panic=1 pci=off quiet loglevel=1 \
     i8042.noaux i8042.nokbd i8042.nomux i8042.nopnp i8042.dumbkbd \
     init=/duck-guest-init"
        .to_string()
}

/// the complete VM configuration Firecracker boots from. Pure.
pub fn boot_config(cfg: &VmConfig) -> serde_json::Value {
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
            "boot_args": boot_args(),
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
            manifest: "/run/ducktape/run7/manifest.bin".into(),
            agent_volume: Some("/srv/agents/a1/cache.ext4".into()),
            assets: "/run/ducktape/run7/assets.ext4".into(),
            workspace: "/run/ducktape/run7/ws.ext4".into(),
            vcpus: 4,
            mem_mib: 8192,
            vsock_uds: "/run/ducktape/run7/v.sock".into(),
            tap: Some("dtap7".into()),
        }
    }

    /// `acpi=off` measures as a 69 ms win and silently drops every vCPU but the
    /// boot one, because Firecracker enumerates them through ACPI. A node would
    /// sell four cores and deliver one. This is a prohibition, so it is a test.
    #[test]
    fn the_boot_args_never_disable_acpi() {
        let args = boot_args();
        assert!(
            !args.contains("acpi=off"),
            "acpi=off drops all but the boot vCPU: {args}"
        );
    }

    /// A guest panic must reboot, not park at the prompt holding the run's
    /// whole memory footprint until the idle timeout.
    #[test]
    fn a_panicking_guest_reboots_rather_than_parking() {
        let args = boot_args();
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
        assert!(by_id("manifest").read_only, "the run may not rewrite it");
        assert!(by_id("assets").read_only, "the context doc is an input");
        assert!(!by_id("workspace").read_only, "the run's output goes here");
        assert!(!by_id("agent").read_only, "the build cache is written");
    }

    /// The guest reads its manifest knowing NOTHING — before any mount, before
    /// any device name it could look up. So the manifest device's position is a
    /// constant on both sides, and the two must agree: a run whose agent volume
    /// pushed the manifest to another letter would read a filesystem image as
    /// a manifest and die naming neither.
    #[test]
    fn the_manifest_device_is_where_the_guest_looks_for_it() {
        for volume in [Some("/srv/agents/a1/cache.ext4".into()), None] {
            let with = VmConfig {
                agent_volume: volume,
                ..cfg()
            };
            let manifest = guest_drives(&with)
                .into_iter()
                .find(|drive| drive.drive_id == "manifest")
                .expect("a manifest drive");
            assert_eq!(manifest.device, crate::guest_manifest::MANIFEST_DEVICE);
            assert_eq!(
                manifest.mountpoint, None,
                "the manifest is read raw; mounting it would need a manifest"
            );
        }
    }

    /// The device names shift with how many drives a run gets. The manifest's
    /// mountpoints must shift with them, or the guest mounts the workspace at
    /// another device's mountpoint and the run's output goes nowhere.
    #[test]
    fn the_device_names_track_how_many_drives_the_run_got() {
        let at = |mounts: &[GuestMount]| -> Vec<(String, String)> {
            mounts
                .iter()
                .map(|m| (m.device.clone(), m.at.clone()))
                .collect()
        };
        assert_eq!(
            at(&manifest_mounts(&cfg())),
            vec![
                ("/dev/vdc".to_string(), AGENT_VOLUME_MOUNTPOINT.to_string()),
                ("/dev/vdd".to_string(), ASSETS_MOUNTPOINT.to_string()),
                ("/dev/vde".to_string(), WORKSPACE_MOUNTPOINT.to_string()),
            ]
        );

        let without_cache = VmConfig {
            agent_volume: None,
            ..cfg()
        };
        assert_eq!(
            at(&manifest_mounts(&without_cache)),
            vec![
                ("/dev/vdc".to_string(), ASSETS_MOUNTPOINT.to_string()),
                ("/dev/vdd".to_string(), WORKSPACE_MOUNTPOINT.to_string()),
            ]
        );
    }

    /// Firecracker enforces the read-only bit AT THE DEVICE, so a guest that
    /// mounts a read-only drive read-write fails with EACCES and dies before it
    /// dials back. The manifest is the only channel that can tell it.
    #[test]
    fn the_manifest_carries_each_drives_read_only_bit() {
        let mounts = manifest_mounts(&cfg());
        let by_at = |at: &str| {
            mounts
                .iter()
                .find(|m| m.at == at)
                .unwrap_or_else(|| panic!("{at} is mounted"))
                .clone()
        };
        assert!(by_at(ASSETS_MOUNTPOINT).read_only, "the assets are inputs");
        assert!(!by_at(WORKSPACE_MOUNTPOINT).read_only, "output goes here");
        assert!(
            !by_at(AGENT_VOLUME_MOUNTPOINT).read_only,
            "cache is written"
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
            .position(|m| m.at == ASSETS_MOUNTPOINT)
            .expect("assets");
        let workspace = mounts
            .iter()
            .position(|m| m.at == WORKSPACE_MOUNTPOINT)
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
            assert_ne!(
                mount.device, "/dev/vda",
                "the init must not mount the rootfs"
            );
        }
    }

    /// "Offline" has to mean no interface, not an interface that cannot route.
    #[test]
    fn a_run_without_a_tap_gets_no_network_device_at_all() {
        let offline = VmConfig { tap: None, ..cfg() };
        let config = boot_config(&offline);
        assert!(config.get("network-interfaces").is_none(), "{config}");

        let online = boot_config(&cfg());
        assert_eq!(online["network-interfaces"][0]["host_dev_name"], "dtap7");
    }

    /// The whole point of the seam: the size the buyer paid for is the size the
    /// hypervisor enforces, with no cgroup in between.
    #[test]
    fn the_machine_config_carries_the_runs_exact_size() {
        let config = boot_config(&cfg());
        assert_eq!(config["machine-config"]["vcpu_count"], 4);
        assert_eq!(config["machine-config"]["mem_size_mib"], 8192);
        assert_eq!(config["machine-config"]["smt"], false);
    }

    /// Nothing per-run may ride the kernel command line. Firecracker caps it
    /// near 2 KiB and a run's argv and env come from a capability SPEC —
    /// measured, codex's broker overrides made a 2094-byte cmdline and the VMM
    /// refused to boot with `Invalid cmdline capacity provided`. The manifest
    /// device exists so this string can stay fixed and short.
    #[test]
    fn the_boot_args_carry_nothing_per_run() {
        let config = boot_config(&cfg());
        let args = config["boot-source"]["boot_args"]
            .as_str()
            .expect("boot args");
        assert_eq!(args, boot_args(), "the cmdline is the same for every run");
        assert!(args.len() < 512, "{} bytes: {args}", args.len());
        for host_path in ["/srv/agents", "/run/ducktape", "/srv/guest"] {
            assert!(!args.contains(host_path), "{host_path} leaked into {args}");
        }
    }
}
