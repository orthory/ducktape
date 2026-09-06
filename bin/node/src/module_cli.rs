//! `ducktape module …` — the operator's side of a live code swap.
//!
//! `update`/`register` stage a component at this node's owner-gated admin
//! route (which fans it out to every validator and returns their receipts),
//! then drive the governance proposal that schedules it. `status` reads the
//! modules registry. Nothing here runs inside the node.

use std::path::PathBuf;

use commonware_cryptography::Signer as _;

use crate::cli::{CeremonyOutcome, GovSigner, rpc_call, rpc_query};
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
    /// the modules registry: active code and any pending swap per module
    Status(StatusArgs),
}

/// `<id> <component.wasm> [--index <index.wasm>] [--after N]` — shared by update and register.
#[derive(Debug, clap::Args)]
pub struct StageArgs {
    /// the module id the code belongs to
    #[arg(value_name = "ID")]
    pub id: String,
    /// the component bytes to stage
    #[arg(value_name = "COMPONENT.WASM")]
    pub component: PathBuf,
    /// Optional mapper deployed and activated with this component; omission removes it.
    #[arg(long, value_name = "INDEX.WASM")]
    pub index: Option<PathBuf>,
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

/// which staging verb is running — the only difference between the two is
/// the governance action they propose.
#[derive(Clone, Copy, Debug)]
enum Verb {
    Update,
    Register,
}

impl Verb {
    /// the two-token spelling the ceremony narrates (`ducktape module update …`).
    fn name(self) -> &'static str {
        match self {
            Verb::Update => "module update",
            Verb::Register => "module register",
        }
    }

    /// the governance action for this verb; `<id>@<12hex>` names the proposal
    /// (governance only requires the name be non-empty).
    fn action(
        self,
        module_id: &str,
        activation_height: u64,
        code_hash: [u8; 32],
    ) -> governance::GovAction {
        let name = format!("{module_id}@{}", short(&code_hash));
        let module_id = module_id.to_string();
        let code_hash = code_hash.to_vec();
        match self {
            Verb::Update => governance::GovAction::UpdateModule {
                name,
                module_id,
                activation_height,
                code_hash,
            },
            Verb::Register => governance::GovAction::RegisterModule {
                name,
                module_id,
                activation_height,
                code_hash,
            },
        }
    }
}

fn cmd_update(args: StageArgs) -> CommandResult {
    cmd_stage_and_schedule(args, Verb::Update)
}

fn cmd_register(args: StageArgs) -> CommandResult {
    cmd_stage_and_schedule(args, Verb::Register)
}

/// the ceremony's "same proposal" test for a module verb: the same variant,
/// module and code is the same proposal whatever activation height the
/// proposer computed — each member computes its own from its own height —
/// PROVIDED that activation still clears `floor`, this node's
/// `height + MIN_SWAP_LEAD`. the registry applies that floor at execute
/// time, so a proposal already inside it can never be scheduled: joining it
/// would land one more ballot on a doomed proposal and leave the operator
/// unable to schedule those bytes until it expires. such a proposal is
/// skipped and a fresh one minted.
fn matches_module_action<'a>(
    verb: Verb,
    module_id: &'a str,
    code_hash: &'a [u8],
    floor: u64,
) -> impl Fn(&governance::GovAction) -> bool + 'a {
    use governance::GovAction;
    move |action| match (verb, action) {
        (
            Verb::Update,
            GovAction::UpdateModule {
                module_id: m,
                code_hash: h,
                activation_height,
                ..
            },
        )
        | (
            Verb::Register,
            GovAction::RegisterModule {
                module_id: m,
                code_hash: h,
                activation_height,
                ..
            },
        ) => {
            let same_code = m == module_id && h.as_slice() == code_hash;
            let still_schedulable = *activation_height > floor;
            same_code && still_schedulable
        }
        (Verb::Update, _) | (Verb::Register, _) => false,
    }
}

