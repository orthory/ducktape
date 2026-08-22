//! what this VM is supposed to run, handed in on its own block device.
//!
//! NOT the kernel command line, which is the obvious channel and the wrong one:
//! Firecracker caps a cmdline near 2 KiB, and a run's argv and env are written
//! from a capability SPEC — codex's broker overrides alone measured 2094 bytes
//! and the VMM refused to boot at all, with `Invalid cmdline capacity
//! provided`. A cap a spec author can cross by adding one environment variable
//! is a defect, not a budget.
//!
//! So the manifest rides a tiny device of its own, read RAW: no filesystem to
//! mount, which matters because the manifest is what says which filesystems to
//! mount. Its position is fixed ([`MANIFEST_DEVICE`]) so the guest can find it
//! knowing nothing at all.

use serde::{Deserialize, Serialize};

/// where the manifest device always appears. Attached immediately after the
/// root device, so its name does not move when a run's other drives change.
pub const MANIFEST_DEVICE: &str = "/dev/vdb";

/// the manifest device's size on the host. 64 KiB is far past any real argv and
/// env, and a virtio-blk backing file must be a whole number of 512-byte
/// sectors.
pub const MANIFEST_DEVICE_BYTES: u64 = 64 * 1024;

/// the blob's fixed header: the JSON payload's length, little-endian.
const LENGTH_PREFIX: usize = 4;

/// one block device for the guest init to mount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestMount {
    pub device: String,
    pub at: String,
    /// mount `MS_RDONLY`.
    ///
    /// Not cosmetic and not inferable in the guest: Firecracker refuses writes
    /// to a drive configured `is_read_only`, so mounting one read-WRITE fails
    /// outright with EACCES — which reaches the operator as "the guest never
    /// dialled back", naming nothing. The host knows which drives it attached
    /// read-only, so it says so.
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: String,
    /// each block device the host attached, in mount order. Carried explicitly
    /// rather than inferred from a fixed `/dev/vdb`, `/dev/vdc` ordering: the
    /// host decides how many drives a run gets (a run with no persistent agent
    /// volume gets one fewer), and an implicit ordering contract split across
    /// two files silently mounts the workspace at the cache's mountpoint the
    /// first time that count changes.
    pub mounts: Vec<GuestMount>,
    /// loopback ports the guest serves and forwards to the host over vsock, in
    /// the same order the host bound its listeners: the run's credential broker
    /// and, when the run has one, the node's run-action RPC.
    ///
    /// The CLI dials these as ordinary HTTP on `127.0.0.1`; it never learns the
    /// far end is outside the VM, and the credential never enters the VM at
    /// all. ORDER IS THE MAPPING — entry N rides vsock port
    /// `TUNNEL_PORT_BASE + N` — so reordering this list silently connects a run
    /// to the wrong service.
    #[serde(default)]
    pub tunnel_ports: Vec<u16>,
}

/// the device's contents: a little-endian length, then that many bytes of JSON.
///
/// Refused rather than truncated when it does not fit: a manifest cut short
/// reaches the guest as unparseable JSON and the operator as "the guest never
/// dialled back", while this says what is too big and by how much, on the host,
/// where the spec that caused it can be fixed.
pub fn encode(manifest: &RunManifest) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(manifest).expect("a manifest always serializes");
    let capacity = MANIFEST_DEVICE_BYTES as usize - LENGTH_PREFIX;
    if json.len() > capacity {
        return Err(format!(
            "run manifest is {} bytes, over the {capacity}-byte manifest device; \
             the run's argv and env are too large",
            json.len()
        ));
    }
    let mut blob = Vec::with_capacity(MANIFEST_DEVICE_BYTES as usize);
    blob.extend_from_slice(&(json.len() as u32).to_le_bytes());
    blob.extend_from_slice(&json);
    blob.resize(MANIFEST_DEVICE_BYTES as usize, 0);
    Ok(blob)
}

