//! Synchronous operator commands for network setup and membership.
//!
//! Command handlers live outside the node runtime so boot orchestration is not
//! coupled to filesystem setup, local RPC calls, or membership ceremonies.

use std::path::PathBuf;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};

use crate::cli_args::{
    AdmitArgs, InitArgs, InviteArgs, JoinCmd, JoinQuery, KeyArgs, MemberCmd, OpCmd, PubkeyArgs,
    ResidentCmd, Selector, SelectorArgs, StatusArgs, WorkCmd, WorkTargetArgs,
};
use crate::config;
use crate::work_admission::{self, AdmitTarget, WorkAdmission};
use config::{hex_bytes, unhex};

type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// route one operator verb to its handler — ONE visible dispatch, nothing in
/// the arms but delegation. (`run` never reaches here; `main.rs` owns the
/// node-boot path.) the grammar itself lives in `cli_args.rs`.
pub(super) fn run(op: OpCmd) -> CommandResult {
    match op {
        OpCmd::Key(args) => cmd_keygen(args),
        OpCmd::Init(args) => cmd_init(args),
        OpCmd::Invite(args) => cmd_invite(args),
        OpCmd::Admit(args) => cmd_admit(args),
        OpCmd::Join(cmd) => dispatch_join(cmd),
        OpCmd::List => cmd_list(),
        OpCmd::Status(args) => cmd_node_status(args),
        OpCmd::Peers(args) => cmd_node_peers(args),
        OpCmd::Resident(cmd) => dispatch_resident(cmd),
        OpCmd::Member(cmd) => dispatch_member(cmd),
        OpCmd::Work(cmd) => dispatch_work(cmd),
    }
}

fn dispatch_work(cmd: WorkCmd) -> CommandResult {
    match cmd {
        WorkCmd::List(args) => cmd_work_list(args),
        WorkCmd::Admit(args) => cmd_work_admit(args),
        WorkCmd::Revoke(args) => cmd_work_revoke(args),
    }
}

/// the workspace directory a `node work` verb reads and writes. The policy sits
/// beside `node.toml`, and both the node and the compute daemon read it there.
fn work_workspace(selector: &Selector) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cfg_path = selector.config_path()?;
    Ok(cfg_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf())
}

/// `anyone` is the literal, everything else is an account (hex id or display
/// name — the same resolution `user cred grant` takes). A hex id resolves
/// offline; a display name needs the node to answer.
fn resolve_work_target(
    workspace: &std::path::Path,
    input: &str,
) -> Result<AdmitTarget, Box<dyn std::error::Error>> {
    if input == work_admission::ANYONE {
        return Ok(AdmitTarget::Anyone);
    }
    let base = config::http_base_in(workspace)?;
    Ok(AdmitTarget::Account(crate::cred_cli::resolve_account(
        &base, input,
    )?))
}

fn cmd_work_list(args: SelectorArgs) -> CommandResult {
    let workspace = work_workspace(&args.selector)?;
    match work_admission::load(&workspace)? {
        WorkAdmission::Owner => {
            println!("owner only — this node runs its owner's work and its own submissions")
        }
        WorkAdmission::Anyone => {
            println!("anyone — every network member may run a workload on this node")
        }
        WorkAdmission::Accounts(accounts) => {
            println!("owner, plus {} admitted account(s):", accounts.len());
            for account in &accounts {
                println!("  {}", hex_bytes(account));
            }
        }
    }
    println!(
        "policy: {}",
        work_admission::policy_path(&workspace).display()
    );
    Ok(())
}

fn cmd_work_admit(args: WorkTargetArgs) -> CommandResult {
    let workspace = work_workspace(&args.selector)?;
    let target = resolve_work_target(&workspace, &args.target)?;
    let policy = work_admission::load(&workspace)?.with(target.clone());
    work_admission::save(&workspace, &policy)?;
    match target {
        AdmitTarget::Anyone => {
            // the `service enable` consent-screen precedent: the widening that
            // re-opens the hole says so on the way in, on stderr so stdout stays
            // scriptable.
            eprintln!(
                "consent: every network member may now run a workload on this node, and any \
                 workload here may draw on every credential this node has been granted. \
                 Narrow it with `ducktape node work revoke anyone`."
            );
            println!("admitted: anyone");
        }
        AdmitTarget::Account(account) => println!("admitted: {}", hex_bytes(&account)),
    }
    Ok(())
}

fn cmd_work_revoke(args: WorkTargetArgs) -> CommandResult {
    let workspace = work_workspace(&args.selector)?;
    let target = resolve_work_target(&workspace, &args.target)?;
    let current = work_admission::load(&workspace)?;
    // revoking ONE account while the policy admits everyone would write a file
    // that changes nothing and print success: refuse instead of fail-quiet.
    let narrowing_one_from_anyone =
        current == WorkAdmission::Anyone && !matches!(target, AdmitTarget::Anyone);
    if narrowing_one_from_anyone {
        return Err(
            "this node admits anyone, so revoking one account changes nothing — \
                    run `ducktape node work revoke anyone` first"
                .into(),
        );
    }
    work_admission::save(&workspace, &current.without(target.clone()))?;
    match target {
        AdmitTarget::Anyone => println!("revoked: anyone"),
        AdmitTarget::Account(account) => println!("revoked: {}", hex_bytes(&account)),
    }
    Ok(())
}

fn dispatch_resident(cmd: ResidentCmd) -> CommandResult {
    match cmd {
        ResidentCmd::Accept(args) => cmd_invite_accept(args),
        ResidentCmd::Remove(args) => cmd_resident_remove(args),
    }
}

fn dispatch_member(cmd: MemberCmd) -> CommandResult {
    match cmd {
        MemberCmd::Promote(args) => cmd_promote(args),
        MemberCmd::Remove(args) => cmd_member_remove(args),
        MemberCmd::Leave(args) => cmd_member_leave(args),
        MemberCmd::Status(args) => cmd_member_status(args),
    }
}

/// `join` is BOTH a leaf verb (`join <blob>`) and a subfamily prefix
/// (`join requests`, `join state`) — a subcommand token wins.
fn dispatch_join(cmd: JoinCmd) -> CommandResult {
    match cmd.query {
        Some(JoinQuery::Requests(args)) => cmd_join_requests(args),
        Some(JoinQuery::State(args)) => cmd_join_state(args),
        None => cmd_join(cmd),
    }
}

/// `list` — enumerate the workspace registry, one `chain-id<TAB>config-path`
/// line per registered network on stdout. an empty registry prints a friendly
/// notice on stderr and exits 0 (nothing registered is not an error).
fn cmd_list() -> CommandResult {
    let workspaces = config::list_workspaces()?;
    if workspaces.is_empty() {
        eprintln!(
            "no workspaces registered under {}",
            config::workspaces_root()?.display()
        );
        return Ok(());
    }
    for (chain_id, config_path) in workspaces {
        println!("{chain_id}\t{}", config_path.display());
    }
    Ok(())
}

