//! node.toml — the raw file shape (both config shapes in one struct) and
//! the workspace plumbing merge that keeps init/join idempotent.
//!
//! The generated file is COMPLETE: every operator key is printed — live keys
//! carry the merged working values (which default to a node that works out
//! of the box: mesh, HTTP, gateway, RPC, and the WireGuard plane all on),
//! and absent optional keys are printed as comments showing the default they
//! fall back to. No bare node.toml: the file is its own documentation.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::DEFAULT_PRIMARY_COORDINATOR;
use super::resolve::DEFAULT_CHECKPOINT_BLOCKS;

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

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeToml {
    // --- the network shape ---
    /// path to the network descriptor; PRESENT means the network shape.
    pub network: Option<String>,
    /// path to the identity secret; default "identity.key" beside node.toml.
    pub key_file: Option<String>,

    // --- the dev-seed shape (legacy; see module docs) ---
    pub id: Option<u64>,
    pub namespace: Option<String>,
    pub peer_seeds: Option<Vec<u64>>,
    pub validator_seeds: Option<Vec<u64>>,
    pub bootstrapper_addr: Option<String>,

    // --- shared plumbing ---
    pub listen: String,
    pub advertised: Option<String>,
    pub storage_dir: Option<String>,
    pub rpc_listen: Option<String>,
    pub http_listen: Option<String>,
    /// Dedicated, least-privilege browser gateway. It exposes no node API and
    /// is normally loopback-only; route windows use per-session
    /// `<token>.localhost` origins on this listener.
    pub gateway_listen: Option<String>,
    /// sealed blocks between recovery checkpoints (node-local operator
    /// policy — never part of the network descriptor). default 32.
    pub checkpoint_blocks: Option<u64>,
    /// the UDP endpoint this node advertises for its WireGuard tunnel;
    /// PRESENT stages the node-driven reachability plane (node-local
    /// operator policy, like checkpoint_blocks). absent = plane off.
    pub wireguard_listen: Option<String>,
    /// RETIRED as an operator option — the node always drives the in-process
    /// userspace backend (no privilege, no host mutation). The key is still
    /// parsed for two reasons only: "socket" is tolerated so files written
    /// before the retirement keep booting, and "fake" remains the test-harness
    /// seam (record configs in memory, no tunnels). "tun"/"real" fail loudly.
    pub wireguard_effect: Option<String>,
    /// the UDP endpoint this node's invite intro listener binds — where a
    /// fresh joiner announces its keys (token-authenticated) so the tunnel
    /// can come up before any p2p. defaults to `wireguard_listen` with the
    /// port + 1; only meaningful when the plane runs.
    pub invite_listen: Option<String>,
    /// opt-in shipped-index warm start when joining (node-local operator
    /// policy, like checkpoint_blocks): fetch the sync source's derived
    /// index checkpoints alongside state-sync. the derived tier has no
    /// root, so these bytes are UNVERIFIABLE — off, the default, means the
    /// index heals from verified state instead (indexable spec §7).
    pub sync_index: Option<bool>,
    /// whether this node publishes its discovered provider set into the
    /// capability registry (node-local operator policy, like
    /// checkpoint_blocks; default false). `false` makes an ACCEPT-LANE-ONLY
    /// provider: the node still resolves and executes capabilities its host
    /// carries, but never enters any tag's rendezvous pool — it serves only
    /// UNASSIGNED announcements, by racing `SagaMsg::Accept` like any other
    /// capable node. announcing stays truthful either way: this can hide a
    /// real provider, never fabricate one.
    pub announce_capabilities: Option<bool>,
    /// the AMBIENT rendezvous coordinator this node's reachability plane
    /// binds — never carried in an invite (see `coordinator_ingress`'s
    /// doc). `"host:port"` overrides the compiled-in
    /// `DEFAULT_PRIMARY_COORDINATOR`; `"none"`/`"off"`/`"direct"` disables
    /// coordination outright; the key ABSENT (the pre-feature default)
    /// re-derives the compiled default at runtime — bit-identical to today.
    pub primary_coordinator: Option<String>,
    /// the TCP relay a joiner's first-contact fallback dials when every UDP
    /// path is dead (Join v2 item 2). `"host:port"` REPLACES the derived
    /// list; `"none"`/`"off"`/`"direct"` disables the fallback outright
    /// (mirroring `primary_coordinator`'s sentinels); the key ABSENT (the
    /// zero-config joiner default) derives the relay from the ambient
    /// coordinator — coordinator host, TCP/443.
    pub coordinator_relay: Option<String>,
    /// the UDP endpoint this node advertises for its WireGuard tunnel,
    /// independent of `wireguard_listen` (which stays bind-only): a
    /// concrete `"host:port"` (hostname resolved once at plane start, like
    /// the mesh `advertised`) always wins; the key ABSENT falls back to
    /// today's derivation (`wireguard_listen`'s IP when concrete, endpoint-
    /// less/roaming when unspecified) — bit-identical to today.
    pub wireguard_advertised: Option<String>,

    /// how provider runs are spawned (node-local operator policy, like
    /// checkpoint_blocks). absent/`"direct"` = the plain host spawn (default);
    /// `"podman"` = a rootless container that enforces each run's numeric
    /// limits AND makes this node announce its probed capacity; `"tart"` uses
    /// one ephemeral Apple-Silicon VM per run. any other value is a loud config
    /// error.
    pub sandbox: Option<String>,
    /// the provider environment image: a container image for Podman or a VM
    /// image for Tart. defaults to `docker.io/library/node:22-slim` for Podman
    /// and the Cirrus Labs Sonoma base image for Tart. ignored for Direct.
    pub sandbox_image: Option<String>,
    /// override the probed core count this node announces as sandbox capacity
    /// (Podman or Tart). WINS over the `/proc`/sysctl probe.
    pub sandbox_cores: Option<u64>,
    /// override the probed total-memory GiB this node announces as sandbox
    /// capacity (Podman or Tart). WINS over the probe.
    pub sandbox_mem_gb: Option<u64>,
}

