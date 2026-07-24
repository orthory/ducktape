//! the typed `ducktape node` grammar — clap derive. this file is only the
//! SHAPE (verbs, flags, help text); the handlers live in `cli.rs`, the run
//! path in `main.rs`. required arguments are enforced (and rendered) by clap,
//! optional ones are optional because every one has a working default.

use std::path::PathBuf;

use crate::config;

/// the `ducktape node` verb tree. `run` is the node-boot path (owned by
/// `main.rs`); every other verb is a synchronous operator command.
// parsed once on the stack and immediately consumed — variant size is noise.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, clap::Subcommand)]
pub enum NodeCmd {
    /// run a workspace's node until killed (^C checkpoints and exits)
    Run(RunArgs),
    #[command(flatten)]
    Op(OpCmd),
}

/// the operator verbs — everything under `ducktape node` except `run`.
#[derive(Debug, clap::Subcommand)]
pub enum OpCmd {
    /// generate or reuse a node identity (prints its pubkey)
    Key(KeyArgs),
    /// found a new network (default dir: ~/.ducktape/workspaces/<chain-id>)
    Init(InitArgs),
    /// mint a single-use bearer invite blob
    Invite(InviteArgs),
    /// pre-genesis: add a key to the validator set
    Admit(AdmitArgs),
    /// materialize a workspace from an invite blob
    Join(JoinCmd),
    /// list registered workspaces (chain-id + config path)
    List,
    /// the running node's tip: height + root hash (reads the local rpc)
    Status(StatusArgs),
    /// the running node's direct peers: connection, traffic, sync heights
    Peers(StatusArgs),
    /// resident standing: the staged-admission tier
    #[command(subcommand)]
    Resident(ResidentCmd),
    /// consensus-quorum membership
    #[command(subcommand)]
    Member(MemberCmd),
}

#[derive(Debug, clap::Subcommand)]
pub enum ResidentCmd {
    /// grant resident standing to a joiner (drives governance on the running node)
    Accept(PubkeyArgs),
    /// revoke resident standing
    Remove(PubkeyArgs),
}

#[derive(Debug, clap::Subcommand)]
pub enum MemberCmd {
    /// seat a key in the consensus quorum
    Promote(PubkeyArgs),
    /// remove a validator from the set
    Remove(PubkeyArgs),
    /// this node drives its own removal
    Leave(SelectorArgs),
    /// print in-set + validator count for this node
    Status(StatusArgs),
}

/// which workspace a verb operates on. resolution ladder, first hit wins:
/// `-n/--network` (registry), `--config`, `./node.toml` when present, then
/// the single registered workspace when exactly one exists.
#[derive(Debug, Default, clap::Args)]
pub struct Selector {
    /// a registered workspace's chain id — unique prefix ok (`node list`)
    #[arg(
        short = 'n',
        long = "network",
        value_name = "CHAIN-ID",
        conflicts_with = "config"
    )]
    pub network: Option<String>,
    /// explicit path to a workspace's node.toml
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