/// `module update|register <id> <component.wasm> [--index <index.wasm>] [--after N]`: stage the bytes
/// at this node (fan-out to every validator), refuse unless every member holds
/// them, then drive the governance proposal that schedules the swap at this
/// node's height + N and read the registry back for its verdict.
fn cmd_stage_and_schedule(args: StageArgs, verb: Verb) -> CommandResult {
    config::validate_module_id(&args.id)?;
    // the static half of the registry's lead rule, checked before anything is
    // staged or proposed: an activation at or under the floor is refused at
    // execute whatever the ceremony does.
    let lead_too_short = args.after <= modules::MIN_SWAP_LEAD;
    if lead_too_short {
        return Err(format!(
            "--after {} cannot schedule anything: activation must exceed height+MIN_SWAP_LEAD ({}), \
             and the ceremony's own blocks eat into the lead — leave room (the default is 50)",
            args.after,
            modules::MIN_SWAP_LEAD
        )
        .into());
    }
    let component = std::fs::read(&args.component)
        .map_err(|e| format!("read {}: {e}", args.component.display()))?;
    let index = args
        .index
        .as_ref()
        .map(|path| std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display())))
        .transpose()?;
    let bytes = module_artifact::ModuleArtifact { component, index }.encode();
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let node = crate::cli::DrivenNode::of(&resolved, verb.name())?;
    let rpc_addr = node.rpc();
    let http_base = node.http_base();
    let code_hash: [u8; 32] = {
        use sha2::Digest as _;
        sha2::Sha256::digest(&bytes).into()
    };

    // 1. the registry's static rules, before anything is staged or proposed:
    //    each would reject governance's execute in-kernel and leave a
    //    proposal open for its whole voting period.
    let live_modules = read_live_modules(rpc_addr)?;
    let precheck = registry_precheck(
        verb,
        &read_module_status(rpc_addr)?,
        &live_modules,
        &args.id,
        &code_hash,
    )?;
    match precheck {
        Precheck::Proceed => {}
        // a member running the verb after the deciding ballot (or after the
        // activation): the registry already holds these bytes, so there is no
        // proposal to join or mint.
        Precheck::AlreadyHeld(held) => {
            eprintln!(
                "{} → {} is already {} — nothing to do; track with: ducktape module status",
                args.id,
                hex_bytes(&code_hash),
                held.word()
            );
            return Ok(());
        }
    }

    // 2. stage + fan-out; 3. every member holds the bytes or nothing is proposed
    // the token lives in the node's workspace — its `storage_dir` in the dev
    // shape, which is NOT the config file's directory.
    let reply = stage_component(http_base, &resolved.service.workspace, &bytes)?;
    digest_matches(&reply, &bytes)?;
    eprintln!(
        "staged {} ({} bytes), {} peer receipt(s)",
        hex_bytes(&code_hash),
        reply.len,
        reply.receipts.len()
    );
    // the fan-out never pushes to the staging node itself, so the one valset
    // member allowed no receipt is THIS NODE's key — not the governance
    // signer's, which under share governance is an account key.
    let me_hex = hex_bytes(resolved.signer.public_key().as_ref());
    let members = crate::cli::read_members(rpc_addr)?;
    note_non_member_holdouts(&reply, &members);
    refuse_unless_every_validator_holds(&reply, &members, &me_hex)?;
    let signer = crate::cli::gov_signer(rpc_addr, &cfg_path, &resolved)?;
    let pubkey_hex = signer_pubkey_hex(&signer);

    // 4. activation = this node's height + N (each member computes its own).
    //    an `--after` that wraps would land UNDER the floor in release, where
    //    the matcher's debug assert is compiled out: the verb would mint a
    //    proposal the registry refuses at execute.
    let height = current_height(rpc_addr)?;
    let activation_height = height
        .checked_add(args.after)
        .ok_or("--after overflows the chain height")?;
    let floor = height + modules::MIN_SWAP_LEAD;

    // 5. the ceremony: join an open proposal for the same (verb, id, hash)
    //    that can still be scheduled, or propose; cast yes; execute when
    //    decidable
    let matches = matches_module_action(verb, &args.id, &code_hash, floor);
    let ceremony = crate::cli::drive_proposal_ceremony(
        &node,
        &signer,
        &pubkey_hex,
        verb.name(),
        "module:",
        verb.action(&args.id, activation_height, code_hash),
        &matches,
    );
    let outcome = match ceremony {
        Ok(outcome) => outcome,
        Err(error) => return Err(ceremony_failed(rpc_addr, &args.id, &code_hash, error)),
    };
    match outcome {
        CeremonyOutcome::AwaitingBallots => Ok(()),
        CeremonyOutcome::Passed => confirm_scheduled(rpc_addr, &args.id, &code_hash),
    }
}