/// read a raw node.toml plus its base directory (which relative paths inside
/// the file resolve against).
pub fn load_node_toml(cfg_path: &Path) -> Result<(NodeToml, PathBuf), String> {
    let text = std::fs::read_to_string(cfg_path).map_err(|e| format!("read {cfg_path:?}: {e}"))?;
    let raw: NodeToml = toml::from_str(&text).map_err(|e| format!("{cfg_path:?}: {e}"))?;
    let base = cfg_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok((raw, base))
}

/// a workspace's plumbing (everything in node.toml that is not the network
/// reference), merged from three layers: explicit flags win, else values an
/// EXISTING node.toml already carries — network- or dev-shape alike, so
/// joining inside the desktop app's solo dir inherits its http port instead
/// of resetting it — else the WORKING defaults (`DEFAULT_*`: mesh, HTTP,
/// gateway, RPC, and the WireGuard plane all up — a flagless init/join
/// yields a node that works out of the box). always writing the merged
/// result makes init/join idempotent AND partial-flag-safe (one flag never
/// resets the others). note the flip side: deleting a defaulted key from
/// the file turns it back into "absent", which the next rewrite re-fills —
/// surfaces are turned off by editing values, not by deleting lines.
pub struct Plumbing {
    pub listen: String,
    pub advertised: Option<String>,
    pub http_listen: Option<String>,
    pub gateway_listen: Option<String>,
    pub rpc_listen: Option<String>,
    /// merged like the rest — a hand-edited storage_dir survives rewrites.
    pub storage_dir: String,
    /// merged from an existing file only (no flag); a WireGuard join seeds a
    /// default AFTER the merge when the invite carries a tunnel bootstrap.
    pub wireguard_listen: Option<String>,
    /// merged from explicit flags or existing file; defaults from
    /// `wireguard_listen` when absent.
    pub invite_listen: Option<String>,
    /// merged like the rest; see `NodeToml::primary_coordinator`. Absent
    /// (`None`) preserves today's behavior exactly (the runtime re-derives
    /// the compiled default / whatever the descriptor already encodes).
    pub primary_coordinator: Option<String>,
    /// merged like the rest; see `NodeToml::wireguard_advertised`.
    pub wireguard_advertised: Option<String>,
    // --- file-only keys (no CLI flag): merged from an existing node.toml so
    // --- a hand-set value survives every rewrite (the "GOTCHA" chain).
    pub coordinator_relay: Option<String>,
    pub checkpoint_blocks: Option<u64>,
    pub sync_index: Option<bool>,
    pub announce_capabilities: Option<bool>,
    pub sandbox: Option<String>,
    pub sandbox_image: Option<String>,
    pub sandbox_cores: Option<u64>,
    pub sandbox_mem_gb: Option<u64>,
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
    let existing: Option<NodeToml> = if path.exists() {
        Some(load_node_toml(&path)?.0)
    } else {
        None
    };
    let e = existing.as_ref();
    Ok(Plumbing {
        listen: listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.listen.clone()))
            .unwrap_or_else(|| DEFAULT_MESH_LISTEN.into()),
        advertised: advertised
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.advertised.clone())),
        http_listen: http_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.http_listen.clone()))
            .or_else(|| Some(DEFAULT_HTTP_LISTEN.into())),
        gateway_listen: gateway_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.gateway_listen.clone()))
            .or_else(|| Some(DEFAULT_GATEWAY_LISTEN.into())),
        rpc_listen: rpc_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.rpc_listen.clone()))
            .or_else(|| Some(DEFAULT_RPC_LISTEN.into())),
        storage_dir: e
            .and_then(|r| r.storage_dir.clone())
            .unwrap_or_else(|| "storage".into()),
        wireguard_listen: wireguard_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_listen.clone()))
            .or_else(|| Some(DEFAULT_WIREGUARD_LISTEN.into())),
        invite_listen: invite_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.invite_listen.clone())),
        primary_coordinator: primary_coordinator
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.primary_coordinator.clone())),
        wireguard_advertised: wireguard_advertised
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_advertised.clone())),
        coordinator_relay: e.and_then(|r| r.coordinator_relay.clone()),
        checkpoint_blocks: e.and_then(|r| r.checkpoint_blocks),
        sync_index: e.and_then(|r| r.sync_index),
        announce_capabilities: e.and_then(|r| r.announce_capabilities),
        sandbox: e.and_then(|r| r.sandbox.clone()),
        sandbox_image: e.and_then(|r| r.sandbox_image.clone()),
        sandbox_cores: e.and_then(|r| r.sandbox_cores),
        sandbox_mem_gb: e.and_then(|r| r.sandbox_mem_gb),
    })
}