/// read a manifest back out of the device's bytes.
///
/// `blob` is whatever the guest read off the device — a whole sector-aligned
/// device, not a trimmed payload — so the length prefix is what separates the
/// manifest from the zero padding behind it.
pub fn decode(blob: &[u8]) -> Result<RunManifest, String> {
    if blob.len() < LENGTH_PREFIX {
        return Err("run manifest device is empty".to_string());
    }
    let (header, body) = blob.split_at(LENGTH_PREFIX);
    let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    if len == 0 {
        return Err("run manifest device carries no payload".to_string());
    }
    if len > body.len() {
        return Err(format!(
            "run manifest claims {len} bytes but the device holds {}",
            body.len()
        ));
    }
    let manifest: RunManifest = serde_json::from_slice(&body[..len])
        .map_err(|e| format!("run manifest is not valid JSON: {e}"))?;
    // An empty argv would boot a guest that runs nothing and then sits there
    // until the host's idle timeout — a silent hang costs a run's whole
    // wall-clock budget to diagnose, so it is refused here where the reason is
    // still nameable.
    if manifest.argv.is_empty() {
        return Err("run manifest carries an empty argv; there is nothing to exec".to_string());
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RunManifest {
        RunManifest {
            argv: vec!["/usr/bin/claude".into(), "-p".into(), "say hi".into()],
            env: vec![
                ("HOME".into(), "/root".into()),
                ("PATH".into(), "/usr/bin:/bin".into()),
            ],
            cwd: "/workspace".into(),
            mounts: vec![
                GuestMount {
                    device: "/dev/vdb".into(),
                    at: "/agent".into(),
                    read_only: false,
                },
                GuestMount {
                    device: "/dev/vdc".into(),
                    at: "/duck".into(),
                    read_only: true,
                },
            ],
            tunnel_ports: vec![8931, 8932],
        }
    }

    #[test]
    fn a_manifest_round_trips_through_the_device_blob() {
        let manifest = sample();
        let blob = encode(&manifest).expect("encodes");
        assert_eq!(
            blob.len() as u64,
            MANIFEST_DEVICE_BYTES,
            "the blob IS the device: virtio-blk needs whole sectors"
        );
        assert_eq!(decode(&blob).expect("decodes"), manifest);
    }

    /// The guest reads the whole device, so the payload is followed by however
    /// much zero padding the device size leaves. Only the length prefix
    /// separates the two.
    #[test]
    fn the_padding_behind_the_payload_is_not_part_of_it() {
        let blob = encode(&sample()).expect("encodes");
        let json_len = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
        assert!(
            blob[LENGTH_PREFIX + json_len..].iter().all(|b| *b == 0),
            "the tail must be zeros"
        );
        assert_eq!(decode(&blob).expect("decodes"), sample());
    }

    /// The host writes this device, but a guest that read a torn or blank one
    /// must fail with a nameable error rather than run nothing and hang until
    /// the idle timeout.
    #[test]
    fn a_malformed_manifest_is_a_named_error() {
        assert!(decode(&[]).is_err(), "no device at all");
        assert!(decode(&[0, 0, 0, 0]).is_err(), "a blank device");
        assert!(decode(&[255, 255, 0, 0, b'{']).is_err(), "a torn payload");

        let mut garbage = vec![0u8; 64];
        let body = b"{\"nope\":1}";
        garbage[..LENGTH_PREFIX].copy_from_slice(&(body.len() as u32).to_le_bytes());
        garbage[LENGTH_PREFIX..LENGTH_PREFIX + body.len()].copy_from_slice(body);
        assert!(decode(&garbage).is_err(), "well-framed but not a manifest");
    }

    #[test]
    fn an_empty_argv_is_refused() {
        let blob = encode(&RunManifest {
            argv: Vec::new(),
            env: Vec::new(),
            cwd: "/workspace".into(),
            mounts: Vec::new(),
            tunnel_ports: Vec::new(),
        })
        .expect("encodes");
        let err = decode(&blob).expect_err("refused");
        assert!(err.contains("argv"), "{err}");
    }

    /// The size that a kernel command line did NOT have. A manifest past the
    /// device is refused on the host, naming the size — not truncated into a
    /// guest that cannot say what went wrong.
    #[test]
    fn a_manifest_too_large_for_its_device_is_refused_on_the_host() {
        let huge = RunManifest {
            argv: vec!["/opt/duck/bin/codex".into()],
            env: vec![("BIG".into(), "x".repeat(MANIFEST_DEVICE_BYTES as usize))],
            ..sample()
        };
        let err = encode(&huge).expect_err("refused");
        assert!(err.contains("too large"), "{err}");
    }
}
