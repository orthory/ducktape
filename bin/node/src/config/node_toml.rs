//! node.toml — the raw file shapes and the workspace plumbing merge that
//! keeps init/join idempotent.
//!
//! Two shapes, two structs, no unions:
//! - [`NodeToml`] is the OPERATOR file (network shape). Every key is
//!   REQUIRED: a file missing one refuses to parse loudly instead of
//!   silently meaning something, and `deny_unknown_fields` refuses retired
//!   keys the same way. init/join always write the complete set, so the
//!   file is its own documentation — no bare node.toml.
//! - [`DevSeedToml`] is the dev-seed harness shape (cluster e2e, wg-smoke):
//!   seed-derived identities, no descriptor, minimal keys. Not an operator
//!   surface.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize as _;

use super::DEFAULT_PRIMARY_COORDINATOR;
use super::resolve::{DEFAULT_CHECKPOINT_BLOCKS, DEFAULT_PODMAN_IMAGE};

/// the generated defaults: a fresh init/join with no flags yields a node
/// with every surface up. Loopback for the operator surfaces (HTTP app
/// API, browser gateway, admin RPC), dual-stack for the mesh, and the
/// conventional WireGuard port for the tunnel plane.
pub const DEFAULT_MESH_LISTEN: &str = "[::]:52200";
pub const DEFAULT_HTTP_LISTEN: &str = "127.0.0.1:8844";
pub const DEFAULT_RPC_LISTEN: &str = "127.0.0.1:8845";
/// port 0 on purpose: the browser gateway prints its bound port and its
/// consumers re-read it per session; a fixed port would only collide.
pub const DEFAULT_GATEWAY_LISTEN: &str = "127.0.0.1:0";
pub const DEFAULT_WIREGUARD_LISTEN: &str = "0.0.0.0:51820";

/// the operator node.toml — the network shape, every key required.
///
/// Where "unset" is a meaningful state it is an EXPLICIT value, never a
/// missing key: `"none"` (primary_coordinator, coordinator_relay),
/// `"auto"` (wireguard_advertised), `0` = probe (sandbox_cores,
/// sandbox_mem_gb).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeToml {
    /// path to the network descriptor, resolved beside this file.
    pub network: String,
    /// path to the identity secret, resolved beside this file.
    pub key_file: String,
    /// the p2p mesh listener.
    pub listen: String,
    /// what peers are told to dial: `"overlay"` (the chain-derived ULA —
    /// the right value for a member behind NAT) or a concrete dialable
    /// address.
    pub advertised: String,
    pub storage_dir: String,
    /// the HTTP app API.
    pub http_listen: String,
    /// the least-privilege browser gateway; must bind 127.0.0.1.
    pub gateway_listen: String,
    /// the local admin RPC bridge.
    pub rpc_listen: String,
    /// the UDP endpoint of this node's WireGuard tunnel plane.
    pub wireguard_listen: String,
    /// the UDP invite intro listener (where a fresh joiner announces its
    /// keys, token-authenticated, before any p2p).
    pub invite_listen: String,
    /// the advertised tunnel endpoint, independent of the bind:
    /// `"host:port"`, or `"auto"` = derive from `wireguard_listen` (its IP
    /// when concrete, endpoint-less/roaming when unspecified).
    pub wireguard_advertised: String,
    /// the ambient rendezvous coordinator: `"host:port"`, or `"none"` to
    /// run without coordination.
    pub primary_coordinator: String,
    /// the TCP first-contact fallback relay: `"host:port"`, or `"none"`.
    pub coordinator_relay: String,
    /// sealed blocks between recovery checkpoints.
    pub checkpoint_blocks: u64,
    /// shipped-index warm start on join (default ON — the read model arrives
    /// warm from the sync source, unverified by design). `false` opts this
    /// node down to consensus-only: verified state, derived views empty at
    /// the boundary.
    pub sync_index: bool,
    /// whether this node publishes its provider set into the capability
    /// registry; `false` = accept-lane-only provider.
    pub announce_capabilities: bool,
    /// provider run isolation: `"direct"`, `"podman"`, or `"tart"`.
    pub sandbox: String,
    /// the provider environment image (used by podman/tart; ignored for
    /// direct).
    pub sandbox_image: String,
    /// announced sandbox capacity; `0` = probe the host.
    pub sandbox_cores: u64,
    /// announced sandbox capacity in GiB; `0` = probe the host.
    pub sandbox_mem_gb: u64,
}