/// the post-merge reachability normalization both generator verbs share: an
/// overlay-advertising node needs a dual-stack mesh listener (members
/// reverse-dial its ULA over the tunnels, and the overlay is IPv6). An
/// EXPLICIT `--listen` is never rewritten — the fixup only upgrades a value
/// inherited from an old file (the desktop solo dir's `127.0.0.1:0` shape).
pub fn seed_reachability_defaults(p: &mut Plumbing, listen_flag_passed: bool) {
    if !listen_flag_passed {
        let port: u16 = p
            .listen
            .parse::<std::net::SocketAddr>()
            .map(|a| a.port())
            .unwrap_or(0);
        let needs_dual_stack_upgrade = port == 0 || !p.listen.starts_with('[');
        if needs_dual_stack_upgrade {
            p.listen = format!("[::]:{}", if port == 0 { 52200 } else { port });
        }
    }
    // "overlay" needs the v6 listener it just ensured; an explicit v4-only
    // listen keeps advertising its socket address instead.
    let overlay_capable = p.listen.starts_with('[');
    if p.advertised.is_none() && overlay_capable {
        p.advertised = Some("overlay".into());
    }
}

/// one printed config line per invocation: the live `key = value` when the
/// option is set, else the commented fallback. Three shapes:
/// - `str` / `raw`: absent prints `# key = <default>  # note` — a CONCRETE
///   default (uncommenting it must be a no-op today; the parity test pins
///   every such line against the compiled default).
/// - `note`: absent prints `# key — note` (the default is derived, there is
///   no single value to show).
macro_rules! keyline {
    (str $s:ident, $key:ident: $opt:expr, $def:expr, $note:expr) => {
        match &$opt {
            Some(v) => {
                let _ = writeln!($s, concat!(stringify!($key), " = \"{}\""), v);
            }
            None => {
                let _ = writeln!(
                    $s,
                    concat!("# ", stringify!($key), " = \"{}\"  # {}"),
                    $def, $note
                );
            }
        }
    };
    (raw $s:ident, $key:ident: $opt:expr, $def:expr, $note:expr) => {
        match &$opt {
            Some(v) => {
                let _ = writeln!($s, concat!(stringify!($key), " = {}"), v);
            }
            None => {
                let _ = writeln!(
                    $s,
                    concat!("# ", stringify!($key), " = {}  # {}"),
                    $def, $note
                );
            }
        }
    };
    (note $s:ident, $key:ident: $opt:expr, $note:expr) => {
        match &$opt {
            Some(v) => {
                let _ = writeln!($s, concat!(stringify!($key), " = \"{}\""), v);
            }
            None => {
                let _ = writeln!($s, concat!("# ", stringify!($key), " — {}"), $note);
            }
        }
    };
    (rawnote $s:ident, $key:ident: $opt:expr, $note:expr) => {
        match &$opt {
            Some(v) => {
                let _ = writeln!($s, concat!(stringify!($key), " = {}"), v);
            }
            None => {
                let _ = writeln!($s, concat!("# ", stringify!($key), " — {}"), $note);
            }
        }
    };
}