impl Selector {
    /// resolve the ladder to a node.toml path. steps 1–3 are the historical
    /// behavior; step 4 (the lone registered workspace) is what lets a
    /// freshly-init'd machine run `ducktape node run` with no flags at all.
    pub fn config_path(&self) -> Result<PathBuf, String> {
        if let Some(needle) = &self.network {
            return config::find_workspace_config(needle);
        }
        if let Some(path) = &self.config {
            return Ok(path.clone());
        }
        let local = PathBuf::from("node.toml");
        if local.exists() {
            return Ok(local);
        }
        let mut workspaces = config::list_workspaces()?;
        match workspaces.len() {
            1 => {
                let (chain_id, path) = workspaces.swap_remove(0);
                eprintln!("using workspace {chain_id} ({})", path.display());
                Ok(path)
            }
            0 => Err(
                "no workspace selected: no ./node.toml here and no registered workspaces — \
                 found one with `ducktape node init` or `ducktape node join <invite>`"
                    .into(),
            ),
            _ => Err(format!(
                "no workspace selected and several are registered — pick one with -n:\n{}",
                workspaces
                    .iter()
                    .map(|(chain_id, _)| format!("  {chain_id}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )),
        }
    }
}

/// a verb whose only arguments are the workspace selector.
#[derive(Debug, clap::Args)]
pub struct SelectorArgs {
    #[command(flatten)]
    pub selector: Selector,
}

/// selector + the machine-readable output toggle.
#[derive(Debug, clap::Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub selector: Selector,
    /// emit one machine-readable JSON object instead of prose
    #[arg(long)]
    pub json: bool,
}

/// a membership verb: the subject key + the workspace selector.
#[derive(Debug, clap::Args)]
pub struct PubkeyArgs {
    /// the subject's hex node pubkey
    #[arg(value_name = "HEX-PUBKEY")]
    pub pubkey: String,
    #[command(flatten)]
    pub selector: Selector,
}

#[derive(Debug, clap::Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub selector: Selector,
    /// exit once state sync completes instead of validating
    #[arg(long)]
    pub sync_only: bool,
}

#[derive(Debug, clap::Args)]
pub struct KeyArgs {
    /// write the identity file here
    #[arg(long, value_name = "PATH", conflicts_with = "dir")]
    pub out: Option<PathBuf>,
    /// mint (or reuse) <DIR>/identity.key, creating the dir
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct InitArgs {
    /// human-readable network name (the chain id becomes <name>#<salt>)
    #[arg(long, value_name = "NAME")]
    pub name: String,
    /// found the network here instead of the registry default
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    /// workstation node: run agent sessions in a sandbox and announce this
    /// host's provider capabilities (sets sandbox = podman/tart + announce). A
    /// plain consensus node omits this and stays `sandbox = "direct"`.
    #[arg(long)]
    pub compute: bool,
    #[command(flatten)]
    pub plumbing: PlumbingArgs,
}

#[derive(Debug, clap::Args)]
pub struct InviteArgs {
    /// the standing this invite grants when redeemed
    #[arg(long, value_enum, default_value_t = InviteRoleArg::Resident)]
    pub role: InviteRoleArg,
    /// days until the token expires (default: 30 resident, 1 client)
    #[arg(long, value_name = "N")]
    pub ttl_days: Option<u64>,
    #[command(flatten)]
    pub selector: Selector,
}

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum InviteRoleArg {
    /// a node that joins the mesh and pre-syncs state
    Resident,
    /// submit-only access, redeemed with `ducktape user redeem-invite`
    Client,
}

impl From<InviteRoleArg> for config::InviteRole {
    fn from(role: InviteRoleArg) -> Self {
        match role {
            InviteRoleArg::Resident => config::InviteRole::Resident,
            InviteRoleArg::Client => config::InviteRole::Client,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct AdmitArgs {
    /// the hex node pubkey to seed into the genesis validator set
    #[arg(value_name = "HEX-PUBKEY")]
    pub pubkey: String,
    #[command(flatten)]
    pub selector: Selector,
}

/// `join` is both a leaf (`join <blob>`) and a prefix (`join requests`,
/// `join state`) — a subcommand token wins, anything else is the blob.
#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct JoinCmd {
    #[command(subcommand)]
    pub query: Option<JoinQuery>,
    /// the one-line invite blob a member minted
    #[arg(value_name = "INVITE-BLOB")]
    pub blob: Option<String>,
    /// materialize here instead of the registry dir named by the chain id
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    #[command(flatten)]
    pub plumbing: PlumbingArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum JoinQuery {
    /// list parked joiners delivered to this member's node (JSON)
    Requests(SelectorArgs),
    /// this node's authoritative onboarding phase (JSON)
    State(SelectorArgs),
}

/// the network plumbing `init` and `join` share. every flag overrides a
/// compiled default (or a value an existing node.toml already carries); an
/// absent flag is deliberately NOT persisted, so the runtime keeps
/// re-deriving the same default the descriptor was founded with.
#[derive(Debug, Default, clap::Args)]
pub struct PlumbingArgs {
    /// p2p mesh listen address
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub listen: Option<String>,
    /// the address other members dial (or "overlay")
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub advertised: Option<String>,
    /// node HTTP API listen address
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub http: Option<String>,
    /// browser gateway listen address
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub gateway: Option<String>,
    /// local operator rpc listen address
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub rpc: Option<String>,
    /// ambient coordinator, `host:port` or `none`
    #[arg(long, value_name = "HOST:PORT|none", hide_short_help = true)]
    pub primary_coordinator: Option<String>,
    /// WireGuard UDP listen address (enables the reachability plane)
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub wireguard_listen: Option<String>,
    /// externally visible WireGuard endpoint (port-forwarded setups)
    #[arg(long, value_name = "HOST:PORT", hide_short_help = true)]
    pub wireguard_advertised: Option<String>,
    /// invite intro listener address
    #[arg(long, value_name = "ADDR", hide_short_help = true)]
    pub invite_listen: Option<String>,
}
