//! node.toml — the raw file shape (both config shapes in one struct) and
//! the workspace plumbing merge that keeps init/join idempotent.

use std::path::{Path, PathBuf};

use super::resolve::parse_wireguard_effect;

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
    /// which `WireGuardEffect` the reachability plane drives: "tun"
    /// (default — configure an actual interface via the userspace WireGuard
    /// runtime; needs root/CAP_NET_ADMIN; "real" is the legacy alias),
    /// "socket" (the ADR's TUN-less in-process backend: no privilege, no
    /// host mutation — overlay reachability exists only inside this
    /// process), or "fake" (record configs in memory; for dev/sim runs, and
    /// for several same-chain nodes on one host, which would otherwise
    /// fight over one interface name).
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
    /// checkpoint_blocks; default true). `false` makes an ACCEPT-LANE-ONLY
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
    /// limits AND makes this node announce its probed capacity; `"tart"` is
    /// accepted and reserved (resolves to Direct until a sibling branch lands
    /// the backend). any other value is a loud config error.
    pub sandbox: Option<String>,
    /// the container image the `podman` sandbox runs each provider in;
    /// default `docker.io/library/node:22-slim`. ignored unless `sandbox =
    /// "podman"`.
    pub sandbox_image: Option<String>,
    /// override the probed core count this node announces as sandbox capacity
    /// (`podman` only). WINS over the `/proc`/sysctl probe.
    pub sandbox_cores: Option<u64>,
    /// override the probed total-memory GiB this node announces as sandbox
    /// capacity (`podman` only). WINS over the probe.
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
/// of resetting it — else defaults. always writing the merged result makes
/// init/join idempotent AND partial-flag-safe (one flag never resets the
/// others).
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
    /// merged like the rest — a hand-set value survives; the desktop app
    /// passes "socket" here (overlay-net ADR phase 4) while the parse
    /// default for a file without the key stays `tun`.
    pub wireguard_effect: Option<String>,
    /// merged like the rest; see `NodeToml::primary_coordinator`. Absent
    /// (`None`) preserves today's behavior exactly (the runtime re-derives
    /// the compiled default / whatever the descriptor already encodes).
    pub primary_coordinator: Option<String>,
    /// merged like the rest; see `NodeToml::wireguard_advertised`.
    pub wireguard_advertised: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn merged_plumbing(
    dir: &Path,
    listen: Option<&str>,
    advertised: Option<&str>,
    http_listen: Option<&str>,
    gateway_listen: Option<&str>,
    rpc_listen: Option<&str>,
    wireguard_effect: Option<&str>,
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
    // reject a typo'd effect value at the verb, before anything lands on disk
    // — resolve() would only catch it on the node's NEXT boot.
    parse_wireguard_effect(wireguard_effect)?;
    Ok(Plumbing {
        listen: listen
            .map(str::to_string)
            .or_else(|| e.map(|r| r.listen.clone()))
            .unwrap_or_else(|| "127.0.0.1:0".into()),
        advertised: advertised
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.advertised.clone())),
        http_listen: http_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.http_listen.clone())),
        gateway_listen: gateway_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.gateway_listen.clone())),
        rpc_listen: rpc_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.rpc_listen.clone())),
        storage_dir: e
            .and_then(|r| r.storage_dir.clone())
            .unwrap_or_else(|| "storage".into()),
        wireguard_listen: wireguard_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_listen.clone())),
        invite_listen: invite_listen
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.invite_listen.clone())),
        wireguard_effect: wireguard_effect
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_effect.clone())),
        primary_coordinator: primary_coordinator
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.primary_coordinator.clone())),
        wireguard_advertised: wireguard_advertised
            .map(str::to_string)
            .or_else(|| e.and_then(|r| r.wireguard_advertised.clone())),
    })
}

