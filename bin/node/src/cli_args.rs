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

/// which NODE a verb DIALS: the http base of a running node's `/v1` surface.
/// Deliberately a different question from [`Selector`], which resolves a
/// workspace's node.toml PATH for the daemon that IS the node.
///
/// `--node` is an http base here and means nothing else anywhere: the `agent`
/// family's host targeting — which PEER runs the work, a display name or a raw
/// node key — is `--host-node`, because it is a different type of input.
#[derive(Debug, Default, clap::Args)]
pub struct NodeAddr {
    /// the node's http base url (wins over -n/--network and DUCKTAPE_NODE)
    #[arg(long, value_name = "HTTP-URL", global = true)]
    pub node: Option<String>,
    /// a registered workspace's chain id — resolves to its node.toml http_listen
    #[arg(
        short = 'n',
        long = "network",
        value_name = "CHAIN-ID",
        global = true
    )]
    pub network: Option<String>,
}

/// one rung of the node-addressing ladder — ONE tagged value, so the precedence
/// is a single ordered expression instead of a hand-written `if` chain per
/// family. Four of those existed and disagreed about `DUCKTAPE_NODE`, so
/// `ducktape fs`, `ducktape agent` and `ducktape user redeem-invite` could each
/// dial a DIFFERENT node in one shell.
#[derive(Debug)]
enum Rung {
    /// `--node <http-url>`
    Flag(String),
    /// `-n/--network <chain-id>` → the workspace node.toml's `http_listen`
    Network(String),
    /// the `DUCKTAPE_NODE` environment variable
    Env(String),
    /// the caller's own ambient address (`fs` inside a checkout: the `.duckfs`
    /// index's recorded node url)
    Context(String),
    /// the single registered workspace, when exactly one is registered
    LoneWorkspace,
}

/// the message every unresolved address ends with — it names every rung, so a
/// user who hit the bottom of the ladder can see all of it.
const NO_NODE_ADDRESS: &str =
    "no node address: pass --node <http-url>, -n/--network <id>, or set DUCKTAPE_NODE";

/// turn a chosen rung into the http base. The one `match`: a new rung must be
/// routed here or the build fails.
fn rung_base(rung: Rung) -> Result<String, String> {
    match rung {
        Rung::Flag(url) | Rung::Env(url) | Rung::Context(url) => Ok(trim_base(&url)),
        Rung::Network(needle) => http_of_workspace(&needle),
        Rung::LoneWorkspace => lone_workspace_base(),
    }
}

