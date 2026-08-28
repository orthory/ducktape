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

/// what the node's stage route answers with: the digest it ingested, its
/// length, and one receipt per peer the code plane fanned the bytes out to.
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct StageReply {
    /// hex sha256 of the artifact as the node stored it.
    pub digest: String,
    /// the artifact's length in bytes, as the node counted it.
    pub len: u64,
    /// one row per member — empty when the node fanned nothing out.
    pub receipts: Vec<PeerReceipt>,
}

/// one peer's answer to the fan-out (`noded::module_code::CodePeerReceipt`).
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct PeerReceipt {
    /// hex of the peer's public key.
    pub peer: String,
    /// "stored" | "already-have" | the peer's refusal reason.
    pub status: String,
    /// whether that peer now holds the bytes.
    pub ok: bool,
}

/// Stage the bytes at this node's owner-gated admin route with fan-out on: the
/// node stores them, pushes them to every validator, and answers with the
/// digest and one receipt per peer. Loopback exposure wants the operator token
/// minted beside `node.toml` at boot.
pub(super) fn stage_component(
    http_base: &str,
    workspace: &std::path::Path,
    bytes: &[u8],
) -> Result<StageReply, String> {
    let token = noded::admin::read_operator_token(workspace)?;
    let resp = reqwest::blocking::Client::new()
        .post(format!(
            "{http_base}/v1/admin/module-code/stage?fanout=true"
        ))
        .header(noded::admin::ADMIN_TOKEN_HEADER, token)
        .body(bytes.to_vec())
        .send()
        .map_err(|e| format!("stage: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    let refused = !status.is_success();
    if refused {
        return Err(format!("stage rejected ({status}): {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("stage reply: {e}: {text}"))
}

/// One non-ok receipt and nothing is proposed: the swap only makes sense once
/// every validator can run the code. The refusal names each holdout peer with
/// the status token its node reported.
pub(super) fn refuse_on_bad_receipts(reply: &StageReply) -> Result<(), String> {
    let failing: Vec<&PeerReceipt> = reply.receipts.iter().filter(|r| !r.ok).collect();
    let every_peer_holds_it = failing.is_empty();
    if every_peer_holds_it {
        return Ok(());
    }
    let mut table = String::from("peer  status\n");
    for r in failing {
        table.push_str(&format!("{}  {}\n", r.peer, r.status));
    }
    table.push_str(
        "not proposed: every validator must hold the bytes before a swap is scheduled \
         — re-run once they are reachable (staging is idempotent)",
    );
    Err(table)
}

/// The digest the proposal will carry is the sha256 of the bytes WE read; the
/// node's answer must agree or something between us rewrote the file.
pub(super) fn digest_matches(reply: &StageReply, bytes: &[u8]) -> Result<[u8; 32], String> {
    use sha2::Digest as _;
    let ours: [u8; 32] = sha2::Sha256::digest(bytes).into();
    let theirs = config::unhex(&reply.digest).map_err(|e| format!("stage digest: {e}"))?;
    let agree = theirs[..] == ours[..];
    if !agree {
        return Err(format!(
            "stage digest {} is not the sha256 of the file we read ({})",
            reply.digest,
            hex_bytes(&ours)
        ));
    }
    Ok(ours)
}

/// how much of a code hash a row shows — enough to tell two builds apart at a
/// glance, short enough that the table stays one line per module.
const SHORT_HASH: usize = 12;

/// one row per module: `id  active  pending`. Either column is `—` when there
/// is nothing to show; pending is otherwise `<hash> ready <k|✓> activation <h>`.
pub(super) fn render_status(modules: &[lifecycle::ModuleCode]) -> String {
    let id_width = modules
        .iter()
        .map(|m| m.module_id.len())
        .max()
        .unwrap_or_default()
        .max(2);
    let mut out = format!("{:<id_width$}  {:<SHORT_HASH$}  pending\n", "id", "active");
    for m in modules {
        // `module register` writes an EMPTY active hash and leaves it empty
        // until the swap activates — the first thing an operator looks at.
        let never_activated = m.active_code_hash.is_empty();
        let active = if never_activated {
            "—".to_string()
        } else {
            short(&m.active_code_hash)
        };
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
            "{:<id_width$}  {active:<SHORT_HASH$}  {pending}\n",
            m.module_id
        ));
    }
    out
}

/// how far a pending swap's readiness has come: the count of validators that
/// signalled, or `✓` once the latch covered the whole set.
fn readiness_word(swap: &lifecycle::ScheduledSwap) -> String {
    if swap.ready {
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
    fn a_single_bad_receipt_refuses_and_names_the_peer() {
        let reply = StageReply {
            digest: "00".repeat(32),
            len: 3,
            receipts: vec![
                PeerReceipt {
                    peer: "aa11".into(),
                    status: "stored".into(),
                    ok: true,
                },
                PeerReceipt {
                    peer: "bb22".into(),
                    status: "already-have".into(),
                    ok: true,
                },
                PeerReceipt {
                    peer: "cc33".into(),
                    status: "module_artifact_too_large".into(),
                    ok: false,
                },
            ],
        };
        let err = refuse_on_bad_receipts(&reply).unwrap_err();
        assert!(err.contains("cc33  module_artifact_too_large"), "{err}");
        assert!(!err.contains("aa11"), "ok peers are not listed: {err}");
        let all_ok = StageReply {
            receipts: reply.receipts[..2].to_vec(),
            ..reply
        };
        assert!(refuse_on_bad_receipts(&all_ok).is_ok());
    }

    #[test]
    fn the_node_digest_must_be_the_bytes_we_read() {
        use sha2::Digest as _;
        let bytes = b"component";
        let want = sha2::Sha256::digest(bytes);
        let good = StageReply {
            digest: hex_bytes(&want),
            len: 9,
            receipts: vec![],
        };
        assert_eq!(digest_matches(&good, bytes).unwrap()[..], want[..]);
        let lying = StageReply {
            digest: "ff".repeat(32),
            len: 9,
            receipts: vec![],
        };
        let err = digest_matches(&lying, bytes).unwrap_err();
        assert!(err.contains("digest"), "{err}");
    }

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
            // `module register`: no active code at all until the swap lands.
            ModuleCode {
                module_id: "runs".into(),
                active_code_hash: Vec::new(),
                pending: Some(ScheduledSwap {
                    name: "runs-1".into(),
                    activation_height: 120,
                    code_hash: next.clone(),
                    readiness: Vec::new(),
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
        // the active column stays 12 wide even when it is a single dash, so
        // the pending hashes line up with the row above.
        assert_eq!(
            lines[3],
            "runs   —             cdcdcdcdcdcd  ready 0  activation 120"
        );
    }
}