/// `status [--config <path> | -n <chain-id>] [--json]` — read the RUNNING
/// node's tip off its local rpc and print one machine-parseable line to
/// stdout:
///
/// ```text
/// height=<h> root_hash=<hex>
/// ```
///
/// `height=none` means no block has finalized yet. `--json` emits the rpc's
/// full status object (height, root_hash, every module root). requires the
/// node to be up — the same local rpc lane as `member status`.
fn cmd_node_status(args: StatusArgs) -> CommandResult {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("node status reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let reply = rpc_call(&rpc_addr, &serde_json::json!({ "cmd": "status" }))?;
    if reply["ok"] != true {
        return Err(format!("status: {}", reply["error"]).into());
    }
    let status = &reply["status"];
    if args.json {
        println!("{status}");
        return Ok(());
    }
    let height = match status["height"].as_u64() {
        Some(h) => h.to_string(),
        None => "none".into(),
    };
    let root_hash = status["root_hash"].as_str().unwrap_or("");
    println!("height={height} root_hash={root_hash}");
    // WHOSE node this is — the fact `user cred grant <account>`, `node work
    // admit <account>` and `agent sched --host-node` all stand on, and the one
    // an operator otherwise had to hand-write a `/v1/query` `OfNode` for.
    // Prose only: `--json` is the rpc's status document verbatim and stays a
    // byte-for-byte contract.
    println!(
        "{}",
        account_line(&rpc_addr, resolved.signer.public_key().as_ref())
    );
    Ok(())
}

/// The `account=` line under `node status`: this node's own account, named.
///
/// Best-effort, and quiet about its own failures on purpose: the node's tip is
/// the answer `status` promises, and a chain that cannot serve an identity read
/// must not turn a working status into an error. An UNBOUND node is not a
/// failure at all — it is the ordinary state between `node run` and
/// `user account-init`, and the only state with a next step worth printing.
///
/// `node_key` comes from the workspace this verb already resolved, NOT from the
/// rpc status document — which carries height, root hash and module roots and
/// has never named the node. Asking the running process would be the stricter
/// source, but this verb opened `identity.key` two lines up to find the rpc
/// address at all, so there is no new exposure to buy with the round trip.
fn account_line(rpc_addr: &str, node_key: &[u8]) -> String {
    let query = identity::encode_query(&identity::IdentityQuery::OfNode {
        node_key: node_key.to_vec(),
    });
    let Ok(view) = rpc_query(rpc_addr, "identity", &query)
        .and_then(|bytes| identity::decode_reply(&bytes).map_err(|e| e.to_string()))
    else {
        return "account=unknown (the identity module did not answer)".into();
    };
    let identity::IdentityReply::Account(Some(account)) = view else {
        return "account=none — claim one with: ducktape user account-init --name <you>".into();
    };
    let id = hex_bytes(&account.account_id);
    match account.display_name {
        Some(name) => format!("account={name} ({})", id.get(..16).unwrap_or(&id)),
        None => format!("account={id}"),
    }
}

/// `peers [--config <path> | -n <chain-id>] [--json]` — the RUNNING node's
/// direct-peer sample off its local rpc: one `key=value` line per peer.
/// `--json` emits one raw [`noded::peers::PeersView`] sample (cumulative
/// counters — consumers derive rates from deltas); the prose form takes a
/// second sample after one second so the line can carry live `…/s` rates.
fn cmd_node_peers(args: StatusArgs) -> CommandResult {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("node peers reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let first = peers_rpc(&rpc_addr)?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string(&first).expect("peers view serializes")
        );
        return Ok(());
    }
    if first.peers.is_empty() {
        println!("no direct peers");
        return Ok(());
    }
    // cumulative counters only become rates as a delta over time: hold one
    // second, sample again, and let the SECOND sample carry the truth.
    std::thread::sleep(std::time::Duration::from_secs(1));
    let second = peers_rpc(&rpc_addr)?;
    for peer in &second.peers {
        let baseline = first.peers.iter().find(|p| p.peer == peer.peer);
        println!("{}", peer_line(peer, baseline, &first, &second));
    }
    Ok(())
}

/// one `peers` rpc round-trip, decoded to the shared view.
fn peers_rpc(addr: &str) -> Result<noded::peers::PeersView, String> {
    let reply = rpc_call(addr, &serde_json::json!({ "cmd": "peers" }))?;
    if reply["ok"] != true {
        return Err(format!("peers: {}", reply["error"]));
    }
    serde_json::from_value(reply["peers"].clone()).map_err(|e| format!("peers reply: {e}"))
}

/// one peer's `key=value` prose line. rates appear only when the peer was
/// present in the baseline sample — a peer first seen mid-measurement has no
/// honest denominator.
fn peer_line(
    peer: &noded::peers::PeerView,
    baseline: Option<&noded::peers::PeerView>,
    first: &noded::peers::PeersView,
    second: &noded::peers::PeersView,
) -> String {
    let mut line = format!("peer={}", peer.peer);
    if let Some(role) = &peer.role {
        line.push_str(&format!(" role={role}"));
    }
    match peer.connected_since_ms {
        Some(since) => {
            let for_secs = second.sampled_at_ms.saturating_sub(since) / 1000;
            line.push_str(&format!(" connected={}", human_duration(for_secs)));
        }
        None => line.push_str(" connected=no"),
    }
    line.push_str(&format!(
        " msgs_tx={} msgs_rx={}",
        peer.msgs_sent, peer.msgs_received
    ));
    let dt_secs = (second.sampled_at_ms.saturating_sub(first.sampled_at_ms)).max(1) as f64 / 1000.0;
    if let Some(base) = baseline {
        let tx_rate = (peer.msgs_sent.saturating_sub(base.msgs_sent)) as f64 / dt_secs;
        let rx_rate = (peer.msgs_received.saturating_sub(base.msgs_received)) as f64 / dt_secs;
        line.push_str(&format!(" tx/s={tx_rate:.1} rx/s={rx_rate:.1}"));
    }
    let Some(sync) = &peer.statesync else {
        return line;
    };
    line.push_str(&format!(" sync_bytes={}", sync.bytes_tx));
    let baseline_sync = baseline.and_then(|b| b.statesync.as_ref());
    if let Some(base) = baseline_sync {
        let byte_rate = (sync.bytes_tx.saturating_sub(base.bytes_tx)) as f64 / dt_secs;
        line.push_str(&format!(" sync_B/s={byte_rate:.0}"));
    }
    if let Some(height) = sync.served_height {
        line.push_str(&format!(" sync_height={height}"));
    }
    if let Some(boundary) = sync.boundary_height {
        line.push_str(&format!(" sync_boundary={boundary}"));
    }
    line.push_str(&format!(" sync_idle={}s", sync.idle_seconds));
    if let Some(kind) = &sync.last_request_kind {
        line.push_str(&format!(" sync_last={kind}"));
    }
    line
}

/// seconds → compact `42s` / `3m12s` / `2h05m` prose.
fn human_duration(secs: u64) -> String {
    let (hours, minutes, seconds) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if hours > 0 {
        return format!("{hours}h{minutes:02}m");
    }
    if minutes > 0 {
        return format!("{minutes}m{seconds:02}s");
    }
    format!("{seconds}s")
}

// ============================================================================
// onboarding verbs — key / init / invite / admit / join.
// ============================================================================

/// `key` — generate (or reuse) a persisted ed25519 identity. pubkey on stdout
/// (scriptable); provenance on stderr. `--dir <dir>` mints (or reuses)
/// `<dir>/identity.key`, creating the dir: this is the JOIN CODE an invitee
/// hands the inviter so the invite can be locked to this key before the
/// workspace joins anything.
fn cmd_keygen(args: KeyArgs) -> Result<(), Box<dyn std::error::Error>> {
    let out = match args.dir {
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;
            dir.join("identity.key")
        }
        None => args.out.unwrap_or_else(|| PathBuf::from("identity.key")),
    };
    let (key, generated) = config::load_or_generate_identity(&out)?;
    println!("{}", hex_bytes(key.public_key().as_ref()));
    eprintln!(
        "{} identity at {}",
        if generated { "generated" } else { "reusing" },
        out.display()
    );
    Ok(())
}

/// the compute adapter as the `[sandbox]` table generation writes it (`0` =
/// probe the host at boot) plus the probeable backend, so generation and
/// detection share one choice. One adapter per OS: Firecracker on Linux, the
/// vz shim on macOS. `node init` writes the block commented out, so a host
/// that cannot run a microVM gets the probe's loud error when an operator
/// uncomments it, not a silent misconfiguration now.
fn platform_sandbox() -> (config::SandboxToml, provider_host::SandboxBackend) {
    // This only writes the default [sandbox] TOML at `node init`. The images
    // need not exist yet — `node init` runs before `ops/build-guest-rootfs.sh`
    // on a fresh box, and the loud error belongs to the boot probe, where an
    // operator who uncommented the table is standing.
    let vmm = provider_host::Vmm::platform_default();
    let guest = std::path::Path::new(config::DEFAULT_GUEST_DIR);
    let (kernel, rootfs) = (guest.join("vmlinux"), guest.join("rootfs.ext4"));
    let backend = provider_host::SandboxBackend::MicroVm {
        vmm,
        kernel: kernel.clone(),
        rootfs: rootfs.clone(),
    };
    let table = config::SandboxToml {
        runtime: vmm.config_token().into(),
        kernel,
        rootfs,
        cores: 0,
        mem_gb: 0,
    };
    (table, backend)
}

/// fresh-workspace compute detection (`init`, `join`): the platform adapter's
/// runtime binary on PATH ⇒ a live `[sandbox]` table, with a stderr note;
/// absent ⇒ `None` (today's commented example).
///
/// The table says only HOW runs would be isolated on this host — it grants
/// nothing. Whether this node runs a compute service at all is the user's
/// `ducktape service enable compute`, so detection can stay eager: it makes
/// the interactive terminal plane work out of the box and leaves the compute
/// plane dark until someone consents to it.
fn detect_platform_sandbox() -> Option<config::SandboxToml> {
    let (table, backend) = platform_sandbox();
    let Ok(runtime_path) = backend.probe() else {
        return None;
    };
    eprintln!(
        "compute plane: {} found at {} — writing a live [sandbox] table \
         (announce stays off; delete the table for a consensus-only node)",
        table.runtime,
        runtime_path.display()
    );
    Some(table)
}