/// a passed proposal only ASKED the registry; the CLI's success line is the
/// registry's word, not governance's. read it back before saying "scheduled" —
/// the HEIGHT too: a run that joined an open proposal decided the FIRST
/// proposer's target, not the one it computed from its own height, so its own
/// number would name a block the swap never lands on.
fn confirm_scheduled(rpc_addr: &str, id: &str, code_hash: &[u8; 32]) -> CommandResult {
    let held = registry_holds(&read_module_status(rpc_addr)?, id, code_hash);
    match held {
        Some(Held::Pending { activation_height }) => {
            println!(
                "scheduled {id} → {} at height {activation_height}; track with: ducktape module status",
                hex_bytes(code_hash)
            );
            Ok(())
        }
        // the swap crossed its own activation between the execute and this
        // read: the registry holds the bytes as ACTIVE and there is no pending
        // height left to name.
        Some(Held::Active) => {
            println!(
                "{id} → {} is active; track with: ducktape module status",
                hex_bytes(code_hash)
            );
            Ok(())
        }
        None => Err(format!(
            "proposal passed but the modules registry holds no swap for {id} → {}. {}",
            hex_bytes(code_hash),
            registry_rules()
        )
        .into()),
    }
}

/// the ceremony failed. the one failure the registry causes: governance's
/// `Execute` emits the schedule to the modules registry in the SAME op, so a registry
/// refusal rejects the whole op — the proposal never settles and the ceremony
/// times out waiting for the tally. the proposal carries no reason, so on
/// exactly that failure, with the registry holding nothing for these bytes,
/// the rules are named here. every other ceremony error passes through.
fn ceremony_failed(
    rpc_addr: &str,
    id: &str,
    code_hash: &[u8; 32],
    error: Box<dyn std::error::Error>,
) -> Box<dyn std::error::Error> {
    let tally_never_settled = error.to_string() == crate::cli::TALLY_SETTLE_TIMEOUT;
    if !tally_never_settled {
        return error;
    }
    let Ok(modules) = read_module_status(rpc_addr) else {
        return error;
    };
    let scheduled = registry_holds(&modules, id, code_hash).is_some();
    if scheduled {
        return error;
    }
    format!(
        "{error}: governance's execute was refused by the modules registry, which holds no swap \
         for {id} → {}. {}",
        hex_bytes(code_hash),
        registry_rules()
    )
    .into()
}

/// what the committed registry lets this verb do with these bytes.
#[derive(Debug)]
enum Precheck {
    /// nothing in the registry stands in the way — stage and propose.
    Proceed,
    /// the registry already carries these bytes for the module: a member
    /// running after the deciding ballot, or after the activation. nothing to
    /// do — and the two are different facts, so the line names which.
    AlreadyHeld(Held),
}

/// how the registry carries a code hash for a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    /// the module is RUNNING this code.
    Active,
    /// a scheduled swap will activate this code at its height.
    Pending { activation_height: u64 },
}

impl Held {
    /// the word the operator's "nothing to do" line uses.
    fn word(self) -> &'static str {
        match self {
            Held::Active => "active",
            Held::Pending { .. } => "scheduled",
        }
    }
}

/// the registry's STATIC rules (`modules` `handle_schedule_swap` /
/// `handle_schedule_register`), decided from the committed registry before
/// anything is staged or proposed. each refusal here would otherwise reject
/// governance's execute in-kernel and leave a proposal open for its whole
/// voting period; the wording mirrors the registry's own. pure.
fn registry_precheck(
    verb: Verb,
    modules: &[modules::ModuleCode],
    live_modules: &[String],
    id: &str,
    code_hash: &[u8; 32],
) -> Result<Precheck, String> {
    if let Some(held) = registry_holds(modules, id, code_hash) {
        return Ok(Precheck::AlreadyHeld(held));
    }
    let already_live = live_modules.iter().any(|live| live == id);
    let registering_live_module = matches!(verb, Verb::Register) && already_live;
    if registering_live_module {
        return Err(format!(
            "module {id} is already registered (code changes go through `module update`)"
        ));
    }
    let entry = modules.iter().find(|m| m.module_id == id);
    let other_swap_pending = entry.map(|m| m.pending.is_some());
    match (verb, other_swap_pending) {
        (Verb::Register, None) | (Verb::Update, Some(false)) => Ok(Precheck::Proceed),
        (Verb::Register, Some(true)) | (Verb::Register, Some(false)) => Err(format!(
            "module {id} is already registered (code changes go through `module update`)"
        )),
        (Verb::Update, None) => Err(format!(
            "cannot schedule a swap for unregistered module {id} (`module register` admits a new id)"
        )),
        (Verb::Update, Some(true)) => Err(format!(
            "module {id} already has a pending swap (cancel it first)"
        )),
    }
}