impl NodeToml {
    /// `wireguard_advertised` with the sentinel mapped back to the runtime
    /// derivation: `"auto"` means "derive from `wireguard_listen`".
    pub fn wireguard_advertised_value(&self) -> Option<&str> {
        let is_auto = self.wireguard_advertised == "auto";
        (!is_auto).then_some(self.wireguard_advertised.as_str())
    }
}

/// the dev-seed harness shape: deterministic seed identities, no
/// descriptor. node 0 bootstraps nobody; everyone else dials peer_seeds[0]
/// at `bootstrapper_addr`. Only the test harnesses write this shape, so
/// its plumbing stays optional — a harness file says exactly what the test
/// needs and nothing else.
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevSeedToml {
    pub id: u64,
    pub namespace: String,
    pub peer_seeds: Vec<u64>,
    pub validator_seeds: Option<Vec<u64>>,
    /// `None` for node 0 (bootstraps nobody) — a semantic state, not a
    /// default.
    pub bootstrapper_addr: Option<String>,
    pub listen: String,
    pub advertised: Option<String>,
    pub storage_dir: Option<String>,
    pub rpc_listen: Option<String>,
    pub http_listen: Option<String>,
    pub gateway_listen: Option<String>,
    pub checkpoint_blocks: Option<u64>,
    /// PRESENT = the reachability plane runs (userspace socket backend).
    pub wireguard_listen: Option<String>,
    pub invite_listen: Option<String>,
    pub wireguard_advertised: Option<String>,
    pub primary_coordinator: Option<String>,
    pub coordinator_relay: Option<String>,
    pub sync_index: Option<bool>,
    pub announce_capabilities: Option<bool>,
    pub sandbox: Option<String>,
    pub sandbox_image: Option<String>,
    pub sandbox_cores: Option<u64>,
    pub sandbox_mem_gb: Option<u64>,
}

/// both file shapes, discriminated by the `network` key: PRESENT means the
/// operator (network) shape.
pub enum RawNodeToml {
    Network(NodeToml),
    DevSeed(DevSeedToml),
}

/// read a raw node.toml plus its base directory (which relative paths
/// inside the file resolve against). the `network` key picks the shape;
/// each shape then parses STRICTLY (all required keys present, no unknown
/// keys).
pub fn load_raw_node_toml(cfg_path: &Path) -> Result<(RawNodeToml, PathBuf), String> {
    let text = std::fs::read_to_string(cfg_path).map_err(|e| format!("read {cfg_path:?}: {e}"))?;
    let value: toml::Value = toml::from_str(&text).map_err(|e| format!("{cfg_path:?}: {e}"))?;
    let base = cfg_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let is_network_shape = value.get("network").is_some();
    let raw = if is_network_shape {
        RawNodeToml::Network(
            NodeToml::deserialize(value).map_err(|e| format!("{cfg_path:?}: {e}"))?,
        )
    } else {
        RawNodeToml::DevSeed(
            DevSeedToml::deserialize(value).map_err(|e| format!("{cfg_path:?}: {e}"))?,
        )
    };
    Ok((raw, base))
}

/// read a network-shape node.toml, refusing the dev-seed shape — the
/// workspace verbs (init/join/invite) only ever operate on operator files.
pub fn load_node_toml(cfg_path: &Path) -> Result<(NodeToml, PathBuf), String> {
    match load_raw_node_toml(cfg_path)? {
        (RawNodeToml::Network(raw), base) => Ok((raw, base)),
        (RawNodeToml::DevSeed(_), _) => Err(format!(
            "{cfg_path:?} is a dev-seed (harness) config — the workspace verbs need the \
             network shape"
        )),
    }
}

