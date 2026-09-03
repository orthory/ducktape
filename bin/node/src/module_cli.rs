//! `ducktape module …` — the operator's side of a live code swap.
//!
//! `update`/`register` stage a component at this node's owner-gated admin
//! route (which fans it out to every validator and returns their receipts),
//! then drive the governance proposal that schedules it. `status` reads the
//! lifecycle registry. Nothing here runs inside the node.

use std::path::PathBuf;

use commonware_cryptography::Signer as _;
use topology::PRODUCTION;

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

/// `module update|register <id> <component.wasm> [--after N]`: stage the bytes
/// at this node (fan-out to every validator), refuse unless every member holds
/// them, then drive the governance proposal that schedules the swap at this
/// node's height + N and read the registry back for its verdict.
fn cmd_stage_and_schedule(args: StageArgs, verb: Verb) -> CommandResult {
    config::validate_module_id(&args.id)?;
    // the static half of the registry's lead rule, checked before anything is
    // staged or proposed: an activation at or under the floor is refused at
    // execute whatever the ceremony does.
    let lead_too_short = args.after <= lifecycle::MIN_SWAP_LEAD;
    if lead_too_short {
        return Err(format!(
            "--after {} cannot schedule anything: activation must exceed height+MIN_SWAP_LEAD ({}), \
             and the ceremony's own blocks eat into the lead — leave room (the default is 50)",
            args.after,
            lifecycle::MIN_SWAP_LEAD
        )
        .into());
    }
    let bytes = std::fs::read(&args.component)
        .map_err(|e| format!("read {}: {e}", args.component.display()))?;
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved.rpc_listen.clone().ok_or(format!(
        "{} drives the node's local rpc — set `rpc_listen` in node.toml",
        verb.name()
    ))?;
    let http_listen = resolved.service.http_listen.as_deref().ok_or(format!(
        "{} stages over the node's http surface — set `http_listen` in node.toml",
        verb.name()
    ))?;
    let http_base = config::http_base_of(http_listen);
    let code_hash: [u8; 32] = {
        use sha2::Digest as _;
        sha2::Sha256::digest(&bytes).into()
    };

    // 1. the registry's static rules, before anything is staged or proposed:
    //    each would reject governance's execute in-kernel and leave a
    //    proposal open for its whole voting period.
    let precheck = registry_precheck(verb, &read_module_status(&rpc_addr)?, &args.id, &code_hash)?;
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
    let reply = stage_component(&http_base, &resolved.service.workspace, &bytes)?;
    digest_matches(&reply, &bytes)?;
    eprintln!(
        "staged {} ({} bytes), {} peer receipt(s)",
        hex_bytes(&code_hash),
        reply.len,
        reply.receipts.len()
    );
    refuse_on_bad_receipts(&reply)?;
    // the fan-out never pushes to the staging node itself, so the one valset
    // member allowed no receipt is THIS NODE's key — not the governance
    // signer's, which under share governance is an account key.
    let me_hex = hex_bytes(resolved.signer.public_key().as_ref());
    let members = crate::cli::read_members(&rpc_addr)?;
    refuse_on_missing_receipts(&reply, &members, &me_hex)?;
    let signer = crate::cli::gov_signer(&rpc_addr, &cfg_path, &resolved)?;
    let pubkey_hex = signer_pubkey_hex(&signer);

    // 4. activation = this node's height + N (each member computes its own).
    //    an `--after` that wraps would land UNDER the floor in release, where
    //    the matcher's debug assert is compiled out: the verb would mint a
    //    proposal the registry refuses at execute.
    let height = current_height(&rpc_addr)?;
    let activation_height = height
        .checked_add(args.after)
        .ok_or("--after overflows the chain height")?;
    let floor = height + lifecycle::MIN_SWAP_LEAD;

    // 5. the ceremony: join an open proposal for the same (verb, id, hash)
    //    that can still be scheduled, or propose; cast yes; execute when
    //    decidable
    let matches = matches_module_action(verb, &args.id, &code_hash, floor);
    let ceremony = crate::cli::drive_proposal_ceremony(
        &rpc_addr,
        &signer,
        &pubkey_hex,
        verb.name(),
        "module:",
        verb.action(&args.id, activation_height, code_hash),
        &matches,
    );
    let outcome = match ceremony {
        Ok(outcome) => outcome,
        Err(error) => return Err(ceremony_failed(&rpc_addr, &args.id, &code_hash, error)),
    };
    match outcome {
        CeremonyOutcome::AwaitingBallots => Ok(()),
        CeremonyOutcome::Passed => {
            confirm_scheduled(&rpc_addr, &args.id, &code_hash, activation_height)
        }
    }
}