/// the registry's schedule rules, for a refusal it does not narrate itself.
fn registry_rules() -> String {
    format!(
        "its rules: activation must exceed height+MIN_SWAP_LEAD ({}) at EXECUTE time, so --after must \
         leave room for the ceremony's own blocks (the default is 50); one pending swap per module \
         (cancel it first); `register` needs an unregistered id and `update` a registered one; the \
         code must differ from the active code",
        modules::MIN_SWAP_LEAD
    )
}

/// how the registry carries `code_hash` for `id`, if it carries it at all.
/// pure: the one read behind both "nothing to do" and the post-`Passed`
/// confirmation.
fn registry_holds(modules: &[modules::ModuleCode], id: &str, code_hash: &[u8; 32]) -> Option<Held> {
    let entry = modules.iter().find(|m| m.module_id == id)?;
    let our_pending_swap = entry.pending.as_ref().filter(|p| p.code_hash == code_hash);
    if let Some(pending) = our_pending_swap {
        return Some(Held::Pending {
            activation_height: pending.activation_height,
        });
    }
    let already_active = entry.active_code_hash == code_hash;
    already_active.then_some(Held::Active)
}

/// The running host determines which ids are occupied, including modules
/// with no code-registry record. A default build catalog says nothing about
/// the module set of this network.
fn read_live_modules(rpc_addr: &str) -> Result<Vec<String>, String> {
    let reply = rpc_call(rpc_addr, &serde_json::json!({ "cmd": "status" }))?;
    if reply["ok"] != true {
        return Err(format!("status: {}", reply["error"]));
    }
    let Some(modules) = reply["status"]["modules"].as_object() else {
        return Err("node status carries no module roster".into());
    };
    Ok(modules.keys().cloned().collect())
}

/// this node's committed height, from the rpc status snapshot.
fn current_height(rpc_addr: &str) -> Result<u64, String> {
    let reply = rpc_call(rpc_addr, &serde_json::json!({ "cmd": "status" }))?;
    if reply["ok"] != true {
        return Err(format!("status: {}", reply["error"]));
    }
    reply["status"]["height"]
        .as_u64()
        .ok_or_else(|| "node status carries no height".into())
}