/// a workspace's COMPLETE plumbing (everything in node.toml that is not
/// the network reference) — every field concrete, mirroring the required
/// file 1:1. Built by [`merged_plumbing`] from three layers: explicit
/// flags win, else the values an EXISTING network-shape node.toml already
/// carries, else the WORKING defaults (`DEFAULT_*`: mesh, HTTP, gateway,
/// RPC, and the WireGuard plane all up — a flagless init/join yields a
/// node that works out of the box). always writing the merged result makes
/// init/join idempotent AND partial-flag-safe (one flag never resets the
/// others).
pub struct Plumbing {
    pub listen: String,
    pub advertised: String,
    pub storage_dir: String,
    pub http_listen: String,
    pub gateway_listen: String,
    pub rpc_listen: String,
    pub wireguard_listen: String,
    pub invite_listen: String,
    pub wireguard_advertised: String,
    pub primary_coordinator: String,
    pub coordinator_relay: String,
    pub checkpoint_blocks: u64,
    pub sync_index: bool,
    pub announce_capabilities: bool,
    pub sandbox: String,
    pub sandbox_image: String,
    pub sandbox_cores: u64,
    pub sandbox_mem_gb: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn merged_plumbing(
    dir: &Path,
    listen: Option<&str>,
    advertised: Option<&str>,
    http_listen: Option<&str>,
    gateway_listen: Option<&str>,
    rpc_listen: Option<&str>,
    wireguard_listen: Option<&str>,
    invite_listen: Option<&str>,
    primary_coordinator: Option<&str>,
    wireguard_advertised: Option<&str>,
) -> Result<Plumbing, String> {
    let path = dir.join("node.toml");
    // an existing file must be a VALID network-shape file to contribute —
    // an incomplete or dev-seed file aborts the verb instead of being
    // silently half-inherited.
    let existing: Option<NodeToml> = if path.exists() {
        Some(load_node_toml(&path)?.0)
    } else {
        None
    };
    let e = existing.as_ref();
    let listen = listen
        .map(str::to_string)
        .or_else(|| e.map(|r| r.listen.clone()))
        .unwrap_or_else(|| DEFAULT_MESH_LISTEN.into());
    // "overlay" needs an IPv6 mesh listener (members reverse-dial the ULA
    // over tunnels); a v4-only listen advertises its own socket address.
    let derived_advertised = if listen.starts_with('[') {
        "overlay".to_string()
    } else {
        listen.clone()
    };
    let wireguard_listen = wireguard_listen
        .map(str::to_string)
        .or_else(|| e.map(|r| r.wireguard_listen.clone()))
        .unwrap_or_else(|| DEFAULT_WIREGUARD_LISTEN.into());
    let derived_invite_listen = derive_invite_listen(&wireguard_listen)?;
    let primary_coordinator = primary_coordinator
        .map(str::to_string)
        .or_else(|| e.map(|r| r.primary_coordinator.clone()))
        .unwrap_or_else(|| DEFAULT_PRIMARY_COORDINATOR.into());
    let derived_relay = derive_coordinator_relay(&primary_coordinator);
    Ok(Plumbing {
        advertised: advertised
            .map(str::to_string)
            .or_else(|| e.map(|r| r.advertised.clone()))
            .unwrap_or(derived_advertised),
        listen,
        http_listen: http_listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.http_listen.clone()))
            .unwrap_or_else(|| DEFAULT_HTTP_LISTEN.into()),
        gateway_listen: gateway_listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.gateway_listen.clone()))
            .unwrap_or_else(|| DEFAULT_GATEWAY_LISTEN.into()),
        rpc_listen: rpc_listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.rpc_listen.clone()))
            .unwrap_or_else(|| DEFAULT_RPC_LISTEN.into()),
        storage_dir: e
            .map(|r| r.storage_dir.clone())
            .unwrap_or_else(|| "storage".into()),
        invite_listen: invite_listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.invite_listen.clone()))
            .unwrap_or(derived_invite_listen),
        wireguard_advertised: wireguard_advertised
            .map(str::to_string)
            .or_else(|| e.map(|r| r.wireguard_advertised.clone()))
            .unwrap_or_else(|| "auto".into()),
        coordinator_relay: e
            .map(|r| r.coordinator_relay.clone())
            .unwrap_or(derived_relay),
        checkpoint_blocks: e
            .map(|r| r.checkpoint_blocks)
            .unwrap_or(DEFAULT_CHECKPOINT_BLOCKS),
        sync_index: e.map(|r| r.sync_index).unwrap_or(true),
        announce_capabilities: e.map(|r| r.announce_capabilities).unwrap_or(false),
        sandbox: e
            .map(|r| r.sandbox.clone())
            .unwrap_or_else(|| "direct".into()),
        sandbox_image: e
            .map(|r| r.sandbox_image.clone())
            .unwrap_or_else(|| DEFAULT_PODMAN_IMAGE.into()),
        sandbox_cores: e.map(|r| r.sandbox_cores).unwrap_or(0),
        sandbox_mem_gb: e.map(|r| r.sandbox_mem_gb).unwrap_or(0),
        wireguard_listen,
        primary_coordinator,
    })
}