/// a passed proposal only ASKED the registry; the CLI's success line is the
/// registry's word, not governance's. read it back before saying "scheduled".
fn confirm_scheduled(
    rpc_addr: &str,
    id: &str,
    code_hash: &[u8; 32],
    activation_height: u64,
) -> CommandResult {
    let scheduled = registry_holds(&read_module_status(rpc_addr)?, id, code_hash).is_some();
    if !scheduled {
        return Err(format!(
            "proposal passed but the lifecycle registry holds no swap for {id} → {}. {}",
            hex_bytes(code_hash),
            registry_rules()
        )
        .into());
    }
    println!(
        "scheduled {id} → {} at height {activation_height}; track with: ducktape module status",
        hex_bytes(code_hash)
    );
    Ok(())
}

/// the ceremony failed. the one failure the registry causes: governance's
/// `Execute` emits the schedule to lifecycle in the SAME op, so a registry
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
        "{error}: governance's execute was refused by the lifecycle registry, which holds no swap \
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
    Pending,
}

impl Held {
    /// the word the operator's "nothing to do" line uses.
    fn word(self) -> &'static str {
        match self {
            Held::Active => "active",
            Held::Pending => "scheduled",
        }
    }
}

/// the registry's STATIC rules (`lifecycle` `handle_schedule_swap` /
/// `handle_schedule_register`), decided from the committed registry before
/// anything is staged or proposed. each refusal here would otherwise reject
/// governance's execute in-kernel and leave a proposal open for its whole
/// voting period; the wording mirrors the registry's own. pure.
fn registry_precheck(
    verb: Verb,
    modules: &[lifecycle::ModuleCode],
    id: &str,
    code_hash: &[u8; 32],
) -> Result<Precheck, String> {
    // a genesis id is code this host already runs, so lifecycle's
    // `handle_schedule_register` refuses to re-admit it (`ctx.module_root`) —
    // and that refusal lands in-kernel, leaving the proposal open for its whole
    // voting period. the registry read cannot see it: a NATIVE module carries no
    // registry entry at all.
    let registering_a_genesis_module = matches!(verb, Verb::Register) && PRODUCTION.contains(&id);
    if registering_a_genesis_module {
        return Err(format!(
            "{id} is a genesis module — its code changes with `module update`, not `register`"
        ));
    }
    if let Some(held) = registry_holds(modules, id, code_hash) {
        return Ok(Precheck::AlreadyHeld(held));
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
        lifecycle::MIN_SWAP_LEAD
    )
}