/// `init --name <human name> [--dir <dir>] [--listen a] [--advertised a] [--http a]
/// [--rpc a] [--primary-coordinator host:port|none]
/// [--wireguard-listen a] [--wireguard-advertised host:port] [--invite-listen a]`
/// — found a network: mint the chain-id, write the descriptor + node config,
/// seed the genesis validator set with this identity. Every flag is optional:
/// the generated config defaults to a WORKING node (mesh `[::]:52200`,
/// overlay advertise, HTTP 8844, RPC 8845, gateway, WireGuard 51820) and
/// prints every key, so the file itself documents what to change. without
/// `--dir` the workspace lands in the registry
/// (`~/.ducktape/workspaces/<chain-id>/`), where `-n <chain-id>` finds it.
fn cmd_init(args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let name = &args.name;
    let explicit_dir = args.dir.is_some();
    // the workspace dir: `--dir` is the explicit escape hatch; the default is
    // the registry — `~/.ducktape/workspaces/<chain-id>/` — so the network is
    // addressable by `-n <chain-id>` (run/invite/list) from the moment it is
    // founded. the default dir is NAMED by the chain id, and the chain id is
    // minted from the identity pubkey, so the key is born in memory and only
    // persisted once the dir exists.
    let (dir, key, generated, chain_id) = match args.dir {
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;
            let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
            let chain_id = config::mint_chain_id(name, &key.public_key());
            (dir, key, generated, chain_id)
        }
        None => {
            let key = config::generate_identity();
            let chain_id = config::mint_chain_id(name, &key.public_key());
            let dir = config::default_workspace_dir(&chain_id)?;
            std::fs::create_dir_all(&dir)?;
            config::write_identity(&dir.join("identity.key"), &key)?;
            (dir, key, true, chain_id)
        }
    };
    // re-running init would mint a FRESH chain-id and reset the validator set
    // to just this identity — silently un-founding the network under every
    // holder of an existing invite. founding is once per directory. (the
    // registry default cannot trip this: its dir is named by the fresh id.)
    let descriptor_path = dir.join("network.toml");
    if descriptor_path.exists() {
        return Err(format!(
            "{} already exists — this directory is already a network. use `invite`/`admit` \
             for membership, or delete the file to re-found from scratch",
            descriptor_path.display()
        )
        .into());
    }
    let net = &args.plumbing;
    let primary_coordinator =
        config::primary_coordinator_or_default(net.primary_coordinator.as_deref())?;
    // node.toml is COMPLETE: merged_plumbing pins the same compiled
    // coordinator default `apply_primary_coordinator` bakes into the
    // descriptor, so the two never silently disagree (see `docs`:
    // coordinator is ambient, node-local).
    let fresh_workspace = !dir.join("node.toml").exists();
    let mut plumbing = config::merged_plumbing(
        &dir,
        net.listen.as_deref(),
        net.advertised.as_deref(),
        net.http.as_deref(),
        net.gateway.as_deref(),
        net.rpc.as_deref(),
        net.wireguard_listen.as_deref(),
        net.invite_listen.as_deref(),
        net.primary_coordinator.as_deref(),
        net.wireguard_advertised.as_deref(),
    )?;
    // a FRESH workspace detects the platform runtime and writes the table (it
    // describes HOW runs are isolated, and grants nothing); an existing
    // node.toml keeps whatever the operator chose — a deleted table is never
    // resurrected. Turning the compute plane ON is `ducktape service enable
    // compute`, never an init flag.
    if fresh_workspace {
        plumbing.sandbox = detect_platform_sandbox();
    }

    let me = key.public_key();
    let mut descriptor = config::NetworkDescriptor {
        chain_id: chain_id.clone(),
        validators: vec![hex_bytes(me.as_ref())],
        bootstrap: Vec::new(),
        reach: Vec::new(),
        coordination: None,
    };
    if let Some(addr) = config::dialable(Some(&plumbing.advertised), &plumbing.listen)? {
        descriptor.add_bootstrap(&me, &addr);
    }
    if let Some(coord) = &primary_coordinator {
        descriptor.apply_primary_coordinator(&me, coord)?;
    }
    descriptor.save(&descriptor_path)?;
    config::write_node_toml(&dir, &plumbing)?;
    eprintln!(
        "{} identity {}",
        if generated { "generated" } else { "reusing" },
        hex_bytes(me.as_ref())
    );
    eprintln!("network {chain_id} initialized in {}", dir.display());
    // a registry-default workspace is addressable by chain id; an explicit
    // --dir may live outside the registry, so its hints stay path-based.
    let selector = match explicit_dir {
        true => format!("--config {}/node.toml", dir.display()),
        false => format!("-n '{chain_id}'"),
    };
    eprintln!("start:  ducktape node run {selector}");
    eprintln!("invite: ducktape node invite {selector}");
    println!("{chain_id}");
    Ok(())
}

/// `invite [--config node.toml] [--ttl-days N]` — emit the one-line paste
/// blob: the whole join credential. minting IS the admission decision — the
/// blob carries the descriptor with THIS member's dial hint folded in (and
/// persisted, so every future invite carries it), the inviter's WireGuard
/// bootstrap when the reachability plane is configured (`wireguard_listen`),
/// an expiry, and a single-use INVITE TOKEN, the whole envelope signed by
/// this member's identity. the joiner's node redeems the token automatically
/// (governance `Redeem`) — no member approval step follows. an invite grants
/// RESIDENT standing only; submitting ops needs no invite at all.
fn cmd_invite(args: InviteArgs) -> Result<(), Box<dyn std::error::Error>> {
    // every invite is BEARER (the targeted form was dropped — see the join
    // ADR): there is no `--target` — whoever redeems the single-use token
    // first wins. the invite is the admission credential itself, kept off the
    // wire by the sealed first-contact intro.
    let ttl_days: u64 = match args.ttl_days {
        Some(v) => v,
        // the operator-friendly onboarding default (a LOST blob is the residual
        // risk — single-use + sealing cover interception).
        None => config::DEFAULT_INVITE_TTL_DAYS,
    };
    if ttl_days == 0 {
        return Err("--ttl-days must be at least 1".into());
    }
    let cfg_path = args.selector.config_path()?;
    let (raw, base) = config::load_node_toml(&cfg_path)?;
    let descriptor_path = base.join(&raw.network);
    let mut descriptor = config::NetworkDescriptor::load(&descriptor_path)?;
    let key = config::load_identity(&base.join(&raw.key_file))?;
    let dial_hint = config::dialable(Some(&raw.advertised), &raw.listen)?;
    let has_coordinated_reach = descriptor.has_coordinated_reach()?;
    if let Some(addr) = &dial_hint {
        descriptor.add_bootstrap(&key.public_key(), addr);
    }
    descriptor.save(&descriptor_path)?;

    // the WireGuard bootstrap: endpoints are minted from the advertised host
    // (the listen IP is usually unspecified) + the plane's UDP ports; the
    // mesh port is where the joiner dials this member's overlay ULA once the
    // tunnel routes. the bootstrap is mandatory (the overlay plane
    // carries the data planes and the sealed first-contact intro) — and the
    // network shape always runs the plane (`wireguard_listen` is required).
    let wg_listen: std::net::SocketAddr = raw
        .wireguard_listen
        .parse()
        .map_err(|e| format!("wireguard_listen: {e}"))?;
    let wireguard = {
        let (wg_keypair, _) =
            reachability::WireGuardKeypair::load_or_generate(&base.join("wireguard.key"))
                .map_err(|e| format!("wireguard key: {e}"))?;
        let mesh_port: u16 = raw
            .listen
            .parse::<std::net::SocketAddr>()
            .map(|a| a.port())
            .map_err(|e| format!("listen {:?}: {e}", raw.listen))?;
        let host = match config::endpoint_host(
            Some(&raw.advertised),
            &raw.listen,
            wg_listen,
            raw.wireguard_advertised_value(),
        ) {
            Ok(host) => Some(host),
            Err(_) if has_coordinated_reach => {
                // Coordinated reach gives the joiner a rendezvous path; there is
                // deliberately no inviter-hosted underlay endpoint to bake in.
                None
            }
            Err(err) => return Err(err.into()),
        };
        match host {
            Some(host) => {
                let intro_port =
                    config::resolved_invite_listen(Some(&raw.invite_listen), wg_listen)?.port();
                // the tunnel endpoint carries the FULL advertised host:port when
                // `wireguard_advertised` is configured — the external port can
                // differ from the bind port in the port-forwarded setup the key
                // exists for. The intro stays host + intro port.
                let endpoint = config::invite_wireguard_endpoint(
                    Some(&raw.advertised),
                    &raw.listen,
                    wg_listen,
                    raw.wireguard_advertised_value(),
                )?;
                config::InviteWireGuard {
                    public_key: wg_keypair.public_key().0,
                    endpoint: Some(endpoint),
                    intro: Some(format!("{host}:{intro_port}")),
                    mesh_port,
                }
            }
            None => config::InviteWireGuard {
                public_key: wg_keypair.public_key().0,
                endpoint: None,
                intro: None,
                mesh_port,
            },
        }
    };

    // the fronts: every reachable member the inviter already meshes with, read
    // from the persisted mesh state so a joiner can bring its tunnel up against
    // ANY of them, not just the inviter (the unified all-paths invite). A
    // host-capable member rides as a direct front, a NAT'd-but-registered one
    // as a coordinated (by-identity) front. No mesh state yet → no fronts.
    let storage = base.join(&raw.storage_dir);
    let mesh_state_file = storage.join("mesh-state.json");
    let chain_id = descriptor.genesis_namespace();
    let own: [u8; 32] = key
        .public_key()
        .as_ref()
        .try_into()
        .expect("ed25519 public key is 32 bytes");
    let fronts = match reachability::store::load(&mesh_state_file, &chain_id) {
        Ok(Some(mesh)) => {
            let fronts = config::fronts_from_adverts(&mesh.adverts, &own);
            if fronts.is_empty() {
                eprintln!(
                    "[invite] persisted mesh at {} holds no other members — the invite \
                     carries only the inviter's own paths",
                    mesh_state_file.display()
                );
            }
            fronts
        }
        Ok(None) => {
            eprintln!(
                "[invite] no persisted mesh state at {} — the invite carries no member \
                 fronts (only the inviter's own paths); mint again once the mesh has peers",
                mesh_state_file.display()
            );
            Vec::new()
        }
        Err(e) => {
            eprintln!(
                "[invite] mesh state at {} unreadable ({e}) — the invite carries no member \
                 fronts",
                mesh_state_file.display()
            );
            Vec::new()
        }
    };

    // stop embedding a coordinator address in the invite: the joiner reaches
    // every path through its OWN ambient coordinator (config/default), never a
    // coordinator baked into the blob. The inviter still registers with its own
    // coordinator via its own config; here we only strip Coordinated reach
    // hints from the ENCODED copy — the on-disk descriptor keeps its config.
    let mut invite_descriptor = descriptor.clone();
    invite_descriptor
        .reach
        .retain(|hint| !hint.trim_start().starts_with("coordinated:"));

    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock is past the epoch")
        .as_secs()
        + ttl_days * 24 * 60 * 60;
    // the expiry lives INSIDE the token (signed), not as a separate blob field.
    // every invite is bearer.
    let token = config::mint_invite_token(&key, descriptor.genesis_namespace().as_bytes(), expires);
    let blob_string = config::encode_invite(&invite_descriptor, &token, &wireguard, &fronts, &key)?;
    println!("{blob_string}");
    Ok(())
}

