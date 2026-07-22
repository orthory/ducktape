//! Synchronous operator commands for network setup and membership.
//!
//! Command handlers live outside the node runtime so boot orchestration is not
//! coupled to filesystem setup, local RPC calls, or membership ceremonies.

use std::path::PathBuf;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};

use crate::cli_args::{
    AdmitArgs, InitArgs, InviteArgs, JoinCmd, JoinQuery, KeyArgs, MemberCmd, OpCmd, PubkeyArgs,
    ResidentCmd, SelectorArgs, StatusArgs, UpgradeCmd,
};
use crate::{MAX_PROTOCOL_VERSION, config};
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
        OpCmd::Resident(cmd) => dispatch_resident(cmd),
        OpCmd::Member(cmd) => dispatch_member(cmd),
        OpCmd::Upgrade(cmd) => dispatch_upgrade(cmd),
    }
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

fn dispatch_upgrade(cmd: UpgradeCmd) -> CommandResult {
    match cmd {
        UpgradeCmd::Status(args) => cmd_upgrade_status(args),
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

/// `init --name <human name> [--dir <dir>] [--listen a] [--advertised a] [--http a]
/// [--rpc a] [--primary-coordinator host:port|none]
/// [--wireguard-listen a] [--wireguard-advertised host:port] [--invite-listen a]
/// [--wireguard-effect socket|tun|fake]` — found a network: mint the
/// chain-id, write the descriptor + node config, seed the genesis validator
/// set with this identity. without `--dir` the workspace lands in the
/// registry (`~/.ducktape/workspaces/<chain-id>/`), where `-n <chain-id>`
/// finds it.
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
    // node.toml carries the SAME raw flag value (not the defaulted/
    // normalized `primary_coordinator` above) — an absent flag leaves the
    // key absent too, so the runtime re-derives the identical compiled
    // default `apply_primary_coordinator` just baked into the descriptor;
    // an explicit "none"/host:port is persisted verbatim so the two never
    // silently disagree (see `docs`: coordinator is ambient, node-local).
    let mut plumbing = config::merged_plumbing(
        &dir,
        net.listen.as_deref(),
        net.advertised.as_deref(),
        net.http.as_deref(),
        net.gateway.as_deref(),
        net.rpc.as_deref(),
        net.wireguard_effect.as_deref(),
        net.wireguard_listen.as_deref(),
        net.invite_listen.as_deref(),
        net.primary_coordinator.as_deref(),
        net.wireguard_advertised.as_deref(),
    )?;
    if primary_coordinator.is_some() {
        if plumbing.wireguard_listen.is_none() {
            plumbing.wireguard_listen = Some("0.0.0.0:51820".into());
        }
        if net.listen.is_none() {
            let port: u16 = plumbing
                .listen
                .parse::<std::net::SocketAddr>()
                .map(|a| a.port())
                .unwrap_or(0);
            if port == 0 || !plumbing.listen.starts_with('[') {
                plumbing.listen = format!("[::]:{}", if port == 0 { 52200 } else { port });
            }
        }
        if plumbing.advertised.is_none() {
            plumbing.advertised = Some("overlay".into());
        }
    }

    let me = key.public_key();
    let mut descriptor = config::NetworkDescriptor {
        chain_id: chain_id.clone(),
        scheme: config::SCHEME_ED25519.into(),
        validators: vec![hex_bytes(me.as_ref())],
        bootstrap: Vec::new(),
        reach: Vec::new(),
        coordination: None,
    };
    if let Some(addr) = config::dialable(plumbing.advertised.as_deref(), &plumbing.listen)? {
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

/// `invite --target <pubkey-hex> [--role resident|client] [--config
/// node.toml] [--ttl-days N]` — emit the one-line paste blob: the whole
/// join credential. minting IS the admission decision — the blob carries
/// the descriptor with THIS member's dial hint folded in (and persisted, so
/// every future invite carries it), the inviter's WireGuard bootstrap when
/// the reachability plane is configured (`wireguard_listen`), an expiry, and
/// a single-use INVITE TOKEN, the whole envelope signed by this member's
/// identity. the joiner's node redeems the token automatically (governance
/// `Redeem`) — no member approval step follows.
///
/// `--role client` grants submit-only CLIENT standing (redeemed via
/// `user-redeem-invite`, never `join`); WITHOUT `--target` that is a BEARER
/// invite — single-use, 1-day default TTL, first valid redeemer wins. the
/// resident role always requires a target.
fn cmd_invite(args: InviteArgs) -> Result<(), Box<dyn std::error::Error>> {
    // every invite is BEARER (기명 dropped in Join Protocol v2): `--role`
    // (default resident) selects the standing plane, and there is no `--target`
    // — whoever redeems the single-use token first wins. A resident invite is
    // the admission credential itself, kept off the wire by the sealed
    // first-contact intro; a client invite redeems over `user-redeem-invite`.
    let role = config::InviteRole::from(args.role);
    // reject a stale `--target` loudly rather than silently ignoring it: the
    // habit meant something in v1 and must not appear to still work.
    if args.target.is_some() {
        return Err(
            "--target was removed: every invite is now bearer (무기명) — single-use, and \
             (for a resident invite) sealed to the receiving member at first contact. \
             drop --target and hand the blob to the invitee directly."
                .into(),
        );
    }
    let ttl_days: u64 = match args.ttl_days {
        Some(v) => v,
        // a client invite defaults to a tight window; a resident invite keeps
        // the operator-friendly onboarding default (a LOST blob is the residual
        // risk — single-use + sealing cover interception).
        None if role == config::InviteRole::Client => config::DEFAULT_BEARER_INVITE_TTL_DAYS,
        None => config::DEFAULT_INVITE_TTL_DAYS,
    };
    if ttl_days == 0 {
        return Err("--ttl-days must be at least 1".into());
    }
    let cfg_path = args.selector.config_path()?;
    let (raw, base) = config::load_node_toml(&cfg_path)?;
    let network_rel = raw
        .network
        .as_deref()
        .ok_or("invite needs a network-shape config (no `network` field found)")?;
    let descriptor_path = base.join(network_rel);
    let mut descriptor = config::NetworkDescriptor::load(&descriptor_path)?;
    let key = config::load_identity(&base.join(raw.key_file.as_deref().unwrap_or("identity.key")))?;
    let dial_hint = config::dialable(raw.advertised.as_deref(), &raw.listen)?;
    let has_coordinated_reach = descriptor.has_coordinated_reach()?;
    match &dial_hint {
        Some(addr) => descriptor.add_bootstrap(&key.public_key(), addr),
        // an invite must carry SOME dialable member. a member that joined via
        // an invite holds its dial hints as `reach` (bootstrap is empty), so
        // check the union, not just bootstrap — else a reachable NAT'd member
        // is wrongly refused. reachability-plane inviters are exempt when
        // they carry either a direct WireGuard bootstrap or coordinated reach.
        None if raw.wireguard_listen.is_none()
            && !has_coordinated_reach
            && descriptor
                .reach_hints()
                .map(|h| h.is_empty())
                .unwrap_or(true) =>
        {
            return Err(
                "no dialable address: give node.toml a concrete `listen` port or an \
                        `advertised` addr, or configure a primary coordinator, so a joiner can \
                        reach the network"
                    .into(),
            );
        }
        None => {}
    }
    descriptor.save(&descriptor_path)?;

    // the WireGuard bootstrap: present iff this member runs the reachability
    // plane. endpoints are minted from the advertised host (the listen IP is
    // usually unspecified) + the plane's UDP ports; the mesh port is where
    // the joiner dials this member's overlay ULA once the tunnel routes.
    // the WireGuard bootstrap is MANDATORY in v2 (the overlay plane carries the
    // data planes and the sealed first-contact intro), so minting REQUIRES a
    // configured reachability plane — a WG-less invite no longer exists.
    let Some(wg_listen) = config::resolved_wireguard_listen(raw.wireguard_listen.as_deref())? else {
        return Err(
            "this member runs no reachability plane, but a v2 invite must carry a WireGuard \
             bootstrap. set `wireguard_listen` in node.toml (or configure a primary coordinator, \
             which enables the plane) and mint again."
                .into(),
        );
    };
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
            raw.advertised.as_deref(),
            &raw.listen,
            wg_listen,
            raw.wireguard_advertised.as_deref(),
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
                    config::resolved_invite_listen(raw.invite_listen.as_deref(), wg_listen)?.port();
                // the tunnel endpoint carries the FULL advertised host:port when
                // `wireguard_advertised` is configured — the external port can
                // differ from the bind port in the port-forwarded setup the key
                // exists for. The intro stays host + intro port.
                let endpoint = config::invite_wireguard_endpoint(
                    raw.advertised.as_deref(),
                    &raw.listen,
                    wg_listen,
                    raw.wireguard_advertised.as_deref(),
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
    let storage = base.join(raw.storage_dir.as_deref().unwrap_or("storage"));
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
    // every invite is bearer; the role selects the standing plane.
    let token = config::mint_invite_token(
        &key,
        descriptor.genesis_namespace().as_bytes(),
        role,
        expires,
    );
    let blob_string = config::encode_invite(
        &invite_descriptor,
        &token,
        &wireguard,
        &fronts,
        &key,
    )?;
    if role == config::InviteRole::Client {
        eprintln!(
            "[invite] bearer CLIENT invite (single-use, expires in {ttl_days} day(s)) — \
             redeem with: ducktape user redeem-invite <blob> --node <member-http-url> \
             --key <user.key>",
        );
    }
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
    let network_rel = raw
        .network
        .as_deref()
        .ok_or("admit needs a network-shape config")?;
    let storage = base.join(raw.storage_dir.as_deref().unwrap_or("storage"));
    if storage.exists() {
        return Err(format!(
            "{} already has state — a running network admits members via governance \
             (AddValidator), not by editing genesis",
            storage.display()
        )
        .into());
    }
    let descriptor_path = base.join(network_rel);
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
    let conn = std::net::TcpStream::connect(addr)
        .map_err(|e| format!("connect rpc {addr}: {e} (is the node running?)"))?;
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

/// the `--json` projection of upgrade-status when the module IS present: the
/// same facts the prose form prints, plus `binary_can_execute` — the inverse of
/// the WARNING condition (false iff a pending upgrade targets a version this
/// binary is too old to run). `pending` mirrors the prose `pending: none` as a
/// JSON `null`. borrows the reply so no field is copied.
#[derive(serde::Serialize)]
struct UpgradeStatusJson<'a> {
    current_version: u32,
    max_supported: u32,
    pending: Option<&'a lifecycle::ScheduledUpgrade>,
    ready_count: u64,
    member_count: u64,
    armed: bool,
    binary_can_execute: bool,
}

/// build the `--json` projection from the module reply. pure (no IO) so the
/// shape is unit-testable without a live node.
fn upgrade_status_json(status: &lifecycle::UpgradeStatus, max_supported: u32) -> UpgradeStatusJson<'_> {
    // WARNING fires iff a pending upgrade targets a version above this binary's
    // ceiling; `binary_can_execute` is its inverse (true when nothing pending).
    let binary_can_execute = status
        .pending
        .as_ref()
        .is_none_or(|up| up.to_version <= max_supported);
    UpgradeStatusJson {
        current_version: status.current_version,
        max_supported,
        pending: status.pending.as_ref(),
        ready_count: status.ready_count,
        member_count: status.member_count,
        armed: status.armed,
        binary_can_execute,
    }
}

/// `upgrade status [--config <path> | -n <chain-id>] [--json]` — query the
/// upgrade module Status over this node's local rpc and print `current_version`,
/// the single pending upgrade, the readiness verdict (`ready_count` of
/// `member_count`, `armed`), and the `max_supported` version this binary can
/// execute. `--json` emits the same facts as one machine-readable object
/// (module-absent → `{"available":false,"max_supported":N}`). degrades
/// gracefully on a net
/// WITHOUT the module (pre-retrofit): the query errors and we report baseline.
fn cmd_upgrade_status(args: StatusArgs) -> Result<(), Box<dyn std::error::Error>> {
    use lifecycle::{LifecycleQuery, LifecycleReply, decode_reply, encode_query};
    let want_json = args.json;
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("upgrade status drives the node's local rpc — set `rpc_listen` in node.toml")?;

    let raw = match rpc_query(&addr, host::LIFECYCLE_MODULE_ID, &encode_query(&LifecycleQuery::UpgradeStatus)) {
        Ok(bytes) => bytes,
        // module absent (pre-retrofit) or unreachable: report the binary baseline
        // rather than failing — the CLI is inert on a net without the module.
        Err(e) => {
            if want_json {
                println!(
                    "{}",
                    serde_json::json!({ "available": false, "max_supported": MAX_PROTOCOL_VERSION })
                );
            } else {
                println!(
                    "upgrade module not available ({e}) — this binary supports up to protocol v{MAX_PROTOCOL_VERSION}"
                );
            }
            return Ok(());
        }
    };
    let LifecycleReply::UpgradeStatus(status) = decode_reply(&raw)? else {
        return Err("lifecycle returned an unexpected reply".into());
    };
    if want_json {
        let out = upgrade_status_json(&status, MAX_PROTOCOL_VERSION);
        println!("{}", serde_json::to_string(&out).expect("serializable"));
        return Ok(());
    }
    println!("current_version: {}", status.current_version);
    println!("max_supported (this binary): {MAX_PROTOCOL_VERSION}");
    match &status.pending {
        Some(up) => {
            println!(
                "pending: name={} activation_height={} to_version={}",
                up.name, up.activation_height, up.to_version
            );
            println!(
                "readiness: {} of {} boundary members ready",
                status.ready_count, status.member_count
            );
            println!("armed (R == n): {}", status.armed);
            if up.to_version > MAX_PROTOCOL_VERSION {
                println!(
                    "WARNING: this binary (v{MAX_PROTOCOL_VERSION}) cannot execute to_version {} \
                     — install the newer node binary before H or this node aborts the upgrade",
                    up.to_version
                );
            }
        }
        None => println!("pending: none"),
    }
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
                "admitted {pubkey_hex} as STANDBY: the joiner's parked node will verify a \
                 state sync, announce itself online, and join the consensus quorum at the \
                 activation cutover — no quorum slot is spent until the node is actually up"
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
/// required power. this casts only its account ballot (or the legacy node
/// ballot), and member remove prints the remaining threshold plus the command
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
/// [--invite-listen a] [--wireguard-effect socket|tun|fake]
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
    // a CLIENT invite grants submit access, not a node — a node redeeming it
    // would gate-fail terminally at the lobby (V8); fail at paste time with
    // the right pointer instead. (bearer invites are client-only, so this
    // also filters every bearer blob.)
    if invite.token.role == config::InviteRole::Client {
        return Err("this is a CLIENT invite — it grants submit access, not a node. \
                    redeem it with `ducktape user redeem-invite <blob> --node \
                    <member-http-url> --key <user.key>`"
            .into());
    }
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
    // (기명 dropped in v2): there is no target to match, so any freshly minted
    // key may redeem — the OOB "hand the inviter your join code first" step is
    // gone. The redeeming key is bound by the join proof and the token is
    // single-use, so a paste simply admits whoever runs it.
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me_hex = hex_bytes(key.public_key().as_ref());
    config::guard_join_descriptor(&dir, &descriptor)?;
    // plumbing merges: explicit flags win, an existing node.toml's values
    // (network- or dev-shape) survive, defaults fill the rest. computed
    // BEFORE anything lands on disk so a corrupt existing node.toml aborts
    // the join without leaving a half-migrated dir. the file is ALWAYS
    // rewritten in the network shape — a join must take effect even in a dir
    // holding the app's dev-shape solo config.
    let net = &args.plumbing;
    let mut plumbing = config::merged_plumbing(
        &dir,
        net.listen.as_deref(),
        net.advertised.as_deref(),
        net.http.as_deref(),
        net.gateway.as_deref(),
        net.rpc.as_deref(),
        net.wireguard_effect.as_deref(),
        net.wireguard_listen.as_deref(),
        net.invite_listen.as_deref(),
        net.primary_coordinator.as_deref(),
        net.wireguard_advertised.as_deref(),
    )?;
    if config::invite_requires_reachability_defaults(&invite) {
        // a WireGuard or Coordinated invite makes the reachability plane the
        // dial path, so the joiner's defaults change shape: its own plane
        // comes up (wireguard_listen), its mesh listens dual-stack on a
        // CONCRETE port and advertises the overlay ULA (members reverse-dial
        // it over the tunnels). explicit flags and an existing node.toml
        // still win.
        if plumbing.wireguard_listen.is_none() {
            plumbing.wireguard_listen = Some("0.0.0.0:51820".into());
        }
        if net.listen.is_none() {
            let port: u16 = plumbing
                .listen
                .parse::<std::net::SocketAddr>()
                .map(|a| a.port())
                .unwrap_or(0);
            if port == 0 || !plumbing.listen.starts_with('[') {
                plumbing.listen = format!("[::]:{}", if port == 0 { 52200 } else { port });
            }
        }
        if plumbing.advertised.is_none() {
            plumbing.advertised = Some("overlay".into());
        }
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
        // every offered front gets the same overlay-ULA Direct hint the inviter
        // does: once ANY candidate's tunnel comes up, the mesh dialer can reach
        // that member's overlay ULA and ride the mesh from there. A hint whose
        // tunnel never comes up simply fails to dial — harmless.
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
        // present in v2); kept beside the token so `run_node` brings the
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
mod json_output_tests {
    use super::*;
    use lifecycle::{ScheduledUpgrade, UpgradeStatus};

    fn status(pending: Option<ScheduledUpgrade>, armed: bool) -> UpgradeStatus {
        UpgradeStatus {
            current_version: 1,
            pending,
            members: vec![vec![1u8; 32], vec![2u8; 32]],
            ready: vec![vec![1u8; 32]],
            member_count: 2,
            ready_count: 1,
            armed,
        }
    }

    /// the `--json` upgrade-status object decodes and mirrors the prose facts,
    /// with `binary_can_execute` true when the pending version is within reach.
    #[test]
    fn upgrade_status_json_pending_within_reach() {
        let up = ScheduledUpgrade {
            name: "forge-multi".into(),
            activation_height: 100,
            to_version: MAX_PROTOCOL_VERSION,
        };
        let st = status(Some(up), true);
        let out = upgrade_status_json(&st, MAX_PROTOCOL_VERSION);
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&out).unwrap()).unwrap();
        assert_eq!(v["current_version"], 1);
        assert_eq!(v["max_supported"], MAX_PROTOCOL_VERSION);
        assert_eq!(v["pending"]["name"], "forge-multi");
        assert_eq!(v["pending"]["activation_height"], 100);
        assert_eq!(v["pending"]["to_version"], MAX_PROTOCOL_VERSION);
        assert_eq!(v["ready_count"], 1);
        assert_eq!(v["member_count"], 2);
        assert_eq!(v["armed"], true);
        assert_eq!(v["binary_can_execute"], true);
    }

    /// a pending upgrade targeting a version above this binary's ceiling is the
    /// WARNING condition — `binary_can_execute` is false.
    #[test]
    fn upgrade_status_json_pending_too_new_cannot_execute() {
        let up = ScheduledUpgrade {
            name: "future".into(),
            activation_height: 5,
            to_version: MAX_PROTOCOL_VERSION + 1,
        };
        let st = status(Some(up), false);
        let out = upgrade_status_json(&st, MAX_PROTOCOL_VERSION);
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&out).unwrap()).unwrap();
        assert_eq!(v["binary_can_execute"], false);
        assert_eq!(v["to_version"], serde_json::Value::Null); // to_version lives under `pending`
        assert_eq!(v["pending"]["to_version"], MAX_PROTOCOL_VERSION + 1);
    }

    /// no pending upgrade → `pending` is JSON null and the binary can execute.
    #[test]
    fn upgrade_status_json_no_pending_is_null() {
        let st = status(None, false);
        let out = upgrade_status_json(&st, MAX_PROTOCOL_VERSION);
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&out).unwrap()).unwrap();
        assert_eq!(v["pending"], serde_json::Value::Null);
        assert_eq!(v["binary_can_execute"], true);
        assert_eq!(v["armed"], false);
    }

    /// the module-absent fallback shape the verb prints when the query errors.
    #[test]
    fn upgrade_status_json_module_absent_shape() {
        let v = serde_json::json!({ "available": false, "max_supported": MAX_PROTOCOL_VERSION });
        assert_eq!(v["available"], false);
        assert_eq!(v["max_supported"], MAX_PROTOCOL_VERSION);
    }

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

    /// the drift guard: every node verb token and every non-hidden long flag
    /// in the CLAP TREE (the grammar itself, not a parallel table) must appear
    /// in BOTH completion files, and every family name too. renaming a verb or
    /// adding a flag without updating the hand-written completions fails here.
    #[test]
    fn completion_files_cover_the_verb_table() {
        let (bash, zsh) = completions();
        for family in ["node", "user", "gateway", "fs", "mcp"] {
            assert!(bash.contains(family), "ducktape.bash missing family {family:?}");
            assert!(zsh.contains(family), "ducktape.zsh missing family {family:?}");
        }
        fn walk(cmd: &clap::Command, bash: &str, zsh: &str) {
            for sub in cmd.get_subcommands() {
                if sub.is_hide_set() {
                    continue;
                }
                let token = sub.get_name();
                if token == "help" {
                    continue;
                }
                assert!(bash.contains(token), "ducktape.bash missing token {token:?}");
                assert!(zsh.contains(token), "ducktape.zsh missing token {token:?}");
                for arg in sub.get_arguments() {
                    if arg.is_hide_set() {
                        continue;
                    }
                    let Some(long) = arg.get_long() else { continue };
                    if long == "help" {
                        continue;
                    }
                    let flag = format!("--{long}");
                    assert!(bash.contains(&flag), "ducktape.bash missing flag {flag}");
                    assert!(zsh.contains(&flag), "ducktape.zsh missing flag {flag}");
                }
                walk(sub, bash, zsh);
            }
        }
        let cmd = <crate::Cli as clap::CommandFactory>::command();
        let node = cmd.find_subcommand("node").expect("node family exists");
        walk(node, &bash, &zsh);
    }

    /// the grammar's own consistency check (conflicting ids, broken flatten,
    /// missing subcommand settings all panic here instead of at first use).
    #[test]
    fn the_clap_tree_is_internally_consistent() {
        <crate::Cli as clap::CommandFactory>::command().debug_assert();
    }
}
