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
///
/// EVERY eagerly-bound TCP default sits BELOW [`EPHEMERAL_FLOOR`], and that is
/// a correctness property rather than tidiness — see
/// [`no_tcp_default_sits_in_the_ephemeral_range`]. The mesh listener was at
/// `52200`, inside the range, where any outbound connection on the box can take
/// the port first; commonware's discovery listener `expect`s its bind, so
/// losing that race is an unwinding panic ten seconds into boot. It now sits
/// beside the two operator surfaces, which were never at risk.
pub const DEFAULT_MESH_LISTEN: &str = "[::]:8846";
pub const DEFAULT_HTTP_LISTEN: &str = "127.0.0.1:8844";
pub const DEFAULT_RPC_LISTEN: &str = "127.0.0.1:8845";
/// port 0 on purpose: the browser gateway prints its bound port and its
/// consumers re-read it per session; a fixed port would only collide.
pub const DEFAULT_GATEWAY_LISTEN: &str = "127.0.0.1:0";
/// UDP, and deliberately still the CONVENTIONAL WireGuard port even though it
/// is inside the ephemeral range: a firewall rule, a NAT forward and an
/// operator's muscle memory all key on 51820, and the tunnel plane answers a
/// bind failure with a logged retry rather than a panic — so the trade the mesh
/// port made does not apply here.
pub const DEFAULT_WIREGUARD_LISTEN: &str = "0.0.0.0:51820";

/// The bottom of Linux's default `ip_local_port_range` (32768–60999). A
/// listener whose default port sits above this is racing every outbound
/// connection on the host for it.
#[cfg(test)]
pub const EPHEMERAL_FLOOR: u16 = 32768;

/// the operator node.toml — the network shape, every key required.
///
/// Where "unset" is a meaningful state it is an EXPLICIT value, never a
/// missing key: `"none"` (primary_coordinator, coordinator_relay),
/// `"auto"` (wireguard_advertised), `0` = probe ([sandbox] cores/mem_gb).
/// the ONE table-level exception is `[sandbox]`: its PRESENCE is the
/// compute-plane switch (see [`SandboxToml`]), so a consensus-only node
/// simply has no such table.
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
    /// the compute plane: PRESENT = provider runs execute in this sandbox;
    /// ABSENT = consensus-only node (no provider discovery, no announce, no
    /// terminal plane).
    pub sandbox: Option<SandboxToml>,
}

/// the `[sandbox]` compute-plane table. its PRESENCE is what makes a node a
/// compute node; inside it every key is required. there is deliberately no
/// bare/"direct" runtime — a provider run never executes directly on the
/// host, so the only selectable adapters are the audited in-tree ones.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxToml {
    /// the isolation adapter: `"podman"` or `"tart"`.
    pub runtime: String,
    /// the provider environment image the adapter boots.
    pub image: String,
    /// announced capacity; `0` = probe the host.
    pub cores: u64,
    /// announced capacity in GiB; `0` = probe the host.
    pub mem_gb: u64,
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
    pub sandbox: Option<SandboxToml>,
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
    pub sandbox: Option<SandboxToml>,
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
        sandbox: e.and_then(|r| r.sandbox.clone()),
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
    // the [sandbox] table LAST — everything after a toml table header belongs
    // to the table, so no top-level key may follow it.
    match &p.sandbox {
        Some(sb) => {
            let _ = writeln!(
                s,
                "\n# compute plane: provider runs execute inside this sandbox and the node\n\
                 # can announce capabilities. delete the whole table for a consensus-only node.\n\
                 [sandbox]"
            );
            keyline(&mut s, "runtime", format_args!("\"{}\"", sb.runtime),
                "isolation adapter: \"podman\" | \"tart\" (runs never execute bare on the host)");
            keyline(&mut s, "image", format_args!("\"{}\"", sb.image),
                "the provider environment image");
            keyline(&mut s, "cores", format_args!("{}", sb.cores),
                "announced capacity; 0 = probe the host");
            keyline(&mut s, "mem_gb", format_args!("{}", sb.mem_gb),
                "announced capacity (GiB); 0 = probe the host");
        }
        None => {
            let _ = writeln!(
                s,
                "\n# compute plane (off): uncomment [sandbox] to run providers on this node.\n\
                 # runtime: \"podman\" | \"tart\" — runs never execute bare on the host.\n\
                 #[sandbox]\n\
                 #runtime = \"podman\"\n\
                 #image = \"{DEFAULT_PODMAN_IMAGE}\"\n\
                 #cores = 0\n\
                 #mem_gb = 0"
            );
        }
    }
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
    /// No TCP listener a node binds EAGERLY may default into the ephemeral
    /// range, because the kernel hands those ports out to outbound connections
    /// and the loser of that race is a node that will not start.
    ///
    /// The mesh listener is the one that bit: it sat at `52200`, and
    /// commonware's discovery listener `expect`s its bind inside the runtime,
    /// so losing the race was `thread 'tokio-rt-worker' panicked … BindFailed`
    /// ten seconds into an otherwise healthy boot.
    ///
    /// Scoped to TCP on purpose. `wireguard_listen` is UDP, is the conventional
    /// 51820 that firewalls and NAT forwards are written against, and answers a
    /// failed bind with a logged retry rather than a panic — so it is named
    /// here as a deliberate exclusion instead of quietly not being checked.
    #[test]
    fn no_tcp_default_sits_in_the_ephemeral_range() {
        for (key, value) in [
            ("listen", DEFAULT_MESH_LISTEN),
            ("http_listen", DEFAULT_HTTP_LISTEN),
            ("rpc_listen", DEFAULT_RPC_LISTEN),
        ] {
            let port = value
                .rsplit_once(':')
                .and_then(|(_, port)| port.parse::<u16>().ok())
                .unwrap_or_else(|| panic!("{key} default {value:?} names a port"));
            assert!(
                port < EPHEMERAL_FLOOR,
                "{key} defaults to {port}, inside the kernel's ephemeral range \
                 ({EPHEMERAL_FLOOR}+) — it will lose bind races to outbound sockets"
            );
        }
        // the gateway is the one exception and it is the SAFE direction: port 0
        // asks the kernel for a free port instead of racing for a fixed one.
        assert!(DEFAULT_GATEWAY_LISTEN.ends_with(":0"));
    }

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
        // no [sandbox] table by default: a fresh node is consensus-only, and
        // the commented example in the file must not parse as a live table.
        assert_eq!(raw.sandbox, None);
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
            + "\n[sandbox]\nruntime = \"podman\"\nimage = \"img\"\ncores = 4\nmem_gb = 0\n";
        std::fs::write(dir.join("node.toml"), edited).expect("write");
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None)
            .expect("merge");
        write_node_toml(&dir, &p).expect("rewrite");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.checkpoint_blocks, 7);
        let sandbox = raw.sandbox.expect("hand-added [sandbox] survives rewrite");
        assert_eq!(sandbox.runtime, "podman");
        assert_eq!(sandbox.cores, 4);
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