/// `admit <hex pubkey> [--config node.toml]` — pre-genesis membership: add an
/// identity to the descriptor's validator set. once the network has state,
/// membership changes go through governance (AddValidator), not genesis edits.
fn cmd_admit(args: AdmitArgs) -> Result<(), Box<dyn std::error::Error>> {
    let pubkey_hex = &args.pubkey;
    let key = config::decode_key(pubkey_hex)?;
    let cfg_path = args.selector.config_path()?;
    let (raw, base) = config::load_node_toml(&cfg_path)?;
    let storage = base.join(&raw.storage_dir);
    if storage.exists() {
        return Err(format!(
            "{} already has state — a running network admits members via governance \
             (AddValidator), not by editing genesis",
            storage.display()
        )
        .into());
    }
    let descriptor_path = base.join(&raw.network);
    let mut descriptor = config::NetworkDescriptor::load(&descriptor_path)?;
    descriptor.admit(&key);
    descriptor.save(&descriptor_path)?;
    eprintln!("admitted {pubkey_hex} into {}", descriptor.chain_id);
    eprintln!(
        "re-run `ducktape node invite` and share the REFRESHED invite — genesis must be \
         identical on every member"
    );
    Ok(())
}

// ---- resident accept: post-genesis admission over the local rpc -----------

/// one blocking json-lines rpc round-trip against the LOCAL node.
fn rpc_call(addr: &str, req: &serde_json::Value) -> Result<serde_json::Value, String> {
    use std::io::{BufRead as _, BufReader, Write as _};
    // the same calm sentence the http lane gives, for the same condition: an
    // `os error 111` with a port in it is a diagnosis nobody asked for.
    let conn = std::net::TcpStream::connect(addr).map_err(|error| match error.kind() {
        std::io::ErrorKind::ConnectionRefused => crate::node_http::NODE_NOT_RUNNING.to_string(),
        _ => format!("cannot reach this node's operator rpc on {addr}: {error}"),
    })?;
    conn.set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .map_err(|e| format!("rpc timeout: {e}"))?;
    let mut writer = conn.try_clone().map_err(|e| format!("rpc clone: {e}"))?;
    let mut line = serde_json::to_string(req).expect("rpc request serializes");
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .map_err(|e| format!("rpc write: {e}"))?;
    let mut reply = String::new();
    BufReader::new(conn)
        .read_line(&mut reply)
        .map_err(|e| format!("rpc read: {e}"))?;
    serde_json::from_str(reply.trim()).map_err(|e| format!("rpc reply: {e}"))
}

/// query a module through the rpc; the reply's hex payload, decoded.
fn rpc_query(addr: &str, target: &str, req: &[u8]) -> Result<Vec<u8>, String> {
    let reply = rpc_call(
        addr,
        &serde_json::json!({ "cmd": "query", "target": target, "req_hex": hex_bytes(req) }),
    )?;
    if reply["ok"] != true {
        return Err(format!("query {target}: {}", reply["error"]));
    }
    unhex(
        reply["reply_hex"]
            .as_str()
            .ok_or("query reply carries no payload")?,
    )
}

/// submit an op through the rpc (accepted != finalized — poll afterwards).
fn rpc_submit(addr: &str, target: &str, payload: &[u8]) -> Result<(), String> {
    let reply = rpc_call(
        addr,
        &serde_json::json!({ "cmd": "submit", "target": target, "payload_hex": hex_bytes(payload) }),
    )?;
    if reply["ok"] != true {
        return Err(format!("submit to {target}: {}", reply["error"]));
    }
    Ok(())
}

fn read_members(addr: &str) -> Result<Vec<Vec<u8>>, String> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let raw = rpc_query(addr, "valset", &encode_query(&ValsetQuery::Validators))?;
    match decode_reply(&raw)? {
        ValsetReply::Validators(v) => Ok(v),
        other => Err(format!("expected Validators, got {other:?}")),
    }
}

fn read_residents(addr: &str) -> Result<Vec<Vec<u8>>, String> {
    use valset::{ValsetQuery, ValsetReply, decode_reply, encode_query};
    let raw = rpc_query(addr, "valset", &encode_query(&ValsetQuery::Residents))?;
    match decode_reply(&raw)? {
        ValsetReply::Residents(v) => Ok(v),
        other => Err(format!("expected Residents, got {other:?}")),
    }
}

fn account_of_node(addr: &str, node_key: &[u8]) -> Result<Option<Vec<u8>>, String> {
    use identity::{IdentityQuery, IdentityReply, decode_reply, encode_query};
    let raw = rpc_query(
        addr,
        "identity",
        &encode_query(&IdentityQuery::OfNode {
            node_key: node_key.to_vec(),
        }),
    )?;
    match decode_reply(&raw)? {
        IdentityReply::Account(account) => Ok(account.map(|account| account.account_id)),
        other => Err(format!("expected Account, got {other:?}")),
    }
}

fn proposal_principal(
    addr: &str,
    proposal: &governance::ProposalView,
    node_key: &[u8],
) -> Result<Vec<u8>, String> {
    match proposal.voter_kind {
        governance::VoterKind::ValidatorNode => Ok(node_key.to_vec()),
        governance::VoterKind::Account => account_of_node(addr, node_key)?
            .ok_or_else(|| "this node is not bound to an Identity account".into()),
    }
}