/// the signer's public key as hex: the proposal-id seed the ceremony mints
/// from (and nothing else — the receipt gate is keyed by the NODE's key).
fn signer_pubkey_hex(signer: &GovSigner) -> String {
    match signer {
        GovSigner::Node { key } => hex_bytes(key),
        GovSigner::User { key, .. } => hex_bytes(key.public_key().as_ref()),
    }
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

/// the modules registry over the generic query lane — the same shape
/// `read_members` uses for governance.
fn read_module_status(rpc_addr: &str) -> Result<Vec<modules::ModuleCode>, String> {
    use modules::{ModulesQuery, ModulesReply, decode_reply, encode_query};
    let raw = rpc_query(
        rpc_addr,
        "modules",
        &encode_query(&ModulesQuery::ModuleStatus),
    )?;
    match decode_reply(&raw)? {
        ModulesReply::ModuleStatus { modules } => Ok(modules),
        other => Err(format!("expected ModuleStatus, got {other:?}")),
    }
}

/// what the node's stage route answers with: the digest it ingested, its
/// length, and one receipt per peer the code plane fanned the bytes out to.
#[derive(Debug, Clone, serde::Deserialize)]
struct StageReply {
    /// hex sha256 of the artifact as the node stored it.
    pub digest: String,
    /// the artifact's length in bytes, as the node counted it.
    pub len: u64,
    /// one row per member — empty when the node fanned nothing out.
    pub receipts: Vec<PeerReceipt>,
}

/// one peer's answer to the fan-out (`noded::module_code::CodePeerReceipt`).
#[derive(Debug, Clone, serde::Deserialize)]
struct PeerReceipt {
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
fn stage_component(
    http_base: &str,
    workspace: &std::path::Path,
    bytes: &[u8],
) -> Result<StageReply, String> {
    const PATH: &str = "/v1/admin/module-code/stage";
    let token = noded::admin::read_operator_token(workspace)?;
    // the node answers only once the fan-out settles, and it awaits that with
    // no deadline of its own; a wedged push must surface here, not hang.
    let resp = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("stage client: {e}"))?
        .post(format!("{http_base}{PATH}?fanout=true"))
        .header(noded::admin::ADMIN_TOKEN_HEADER, token)
        .header("content-type", "application/octet-stream")
        .body(bytes.to_vec())
        .send()
        .map_err(|error| crate::node_http::transport_failure(PATH, &error).to_string())?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    let refused = !status.is_success();
    if refused {
        return Err(format!("stage rejected ({status}): {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("stage reply: {e}: {text}"))
}

/// the sentence under every receipt refusal — one wording, whichever gate.
const NOT_PROPOSED: &str = "not proposed: every validator must hold the bytes before a swap is scheduled \
                            — re-run once they are reachable (staging is idempotent)";

/// where one validator stands after the fan-out.
enum Standing<'a> {
    /// the staging node itself, or a peer whose receipt is ok.
    Holds,
    /// a peer the fan-out reached whose node refused or lost the transfer:
    /// the status token it reported.
    Refused(&'a str),
    /// a member the fan-out never reached — no row in the receipt table.
    Unreached,
}

/// `member_hex`'s standing: the staging node holds its own bytes (the fan-out
/// never pushes to itself), every other member is read off its receipt.
fn standing<'a>(reply: &'a StageReply, member_hex: &str, me_hex: &str) -> Standing<'a> {
    if member_hex == me_hex {
        return Standing::Holds;
    }
    let Some(receipt) = reply.receipts.iter().find(|r| r.peer == member_hex) else {
        return Standing::Unreached;
    };
    if receipt.ok {
        Standing::Holds
    } else {
        Standing::Refused(&receipt.status)
    }
}

/// The gate: a swap only makes sense once every VALIDATOR can run the code,
/// and the readiness quorum that arms it is the valset, so the receipt table
/// — one row per overlay peer the fan-out reached, validators and residents
/// alike — is read through the member set. Every member is `me_hex` or holds
/// an ok receipt; one holdout (a refused transfer, or no receipt at all
/// because the node never dialled it) and nothing is proposed. The refusal
/// names each holdout with the status its node reported.
fn refuse_unless_every_validator_holds(
    reply: &StageReply,
    members: &[Vec<u8>],
    me_hex: &str,
) -> Result<(), String> {
    let holdouts: Vec<(String, &str)> = members
        .iter()
        .map(|member| hex_bytes(member))
        .filter_map(|member| match standing(reply, &member, me_hex) {
            Standing::Holds => None,
            Standing::Refused(status) => Some((member, status)),
            Standing::Unreached => Some((member, "no receipt (unreachable)")),
        })
        .collect();
    let every_validator_holds_it = holdouts.is_empty();
    if every_validator_holds_it {
        return Ok(());
    }
    let mut table = String::from("peer  status\n");
    for (peer, status) in holdouts {
        table.push_str(&format!("{peer}  {status}\n"));
    }
    table.push_str(NOT_PROPOSED);
    Err(table)
}

/// A peer outside the valset (a resident, a sentry) that did not take the
/// bytes is noted, never a refusal: it is no part of the readiness quorum,
/// and a replica missing a committed artifact fetches it off a peer before
/// the boundary.
fn note_non_member_holdouts(reply: &StageReply, members: &[Vec<u8>]) {
    let member_hex: std::collections::BTreeSet<String> =
        members.iter().map(|member| hex_bytes(member)).collect();
    let non_member_holdouts = reply
        .receipts
        .iter()
        .filter(|r| !r.ok && !member_hex.contains(&r.peer));
    for receipt in non_member_holdouts {
        eprintln!(
            "note: peer {} is not a validator and did not take the bytes ({}); \
             it fetches them itself before the boundary",
            receipt.peer, receipt.status
        );
    }
}

/// The digest the proposal will carry is the sha256 of the bytes WE read; the
/// node's answer must agree or something between us rewrote the file.
fn digest_matches(reply: &StageReply, bytes: &[u8]) -> Result<[u8; 32], String> {
    use sha2::Digest as _;
    let ours: [u8; 32] = sha2::Sha256::digest(bytes).into();
    let theirs = config::unhex(&reply.digest).map_err(|e| format!("stage digest: {e}"))?;
    let agree = theirs[..] == ours[..];
    if !agree {
        return Err(format!(
            "stage digest {} is not the sha256 of the deployment we packaged ({})",
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
fn render_status(modules: &[modules::ModuleCode]) -> String {
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
fn readiness_word(swap: &modules::ScheduledSwap) -> String {
    if swap.ready_at.is_some() {
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
    use modules::{ModuleCode, ScheduledSwap};

    fn receipt(peer: &[u8], status: &str, ok: bool) -> PeerReceipt {
        PeerReceipt {
            peer: hex_bytes(peer),
            status: status.into(),
            ok,
        }
    }

    fn reply(receipts: Vec<PeerReceipt>) -> StageReply {
        StageReply {
            digest: "00".repeat(32),
            len: 3,
            receipts,
        }
    }

    #[test]
    fn a_validator_whose_node_refused_the_bytes_refuses_with_its_status() {
        let me = vec![0x01u8; 32];
        let stored = vec![0x02u8; 32];
        let refused = vec![0x03u8; 32];
        let reply = reply(vec![
            receipt(&stored, "stored", true),
            receipt(&refused, "module_artifact_too_large", false),
        ]);
        let members = vec![me.clone(), stored.clone(), refused.clone()];
        let err =
            refuse_unless_every_validator_holds(&reply, &members, &hex_bytes(&me)).unwrap_err();
        assert!(err.starts_with("peer  status\n"), "{err}");
        assert!(
            err.contains(&format!(
                "{}  module_artifact_too_large",
                hex_bytes(&refused)
            )),
            "{err}"
        );
        assert!(
            !err.contains(&hex_bytes(&stored)),
            "holders are not listed: {err}"
        );
        assert!(err.contains("not proposed"), "{err}");
    }

    #[test]
    fn a_validator_without_a_receipt_refuses_as_unreachable() {
        let me = vec![0x01u8; 32];
        let answered = vec![0x02u8; 32];
        let silent = vec![0x03u8; 32];
        let reply = reply(vec![receipt(&answered, "stored", true)]);
        let members = vec![me.clone(), answered.clone(), silent.clone()];
        let err =
            refuse_unless_every_validator_holds(&reply, &members, &hex_bytes(&me)).unwrap_err();
        assert!(
            err.contains(&format!("{}  no receipt (unreachable)", hex_bytes(&silent))),
            "{err}"
        );
        assert!(
            !err.contains(&hex_bytes(&answered)),
            "answered peers are not listed: {err}"
        );
        // me + every other member answered: nothing missing
        let present = vec![me.clone(), answered];
        assert!(refuse_unless_every_validator_holds(&reply, &present, &hex_bytes(&me)).is_ok());
        // a staging node outside the valset needs every member's receipt
        let err = refuse_unless_every_validator_holds(&reply, &present, "ff").unwrap_err();
        assert!(err.contains(&hex_bytes(&me)), "{err}");
    }

    /// the receipt table names every overlay peer the fan-out reached, so a
    /// resident that could not take the bytes shows up in it; it is no part
    /// of the readiness quorum and must not hold the proposal.
    #[test]
    fn a_non_validator_holdout_does_not_refuse() {
        let me = vec![0x01u8; 32];
        let other_validator = vec![0x02u8; 32];
        let resident = vec![0x03u8; 32];
        let reply = reply(vec![
            receipt(&other_validator, "already-have", true),
            receipt(&resident, "open failed: io: tcp connect refused", false),
        ]);
        let members = vec![me.clone(), other_validator];
        assert!(refuse_unless_every_validator_holds(&reply, &members, &hex_bytes(&me)).is_ok());
    }

    #[test]
    fn the_registry_precheck_refuses_each_static_rule_before_staging() {
        let ours = [0xabu8; 32];
        let theirs = [0xcdu8; 32];
        let swap = |hash: [u8; 32]| ScheduledSwap {
            name: "x".into(),
            activation_height: 9,
            code_hash: hash.to_vec(),
            readiness: Vec::new(),
            ready_at: None,
        };
        let entry = |active: &[u8], pending: Option<ScheduledSwap>| ModuleCode {
            module_id: "hello".into(),
            active_code_hash: active.to_vec(),
            pending,
            history: Vec::new(),
        };
        let precheck =
            |verb, modules: &[ModuleCode]| registry_precheck(verb, modules, &[], "hello", &ours);

        // register: a free id proceeds; any existing entry refuses
        assert!(matches!(
            precheck(Verb::Register, &[]),
            Ok(Precheck::Proceed)
        ));
        let err = precheck(Verb::Register, &[entry(&theirs, None)]).unwrap_err();
        assert!(err.contains("already registered"), "{err}");
        // update: a missing entry refuses; a registered, quiet one proceeds
        let err = precheck(Verb::Update, &[]).unwrap_err();
        assert!(err.contains("unregistered module hello"), "{err}");
        assert!(matches!(
            precheck(Verb::Update, &[entry(&theirs, None)]),
            Ok(Precheck::Proceed)
        ));
        // a pending swap for OTHER bytes refuses either verb
        let busy = [entry(&theirs, Some(swap([0xefu8; 32])))];
        let err = precheck(Verb::Update, &busy).unwrap_err();
        assert!(err.contains("already has a pending swap"), "{err}");
        assert!(precheck(Verb::Register, &busy).is_err());
        // our own bytes: nothing to do, whichever verb — but a swap that has
        // not activated is "scheduled" and code the module already RUNS is
        // "active", and the operator's line says which.
        let pending_ours = [entry(&[], Some(swap(ours)))];
        assert!(matches!(
            precheck(Verb::Register, &pending_ours),
            // the height comes off the REGISTRY's pending swap — the only
            // number the confirmation line may name.
            Ok(Precheck::AlreadyHeld(Held::Pending {
                activation_height: 9
            }))
        ));
        assert!(matches!(
            precheck(Verb::Update, &[entry(&ours, None)]),
            Ok(Precheck::AlreadyHeld(Held::Active))
        ));
        assert_eq!(
            Held::Pending {
                activation_height: 9
            }
            .word(),
            "scheduled"
        );
        assert_eq!(Held::Active.word(), "active");
        // A familiar name is available when this network does not run it.
        assert!(matches!(
            registry_precheck(Verb::Register, &[], &[], "identity", &ours),
            Ok(Precheck::Proceed)
        ));
        let live = vec!["identity".to_string()];
        let err = registry_precheck(Verb::Register, &[], &live, "identity", &ours).unwrap_err();
        assert!(err.contains("already registered"), "{err}");
        let err = registry_precheck(Verb::Update, &[], &live, "identity", &ours).unwrap_err();
        assert!(err.contains("unregistered module identity"), "{err}");
    }

    #[test]
    fn the_matcher_ignores_the_activation_height_but_not_the_verb_or_the_floor() {
        let hash = [0xabu8; 32];
        let floor = 50;
        let update = Verb::Update.action("hello", 100, hash);
        let register = Verb::Register.action("hello", 200, hash);
        let same_update = matches_module_action(Verb::Update, "hello", &hash, floor);
        assert!(same_update(&Verb::Update.action("hello", 999, hash)));
        assert!(same_update(&update));
        assert!(!same_update(&register), "register is not update");
        assert!(!same_update(&Verb::Update.action("other", 100, hash)));
        assert!(!same_update(&Verb::Update.action(
            "hello",
            100,
            [0xcdu8; 32]
        )));
        // a proposal this node's floor has already overtaken cannot be
        // scheduled by anyone: not joined, whatever its code
        assert!(!same_update(&Verb::Update.action("hello", floor, hash)));
        assert!(same_update(&Verb::Update.action("hello", floor + 1, hash)));
        let same_register = matches_module_action(Verb::Register, "hello", &hash, floor);
        assert!(same_register(&register));
        assert!(!same_register(&update));
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
                history: Vec::new(),
            },
            ModuleCode {
                module_id: "hello".into(),
                active_code_hash: active.clone(),
                pending: Some(ScheduledSwap {
                    name: "hello-2".into(),
                    activation_height: 120,
                    code_hash: next.clone(),
                    readiness: vec![vec![1], vec![2]],
                    ready_at: None,
                }),
                history: Vec::new(),
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
                    ready_at: None,
                }),
                history: Vec::new(),
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
