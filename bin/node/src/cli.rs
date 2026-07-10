//! Synchronous operator commands for network setup and membership.
//!
//! Command handlers live outside the node runtime so boot orchestration is not
//! coupled to filesystem setup, local RPC calls, or membership ceremonies.

use std::path::PathBuf;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};

use crate::{MAX_PROTOCOL_VERSION, cli_flags::parse_flags, config, userkey_cli};
use config::{hex_bytes, unhex};

type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// Run an operator command, or return `None` when the arguments select the
/// long-running node path instead.
pub(super) fn dispatch(command: &str, args: &[String]) -> Option<CommandResult> {
    if let Some(result) = userkey_cli::dispatch(command, args) {
        return Some(result);
    }
    let result = match command {
        "keygen" => cmd_keygen(args),
        "init" => cmd_init(args),
        "invite" => cmd_invite(args),
        "admit" => cmd_admit(args),
        "invite-accept" => cmd_invite_accept(args),
        "promote" => cmd_promote(args),
        "resident-remove" => cmd_resident_remove(args),
        "join-requests" => cmd_join_requests(args),
        "member-remove" => cmd_member_remove(args),
        "member-leave" => cmd_member_leave(args),
        "member-status" => cmd_member_status(args),
        "join" => cmd_join(args),
        "upgrade-status" => cmd_upgrade_status(args),
        _ => return None,
    };
    Some(result)
}

// ============================================================================
// onboarding verbs — keygen / init / invite / admit / join.
// ============================================================================

/// the config a verb operates on: `-n`/`--network <chain id>` resolves through
/// the workspace registry (`~/.ducktape/workspaces`), `--config <path>` is the
/// explicit escape hatch, and the default is ./node.toml — the pre-registry
/// behavior, unchanged.
fn config_path(flags: &std::collections::BTreeMap<String, String>) -> Result<PathBuf, String> {
    if let Some(needle) = flags.get("network") {
        return config::find_workspace_config(needle);
    }
    Ok(PathBuf::from(
        flags
            .get("config")
            .map(String::as_str)
            .unwrap_or("node.toml"),
    ))
}

/// `keygen [--out <path>]` — generate (or reuse) a persisted ed25519 identity.
/// pubkey on stdout (scriptable); provenance on stderr.
fn cmd_keygen(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(
        flags
            .get("out")
            .map(String::as_str)
            .unwrap_or("identity.key"),
    );
    let (key, generated) = config::load_or_generate_identity(&out)?;
    println!("{}", hex_bytes(key.public_key().as_ref()));
    eprintln!(
        "{} identity at {}",
        if generated { "generated" } else { "reusing" },
        out.display()
    );
    Ok(())
}