fn proposal_progress(proposal: &governance::ProposalView, members: &[Vec<u8>]) -> (u64, u64, bool) {
    let powers: std::collections::BTreeMap<&[u8], u64> = if proposal.electorate.is_empty() {
        members
            .iter()
            .map(|member| (member.as_slice(), 1))
            .collect()
    } else {
        proposal
            .electorate
            .iter()
            .map(|(principal, power)| (principal.as_slice(), *power))
            .collect()
    };
    let mut yes = 0u64;
    let mut no = 0u64;
    for (voter, approve) in &proposal.votes {
        let power = powers.get(voter.as_slice()).copied().unwrap_or(0);
        if *approve {
            yes += power;
        } else {
            no += power;
        }
    }
    let total: u64 = powers.values().sum();
    match proposal.voting_rule {
        governance::VotingRule::Threshold { required_yes } => {
            (yes, required_yes, yes >= required_yes)
        }
        governance::VotingRule::ParticipatingMajority { quorum } => {
            let ready = yes + no >= quorum && yes > total - yes;
            (yes, quorum, ready)
        }
    }
}

/// `join requests [--config node.toml]` — the verified join announces parked
/// joiners delivered to THIS member's running node, as one JSON array on
/// stdout (machine-parseable — the app's members view renders it). approving
/// is a separate, deliberate act: `resident accept <joiner>` (or the app's
/// approve button) casts this account's governance ballot; the proposal's
/// frozen rule decides admission.
fn cmd_join_requests(args: SelectorArgs) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("join requests reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let reply = rpc_call(&addr, &serde_json::json!({ "cmd": "join_requests" }))?;
    if reply["ok"] != true {
        return Err(format!("join requests: {}", reply["error"]).into());
    }
    println!(
        "{}",
        reply
            .get("join_requests")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]))
    );
    Ok(())
}

/// `join state [--config node.toml]` — the node's AUTHORITATIVE onboarding
/// phase over its local rpc: `parked | admitted | synced | promoted`, derived
/// from committed standing (not log markers), so it is restart-proof. the
/// desktop app reads this instead of parsing daemon.log, which loses the
/// admission markers across a restart and mis-reads a re-syncing resident as
/// unjoined. prints the `join_state` projection as JSON.
fn cmd_join_state(args: SelectorArgs) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("join state reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let reply = rpc_call(&addr, &serde_json::json!({ "cmd": "join_state" }))?;
    if reply["ok"] != true {
        return Err(format!("join state: {}", reply["error"]).into());
    }
    println!(
        "{}",
        reply
            .get("join_state")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    );
    Ok(())
}

fn read_proposal(addr: &str, id: &str) -> Result<Option<governance::ProposalView>, String> {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let raw = rpc_query(
        addr,
        "governance",
        &encode_query(&GovQuery::Proposal {
            proposal_id: id.into(),
        }),
    )?;
    match decode_reply(&raw)? {
        GovReply::Proposal(view) => Ok(view),
        other => Err(format!("unexpected governance reply: {other:?}")),
    }
}

/// poll a proposal until `pred` accepts its view, ~30s budget (ops finalize
/// within a few pump ticks; the budget covers a mesh still forming quorum).
fn poll_proposal(
    addr: &str,
    id: &str,
    what: &str,
    mut pred: impl FnMut(&Option<governance::ProposalView>) -> bool,
) -> Result<Option<governance::ProposalView>, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let view = read_proposal(addr, id)?;
        if pred(&view) {
            return Ok(view);
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for {what}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

fn cast_yes_once(
    addr: &str,
    proposal_id: &str,
    opened: governance::ProposalView,
    principal: &[u8],
) -> Result<governance::ProposalView, String> {
    use governance::{GovMsg, ProposalStatus, encode_msg};

    if opened.status != ProposalStatus::Open {
        return Ok(opened);
    }
    if opened
        .votes
        .iter()
        .any(|(voter, yes)| voter == principal && *yes)
    {
        eprintln!("ballot already cast as {}", hex_bytes(principal));
        return Ok(opened);
    }
    rpc_submit(
        addr,
        "governance",
        &encode_msg(&GovMsg::Vote {
            proposal_id: proposal_id.into(),
            approve: true,
        }),
    )?;
    let proposal = poll_proposal(addr, proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|proposal| {
            proposal.status != ProposalStatus::Open
                || proposal
                    .votes
                    .iter()
                    .any(|(voter, yes)| voter == principal && *yes)
        })
    })?
    .ok_or_else(|| format!("proposal {proposal_id} disappeared"))?;
    eprintln!("ballot cast as {}", hex_bytes(principal));
    Ok(proposal)
}

/// how a driven membership ceremony left the proposal.
enum CeremonyOutcome {
    /// passed and executed — the set changes at the next epoch cutover.
    Passed,
    /// this ballot landed but the proposal's frozen threshold is outstanding.
    AwaitingBallots,
}

/// drive a governance membership ceremony for `wanted` through this eligible
/// account's running node: adopt an existing OPEN proposal
/// for exactly this action (else mint an unused `<id_prefix><key>:<n>` id and
/// propose), cast a yes ballot, and execute once decidable. idempotent across
/// members — each runs the same verb; the run landing the deciding ballot
/// executes. shared by `resident accept` (AddResident), `member promote`
/// (AddValidator), and `resident remove` (RemoveResident).
fn drive_membership_ceremony(
    rpc_addr: &str,
    me_bytes: &[u8],
    pubkey_hex: &str,
    verb: &str,
    id_prefix: &str,
    wanted: governance::GovAction,
) -> Result<CeremonyOutcome, Box<dyn std::error::Error>> {
    use governance::{GovMsg, ProposalStatus, encode_msg};
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let proposals = match decode_reply(&rpc_query(
        rpc_addr,
        "governance",
        &encode_query(&GovQuery::Proposals),
    )?)? {
        GovReply::Proposals(views) => views,
        other => return Err(format!("unexpected governance reply: {other:?}").into()),
    };
    let proposal_id = match proposals
        .iter()
        .find(|p| p.status == ProposalStatus::Open && p.action == wanted)
    {
        Some(p) => {
            eprintln!("joining open proposal {}", p.proposal_id);
            p.proposal_id.clone()
        }
        None => {
            let prefix: String = pubkey_hex.chars().take(16).collect();
            let id = (0u64..)
                .map(|n| format!("{id_prefix}{prefix}:{n}"))
                .find(|id| !proposals.iter().any(|p| &p.proposal_id == id))
                .expect("the id space is unbounded");
            rpc_submit(
                rpc_addr,
                "governance",
                &encode_msg(&GovMsg::Propose {
                    proposal_id: id.clone(),
                    action: wanted,
                    // a far horizon in consensus-time units (heights advance
                    // about one per finalized op): admission must not expire
                    // under a slow second ballot.
                    voting_period: 1_000_000,
                }),
            )?;
            poll_proposal(rpc_addr, &id, "the proposal to finalize", |p| p.is_some())?;
            eprintln!("proposed {id}");
            id
        }
    };

    let opened = read_proposal(rpc_addr, &proposal_id)?
        .ok_or_else(|| format!("proposal {proposal_id} disappeared"))?;
    let principal = proposal_principal(rpc_addr, &opened, me_bytes)?;
    let after_vote = cast_yes_once(rpc_addr, &proposal_id, opened, &principal)?;

    // Execute only when the proposal's frozen rule says the yes power is
    // irreversible. A shortfall is the normal intermediate state, not an error.
    let members = read_members(rpc_addr)?;
    let (yes, required, ready) = proposal_progress(&after_vote, &members);
    if after_vote.status == ProposalStatus::Open && !ready {
        eprintln!(
            "{yes} of {required} required voting power — waiting on other voters. each runs:\n    \
             ducktape {verb} {pubkey_hex} --config <their node.toml>"
        );
        // `verb` is the full two-token spelling (`node resident accept`, ...)
        // so the guidance reads `ducktape node resident accept <hex>`.
        return Ok(CeremonyOutcome::AwaitingBallots);
    }
    if after_vote.status == ProposalStatus::Open {
        rpc_submit(
            rpc_addr,
            "governance",
            &encode_msg(&GovMsg::Execute {
                proposal_id: proposal_id.clone(),
            }),
        )?;
    }
    let settled = poll_proposal(rpc_addr, &proposal_id, "the tally to settle", |p| {
        p.as_ref().is_some_and(|v| v.status != ProposalStatus::Open)
    })?
    .expect("the poll only accepts a present proposal");
    match settled.status {
        ProposalStatus::Passed => Ok(CeremonyOutcome::Passed),
        status => Err(format!("proposal {proposal_id} settled as {status:?}").into()),
    }
}