/// write a network-shape node.toml into a workspace dir (init/join): the
/// COMPLETE key set — live keys carry the merged working values, absent
/// optional keys are printed as comments showing the default they fall back
/// to, so the file is its own documentation. the file references its
/// siblings relatively, so the whole dir is relocatable. replaces a
/// dev-shape file wholesale (its plumbing survives via [`merged_plumbing`])
/// — a join must actually take effect.
pub fn write_node_toml(dir: &Path, p: &Plumbing) -> Result<PathBuf, String> {
    // the intro port default is wireguard_listen's port + 1 — show it
    // concretely when the tunnel plane is configured.
    let derived_invite_listen = p.wireguard_listen.as_deref().and_then(|w| {
        let addr: std::net::SocketAddr = w.parse().ok()?;
        Some(format!("0.0.0.0:{}", addr.port().checked_add(1)?))
    });
    let mut s = String::from(
        "# ducktape node config (network shape) — see network.toml for the network.\n\
         # every key is here: set keys are live, commented keys show the default\n\
         # used while they stay absent.\n\
         network = \"network.toml\"\n\
         key_file = \"identity.key\"\n",
    );
    let _ = writeln!(s, "listen = \"{}\"", p.listen);
    keyline!(note s, advertised: p.advertised,
        "absent: advertise the mesh listen address; \"overlay\": the chain-derived ULA");
    let _ = writeln!(s, "storage_dir = '{}'", p.storage_dir);
    keyline!(str s, http_listen: p.http_listen, DEFAULT_HTTP_LISTEN,
        "absent: no HTTP app API");
    keyline!(str s, gateway_listen: p.gateway_listen, DEFAULT_GATEWAY_LISTEN,
        "loopback only; port 0 = pick a free port");
    keyline!(str s, rpc_listen: p.rpc_listen, DEFAULT_RPC_LISTEN,
        "absent: no admin RPC");
    keyline!(str s, wireguard_listen: p.wireguard_listen, DEFAULT_WIREGUARD_LISTEN,
        "absent: the WireGuard reachability plane stays OFF");
    match derived_invite_listen {
        Some(derived) => keyline!(str s, invite_listen: p.invite_listen, derived,
            "default: wireguard_listen's port + 1"),
        None => keyline!(note s, invite_listen: p.invite_listen,
            "invite intro listener; default: wireguard_listen's port + 1"),
    }
    keyline!(note s, wireguard_advertised: p.wireguard_advertised,
        "absent: derived from wireguard_listen (concrete IP, else endpoint-less/roaming)");
    keyline!(str s, primary_coordinator: p.primary_coordinator, DEFAULT_PRIMARY_COORDINATOR,
        "absent TRACKS the compiled default; setting it pins; \"none\" disables");
    keyline!(note s, coordinator_relay: p.coordinator_relay,
        "TCP first-contact fallback; absent: derived (coordinator host, port 443); \"none\" disables");
    keyline!(raw s, checkpoint_blocks: p.checkpoint_blocks, DEFAULT_CHECKPOINT_BLOCKS,
        "sealed blocks between recovery checkpoints");
    keyline!(raw s, sync_index: p.sync_index, false,
        "opt-in UNVERIFIABLE shipped-index warm start on join");
    keyline!(raw s, announce_capabilities: p.announce_capabilities, false,
        "true: publish this node's provider set into the capability registry");
    keyline!(str s, sandbox: p.sandbox, "direct",
        "provider run isolation: \"direct\", \"podman\", or \"tart\"");
    keyline!(note s, sandbox_image: p.sandbox_image,
        "provider environment image; defaults per sandbox (node:22-slim / sonoma-base)");
    keyline!(rawnote s, sandbox_cores: p.sandbox_cores,
        "announced sandbox capacity override; absent: probed from the host");
    keyline!(rawnote s, sandbox_mem_gb: p.sandbox_mem_gb,
        "announced sandbox capacity override (GiB); absent: probed from the host");
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

    #[test]
    fn node_config_rejects_retired_duckdns_service_sections() {
        let retired = r#"
            listen = "127.0.0.1:1"
            [[duckdns.services]]
            scope = "account"
            service = "huddle"
        "#;
        assert!(
            toml::from_str::<NodeToml>(retired).is_err(),
            "naming-only DuckDNS must not retain service configuration"
        );
    }

    #[test]
    fn plumbing_merges_flags_over_existing_file_over_defaults() {
        let dir = tmp("plumbing");
        std::fs::write(
            dir.join("node.toml"),
            r#"id = 0
listen = "127.0.0.1:0"
namespace = "ducktape-local"
peer_seeds = [0]
http_listen = "127.0.0.1:8844"
storage_dir = '/data/ducktape'
"#,
        )
        .expect("write");
        // one flag overrides ONLY its field; the http port AND a hand-edited
        // storage_dir survive. keys the file left absent fill from the
        // working defaults.
        let p = merged_plumbing(
            &dir,
            Some("127.0.0.1:53000"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("merge");
        assert_eq!(p.listen, "127.0.0.1:53000");
        assert_eq!(p.http_listen.as_deref(), Some("127.0.0.1:8844"));
        assert_eq!(p.storage_dir, "/data/ducktape");
        assert_eq!(p.rpc_listen.as_deref(), Some(DEFAULT_RPC_LISTEN));
        assert_eq!(p.wireguard_listen.as_deref(), Some(DEFAULT_WIREGUARD_LISTEN));
        assert_eq!(p.gateway_listen.as_deref(), Some(DEFAULT_GATEWAY_LISTEN));
        // and the merged write is network-shape.
        write_node_toml(&dir, &p).expect("write");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.network.as_deref(), Some("network.toml"));
        assert_eq!(raw.http_listen.as_deref(), Some("127.0.0.1:8844"));
        assert_eq!(raw.listen, "127.0.0.1:53000");
        assert_eq!(raw.storage_dir.as_deref(), Some("/data/ducktape"));
        assert_eq!(raw.rpc_listen.as_deref(), Some(DEFAULT_RPC_LISTEN));
    }

    /// The generated file is COMPLETE: every operator key appears (live or as
    /// a commented default), every commented `key = value` line uncomments to
    /// valid TOML, and every uncommented value equals the compiled default —
    /// so uncommenting a line is a no-op until the operator edits it, and the
    /// comments can never drift from the code.
    #[test]
    fn generated_file_is_complete_and_commented_defaults_match_compiled() {
        let dir = tmp("full-print");
        let mut p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None)
            .expect("fresh merge");
        seed_reachability_defaults(&mut p, false);
        write_node_toml(&dir, &p).expect("write");
        let text = std::fs::read_to_string(dir.join("node.toml")).expect("read");

        // the fresh defaults are a WORKING node: every surface up.
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("parse");
        assert_eq!(raw.listen, DEFAULT_MESH_LISTEN);
        assert_eq!(raw.advertised.as_deref(), Some("overlay"));
        assert_eq!(raw.http_listen.as_deref(), Some(DEFAULT_HTTP_LISTEN));
        assert_eq!(raw.rpc_listen.as_deref(), Some(DEFAULT_RPC_LISTEN));
        assert_eq!(raw.gateway_listen.as_deref(), Some(DEFAULT_GATEWAY_LISTEN));
        assert_eq!(
            raw.wireguard_listen.as_deref(),
            Some(DEFAULT_WIREGUARD_LISTEN)
        );

        // every operator key is present, live or documented.
        let operator_keys = [
            "listen",
            "advertised",
            "storage_dir",
            "http_listen",
            "gateway_listen",
            "rpc_listen",
            "wireguard_listen",
            "invite_listen",
            "wireguard_advertised",
            "primary_coordinator",
            "coordinator_relay",
            "checkpoint_blocks",
            "sync_index",
            "announce_capabilities",
            "sandbox",
            "sandbox_image",
            "sandbox_cores",
            "sandbox_mem_gb",
        ];
        for key in operator_keys {
            let live = text.lines().any(|l| l.starts_with(&format!("{key} = ")));
            let documented = text.lines().any(|l| l.starts_with(&format!("# {key} ")));
            assert!(live || documented, "{key} missing from the generated file");
        }

        // uncomment every commented `key = value` line: the result must parse
        // (deny_unknown_fields catches a typo'd key) and equal the compiled
        // default (a stale comment fails here).
        let uncommented: String = text
            .lines()
            .map(|l| {
                let is_commented_kv = l.starts_with("# ")
                    && l[2..]
                        .split_once(" = ")
                        .is_some_and(|(k, _)| k.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
                if is_commented_kv {
                    format!("{}\n", &l[2..])
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        let full: NodeToml =
            toml::from_str(&uncommented).expect("every uncommented default line parses");
        assert_eq!(full.checkpoint_blocks, Some(DEFAULT_CHECKPOINT_BLOCKS));
        assert_eq!(full.sync_index, Some(false));
        assert_eq!(full.announce_capabilities, Some(false));
        assert_eq!(full.sandbox.as_deref(), Some("direct"));
        assert_eq!(
            full.primary_coordinator.as_deref(),
            Some(DEFAULT_PRIMARY_COORDINATOR)
        );
        // wireguard_listen 0.0.0.0:51820 → the derived intro default is +1.
        assert_eq!(full.invite_listen.as_deref(), Some("0.0.0.0:51821"));
    }

    /// file-only keys (no CLI flag) ride the same survive-rewrite chain: a
    /// hand-set value merges through and the rewrite prints it live.
    #[test]
    fn hand_set_file_only_keys_survive_rewrite() {
        let dir = tmp("file-only-keys");
        std::fs::write(
            dir.join("node.toml"),
            "listen = \"127.0.0.1:1\"\ncheckpoint_blocks = 7\nsync_index = true\n\
             sandbox = \"podman\"\nsandbox_cores = 4\ncoordinator_relay = \"relay.example:443\"\n",
        )
        .expect("write");
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None)
            .expect("merge");
        write_node_toml(&dir, &p).expect("rewrite");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.checkpoint_blocks, Some(7));
        assert_eq!(raw.sync_index, Some(true));
        assert_eq!(raw.sandbox.as_deref(), Some("podman"));
        assert_eq!(raw.sandbox_cores, Some(4));
        assert_eq!(raw.coordinator_relay.as_deref(), Some("relay.example:443"));
    }

    /// A legacy file carrying the retired `wireguard_effect` key still merges
    /// (the key is tolerated at parse) — and the rewrite DROPS the key: the
    /// node's behavior is the same with or without it.
    #[test]
    fn plumbing_tolerates_legacy_wireguard_effect_and_rewrite_drops_it() {
        let dir = tmp("plumbing-wg-effect");
        std::fs::write(
            dir.join("node.toml"),
            "listen = \"127.0.0.1:1\"\nwireguard_effect = \"socket\"\n",
        )
        .expect("write");
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None)
            .expect("legacy key must not abort the merge");
        write_node_toml(&dir, &p).expect("write");
        let reread = std::fs::read_to_string(dir.join("node.toml")).expect("reread");
        assert!(
            !reread.contains("wireguard_effect"),
            "the retired key must not be re-minted: {reread}"
        );
    }

    /// Both change (1)/(3) keys ride the SAME `Plumbing` chain: a flag wins,
    /// an existing file's value survives an unflagged re-merge, and
    /// `write_node_toml` round-trips it verbatim (config.rs's "GOTCHA" — a
    /// key not in `Plumbing` is silently dropped on rewrite; this pins that
    /// it is NOT dropped).
    #[test]
    fn plumbing_primary_coordinator_and_wireguard_advertised_flag_wins_and_absence_preserves() {
        let dir = tmp("plumbing-coord-wgadv");
        // fresh dir + flags (the desktop app's init/join shape): written to disk.
        let p = merged_plumbing(
            &dir,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("203.0.113.9:3478"),
            Some("198.51.100.5:41820"),
        )
        .expect("merge");
        assert_eq!(p.primary_coordinator.as_deref(), Some("203.0.113.9:3478"));
        assert_eq!(p.wireguard_advertised.as_deref(), Some("198.51.100.5:41820"));
        write_node_toml(&dir, &p).expect("write");

        // no flags: the hand-settable values on disk survive a re-merge.
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None)
            .expect("re-merge");
        assert_eq!(p.primary_coordinator.as_deref(), Some("203.0.113.9:3478"));
        assert_eq!(p.wireguard_advertised.as_deref(), Some("198.51.100.5:41820"));

        // the flags win over the file (merged_plumbing's standing precedence).
        let p = merged_plumbing(
            &dir,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("none"),
            Some("203.0.113.9:41821"),
        )
        .expect("override");
        assert_eq!(p.primary_coordinator.as_deref(), Some("none"));
        assert_eq!(p.wireguard_advertised.as_deref(), Some("203.0.113.9:41821"));

        // and the round trip re-reads verbatim — the survives-rewrite chain.
        write_node_toml(&dir, &p).expect("write");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.primary_coordinator.as_deref(), Some("none"));
        assert_eq!(raw.wireguard_advertised.as_deref(), Some("203.0.113.9:41821"));
    }
}