/// the intro listener default: `wireguard_listen`'s port + 1, computed at
/// GENERATION time — the file always carries the concrete value.
fn derive_invite_listen(wireguard_listen: &str) -> Result<String, String> {
    let addr: std::net::SocketAddr = wireguard_listen
        .parse()
        .map_err(|e| format!("wireguard_listen {wireguard_listen:?}: {e}"))?;
    let intro_port = addr
        .port()
        .checked_add(1)
        .ok_or_else(|| format!("wireguard_listen {wireguard_listen:?}: no room for port + 1"))?;
    Ok(format!("0.0.0.0:{intro_port}"))
}

/// the relay default: the coordinator's host on TCP/443, or `"none"` when
/// coordination itself is off — computed at GENERATION time.
fn derive_coordinator_relay(primary_coordinator: &str) -> String {
    let coordination_off = matches!(primary_coordinator, "none" | "off" | "direct");
    if coordination_off {
        return "none".into();
    }
    let host = primary_coordinator
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(primary_coordinator);
    format!("{host}:443")
}

/// one entry: a `# note` line ABOVE its live `key = value` line, blank-line
/// separated — the file reads as its own reference sheet.
fn keyline(s: &mut String, key: &str, value: std::fmt::Arguments<'_>, note: &str) {
    let _ = writeln!(s, "\n# {note}\n{key} = {value}");
}