/// `resident accept <hex pubkey> [--config node.toml]` — approve a join request
/// as RESIDENT standing (the staged-admission tier): drive a governance
/// AddResident proposal for `pubkey` through this account's own RUNNING node.
/// the passing proposal's valset Grant schedules the epoch cutover that
/// admits the key to the mesh, at which point its parked node PRE-SYNCS
/// state on a stride cadence. promotion into the quorum is the separate,
/// deliberate `member promote` verb — run it once the resident is warm.
fn cmd_invite_accept(args: PubkeyArgs) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let pubkey_hex = &args.pubkey;
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = args.selector.config_path()?;
    // Full config resolution derives the same node identity the daemon signs
    // with; governance resolves it to an account when shares are active.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("resident accept drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is already a validator — nothing to do");
        return Ok(());
    }
    if read_residents(&rpc_addr)?.contains(&key_bytes) {
        eprintln!(
            "{pubkey_hex} already holds resident standing — promote with \
             `ducktape node member promote {pubkey_hex}` once it is synced"
        );
        return Ok(());
    }
    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "node resident accept",
        "resident:",
        GovAction::AddResident { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "granted resident standing to {pubkey_hex}: the mesh admits it at the next \
                 epoch cutover and its parked node pre-syncs state. promote it into the \
                 quorum once warm:\n    ducktape node member promote {pubkey_hex}"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

/// `member promote <hex pubkey> [--config node.toml]` — seat a key in the
/// consensus quorum: drive a governance AddValidator proposal through this
/// account's own RUNNING node. the passing proposal's valset Join clears any
/// resident standing in the same block and schedules the epoch cutover; a
/// pre-synced resident then catches up a small delta and reboots as a
/// validator, so the quorum only ever gains a warm member. also serves DIRECT
/// (un-staged) admission — exactly the pre-resident `resident accept` semantics.
fn cmd_promote(args: PubkeyArgs) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let pubkey_hex = &args.pubkey;
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("promote drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is already a validator — nothing to do");
        return Ok(());
    }
    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "node member promote",
        "admit:",
        GovAction::AddValidator { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "admitted {pubkey_hex}: the next epoch cutover seats it in the consensus \
                 quorum, and the chain PAUSES at that cutover until its node seats itself \
                 and votes. a warm (pre-synced) resident seats in-process from its own \
                 folded state within moments; a cold node first syncs the frozen boundary. \
                 watch its log for `promoted: validator at epoch …; seating in-process`"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

/// `resident remove <hex pubkey> [--config node.toml]` — revoke resident
/// standing: drive a governance RemoveResident proposal through this account's
/// own RUNNING node. the mirror of `resident accept` with inverted guards — a
/// no-op when the key holds no resident standing, and only the governance
/// electorate may drive it. the passing proposal's valset Revoke schedules the
/// epoch cutover that drops the key from the mesh; its node falls back to a
/// parked joiner, and `resident accept` re-grants. a seated validator is
/// `member remove`'s job — standing never overlaps (Grant refuses validators,
/// Join clears standing).
fn cmd_resident_remove(args: PubkeyArgs) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let pubkey_hex = &args.pubkey;
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = args.selector.config_path()?;
    // Full config resolution derives the same node identity the daemon signs
    // with; governance resolves it to an account when shares are active.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("resident remove drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!(
            "{pubkey_hex} is a seated validator, not a resident — remove it with \
             `ducktape node member remove {pubkey_hex}`"
        );
        return Ok(());
    }
    if !read_residents(&rpc_addr)?.contains(&key_bytes) {
        eprintln!("{pubkey_hex} holds no resident standing — nothing to do");
        return Ok(());
    }
    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "node resident remove",
        "revoke:",
        GovAction::RemoveResident { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "revoked resident standing from {pubkey_hex}: the mesh drops it at the next \
                 epoch cutover and its node parks again. a member re-grants with:\n    \
                 ducktape node resident accept {pubkey_hex}"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

// ---- member remove: post-genesis removal over the local rpc ---------------

/// `member remove <hex pubkey> [--config node.toml]` — post-genesis removal:
/// drive a governance RemoveValidator proposal for `pubkey` through this
/// account's own RUNNING node. the mirror of `resident accept` with inverted
/// guards — a no-op when the key is NOT a member, and only the governance
/// electorate may drive it. idempotent across voters: each runs the same
/// command (propose if absent, cast a yes ballot, execute once decidable); the
/// run that lands the deciding ballot executes. the passing proposal's valset
/// Leave schedules the epoch cutover that drops the key from the tracked set.
fn cmd_member_remove(args: PubkeyArgs) -> Result<(), Box<dyn std::error::Error>> {
    use governance::{GovAction, GovMsg, ProposalStatus, encode_msg};

    let pubkey_hex = &args.pubkey;
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = args.selector.config_path()?;
    // Full config resolution derives the same node identity the daemon signs
    // with; governance resolves it to an account when shares are active.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("member remove drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    // Inverted admission guard: nothing to remove if the key is not a member.
    if !members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is not a validator — nothing to do");
        return Ok(());
    }
    // adopt an existing OPEN proposal for exactly this action, else mint an
    // unused id (settled proposals keep their ids forever — a re-removed key
    // gets a fresh suffix).
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let proposals = match decode_reply(&rpc_query(
        &rpc_addr,
        "governance",
        &encode_query(&GovQuery::Proposals),
    )?)? {
        GovReply::Proposals(views) => views,
        other => return Err(format!("unexpected governance reply: {other:?}").into()),
    };
    let wanted = GovAction::RemoveValidator {
        key: key_bytes.clone(),
    };
    let proposal_id = match proposals
        .iter()
        .find(|p| p.status == ProposalStatus::Open && p.action == wanted)
    {
        Some(p) => {
            eprintln!("joining open proposal {}", p.proposal_id);
            p.proposal_id.clone()
        }
        None => {
            let prefix: String = pubkey_hex.chars().take(16).collect();
            let id = (0u64..)
                .map(|n| format!("remove:{prefix}:{n}"))
                .find(|id| !proposals.iter().any(|p| &p.proposal_id == id))
                .expect("the id space is unbounded");
            rpc_submit(
                &rpc_addr,
                "governance",
                &encode_msg(&GovMsg::Propose {
                    proposal_id: id.clone(),
                    action: wanted,
                    // a far horizon in consensus-time units: removal must not
                    // expire under a slow second ballot.
                    voting_period: 1_000_000,
                }),
            )?;
            poll_proposal(&rpc_addr, &id, "the proposal to finalize", |p| p.is_some())?;
            eprintln!("proposed {id}");
            id
        }
    };

    let opened = read_proposal(&rpc_addr, &proposal_id)?
        .ok_or_else(|| format!("proposal {proposal_id} disappeared"))?;
    let principal = proposal_principal(&rpc_addr, &opened, &me_bytes)?;
    let after_vote = cast_yes_once(&rpc_addr, &proposal_id, opened, &principal)?;

    // Execute only once the proposal's own frozen voting rule is satisfied.
    let members = read_members(&rpc_addr)?;
    let (yes, required, ready) = proposal_progress(&after_vote, &members);
    if after_vote.status == ProposalStatus::Open && !ready {
        eprintln!(
            "{yes} of {required} required voting power — waiting on other voters. each runs:\n    \
             ducktape node member remove {pubkey_hex} --config <their node.toml>"
        );
        return Ok(());
    }
    if after_vote.status == ProposalStatus::Open {
        rpc_submit(
            &rpc_addr,
            "governance",
            &encode_msg(&GovMsg::Execute {
                proposal_id: proposal_id.clone(),
            }),
        )?;
    }
    let settled = poll_proposal(&rpc_addr, &proposal_id, "the tally to settle", |p| {
        p.as_ref().is_some_and(|v| v.status != ProposalStatus::Open)
    })?
    .expect("the poll only accepts a present proposal");
    match settled.status {
        ProposalStatus::Passed => {
            eprintln!("removed {pubkey_hex}: the validator set changes at the next epoch cutover");
            Ok(())
        }
        status => Err(format!("proposal {proposal_id} settled as {status:?}").into()),
    }
}

// ---- member leave: this node drives its OWN removal from the set ----------

/// `member leave [--config node.toml]` — a member drives its OWN removal:
/// resolve this node's identity and route it through the EXACT SAME governance
/// path as `member remove` (a RemoveValidator proposal targeting self). there
/// is no separate governance logic — it hands off to [`cmd_member_remove`] with
/// this node's own pubkey.
///
/// honesty: leaving is NOT unilateral when this account lacks the proposal's
/// required power. This casts only its account ballot, and member remove
/// prints the remaining threshold plus the command
/// other voters run (`member remove <this key>`).
fn cmd_member_leave(args: SelectorArgs) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = args.selector.config_path()?;
    // resolve the running node's identity — the key it signs ballots with, and
    // the one this verb submits for removal.
    let resolved = config::resolve(&cfg_path)?;
    let me_hex = hex_bytes(resolved.signer.public_key().as_ref());
    eprintln!("leaving the network: opening a self-removal for {me_hex}");
    // delegate to member remove targeting SELF — same propose+vote+execute
    // path, same strict-majority honesty, same selector.
    cmd_member_remove(PubkeyArgs {
        pubkey: me_hex,
        selector: args.selector,
    })
}