/// how the registry carries `code_hash` for `id`, if it carries it at all.
/// pure: the one read behind both "nothing to do" and the post-`Passed`
/// confirmation.
fn registry_holds(
    modules: &[lifecycle::ModuleCode],
    id: &str,
    code_hash: &[u8; 32],
) -> Option<Held> {
    let entry = modules.iter().find(|m| m.module_id == id)?;
    let pending_is_ours = entry
        .pending
        .as_ref()
        .is_some_and(|p| p.code_hash == code_hash);
    if pending_is_ours {
        return Some(Held::Pending);
    }
    let already_active = entry.active_code_hash == code_hash;
    already_active.then_some(Held::Active)
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

/// the lifecycle registry over the generic query lane — the same shape
/// `read_members` uses for governance.
fn read_module_status(rpc_addr: &str) -> Result<Vec<lifecycle::ModuleCode>, String> {
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

/// One non-ok receipt and nothing is proposed: the swap only makes sense once
/// every validator can run the code. The refusal names each holdout peer with
/// the status token its node reported.
fn refuse_on_bad_receipts(reply: &StageReply) -> Result<(), String> {
    let failing: Vec<&PeerReceipt> = reply.receipts.iter().filter(|r| !r.ok).collect();
    let every_peer_holds_it = failing.is_empty();
    if every_peer_holds_it {
        return Ok(());
    }
    let mut table = String::from("peer  status\n");
    for r in failing {
        table.push_str(&format!("{}  {}\n", r.peer, r.status));
    }
    table.push_str(NOT_PROPOSED);
    Err(table)
}

/// the sentence under every receipt refusal — one wording, whichever gate.
const NOT_PROPOSED: &str = "not proposed: every validator must hold the bytes before a swap is scheduled \
                            — re-run once they are reachable (staging is idempotent)";

/// A receipt table only names the peers the fan-out REACHED (the node's
/// tracked overlay set minus itself), so a validator the node never dialled
/// has no row at all and `refuse_on_bad_receipts` cannot see it. Every member
/// of the valset must be `me_hex` (the staging node holds the bytes itself) or
/// a receipt peer; a member with neither is listed as unreachable and refuses.
fn refuse_on_missing_receipts(
    reply: &StageReply,
    members: &[Vec<u8>],
    me_hex: &str,
) -> Result<(), String> {
    let missing: Vec<String> = members
        .iter()
        .map(|member| hex_bytes(member))
        .filter(|member| {
            let is_me = member == me_hex;
            let has_receipt = reply.receipts.iter().any(|r| r.peer == *member);
            !is_me && !has_receipt
        })
        .collect();
    let every_member_answered = missing.is_empty();
    if every_member_answered {
        return Ok(());
    }
    let mut table = String::from("peer  status\n");
    for peer in missing {
        table.push_str(&format!("{peer}  no receipt (unreachable)\n"));
    }
    table.push_str(NOT_PROPOSED);
    Err(table)
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
fn render_status(modules: &[lifecycle::ModuleCode]) -> String {
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
    fn a_member_without_a_receipt_refuses_as_unreachable() {
        let me = vec![0x01u8; 32];
        let answered = vec![0x02u8; 32];
        let silent = vec![0x03u8; 32];
        let reply = StageReply {
            digest: "00".repeat(32),
            len: 3,
            receipts: vec![PeerReceipt {
                peer: hex_bytes(&answered),
                status: "stored".into(),
                ok: true,
            }],
        };
        let members = vec![me.clone(), answered.clone(), silent.clone()];
        let err = refuse_on_missing_receipts(&reply, &members, &hex_bytes(&me)).unwrap_err();
        assert!(err.starts_with("peer  status\n"), "{err}");
        assert!(
            err.contains(&format!("{}  no receipt (unreachable)", hex_bytes(&silent))),
            "{err}"
        );
        assert!(
            !err.contains(&hex_bytes(&answered)),
            "answered peers are not listed: {err}"
        );
        assert!(err.contains("not proposed"), "{err}");
        // me + every other member answered: nothing missing
        let present = vec![me.clone(), answered];
        assert!(refuse_on_missing_receipts(&reply, &present, &hex_bytes(&me)).is_ok());
        // a staging node outside the valset needs every member's receipt
        let err = refuse_on_missing_receipts(&reply, &present, "ff").unwrap_err();
        assert!(err.contains(&hex_bytes(&me)), "{err}");
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
            |verb, modules: &[ModuleCode]| registry_precheck(verb, modules, "hello", &ours);

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
            Ok(Precheck::AlreadyHeld(Held::Pending))
        ));
        assert!(matches!(
            precheck(Verb::Update, &[entry(&ours, None)]),
            Ok(Precheck::AlreadyHeld(Held::Active))
        ));
        assert_eq!(Held::Pending.word(), "scheduled");
        assert_eq!(Held::Active.word(), "active");
        // a genesis id carries no registry entry when it is native, so only the
        // topology set catches it — lifecycle would refuse it in-kernel
        let err = registry_precheck(Verb::Register, &[], "identity", &ours).unwrap_err();
        assert!(err.contains("genesis module"), "{err}");
        // update is how a genesis module's code changes: no genesis refusal
        let err = registry_precheck(Verb::Update, &[], "identity", &ours).unwrap_err();
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