/// a trailing slash on the base would double up against every `/v1/...` path.
fn trim_base(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn http_of_workspace(needle: &str) -> Result<String, String> {
    let (_dir, http) = config::resolve_network(needle)?;
    let base = http.ok_or_else(|| {
        format!(
            "network {needle:?} resolves to a workspace with no http listen \
             (its node.toml sets no http_listen) — pass --node <http-url>"
        )
    })?;
    Ok(trim_base(&base))
}

/// the bottom rung: infer the node from the registry when exactly one workspace
/// is registered — the same "a freshly-init'd machine runs with no flags at all"
/// ergonomic [`Selector::config_path`] already has.
fn lone_workspace_base() -> Result<String, String> {
    let mut workspaces = config::list_workspaces()?;
    match workspaces.len() {
        1 => {
            let (chain_id, _path) = workspaces.swap_remove(0);
            http_of_workspace(&chain_id)
        }
        0 => Err(NO_NODE_ADDRESS.into()),
        _ => Err(format!(
            "{NO_NODE_ADDRESS}\nseveral workspaces are registered — pick one with -n:\n{}",
            workspaces
                .iter()
                .map(|(chain_id, _)| format!("  {chain_id}"))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    }
}

/// the ONE read of `DUCKTAPE_NODE` in the CLI — see
/// `the_cli_reads_ducktape_node_in_exactly_one_place`.
fn env_node() -> Option<String> {
    std::env::var("DUCKTAPE_NODE").ok()
}

impl NodeAddr {
    /// the whole ladder, for a caller with no ambient address of its own.
    pub fn resolve(&self) -> Result<String, String> {
        self.resolve_with(|| None)
    }

    /// the whole ladder: `--node` → `-n/--network` → `DUCKTAPE_NODE` →
    /// `context()` → the lone registered workspace.
    ///
    /// `context` is the caller's own ambient address, tried AFTER what the
    /// operator stated and BEFORE the registry inference. `fs` inside a checkout
    /// passes the `.duckfs` index's recorded url: more specific than "the one
    /// workspace registered on this box", less specific than a flag.
    pub fn resolve_with(
        &self,
        context: impl FnOnce() -> Option<String>,
    ) -> Result<String, String> {
        rung_base(self.ladder_rung(env_node(), context))
    }

    /// pick the rung. `env` is a parameter rather than a read so the precedence
    /// is testable without mutating process env — racy across parallel tests,
    /// and `unsafe` since edition 2024.
    fn ladder_rung(&self, env: Option<String>, context: impl FnOnce() -> Option<String>) -> Rung {
        let flag = self.node.clone().filter(|url| !url.is_empty());
        let network = self.network.clone().filter(|id| !id.is_empty());
        let env = env.filter(|url| !url.is_empty());
        // THE PRECEDENCE. Written once, in one expression, for every family.
        flag.map(Rung::Flag)
            .or_else(|| network.map(Rung::Network))
            .or_else(|| env.map(Rung::Env))
            .or_else(|| context().filter(|url| !url.is_empty()).map(Rung::Context))
            .unwrap_or(Rung::LoneWorkspace)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(node: Option<&str>, network: Option<&str>) -> NodeAddr {
        NodeAddr {
            node: node.map(str::to_string),
            network: network.map(str::to_string),
        }
    }

    /// the precedence, pinned rung by rung and hermetically: only the `Flag`,
    /// `Env` and `Context` rungs are resolved to a base (the registry rungs are
    /// asserted as rungs, so no test ever walks `~/.ducktape`).
    #[test]
    fn the_node_address_ladder_ranks_flag_network_env_context_registry() {
        let env = || Some("http://env:1/".to_string());
        let ctx = || Some("http://ctx:1/".to_string());

        // 1. --node wins over everything below it.
        let rung = addr(Some("http://flag:1/"), Some("some-workspace")).ladder_rung(env(), ctx);
        assert_eq!(rung_base(rung).unwrap(), "http://flag:1");

        // 2. -n/--network beats the env — the rung `user redeem-invite` used to
        //    reach only because it ignored DUCKTAPE_NODE entirely.
        assert!(matches!(
            addr(None, Some("some-workspace")).ladder_rung(env(), ctx),
            Rung::Network(id) if id == "some-workspace"
        ));

        // 3. the env beats the caller's ambient context.
        assert_eq!(
            rung_base(addr(None, None).ladder_rung(env(), ctx)).unwrap(),
            "http://env:1"
        );

        // 4. the context beats the registry: `fs commit` inside a checkout must
        //    reach the node it was checked out FROM, not "the one workspace
        //    registered on this box".
        assert_eq!(
            rung_base(addr(None, None).ladder_rung(None, ctx)).unwrap(),
            "http://ctx:1"
        );

        // 5. nothing at all → the registry inference, the bottom rung.
        assert!(matches!(
            addr(None, None).ladder_rung(None, || None),
            Rung::LoneWorkspace
        ));
    }

    /// an empty flag/env/context value is NOT an address — an exported but empty
    /// `DUCKTAPE_NODE` must fall through, not resolve to `""`.
    #[test]
    fn an_empty_value_is_not_a_rung() {
        assert!(matches!(
            addr(Some(""), Some("")).ladder_rung(Some(String::new()), || Some(String::new())),
            Rung::LoneWorkspace
        ));
    }

    /// the fifth-caller guard. `DUCKTAPE_NODE` was read by three families with
    /// three different precedences, so `ducktape fs`, `ducktape agent` and
    /// `ducktape user redeem-invite` could each dial a different node in one
    /// shell. There is now exactly ONE read; a family that hand-writes its own
    /// ladder fails here instead of shipping a fourth answer.
    ///
    /// `bin/node/src/mcp/` is not an exception to find: it binds a RUN's tool
    /// plane to its node through `mcp::identity::ENV_NODE`, which is a different
    /// consumer and not a CLI addressing flag.
    // ponytail: matches the literal `env::var("DUCKTAPE_NODE")` call, so a
    // fifth caller routing through a const would slip past. Escalate to a full
    // parse only if that ever actually happens.
    #[test]
    fn the_cli_reads_ducktape_node_in_exactly_one_place() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut readers = Vec::new();
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read src dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read source");
                let reads_env = text.contains(r#"env::var("DUCKTAPE_NODE")"#)
                    || text.contains(r#"env::var_os("DUCKTAPE_NODE")"#);
                if reads_env {
                    readers.push(
                        path.strip_prefix(&src)
                            .expect("under src")
                            .display()
                            .to_string(),
                    );
                }
            }
        }
        readers.sort();
        assert_eq!(
            readers,
            vec!["cli_args.rs".to_string()],
            "DUCKTAPE_NODE must be read only by the one node-addressing ladder"
        );
    }
}