// ---- member status: is THIS node still in the validator set? --------------

/// `member status [--config <path> | -n <chain-id>] [--json]` — read this
/// node's OWN membership off its RUNNING node's rpc and print one
/// machine-parseable line to stdout:
///
/// ```text
/// in-set=<true|false> validators=<count>
/// ```
///
/// `--json` emits the same two facts as `{"in_set": <bool>, "validators": <n>}`.
///
/// this is the read the desktop shell consults before FORGETTING a workspace
/// (stop + delete): tearing a node down while it is still a current validator of
/// a set of two-or-more strands its pending removal and halts quorum (a live
/// network still needs its signature). the shell refuses a forget when
/// `in-set=true` and `validators>=2`; a lone validator (`validators=1`) or an
/// already-removed key (`in-set=false`) is safe to forget. requires the node to
/// be up (it serves this over the same local rpc as `member remove`).
fn cmd_member_status(args: StatusArgs) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("member status reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();
    let members = read_members(&rpc_addr)?;
    let in_set = members.contains(&me_bytes);
    let validators = members.len();
    if args.json {
        println!(
            "{}",
            serde_json::json!({ "in_set": in_set, "validators": validators })
        );
        return Ok(());
    }
    println!("in-set={in_set} validators={validators}");
    Ok(())
}

/// `join <invite blob> [--dir <dir>] [--listen a] [--advertised a] [--http a]
/// [--rpc a] [--wireguard-listen a] [--wireguard-advertised host:port]
/// [--invite-listen a]
/// [--primary-coordinator host:port|none]` — materialize a workspace
/// from an invite: descriptor + identity (kept across re-joins) + node
/// config, defaulting into the registry dir named by the invite's chain id.
/// prints this identity for the inviter's pre-genesis `admit`.
/// `--primary-coordinator` is node-local plumbing ONLY — it never touches
/// the invite or the joined descriptor (the coordinator is always ambient,
/// per docs/superpowers/specs/2026-07-08-fully-nated-inviter-design.md).
fn cmd_join(args: JoinCmd) -> Result<(), Box<dyn std::error::Error>> {
    let blob = args
        .blob
        .as_deref()
        .ok_or("join needs an invite blob (or a `requests`/`state` subcommand)")?;
    let invite = config::decode_invite(blob)?;
    let mut descriptor = invite.descriptor.clone();
    let explicit_dir = args.dir.is_some();
    // same default as `init`: without `--dir` the workspace materializes in
    // the registry under its chain id (known here from the invite), so the
    // joined node is `-n <chain-id>`-addressable. a re-join for the same
    // chain lands in the same dir and reuses its identity, as before.
    let dir = match args.dir {
        Some(dir) => dir,
        None => config::default_workspace_dir(&descriptor.chain_id)?,
    };
    std::fs::create_dir_all(&dir)?;
    // mint (or reuse) this workspace dir's identity. Every invite is bearer
    // (invites are bearer credentials): there is no target to match, so any freshly minted
    // key may redeem — the OOB "hand the inviter your join code first" step is
    // gone. The redeeming key is bound by the join proof and the token is
    // single-use, so a paste simply admits whoever runs it.
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me_hex = hex_bytes(key.public_key().as_ref());
    config::guard_join_descriptor(&dir, &descriptor)?;
    // plumbing merges: explicit flags win, an existing node.toml's values
    // (network shape only — a dev-seed or incomplete file aborts) survive,
    // working defaults fill the rest. computed BEFORE anything lands on disk
    // so a corrupt existing node.toml aborts the join without leaving a
    // partially written dir.
    let net = &args.plumbing;
    let fresh_workspace = !dir.join("node.toml").exists();
    let mut plumbing = config::merged_plumbing(
        &dir,
        net.listen.as_deref(),
        net.advertised.as_deref(),
        net.http.as_deref(),
        net.gateway.as_deref(),
        net.rpc.as_deref(),
        net.wireguard_listen.as_deref(),
        net.invite_listen.as_deref(),
        net.primary_coordinator.as_deref(),
        net.wireguard_advertised.as_deref(),
    )?;
    // a FRESH joining workspace gets the same compute detection as init: the
    // platform runtime on PATH ⇒ a live [sandbox] table (announce stays off),
    // so agent runs and the terminal plane work without a config edit. a
    // re-join over an existing node.toml keeps the operator's choice.
    if fresh_workspace {
        plumbing.sandbox = detect_platform_sandbox();
    }
    if config::invite_requires_reachability_defaults(&invite) {
        // a WireGuard or Coordinated invite makes the reachability plane the
        // dial path: fold the inviter's (and every offered front's) overlay
        // ULA into this joiner's reach hints so the mesh can dial them the
        // moment a tunnel is up.
        {
            let wg = &invite.wireguard;
            let issuer_identity =
                wireguard::ValidatorIdentity::try_from(invite.token.issuer.as_ref())
                    .map_err(|e| format!("inviter identity: {e:?}"))?;
            let inviter_ula =
                wireguard::ula_v6_member_addr(&descriptor.genesis_namespace(), issuer_identity);
            descriptor.add_reach_route(&config::ReachHint {
                expected_key: invite.token.issuer.clone(),
                reach: config::Reach::Direct(format!("[{inviter_ula}]:{}", wg.mesh_port)),
            });
        }
        for front in &invite.fronts {
            let Ok(member) = ed25519::PublicKey::decode(&front.member_key[..]) else {
                continue;
            };
            let Ok(identity) = wireguard::ValidatorIdentity::try_from(&front.member_key[..]) else {
                continue;
            };
            let ula = wireguard::ula_v6_member_addr(&descriptor.genesis_namespace(), identity);
            descriptor.add_reach_route(&config::ReachHint {
                expected_key: member,
                reach: config::Reach::Direct(format!("[{ula}]:{}", front.mesh_port)),
            });
        }
    }
    descriptor.save(&dir.join("network.toml"))?;
    // identity was minted + target-checked at the top of the join.
    config::write_node_toml(&dir, &plumbing)?;
    // the capability the joining node redeems automatically; a re-join with a
    // fresh invite replaces a stale/spent one.
    config::save_invite_token(&dir, &invite.token)?;
    // the offered fronts, kept beside the token so `run_node` can race the
    // whole union of first-contact paths. Empty clears any stale set.
    config::save_invite_fronts(&dir, &invite.fronts)?;
    {
        // the tunnel bootstrap the joining node dials BEFORE any p2p (always
        // present); kept beside the token so `run_node` brings the
        // interface up first.
        config::save_invite_wireguard(&dir, &invite.token.issuer, &invite.wireguard)?;
        // mint the WireGuard identity NOW so the run's plane and intro
        // announcer read one settled key file instead of racing to create it.
        reachability::WireGuardKeypair::load_or_generate(&dir.join("wireguard.key"))
            .map_err(|e| format!("wireguard key: {e}"))?;
    }
    eprintln!(
        "{} identity {me_hex}",
        if generated { "generated" } else { "reusing" }
    );
    eprintln!(
        "workspace for {} written to {}",
        descriptor.chain_id,
        dir.display()
    );
    let selector = match explicit_dir {
        true => format!("--config {}/node.toml", dir.display()),
        false => format!("-n '{}'", descriptor.chain_id),
    };
    if descriptor.validators.contains(&me_hex) {
        eprintln!("this identity is a member — start: ducktape node run {selector}");
    } else {
        eprintln!(
            "NOT yet a member. start now — `ducktape node run {selector}` redeems \
             this invite automatically: the node joins the network's VPN, syncs state, and \
             comes up as a full node. no approval step follows (minting the invite WAS the \
             approval); a member can later promote it into the quorum with \
             `ducktape node member promote {me_hex}`."
        );
    }
    println!("{me_hex}");
    Ok(())
}

#[cfg(test)]
mod sandbox_detection_tests {
    use crate::config;

