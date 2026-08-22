//! what this VM is supposed to run, handed in on the kernel command line.
//!
//! The command line is the only channel available before any device is up, so
//! the manifest rides it as one base64 token. URL-safe and unpadded on purpose:
//! `+`, `/` and `=` all mean something to a bootloader or a shell somewhere
//! along the way, and a cmdline token cannot contain spaces while argv and env
//! freely do.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use serde::{Deserialize, Serialize};

/// the cmdline key the host sets and [`from_cmdline`] reads back.
pub const CMDLINE_KEY: &str = "duck.manifest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: String,
    /// `(device, mountpoint)` for each block device the host attached, in mount
    /// order. Carried explicitly rather than inferred from a fixed `/dev/vdb`,
    /// `/dev/vdc` ordering: the host decides how many drives a run gets (a run
    /// with no persistent agent volume gets one fewer), and an implicit
    /// ordering contract split across two files silently mounts the workspace
    /// at the cache's mountpoint the first time that count changes.
    pub mounts: Vec<(String, String)>,
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

pub fn encode(manifest: &RunManifest) -> String {
    let json = serde_json::to_vec(manifest).expect("a manifest always serializes");
    B64.encode(json)
}

pub fn parse(encoded: &str) -> Result<RunManifest, String> {
    if encoded.is_empty() {
        return Err("run manifest is empty".to_string());
    }
    let json = B64
        .decode(encoded)
        .map_err(|e| format!("run manifest is not valid base64: {e}"))?;
    let manifest: RunManifest = serde_json::from_slice(&json)
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

/// pull the manifest out of a whole `/proc/cmdline` string.
pub fn from_cmdline(cmdline: &str) -> Result<RunManifest, String> {
    let token = cmdline
        .split_ascii_whitespace()
        .find_map(|token| token.strip_prefix(&format!("{CMDLINE_KEY}=")))
        .ok_or_else(|| format!("no {CMDLINE_KEY}= on the kernel command line"))?;
    parse(token)
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
                ("/dev/vdb".into(), "/agent".into()),
                ("/dev/vdc".into(), "/workspace".into()),
            ],
            tunnel_ports: vec![8931, 8932],
        }
    }

    #[test]
    fn a_manifest_round_trips_through_the_cmdline_encoding() {
        let manifest = sample();
        let encoded = encode(&manifest);
        assert!(
            !encoded.contains(' '),
            "a cmdline token must not contain spaces: {encoded}"
        );
        assert_eq!(parse(&encoded).expect("parses"), manifest);
    }

    /// The encoding must survive the bootloader and the kernel's own cmdline
    /// splitting, so the alphabet may not contain `+`, `/` or `=`.
    #[test]
    fn the_encoding_avoids_characters_the_boot_path_reinterprets() {
        let encoded = encode(&sample());
        for bad in ['+', '/', '=', '"', '\''] {
            assert!(!encoded.contains(bad), "{bad:?} in {encoded}");
        }
    }

    /// The manifest arrives on the kernel command line, which the HOST writes —
    /// but a malformed one must fail with a nameable error rather than boot a
    /// guest that runs nothing and hangs until the idle timeout.
    #[test]
    fn a_malformed_manifest_is_a_named_error() {
        assert!(parse("not-base64!!").is_err());
        assert!(parse("").is_err());
        assert!(parse(&B64.encode(b"{\"nope\":1}")).is_err());
    }

    #[test]
    fn an_empty_argv_is_refused() {
        let err = parse(&encode(&RunManifest {
            argv: Vec::new(),
            env: Vec::new(),
            cwd: "/workspace".into(),
            mounts: Vec::new(),
            tunnel_ports: Vec::new(),
        }))
        .expect_err("refused");
        assert!(err.contains("argv"), "{err}");
    }

    #[test]
    fn the_manifest_is_found_among_the_other_cmdline_tokens() {
        let manifest = sample();
        let cmdline = format!(
            "console=ttyS0 reboot=k panic=-1 {CMDLINE_KEY}={} pci=off",
            encode(&manifest)
        );
        assert_eq!(from_cmdline(&cmdline).expect("found"), manifest);
    }

    #[test]
    fn a_cmdline_without_a_manifest_names_what_is_missing() {
        let err = from_cmdline("console=ttyS0 pci=off").expect_err("refused");
        assert!(err.contains(CMDLINE_KEY), "{err}");
    }
}