/// write the network-shape node.toml (init/join): the COMPLETE key set,
/// every key live under a brief comment — the parser requires every key, so
/// the file IS the reference: what each key does, and what its sentinel
/// values mean. the file references its siblings relatively, so the whole
/// dir is relocatable.
pub fn write_node_toml(dir: &Path, p: &Plumbing) -> Result<PathBuf, String> {
    let mut s = String::from(
        "# ducktape node config (network shape) — see network.toml for the network.\n\
         # every key is required; edit values, don't delete lines (rewrites re-fill).\n",
    );
    keyline(&mut s, "network", format_args!("\"network.toml\""),
        "the network descriptor, beside this file");
    keyline(&mut s, "key_file", format_args!("\"identity.key\""),
        "this node's identity secret, beside this file");
    keyline(&mut s, "listen", format_args!("\"{}\"", p.listen),
        "p2p mesh listener (dual-stack)");
    keyline(&mut s, "advertised", format_args!("\"{}\"", p.advertised),
        "what peers dial: \"overlay\" = the chain ULA, or host:port");
    keyline(&mut s, "storage_dir", format_args!("'{}'", p.storage_dir),
        "chain + module state, beside this file");
    keyline(&mut s, "http_listen", format_args!("\"{}\"", p.http_listen),
        "HTTP app API (keep loopback)");
    keyline(&mut s, "gateway_listen", format_args!("\"{}\"", p.gateway_listen),
        "browser gateway; loopback only, port 0 = pick free");
    keyline(&mut s, "rpc_listen", format_args!("\"{}\"", p.rpc_listen),
        "local admin RPC (keep loopback)");
    keyline(&mut s, "wireguard_listen", format_args!("\"{}\"", p.wireguard_listen),
        "the WireGuard tunnel plane (UDP)");
    keyline(&mut s, "invite_listen", format_args!("\"{}\"", p.invite_listen),
        "invite intro listener (UDP; convention: wireguard port + 1)");
    keyline(&mut s, "wireguard_advertised", format_args!("\"{}\"", p.wireguard_advertised),
        "tunnel endpoint peers dial; \"auto\" = derive from wireguard_listen");
    keyline(&mut s, "primary_coordinator", format_args!("\"{}\"", p.primary_coordinator),
        "ambient rendezvous coordinator; \"none\" disables");
    keyline(&mut s, "coordinator_relay", format_args!("\"{}\"", p.coordinator_relay),
        "TCP first-contact fallback; \"none\" disables");
    keyline(&mut s, "checkpoint_blocks", format_args!("{}", p.checkpoint_blocks),
        "sealed blocks between recovery checkpoints");
    keyline(&mut s, "sync_index", format_args!("{}", p.sync_index),
        "false: consensus-only (skip the unverified index warm start on join)");
    keyline(&mut s, "announce_capabilities", format_args!("{}", p.announce_capabilities),
        "true: publish this node's provider set");
    keyline(&mut s, "sandbox", format_args!("\"{}\"", p.sandbox),
        "provider run isolation: \"direct\" | \"podman\" | \"tart\"");
    keyline(&mut s, "sandbox_image", format_args!("\"{}\"", p.sandbox_image),
        "provider image (podman/tart; unused for direct)");
    keyline(&mut s, "sandbox_cores", format_args!("{}", p.sandbox_cores),
        "announced capacity; 0 = probe the host");
    keyline(&mut s, "sandbox_mem_gb", format_args!("{}", p.sandbox_mem_gb),
        "announced capacity (GiB); 0 = probe the host");
    let path = dir.join("node.toml");
    std::fs::write(&path, s).map_err(|e| format!("write {path:?}: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ducktape-config-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn fresh_default_plumbing(dir: &Path) -> Plumbing {
        merged_plumbing(dir, None, None, None, None, None, None, None, None, None)
            .expect("fresh merge")
    }

    /// the generated file round-trips through the strict parser and its
    /// flagless defaults are a WORKING node: every surface up, every
    /// derivation materialized concretely.
    #[test]
    fn generated_file_is_complete_and_defaults_are_working() {
        let dir = tmp("full-print");
        let p = fresh_default_plumbing(&dir);
        write_node_toml(&dir, &p).expect("write");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("strict parse");
        assert_eq!(raw.listen, DEFAULT_MESH_LISTEN);
        assert_eq!(raw.advertised, "overlay");
        assert_eq!(raw.http_listen, DEFAULT_HTTP_LISTEN);
        assert_eq!(raw.rpc_listen, DEFAULT_RPC_LISTEN);
        assert_eq!(raw.gateway_listen, DEFAULT_GATEWAY_LISTEN);
        assert_eq!(raw.wireguard_listen, DEFAULT_WIREGUARD_LISTEN);
        assert_eq!(raw.invite_listen, "0.0.0.0:51821");
        assert_eq!(raw.wireguard_advertised, "auto");
        assert_eq!(raw.primary_coordinator, DEFAULT_PRIMARY_COORDINATOR);
        assert_eq!(
            raw.coordinator_relay,
            derive_coordinator_relay(DEFAULT_PRIMARY_COORDINATOR)
        );
        assert_eq!(raw.checkpoint_blocks, DEFAULT_CHECKPOINT_BLOCKS);
        assert!(raw.sync_index);
        assert!(!raw.announce_capabilities);
        assert_eq!(raw.sandbox, "direct");
        assert_eq!(raw.sandbox_image, DEFAULT_PODMAN_IMAGE);
        assert_eq!(raw.sandbox_cores, 0);
        assert_eq!(raw.sandbox_mem_gb, 0);
    }

    /// nothing optional: a file missing ANY key refuses to parse, and the
    /// retired `wireguard_effect` key is an unknown-field error — old files
    /// break loudly instead of half-working.
    #[test]
    fn incomplete_or_retired_files_fail_loudly() {
        let dir = tmp("strict");
        let p = fresh_default_plumbing(&dir);
        write_node_toml(&dir, &p).expect("write");
        let full = std::fs::read_to_string(dir.join("node.toml")).expect("read");

        // drop one required key → parse error naming it.
        let missing: String = full
            .lines()
            .filter(|l| !l.starts_with("rpc_listen"))
            .map(|l| format!("{l}\n"))
            .collect();
        std::fs::write(dir.join("node.toml"), missing).expect("write");
        let err = load_node_toml(&dir.join("node.toml")).expect_err("missing key must fail");
        assert!(err.contains("rpc_listen"), "{err}");

        // a retired key → unknown-field error.
        std::fs::write(
            dir.join("node.toml"),
            format!("{full}wireguard_effect = \"socket\"\n"),
        )
        .expect("write");
        let err = load_node_toml(&dir.join("node.toml")).expect_err("retired key must fail");
        assert!(err.contains("wireguard_effect"), "{err}");
    }

    /// flags win over an existing file; unflagged values survive a
    /// re-merge byte-for-byte (idempotent, partial-flag-safe).
    #[test]
    fn plumbing_merges_flags_over_existing_file_over_defaults() {
        let dir = tmp("plumbing");
        let p = fresh_default_plumbing(&dir);
        write_node_toml(&dir, &p).expect("write defaults");

        let p = merged_plumbing(
            &dir,
            Some("127.0.0.1:53000"),
            None,
            Some("127.0.0.1:53001"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("merge");
        assert_eq!(p.listen, "127.0.0.1:53000");
        assert_eq!(p.http_listen, "127.0.0.1:53001");
        // unflagged values came from the existing file, not re-derivation:
        // advertised stays "overlay" (from the file) even though the new
        // listen is v4.
        assert_eq!(p.advertised, "overlay");
        assert_eq!(p.rpc_listen, DEFAULT_RPC_LISTEN);
        write_node_toml(&dir, &p).expect("rewrite");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.listen, "127.0.0.1:53000");
        assert_eq!(raw.http_listen, "127.0.0.1:53001");
        assert_eq!(raw.rpc_listen, DEFAULT_RPC_LISTEN);
    }

    /// file-only keys (no CLI flag) survive every rewrite via the same
    /// chain — a hand-edit is never silently reset.
    #[test]
    fn hand_edited_values_survive_rewrite() {
        let dir = tmp("hand-edit");
        let p = fresh_default_plumbing(&dir);
        write_node_toml(&dir, &p).expect("write defaults");
        let edited = std::fs::read_to_string(dir.join("node.toml"))
            .expect("read")
            .replace("checkpoint_blocks = 32", "checkpoint_blocks = 7")
            .replace("sandbox = \"direct\"", "sandbox = \"podman\"")
            .replace("sandbox_cores = 0", "sandbox_cores = 4");
        std::fs::write(dir.join("node.toml"), edited).expect("write");
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None)
            .expect("merge");
        write_node_toml(&dir, &p).expect("rewrite");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.checkpoint_blocks, 7);
        assert_eq!(raw.sandbox, "podman");
        assert_eq!(raw.sandbox_cores, 4);
    }

    /// the dev-seed shape parses through the same loader, discriminated by
    /// the absent `network` key — and the workspace verbs refuse it.
    #[test]
    fn dev_seed_shape_parses_and_workspace_verbs_refuse_it() {
        let dir = tmp("dev-seed");
        std::fs::write(
            dir.join("node.toml"),
            "id = 0\nnamespace = \"demo\"\npeer_seeds = [0]\nlisten = \"127.0.0.1:0\"\n",
        )
        .expect("write");
        let (raw, _) = load_raw_node_toml(&dir.join("node.toml")).expect("parse");
        assert!(matches!(raw, RawNodeToml::DevSeed(_)));
        let err = load_node_toml(&dir.join("node.toml")).expect_err("verbs refuse dev shape");
        assert!(err.contains("dev-seed"), "{err}");
    }

    #[test]
    fn derived_relay_follows_the_coordinator() {
        assert_eq!(derive_coordinator_relay("coord.example:3478"), "coord.example:443");
        assert_eq!(derive_coordinator_relay("none"), "none");
    }
}