/// `init --name <human name> [--dir .] [--listen a] [--advertised a] [--http a]
/// [--rpc a] [--primary-coordinator host:port|none]
/// [--wireguard-listen a] [--invite-listen a]
/// [--wireguard-effect socket|tun|fake]` — found a network: mint the
/// chain-id, write the descriptor + node config, seed the genesis validator
/// set with this identity.
fn cmd_init(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let name = flags
        .get("name")
        .ok_or("init needs --name <human-readable network name>")?;
    let dir = PathBuf::from(flags.get("dir").map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&dir)?;
    // re-running init would mint a FRESH chain-id and reset the validator set
    // to just this identity — silently un-founding the network under every
    // holder of an existing invite. founding is once per directory.
    let descriptor_path = dir.join("network.toml");
    if descriptor_path.exists() {
        return Err(format!(
            "{} already exists — this directory is already a network. use `invite`/`admit` \
             for membership, or delete the file to re-found from scratch",
            descriptor_path.display()
        )
        .into());
    }
    let primary_coordinator = config::primary_coordinator_or_default(
        flags.get("primary-coordinator").map(String::as_str),
    )?;
    let mut plumbing = config::merged_plumbing(
        &dir,
        flags.get("listen").map(String::as_str),
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
        flags.get("wireguard-effect").map(String::as_str),
        flags.get("wireguard-listen").map(String::as_str),
        flags.get("invite-listen").map(String::as_str),
    )?;
    if primary_coordinator.is_some() {
        if plumbing.wireguard_listen.is_none() {
            plumbing.wireguard_listen = Some("0.0.0.0:51820".into());
        }
        if !flags.contains_key("listen") {
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

    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me = key.public_key();
    let chain_id = config::mint_chain_id(name, &me);
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
    eprintln!("start:  ducktape-node --config {}/node.toml", dir.display());
    eprintln!(
        "invite: ducktape-node invite --config {}/node.toml",
        dir.display()
    );
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
/// (governance `Redeem`) — no member approval step follows.
fn cmd_invite(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let ttl_days: u64 = match flags.get("ttl-days") {
        Some(v) => v.parse().map_err(|e| format!("--ttl-days {v:?}: {e}"))?,
        None => config::DEFAULT_INVITE_TTL_DAYS,
    };
    if ttl_days == 0 {
        return Err("--ttl-days must be at least 1".into());
    }
    let cfg_path = config_path(&flags)?;
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
    let wireguard = match config::resolved_wireguard_listen(raw.wireguard_listen.as_deref())? {
        Some(wg_listen) => {
            let (wg_keypair, _) =
                reachability::WireGuardKeypair::load_or_generate(&base.join("wireguard.key"))
                    .map_err(|e| format!("wireguard key: {e}"))?;
            let mesh_port: u16 = raw
                .listen
                .parse::<std::net::SocketAddr>()
                .map(|a| a.port())
                .map_err(|e| format!("listen {:?}: {e}", raw.listen))?;
            let host =
                match config::endpoint_host(raw.advertised.as_deref(), &raw.listen, wg_listen) {
                    Ok(host) => Some(host),
                    Err(_) if has_coordinated_reach => {
                        // Coordinated reach gives the joiner a rendezvous
                        // path; there is deliberately no inviter-hosted
                        // underlay endpoint to bake into the blob.
                        None
                    }
                    Err(err) => return Err(err.into()),
                };
            match host {
                Some(host) => {
                    let intro_port =
                        config::resolved_invite_listen(raw.invite_listen.as_deref(), wg_listen)?
                            .port();
                    Some(config::InviteWireGuard {
                        public_key: wg_keypair.public_key().0,
                        endpoint: Some(format!("{host}:{}", wg_listen.port())),
                        intro: Some(format!("{host}:{intro_port}")),
                        mesh_port,
                    })
                }
                None => Some(config::InviteWireGuard {
                    public_key: wg_keypair.public_key().0,
                    endpoint: None,
                    intro: None,
                    mesh_port,
                }),
            }
        }
        None => None,
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
    let token = config::mint_invite_token(&key, descriptor.genesis_namespace().as_bytes());
    println!(
        "{}",
        config::encode_invite(
            &invite_descriptor,
            &token,
            wireguard.as_ref(),
            &fronts,
            expires,
            &key
        )?
    );
    Ok(())
}

/// `admit <hex pubkey> [--config node.toml]` — pre-genesis membership: add an
/// identity to the descriptor's validator set. once the network has state,
/// membership changes go through governance (AddValidator), not genesis edits.
fn cmd_admit(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("admit needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let cfg_path = config_path(&flags)?;
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
        "re-run `ducktape-node invite` and share the REFRESHED invite — genesis must be \
         identical on every member"
    );
    Ok(())
}

// ---- invite-accept: post-genesis admission over the local rpc --------------

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

/// `join-requests [--config node.toml]` — the verified join announces parked
/// joiners delivered to THIS member's running node, as one JSON array on
/// stdout (machine-parseable — the app's members view renders it). approving
/// is a separate, deliberate act: `invite-accept <joiner>` (or the app's
/// approve button) casts this member's governance ballot, and a strict
/// majority admits.
fn cmd_join_requests(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("join-requests reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let reply = rpc_call(&addr, &serde_json::json!({ "cmd": "join_requests" }))?;
    if reply["ok"] != true {
        return Err(format!("join-requests: {}", reply["error"]).into());
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

/// `upgrade-status [--config node.toml]` — query the upgrade module Status over
/// this node's local rpc and print `current_version`, the single pending upgrade,
/// the readiness verdict (`ready_count` of `member_count`, `armed`), and the
/// `max_supported` version this binary can execute. degrades gracefully on a net
/// WITHOUT the module (pre-retrofit): the query errors and we report baseline.
fn cmd_upgrade_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use upgrade::{UpgradeQuery, UpgradeReply, decode_reply, encode_query};
    let (_, flags) = parse_flags(args)?;
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let addr = resolved
        .rpc_listen
        .ok_or("upgrade-status drives the node's local rpc — set `rpc_listen` in node.toml")?;

    let raw = match rpc_query(&addr, "upgrade", &encode_query(&UpgradeQuery::Status)) {
        Ok(bytes) => bytes,
        // module absent (pre-retrofit) or unreachable: report the binary baseline
        // rather than failing — the CLI is inert on a net without the module.
        Err(e) => {
            println!(
                "upgrade module not available ({e}) — this binary supports up to protocol v{MAX_PROTOCOL_VERSION}"
            );
            return Ok(());
        }
    };
    let UpgradeReply::Status(status) = decode_reply(&raw)?;
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

/// how a driven membership ceremony left the proposal.
enum CeremonyOutcome {
    /// passed and executed — the set changes at the next epoch cutover.
    Passed,
    /// this ballot landed but a strict majority is still outstanding.
    AwaitingBallots,
}

/// drive a strict-majority governance membership ceremony for `wanted`
/// through this member's own running node: adopt an existing OPEN proposal
/// for exactly this action (else mint an unused `<id_prefix><key>:<n>` id and
/// propose), cast a yes ballot, and execute once decidable. idempotent across
/// members — each runs the same verb; the run landing the deciding ballot
/// executes. shared by `invite-accept` (AddResident), `promote`
/// (AddValidator), and `resident-remove` (RemoveResident).
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

    rpc_submit(
        rpc_addr,
        "governance",
        &encode_msg(&GovMsg::Vote {
            proposal_id: proposal_id.clone(),
            approve: true,
        }),
    )?;
    let after_vote = poll_proposal(rpc_addr, &proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|v| {
            v.status != ProposalStatus::Open
                || v.votes.iter().any(|(voter, yes)| voter == me_bytes && *yes)
        })
    })?
    .expect("the poll only accepts a present proposal");
    eprintln!("ballot cast as {}", hex_bytes(me_bytes));

    // execute only when decidable — a strict-majority shortfall is the
    // normal n>=2 intermediate state, not an error.
    let members = read_members(rpc_addr)?;
    let yes = members
        .iter()
        .filter(|m| {
            after_vote
                .votes
                .iter()
                .any(|(voter, approve)| voter == *m && *approve)
        })
        .count();
    let majority = members.len() / 2 + 1;
    if after_vote.status == ProposalStatus::Open && yes < majority {
        eprintln!(
            "{yes} of {majority} required ballots — waiting on other members. each runs:\n    \
             ducktape-node {verb} {pubkey_hex} --config <their node.toml>"
        );
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

/// `invite-accept <hex pubkey> [--config node.toml]` — approve a join request
/// as RESIDENT standing (the staged-admission tier): drive a governance
/// AddResident proposal for `pubkey` through this member's own RUNNING node.
/// the passing proposal's valset Grant schedules the epoch cutover that
/// admits the key to the mesh, at which point its parked node PRE-SYNCS
/// state on a stride cadence. promotion into the quorum is the separate,
/// deliberate `promote` verb — run it once the resident is warm.
fn cmd_invite_accept(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("invite-accept needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
    // full config resolution (network- or dev-shape) so the verb derives the
    // SAME identity the running node signs with — the ballots this verb
    // casts are signed by the NODE (the ordered lane signs every rpc
    // submit), and that key must be the member.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("invite-accept drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is already a validator — nothing to do");
        return Ok(());
    }
    if read_residents(&rpc_addr)?.contains(&key_bytes) {
        eprintln!(
            "{pubkey_hex} already holds resident standing — promote with \
             `ducktape-node promote {pubkey_hex}` once it is synced"
        );
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members admit \
                    residents"
                .into(),
        );
    }

    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "invite-accept",
        "resident:",
        GovAction::AddResident { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "granted resident standing to {pubkey_hex}: the mesh admits it at the next \
                 epoch cutover and its parked node pre-syncs state. promote it into the \
                 quorum once warm:\n    ducktape-node promote {pubkey_hex}"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

/// `promote <hex pubkey> [--config node.toml]` — seat a key in the consensus
/// quorum: drive a governance AddValidator proposal through this member's own
/// RUNNING node. the passing proposal's valset Join clears any resident
/// standing in the same block and schedules the epoch cutover; a pre-synced
/// resident then catches up a small delta and reboots as a validator, so the
/// quorum only ever gains a warm member. also serves DIRECT (un-staged)
/// admission — exactly the pre-resident `invite-accept` semantics.
fn cmd_promote(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("promote needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
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
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members promote \
                    validators"
                .into(),
        );
    }

    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "promote",
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

/// `resident-remove <hex pubkey> [--config node.toml]` — revoke resident
/// standing: drive a governance RemoveResident proposal through this member's
/// own RUNNING node. the mirror of `invite-accept` with inverted guards — a
/// no-op when the key holds no resident standing, and only members may drive
/// it. the passing proposal's valset Revoke schedules the epoch cutover that
/// drops the key from the mesh; its node falls back to a parked joiner, and
/// `invite-accept` re-grants. a seated validator is `member-remove`'s job —
/// standing never overlaps (Grant refuses validators, Join clears standing).
fn cmd_resident_remove(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::GovAction;

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("resident-remove needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
    // full config resolution so the verb derives the SAME identity the running
    // node signs with — the ballots this verb casts are signed by the NODE, and
    // that key must be a current member.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("resident-remove drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    if members.contains(&key_bytes) {
        eprintln!(
            "{pubkey_hex} is a seated validator, not a resident — remove it with \
             `ducktape-node member-remove {pubkey_hex}`"
        );
        return Ok(());
    }
    if !read_residents(&rpc_addr)?.contains(&key_bytes) {
        eprintln!("{pubkey_hex} holds no resident standing — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members remove \
                    residents"
                .into(),
        );
    }

    match drive_membership_ceremony(
        &rpc_addr,
        &me_bytes,
        pubkey_hex,
        "resident-remove",
        "revoke:",
        GovAction::RemoveResident { key: key_bytes },
    )? {
        CeremonyOutcome::Passed => {
            eprintln!(
                "revoked resident standing from {pubkey_hex}: the mesh drops it at the next \
                 epoch cutover and its node parks again. a member re-grants with:\n    \
                 ducktape-node invite-accept {pubkey_hex}"
            );
            Ok(())
        }
        CeremonyOutcome::AwaitingBallots => Ok(()),
    }
}

// ---- member-remove: post-genesis removal over the local rpc ----------------

/// `member-remove <hex pubkey> [--config node.toml]` — post-genesis removal:
/// drive a governance RemoveValidator proposal for `pubkey` through this
/// member's own RUNNING node. the mirror of `invite-accept` with inverted
/// guards — a no-op when the key is NOT a member, and only members may drive
/// it. idempotent across members: each runs the same command (propose if
/// absent, cast a yes ballot, execute once decidable); the run that lands the
/// deciding ballot executes. the passing proposal's valset Leave schedules the
/// epoch cutover that drops the key from the tracked set.
fn cmd_member_remove(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use governance::{GovAction, GovMsg, ProposalStatus, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    let [pubkey_hex] = pos.as_slice() else {
        return Err("member-remove needs exactly one <hex pubkey>".into());
    };
    let key = config::decode_key(pubkey_hex)?;
    let key_bytes = key.as_ref().to_vec();
    let cfg_path = config_path(&flags)?;
    // full config resolution so the verb derives the SAME identity the running
    // node signs with — the ballots this verb casts are signed by the NODE, and
    // that key must be a current member.
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("member-remove drives the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();

    let members = read_members(&rpc_addr)?;
    // inverted admission guards: nothing to remove if the key is not a member,
    // and only current members may open/decide a removal.
    if !members.contains(&key_bytes) {
        eprintln!("{pubkey_hex} is not a validator — nothing to do");
        return Ok(());
    }
    if !members.contains(&me_bytes) {
        return Err(
            "this node's identity is not a current member — only members remove \
                    validators"
                .into(),
        );
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

    rpc_submit(
        &rpc_addr,
        "governance",
        &encode_msg(&GovMsg::Vote {
            proposal_id: proposal_id.clone(),
            approve: true,
        }),
    )?;
    let after_vote = poll_proposal(&rpc_addr, &proposal_id, "this ballot to finalize", |p| {
        p.as_ref().is_some_and(|v| {
            v.status != ProposalStatus::Open
                || v.votes
                    .iter()
                    .any(|(voter, yes)| voter == &me_bytes && *yes)
        })
    })?
    .expect("the poll only accepts a present proposal");
    eprintln!("ballot cast as {}", hex_bytes(&me_bytes));

    // execute only when decidable — a strict-majority shortfall is the normal
    // n>=2 intermediate state, not an error.
    let members = read_members(&rpc_addr)?;
    let yes = members
        .iter()
        .filter(|m| {
            after_vote
                .votes
                .iter()
                .any(|(voter, approve)| voter == *m && *approve)
        })
        .count();
    let majority = members.len() / 2 + 1;
    if after_vote.status == ProposalStatus::Open && yes < majority {
        eprintln!(
            "{yes} of {majority} required ballots — waiting on other members. each runs:\n    \
             ducktape-node member-remove {pubkey_hex} --config <their node.toml>"
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

// ---- member-leave: this node drives its OWN removal from the set -----------

/// `member-leave [--config node.toml]` — a member drives its OWN removal:
/// resolve this node's identity and route it through the EXACT SAME governance
/// path as `member-remove` (a RemoveValidator proposal targeting self). there
/// is no separate governance logic — it hands off to [`cmd_member_remove`] with
/// this node's own pubkey.
///
/// honesty: leaving is NOT unilateral in a set of n>=2. this casts only this
/// node's own yes-ballot, so the removal stays PENDING until a strict majority
/// (n/2+1) of the members approve — member-remove's own output prints the tally
/// and names the command the remaining members run (`member-remove <this key>`).
/// a lone member (n==1) meets its own majority-of-one and executes at once.
fn cmd_member_leave(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("member-leave takes no positional args (got {pos:?})").into());
    }
    let cfg_path = config_path(&flags)?;
    // resolve the running node's identity — the key it signs ballots with, and
    // the one this verb submits for removal.
    let resolved = config::resolve(&cfg_path)?;
    let me_hex = hex_bytes(resolved.signer.public_key().as_ref());
    eprintln!("leaving the network: opening a self-removal for {me_hex}");
    // delegate to member-remove targeting SELF — same propose+vote+execute
    // path, same strict-majority honesty. rebuild the arg vector so the flags
    // (notably --config) reach the delegate unchanged.
    let mut forwarded = vec![me_hex];
    for (name, value) in &flags {
        forwarded.push(format!("--{name}"));
        forwarded.push(value.clone());
    }
    cmd_member_remove(&forwarded)
}

// ---- member-status: is THIS node still in the validator set? ----------------

/// `member-status [--config node.toml]` — read this node's OWN membership off
/// its RUNNING node's rpc and print one machine-parseable line to stdout:
///
/// ```text
/// in-set=<true|false> validators=<count>
/// ```
///
/// this is the read the desktop shell consults before FORGETTING a workspace
/// (stop + delete): tearing a node down while it is still a current validator of
/// a set of two-or-more strands its pending removal and halts quorum (a live
/// network still needs its signature). the shell refuses a forget when
/// `in-set=true` and `validators>=2`; a lone validator (`validators=1`) or an
/// already-removed key (`in-set=false`) is safe to forget. requires the node to
/// be up (it serves this over the same local rpc as `member-remove`).
fn cmd_member_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("member-status takes no positional args (got {pos:?})").into());
    }
    let cfg_path = config_path(&flags)?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("member-status reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let me_bytes = resolved.signer.public_key().as_ref().to_vec();
    let members = read_members(&rpc_addr)?;
    let in_set = members.contains(&me_bytes);
    println!("in-set={in_set} validators={}", members.len());
    Ok(())
}

/// `join <invite blob> [--dir .] [--listen a] [--advertised a] [--http a]
/// [--rpc a] [--wireguard-listen a] [--invite-listen a]
/// [--wireguard-effect socket|tun|fake]` — materialize a workspace
/// from an invite: descriptor + identity (kept across re-joins) + node
/// config. prints this identity for the inviter's pre-genesis `admit`.
fn cmd_join(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    let [blob] = pos.as_slice() else {
        return Err("join needs exactly one <invite blob>".into());
    };
    let invite = config::decode_invite(blob)?;
    let mut descriptor = invite.descriptor.clone();
    let dir = PathBuf::from(flags.get("dir").map(String::as_str).unwrap_or("."));
    std::fs::create_dir_all(&dir)?;
    config::guard_join_descriptor(&dir, &descriptor)?;
    // plumbing merges: explicit flags win, an existing node.toml's values
    // (network- or dev-shape) survive, defaults fill the rest. computed
    // BEFORE anything lands on disk so a corrupt existing node.toml aborts
    // the join without leaving a half-migrated dir. the file is ALWAYS
    // rewritten in the network shape — a join must take effect even in a dir
    // holding the app's dev-shape solo config.
    let mut plumbing = config::merged_plumbing(
        &dir,
        flags.get("listen").map(String::as_str),
        flags.get("advertised").map(String::as_str),
        flags.get("http").map(String::as_str),
        flags.get("rpc").map(String::as_str),
        flags.get("wireguard-effect").map(String::as_str),
        flags.get("wireguard-listen").map(String::as_str),
        flags.get("invite-listen").map(String::as_str),
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
        if !flags.contains_key("listen") {
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
        if let Some(wg) = &invite.wireguard {
            let issuer_identity =
                wireguard_upgrade::ValidatorIdentity::try_from(invite.token.issuer.as_ref())
                    .map_err(|e| format!("inviter identity: {e:?}"))?;
            let inviter_ula = wireguard_upgrade::ula_v6_member_addr(
                &descriptor.genesis_namespace(),
                issuer_identity,
            );
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
            let Ok(identity) =
                wireguard_upgrade::ValidatorIdentity::try_from(&front.member_key[..])
            else {
                continue;
            };
            let ula =
                wireguard_upgrade::ula_v6_member_addr(&descriptor.genesis_namespace(), identity);
            descriptor.add_reach_route(&config::ReachHint {
                expected_key: member,
                reach: config::Reach::Direct(format!("[{ula}]:{}", front.mesh_port)),
            });
        }
    }
    descriptor.save(&dir.join("network.toml"))?;
    let (key, generated) = config::load_or_generate_identity(&dir.join("identity.key"))?;
    let me_hex = hex_bytes(key.public_key().as_ref());
    config::write_node_toml(&dir, &plumbing)?;
    // the capability the joining node redeems automatically; a re-join with a
    // fresh invite replaces a stale/spent one.
    config::save_invite_token(&dir, &invite.token)?;
    // the offered fronts, kept beside the token so `run_node` can race the
    // whole union of first-contact paths. Empty clears any stale set.
    config::save_invite_fronts(&dir, &invite.fronts)?;
    if let Some(wg) = &invite.wireguard {
        // the tunnel bootstrap the joining node dials BEFORE any p2p; kept
        // beside the token so `run_node` can bring the interface up first.
        config::save_invite_wireguard(&dir, &invite.token.issuer, wg)?;
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
    if descriptor.validators.contains(&me_hex) {
        eprintln!(
            "this identity is a member — start: ducktape-node --config {}/node.toml",
            dir.display()
        );
    } else {
        eprintln!(
            "NOT yet a member. start now — `ducktape-node --config {}/node.toml` redeems \
             this invite automatically: the node joins the network's VPN, syncs state, and \
             comes up as a full node. no approval step follows (minting the invite WAS the \
             approval); a member can later promote it into the quorum with `promote {me_hex}`.",
            dir.display()
        );
    }
    println!("{me_hex}");
    Ok(())
}
