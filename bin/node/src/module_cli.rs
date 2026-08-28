//! `ducktape module …` — the operator's side of a live code swap.
//!
//! `update`/`register` stage a component at this node's owner-gated admin
//! route (which fans it out to every validator and returns their receipts),
//! then drive the governance proposal that schedules it. `status` reads the
//! lifecycle registry. Nothing here runs inside the node.

use std::path::PathBuf;

use crate::cli::rpc_query;
use crate::cli_args::{Selector, StatusArgs};
use crate::config::{self, hex_bytes};

type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// the `module` family's verbs.
#[derive(Debug, clap::Subcommand)]
pub enum ModuleCmd {
    /// schedule a code swap for a registered module
    Update(StageArgs),
    /// register a new module id with its first code
    Register(StageArgs),
    /// the lifecycle registry: active code and any pending swap per module
    Status(StatusArgs),
}

/// `<id> <component.wasm> [--after N]` — shared by update and register.
#[derive(Debug, clap::Args)]
pub struct StageArgs {
    /// the module id the code belongs to
    #[arg(value_name = "ID")]
    pub id: String,
    /// the component bytes to stage
    #[arg(value_name = "COMPONENT.WASM")]
    pub component: PathBuf,
    /// blocks after the current height at which the swap activates
    #[arg(long, default_value_t = 50)]
    pub after: u64,
    #[command(flatten)]
    pub selector: Selector,
}

/// dispatch one `module` verb.
pub fn run(cmd: ModuleCmd) -> CommandResult {
    match cmd {
        ModuleCmd::Update(args) => cmd_update(args),
        ModuleCmd::Register(args) => cmd_register(args),
        ModuleCmd::Status(args) => cmd_status(args),
    }
}

fn cmd_update(args: StageArgs) -> CommandResult {
    Err(lands_next_commit("update", &args))
}

fn cmd_register(args: StageArgs) -> CommandResult {
    Err(lands_next_commit("register", &args))
}

/// the staging verbs land in the next commit; until then refuse in one line
/// that repeats back what was typed, so the grammar is exercisable today.
fn lands_next_commit(verb: &str, args: &StageArgs) -> Box<dyn std::error::Error> {
    format!(
        "module {verb} lands in the next commit (would stage {} for `{}`, activating {} blocks out)",
        args.component.display(),
        args.id,
        args.after
    )
    .into()
}

/// `module status [--config node.toml] [--json]`
fn cmd_status(args: StatusArgs) -> CommandResult {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("module status reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let modules = read_module_status(&rpc_addr)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&modules)?);
        return Ok(());
    }
    print!("{}", render_status(&modules));
    Ok(())
}

/// the lifecycle registry over the generic query lane — the same shape
/// `read_members` uses for governance.
pub(super) fn read_module_status(rpc_addr: &str) -> Result<Vec<lifecycle::ModuleCode>, String> {
    use lifecycle::{LifecycleQuery, LifecycleReply, decode_reply, encode_query};
    let raw = rpc_query(
        rpc_addr,
        "lifecycle",
        &encode_query(&LifecycleQuery::ModuleStatus),
    )?;
    match decode_reply(&raw)? {
        LifecycleReply::ModuleStatus { modules } => Ok(modules),
        other => Err(format!("expected ModuleStatus, got {other:?}")),
    }
}

/// how much of a code hash a row shows — enough to tell two builds apart at a
/// glance, short enough that the table stays one line per module.
const SHORT_HASH: usize = 12;

/// one row per module: `id  active  pending` — pending is `—`, or
/// `<hash> ready <k|✓> activation <h>`.
pub(super) fn render_status(modules: &[lifecycle::ModuleCode]) -> String {
    let id_width = modules
        .iter()
        .map(|m| m.module_id.len())
        .max()
        .unwrap_or(2)
        .max(2);
    let mut out = format!("{:<id_width$}  {:<SHORT_HASH$}  pending\n", "id", "active");
    for m in modules {
        let active = short(&m.active_code_hash);
        let pending = match &m.pending {
            None => "—".to_string(),
            Some(swap) => format!(
                "{}  ready {}  activation {}",
                short(&swap.code_hash),
                readiness_word(swap),
                swap.activation_height
            ),
        };
        out.push_str(&format!(
            "{:<id_width$}  {active}  {pending}\n",
            m.module_id
        ));
    }
    out
}

/// how far a pending swap's readiness has come: the count of validators that
/// signalled, or `✓` once the latch covered the whole set.
fn readiness_word(swap: &lifecycle::ScheduledSwap) -> String {
    let latched = swap.ready;
    if latched {
        return "✓".into();
    }
    swap.readiness.len().to_string()
}

fn short(hash: &[u8]) -> String {
    hex_bytes(hash).chars().take(SHORT_HASH).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifecycle::{ModuleCode, ScheduledSwap};

    #[test]
    fn status_rows_show_active_pending_and_readiness() {
        let active = vec![0xabu8; 32];
        let next = vec![0xcdu8; 32];
        let modules = vec![
            ModuleCode {
                module_id: "acl".into(),
                active_code_hash: active.clone(),
                pending: None,
            },
            ModuleCode {
                module_id: "hello".into(),
                active_code_hash: active.clone(),
                pending: Some(ScheduledSwap {
                    name: "hello-2".into(),
                    activation_height: 120,
                    code_hash: next.clone(),
                    readiness: vec![vec![1], vec![2]],
                    ready: false,
                }),
            },
        ];
        let out = render_status(&modules);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "id     active        pending");
        assert_eq!(lines[1], "acl    abababababab  —");
        assert_eq!(
            lines[2],
            "hello  abababababab  cdcdcdcdcdcd  ready 2  activation 120"
        );
    }
}