    /// The table `node init` would write and the backend the probe would test
    /// must name ONE runtime and ONE pair of images. They are produced together
    /// by [`super::platform_sandbox`] precisely so a host can never be probed
    /// for one thing and configured for another — a drift that would surface as
    /// a boot error on a machine whose images are exactly where init said.
    ///
    /// This is the hermetic half of detection. The other half — a live table
    /// actually landing in node.toml — is not fakeable and must not be: the
    /// probe opens `/dev/kvm` and stats both images, so it answers about the
    /// real host. `workspace_registry_cli` pins the outcome an unprovisioned
    /// host gets.
    #[test]
    fn the_written_table_and_the_probed_backend_name_one_runtime() {
        let (table, backend) = super::platform_sandbox();
        let (vmm, kernel, rootfs) = match &backend {
            provider_host::SandboxBackend::MicroVm {
                vmm,
                kernel,
                rootfs,
            } => (vmm, kernel, rootfs),
            // `Bare` exists only when provider-host is built with its testkit
            // feature, which cargo's feature unification can switch on from
            // another crate in the same invocation. So this arm has to exist
            // without being required — hence the allow, not a `#[cfg]` (the
            // feature belongs to a DIFFERENT crate and is not nameable here).
            #[allow(unreachable_patterns)]
            other => panic!("the platform adapter is a microVM, got {other:?}"),
        };
        assert_eq!(table.runtime, vmm.config_token());
        assert_eq!((&table.kernel, &table.rootfs), (kernel, rootfs));

        let guest = std::path::Path::new(config::DEFAULT_GUEST_DIR);
        assert_eq!(table.kernel, guest.join("vmlinux"));
        assert_eq!(table.rootfs, guest.join("rootfs.ext4"));

        // `0` is "probe the host at boot", not "no cores" — a written table
        // must not pin this box's CPU/RAM into a config that travels.
        assert_eq!((table.cores, table.mem_gb), (0, 0));
    }
}

#[cfg(test)]
mod json_output_tests {
    /// member-status `--json` carries the same two facts as the prose line.
    #[test]
    fn member_status_json_shape() {
        let in_set = true;
        let validators = 3usize;
        let v = serde_json::json!({ "in_set": in_set, "validators": validators });
        assert_eq!(v["in_set"], true);
        assert_eq!(v["validators"], 3);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    fn completions() -> (String, String) {
        let bash = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ops/completions/ducktape.bash"
        ))
        .expect("read ops/completions/ducktape.bash");
        let zsh = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ops/completions/ducktape.zsh"
        ))
        .expect("read ops/completions/ducktape.zsh");
        (bash, zsh)
    }

    /// Exact tokens declared by one completion variable family. Both shipped
    /// files keep each `local` assignment on one line, so the same tiny parser
    /// covers Bash's quoted words and Zsh's parenthesized words.
    fn declaration_tokens(text: &str, stem: &str) -> BTreeSet<String> {
        text.lines()
            .filter_map(|line| line.trim_start().strip_prefix("local "))
            .filter_map(|declaration| declaration.split_once('='))
            .filter(|(name, _)| {
                let exact_stem = *name == stem;
                let nested_stem = name
                    .strip_prefix(stem)
                    .is_some_and(|suffix| suffix.starts_with('_'));
                exact_stem || nested_stem
            })
            .flat_map(|(_, words)| {
                words
                    .trim()
                    .trim_matches(|c| matches!(c, '"' | '(' | ')'))
                    .split_whitespace()
            })
            .map(str::to_string)
            .collect()
    }

    fn grammar_tokens(cmd: &clap::Command, tokens: &mut BTreeSet<String>) {
        for arg in cmd.get_arguments().filter(|arg| !arg.is_hide_set()) {
            if let Some(long) = arg.get_long().filter(|long| *long != "help") {
                tokens.insert(format!("--{long}"));
            }
            if let Some(short) = arg.get_short().filter(|short| *short != 'h') {
                tokens.insert(format!("-{short}"));
            }
        }
        for sub in cmd.get_subcommands().filter(|sub| !sub.is_hide_set()) {
            tokens.insert(sub.get_name().to_string());
            grammar_tokens(sub, tokens);
        }
    }

    fn assert_same_tokens(
        file: &str,
        scope: &str,
        declared: &BTreeSet<String>,
        required: &BTreeSet<String>,
        allowed: &BTreeSet<String>,
    ) {
        for token in required {
            assert!(
                declared.contains(token),
                "{file}: {scope} is missing {token:?}"
            );
        }
        for token in declared {
            assert!(
                allowed.contains(token),
                "{file}: {scope} advertises stale token {token:?}"
            );
        }
    }

    /// The drift guard is bidirectional: every visible Clap verb/flag appears
    /// in both completion files, and every advertised token still exists in
    /// that family's grammar. This catches stale extras as well as omissions.
    #[test]
    fn completion_files_match_the_clap_tree_per_family() {
        let (bash, zsh) = completions();
        let cli = <crate::Cli as clap::CommandFactory>::command();
        let mut top = cli
            .get_subcommands()
            .filter(|family| !family.is_hide_set())
            .map(|family| family.get_name().to_string())
            .collect::<BTreeSet<_>>();
        top.extend(["help", "--help", "-h", "--version", "-V"].map(str::to_string));
        for (file, text) in [("ducktape.bash", &bash), ("ducktape.zsh", &zsh)] {
            let declared = declaration_tokens(text, "families");
            assert_same_tokens(file, "top level", &declared, &top, &top);
        }

        for family in cli.get_subcommands().filter(|family| !family.is_hide_set()) {
            let name = family.get_name();
            if name == "help" {
                continue;
            }
            let mut required = BTreeSet::new();
            grammar_tokens(family, &mut required);
            required.remove("--version");
            required.remove("-V");
            let mut allowed = required.clone();
            allowed.insert("help".into());
            if name == "service" {
                allowed.extend(["compute", "agent", "airlock"].map(str::to_string));
            }
            for (file, text) in [("ducktape.bash", &bash), ("ducktape.zsh", &zsh)] {
                let declared = declaration_tokens(text, name);
                let bare_family = required.is_empty() && declared.is_empty();
                if bare_family {
                    continue;
                }
                assert_same_tokens(file, name, &declared, &required, &allowed);
            }
        }
    }

    /// the guard must actually bite: a verb a sibling family happens to use is
    /// NOT coverage. This is the hole the whole-file match had.
    #[test]
    fn the_family_scope_does_not_borrow_a_siblings_verb() {
        let (bash, _zsh) = completions();
        let service = declaration_tokens(&bash, "service");
        assert!(service.contains("run"), "service declares its own run verb");
        // `promote` lives under `node member`; it must not read as covered here.
        assert!(
            !service.contains("promote"),
            "the service scope must not see node's verbs"
        );
        assert!(
            declaration_tokens(&bash, "gateway").contains("bind"),
            "a family scope still finds its own verbs"
        );
    }

    /// the grammar's own consistency check (conflicting ids, broken flatten,
    /// missing subcommand settings all panic here instead of at first use).
    #[test]
    fn the_clap_tree_is_internally_consistent() {
        <crate::Cli as clap::CommandFactory>::command().debug_assert();
    }

    use super::{human_duration, peer_line};

    /// the peers prose line: rates only with a baseline, statesync tokens
    /// only when the lane reports, durations compacted.
    #[test]
    fn peer_line_carries_rates_only_with_a_baseline() {
        let sample = |sent, bytes| noded::peers::PeerView {
            peer: "ab".repeat(32),
            connected: true,
            connected_since_ms: Some(1_000),
            role: Some("validator".into()),
            msgs_sent: sent,
            msgs_received: 0,
            statesync: Some(noded::peers::StatesyncServeView {
                bytes_tx: bytes,
                frames_served: 2,
                boundary_height: Some(230),
                served_height: Some(230),
                idle_seconds: 4,
                age_seconds: 90,
                last_request_kind: Some("tip_coords".into()),
            }),
        };
        let first = noded::peers::PeersView {
            sampled_at_ms: 10_000,
            height: 5,
            epoch: Some(1),
            peers: vec![sample(100, 1_000)],
        };
        let second = noded::peers::PeersView {
            sampled_at_ms: 12_000,
            height: 6,
            epoch: Some(1),
            peers: vec![sample(150, 3_000)],
        };

        let with_baseline = peer_line(&second.peers[0], Some(&first.peers[0]), &first, &second);
        assert_eq!(
            with_baseline,
            format!(
                "peer={} role=validator connected=11s msgs_tx=150 msgs_rx=0 \
                 tx/s=25.0 rx/s=0.0 sync_bytes=3000 sync_B/s=1000 sync_height=230 \
                 sync_boundary=230 sync_idle=4s sync_last=tip_coords",
                "ab".repeat(32)
            )
        );

        let without_baseline = peer_line(&second.peers[0], None, &first, &second);
        assert!(!without_baseline.contains("tx/s="), "{without_baseline}");
        assert!(
            !without_baseline.contains("sync_B/s="),
            "{without_baseline}"
        );
    }

    #[test]
    fn durations_compact_by_magnitude() {
        assert_eq!(human_duration(42), "42s");
        assert_eq!(human_duration(192), "3m12s");
        assert_eq!(human_duration(7500), "2h05m");
    }
}