/// write a network-shape node.toml into a workspace dir (init/join). the file
/// references its siblings relatively, so the whole dir is relocatable.
/// replaces a dev-shape file wholesale (its plumbing survives via
/// [`merged_plumbing`]) — a join must actually take effect.
pub fn write_node_toml(dir: &Path, p: &Plumbing) -> Result<PathBuf, String> {
    let mut s = String::from(
        "# ducktape node config (network shape) — see network.toml for the network.\n\
         network = \"network.toml\"\nkey_file = \"identity.key\"\n",
    );
    s += &format!("listen = \"{}\"\n", p.listen);
    if let Some(a) = &p.advertised {
        s += &format!("advertised = \"{a}\"\n");
    }
    s += &format!("storage_dir = '{}'\n", p.storage_dir);
    if let Some(h) = &p.http_listen {
        s += &format!("http_listen = \"{h}\"\n");
    }
    if let Some(d) = &p.gateway_listen {
        s += &format!("gateway_listen = \"{d}\"\n");
    }
    if let Some(r) = &p.rpc_listen {
        s += &format!("rpc_listen = \"{r}\"\n");
    }
    if let Some(w) = &p.wireguard_listen {
        s += &format!("wireguard_listen = \"{w}\"\n");
    }
    if let Some(i) = &p.invite_listen {
        s += &format!("invite_listen = \"{i}\"\n");
    }
    if let Some(w) = &p.wireguard_effect {
        s += &format!("wireguard_effect = \"{w}\"\n");
    }
    if let Some(pc) = &p.primary_coordinator {
        s += &format!("primary_coordinator = \"{pc}\"\n");
    }
    if let Some(wa) = &p.wireguard_advertised {
        s += &format!("wireguard_advertised = \"{wa}\"\n");
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
        // storage_dir survive.
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
            None,
        )
        .expect("merge");
        assert_eq!(p.listen, "127.0.0.1:53000");
        assert_eq!(p.http_listen.as_deref(), Some("127.0.0.1:8844"));
        assert_eq!(p.storage_dir, "/data/ducktape");
        assert!(p.rpc_listen.is_none());
        // and the merged write is network-shape.
        write_node_toml(&dir, &p).expect("write");
        let (raw, _) = load_node_toml(&dir.join("node.toml")).expect("reload");
        assert_eq!(raw.network.as_deref(), Some("network.toml"));
        assert_eq!(raw.http_listen.as_deref(), Some("127.0.0.1:8844"));
        assert_eq!(raw.listen, "127.0.0.1:53000");
        assert_eq!(raw.storage_dir.as_deref(), Some("/data/ducktape"));
    }

    #[test]
    fn plumbing_wireguard_effect_flag_wins_absence_preserves_and_typos_abort() {
        let dir = tmp("plumbing-wg-effect");
        // fresh dir + flag (the desktop app's init/join): written to disk.
        let p = merged_plumbing(
            &dir,
            None,
            None,
            None,
            None,
            None,
            Some("socket"),
            None,
            None,
            None,
            None,
        )
        .expect("merge");
        assert_eq!(p.wireguard_effect.as_deref(), Some("socket"));
        write_node_toml(&dir, &p).expect("write");

        // no flag: the hand-settable value on disk survives a re-merge.
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None, None)
            .expect("re-merge");
        assert_eq!(p.wireguard_effect.as_deref(), Some("socket"));

        // the flag wins over the file (merged_plumbing's standing precedence).
        let p = merged_plumbing(
            &dir,
            None,
            None,
            None,
            None,
            None,
            Some("tun"),
            None,
            None,
            None,
            None,
        )
        .expect("override");
        assert_eq!(p.wireguard_effect.as_deref(), Some("tun"));

        // a typo aborts the verb before anything is written.
        let err = merged_plumbing(
            &dir,
            None,
            None,
            None,
            None,
            None,
            Some("sokcet"),
            None,
            None,
            None,
            None,
        )
        .err()
        .expect("a bad effect value must abort the merge");
        assert!(err.contains("wireguard_effect"), "{err}");
    }

    /// Both change (1)/(3) keys ride the SAME `Plumbing` chain as
    /// `wireguard_effect` above: a flag wins, an existing file's value
    /// survives an unflagged re-merge, and `write_node_toml` round-trips it
    /// verbatim (config.rs's "GOTCHA" — a key not in `Plumbing` is silently
    /// dropped on rewrite; this pins that it is NOT dropped).
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
            None,
            Some("203.0.113.9:3478"),
            Some("198.51.100.5:41820"),
        )
        .expect("merge");
        assert_eq!(p.primary_coordinator.as_deref(), Some("203.0.113.9:3478"));
        assert_eq!(p.wireguard_advertised.as_deref(), Some("198.51.100.5:41820"));
        write_node_toml(&dir, &p).expect("write");

        // no flags: the hand-settable values on disk survive a re-merge.
        let p = merged_plumbing(&dir, None, None, None, None, None, None, None, None, None, None)
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
