//! The per-workspace service grants — `services.toml` beside `node.toml` —
//! and the `ducktape service` family that edits them.
//!
//! A service is offchain muscle (compute, storage, ...) with no sovereign
//! identity: identity and transport belong to the node. So a daemon can only
//! SIGNAL presence to its node (`POST /v1/services/hello`, a volatile catalog
//! — see `noded::services`), and the user decides whether it may act. That
//! decision is this file: `service enable <kind>` mints the grant, `service
//! disable <kind>` retires it, and the node's compute plane gates on the
//! result at boot.
//!
//! Why a sibling file rather than a `[services]` table in node.toml: node.toml
//! is the operator's network plumbing, written once by `init`/`join` and
//! hand-rendered with its own documentation. Everything a CLI verb edits at
//! runtime already lives in its own sibling (`gateway-routes.json`,
//! `invite-fronts.json`, `coord.cap`), and this follows that precedent.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use commonware_cryptography::Signer as _;

use crate::config;

pub const FILE_NAME: &str = "services.toml";
const FORMAT_VERSION: u8 = 1;

/// the compute service — the one kind the node itself acts on today (its
/// provider discovery, oracle pool and capability announce). Every other kind
/// is recorded and listed but drives no in-node wiring yet.
pub const COMPUTE_KIND: &str = "compute";

/// bytes of randomness in a grant nonce, matching `INVITE_NONCE_LEN`.
const GRANT_NONCE_LEN: usize = 16;

/// The domain separator for a service instance id.
///
/// The repo has no shared tagged-hash helper and no central registry of
/// numeric kind bytes; the established discipline for minting a NON-object id
/// is a NUL-terminated domain string folded into the preimage
/// (`runs::delegation_id_for`), mirroring the way an invite preimage is bound
/// to `b"ducktape-invite-grant-v1"`. A numeric kind byte is deliberately not
/// used here: service kinds are open string tags (the same shape as capability
/// tags), so there is no closed registry a byte could come from, and widening
/// duckfs's `objects::Kind` — the object store's strict decode gate — would
/// make a grant decodable as a stored object.
const SERVICE_INSTANCE_DOMAIN: &[u8] = b"ducktape/service-instance/v1\0";

/// One granted service: the consent record the user minted at `service
/// enable`, and the whole of its authority. There is no separate token — the
/// presence of this record IS the grant.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceGrant {
    /// the service kind tag (`compute`, ...).
    pub kind: String,
    /// the 32-byte instance id, lowercase hex. Minted from the node, the kind
    /// and `nonce`; see [`mint_instance`].
    pub instance: String,
    /// the grant nonce the instance id was minted from, lowercase hex. Kept so
    /// the id is reproducible from the record alone.
    pub nonce: String,
    /// when the grant was minted (unix seconds) — the consent-epoch marker.
    pub granted_unix: u64,
    /// the capability tags the daemon offered in the reviewed hello.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// the grant scopes the daemon requested in the reviewed hello.
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl ServiceGrant {
    /// the house display form, `compute#deadbeef` — the chain-id convention
    /// (`config::mint_chain_id`): the name, `#`, and the first 4 id bytes.
    pub fn display_id(&self) -> String {
        // the record is validated on load, so the prefix is present; a short
        // string would only ever come from an in-memory value under test.
        let head = self.instance.get(..8).unwrap_or(&self.instance);
        format!("{}#{head}", self.kind)
    }
}

/// the whole `services.toml`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Services {
    pub version: u8,
    /// the grants, kind-sorted and unique. `[[service]]` in the file.
    #[serde(default, rename = "service")]
    pub grants: Vec<ServiceGrant>,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            grants: Vec::new(),
        }
    }
}

impl Services {
    fn validate(&self) -> Result<(), String> {
        if self.version != FORMAT_VERSION {
            return Err(format!("services: unsupported format version {}", self.version));
        }
        let mut previous: Option<&str> = None;
        for grant in &self.grants {
            if !kind_is_well_formed(&grant.kind) {
                return Err(format!(
                    "services: {:?} is not a service kind (1..32 chars of [a-z0-9-])",
                    grant.kind
                ));
            }
            if previous.is_some_and(|old| old >= grant.kind.as_str()) {
                return Err("services: kinds must be unique and sorted".into());
            }
            previous = Some(&grant.kind);
            let instance_ok = grant.instance.len() == 64 && is_hex(&grant.instance);
            if !instance_ok {
                return Err(format!(
                    "services: {} instance must be 64 lowercase hex chars",
                    grant.kind
                ));
            }
            let nonce_ok = grant.nonce.len() == GRANT_NONCE_LEN * 2 && is_hex(&grant.nonce);
            if !nonce_ok {
                return Err(format!(
                    "services: {} nonce must be {} lowercase hex chars",
                    grant.kind,
                    GRANT_NONCE_LEN * 2
                ));
            }
        }
        Ok(())
    }

    /// the grant for `kind`, if the user has enabled it.
    pub fn grant(&self, kind: &str) -> Option<&ServiceGrant> {
        self.grants.iter().find(|grant| grant.kind == kind)
    }
}

/// A kind tag is lowercase alphanumeric plus `-` — the same rule the node's
/// hello boundary enforces, so a signaling kind and a granted kind are always
/// comparable and a kind can never carry the NUL the id preimage separates on.
fn kind_is_well_formed(kind: &str) -> bool {
    let len_ok = !kind.is_empty() && kind.len() <= 32;
    len_ok
        && kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_hex(text: &str) -> bool {
    text.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Mint a service instance id: `sha256(domain ‖ node_id ‖ 0 ‖ kind ‖ 0 ‖
/// nonce)`. Node-scoped (so two nodes never collide), kind-separated (so one
/// node's compute grant is not its storage grant) and nonce-fresh (so a
/// re-enable after a `disable` is a NEW id — the id doubles as the
/// consent-epoch marker).
pub fn mint_instance(node_id: &[u8], kind: &str, nonce: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(SERVICE_INSTANCE_DOMAIN);
    digest.update(node_id);
    digest.update([0]);
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(nonce);
    digest.finalize().into()
}

/// Read a workspace's grants. An ABSENT file is an empty grant set, not an
/// error — a node that never enabled a service simply has none.
pub fn load(workspace: &Path) -> Result<Services, String> {
    let path = workspace.join(FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Services::default());
        }
        Err(error) => return Err(format!("read {path:?}: {error}")),
    };
    let services: Services =
        toml::from_str(&text).map_err(|error| format!("{path:?}: {error}"))?;
    services.validate()?;
    Ok(services)
}

/// The grant for one kind in one workspace — what the node's boot path asks.
pub fn grant_for(workspace: &Path, kind: &str) -> Result<Option<ServiceGrant>, String> {
    Ok(load(workspace)?.grant(kind).cloned())
}

fn save(workspace: &Path, services: &Services) -> Result<(), String> {
    services.validate()?;
    std::fs::create_dir_all(workspace).map_err(|error| format!("create {workspace:?}: {error}"))?;
    let path = workspace.join(FILE_NAME);
    let temporary = workspace.join(format!(".{FILE_NAME}.tmp"));
    // no grants = no file: an empty `services.toml` and a missing one mean the
    // same thing, and leaving the husk behind invites a stale read.
    if services.grants.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("remove {path:?}: {error}")),
        }
        let _ = std::fs::remove_file(&temporary);
        return Ok(());
    }
    let body = toml::to_string_pretty(services).expect("services serialize");
    let text = format!(
        "# the services this workspace's user has granted standing on this node.\n\
         # managed by `ducktape service enable|disable`; the node reads it at boot.\n\
         {body}"
    );
    std::fs::write(&temporary, text).map_err(|error| format!("write {temporary:?}: {error}"))?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("replace {path:?}: {error}"));
    }
    Ok(())
}

// ============================================================================
// list/status state derivation
// ============================================================================

/// What one service's standing is on this node. Exactly three states, because
/// presence and consent are independent: a daemon may signal without a grant,
/// hold a grant while absent, or both.
/// The serde spellings are written out rather than derived from a rename rule:
/// they must equal [`ServiceState::label`] exactly, so `--json` and the table
/// never name one state two ways. `state_tokens_match_the_rendered_labels`
/// pins that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ServiceState {
    /// signaling, no grant — visible to the user, authorized for nothing.
    #[serde(rename = "signaling")]
    Signaling,
    /// granted and signaling — the working state.
    #[serde(rename = "enabled")]
    Enabled,
    /// granted but not signaling. An operational warning, never an error: the
    /// daemon is down, restarting, or was never started.
    #[serde(rename = "enabled-but-absent")]
    EnabledAbsent,
}

/// one rendered row of `service list` / `service status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceRow {
    pub kind: String,
    pub state: ServiceState,
    /// the display id (`compute#deadbeef`); `None` until the kind is granted.
    pub instance: Option<String>,
    /// the signaling daemon's version; `None` when it is not signaling.
    pub version: Option<String>,
    pub capabilities: Vec<String>,
    pub scopes: Vec<String>,
    /// service kinds this daemon declared it wants present, and the subset of
    /// them that nothing on this node provides.
    ///
    /// Rendered as an informational warning and NOTHING else: an unmet need
    /// never blocks enabling, never orders startup and never gates readiness.
    /// A service whose need is unmet still enables, still runs, still serves.
    pub needs: Vec<String>,
    pub unmet_needs: Vec<String>,
}

/// Fold the volatile catalog and the durable grants into one kind-sorted view.
///
/// The offered tags come from the LIVE hello when there is one (that is the
/// truth about what the daemon can do right now) and fall back to what the
/// grant recorded at consent time, so an absent service still shows what it
/// was enabled for.
pub fn rows(signaling: &[noded::services::Signaling], grants: &[ServiceGrant]) -> Vec<ServiceRow> {
    // a need is met when SOME service of that kind is enabled here. Local
    // grant state only — resolving "is there capacity anywhere in the network"
    // would mean a registry query on a display path, and a readiness signal is
    // exactly what this must not become.
    let enabled_kinds: std::collections::BTreeSet<&str> =
        grants.iter().map(|grant| grant.kind.as_str()).collect();
    let unmet = |needs: &[String]| -> Vec<String> {
        needs
            .iter()
            .filter(|need| !enabled_kinds.contains(need.as_str()))
            .cloned()
            .collect()
    };
    let mut rows: Vec<ServiceRow> = signaling
        .iter()
        .map(|live| {
            let granted = grants.iter().find(|grant| grant.kind == live.kind);
            ServiceRow {
                needs: live.needs.clone(),
                unmet_needs: unmet(&live.needs),
                kind: live.kind.clone(),
                state: match granted {
                    Some(_) => ServiceState::Enabled,
                    None => ServiceState::Signaling,
                },
                instance: granted.map(ServiceGrant::display_id),
                version: Some(live.version.clone()),
                capabilities: live.capabilities.clone(),
                scopes: live.scopes.clone(),
            }
        })
        .collect();
    let absent = grants
        .iter()
        .filter(|grant| !signaling.iter().any(|live| live.kind == grant.kind))
        .map(|grant| ServiceRow {
            kind: grant.kind.clone(),
            state: ServiceState::EnabledAbsent,
            instance: Some(grant.display_id()),
            version: None,
            capabilities: grant.capabilities.clone(),
            scopes: grant.scopes.clone(),
            // needs are live hello metadata; an absent daemon declares none.
            needs: Vec::new(),
            unmet_needs: Vec::new(),
        });
    rows.extend(absent);
    rows.sort_by(|a, b| a.kind.cmp(&b.kind));
    rows
}

// ============================================================================
// presentation
// ============================================================================

// Styling vocabulary. These are `anstyle` values, so a renderer below always
// emits the escapes and never asks whether it is talking to a terminal: the
// `anstream` wrapper the verbs print through STRIPS them when the destination
// is not a tty, when `NO_COLOR` is set, or when `TERM=dumb`. One place decides,
// every renderer stays pure and testable.
const BOLD: anstyle::Style = anstyle::Style::new().bold();
const DIM: anstyle::Style = anstyle::Style::new().dimmed();
const RED: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red)));
const GREEN: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green)));
const YELLOW: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow)));

/// Style `text`. Callers pad BEFORE styling: escape bytes carry no display
/// width, so styling a column first would break every alignment below it.
fn paint(style: anstyle::Style, text: &str) -> String {
    format!("{style}{text}{style:#}")
}

impl ServiceState {
    /// the stable token `--json` emits and the table prints.
    pub fn label(self) -> &'static str {
        match self {
            ServiceState::Signaling => "signaling",
            ServiceState::Enabled => "enabled",
            ServiceState::EnabledAbsent => "enabled-but-absent",
        }
    }

    /// the state marker. A glyph is plain UTF-8 text, not an escape, so it
    /// survives a pipe intact and stays greppable; only its color is dropped.
    fn glyph(self) -> &'static str {
        match self {
            ServiceState::Signaling => "·",
            ServiceState::Enabled => "✓",
            ServiceState::EnabledAbsent => "!",
        }
    }

    fn style(self) -> anstyle::Style {
        match self {
            ServiceState::Signaling => DIM,
            ServiceState::Enabled => GREEN,
            ServiceState::EnabledAbsent => YELLOW,
        }
    }
}

/// pad to `width` columns, THEN style — never the other way round.
fn column(text: &str, width: usize, style: anstyle::Style) -> String {
    paint(style, &format!("{text:<width$}"))
}

const KIND_WIDTH: usize = 16;
const STATE_WIDTH: usize = 18;

/// `service list` — one aligned row per service.
fn render_list(rows: &[ServiceRow]) -> String {
    if rows.is_empty() {
        return "no services signaling and none enabled\n\
                start one, then: ducktape service enable <kind>\n"
            .into();
    }
    let mut out = paint(
        DIM,
        &format!(
            "  {:<KIND_WIDTH$} {:<STATE_WIDTH$} {}",
            "KIND", "STATE", "INSTANCE"
        ),
    );
    out.push('\n');
    for row in rows {
        out.push_str(&format!(
            "{} {} {} {}\n",
            paint(row.state.style(), row.state.glyph()),
            column(&row.kind, KIND_WIDTH, BOLD),
            column(row.state.label(), STATE_WIDTH, row.state.style()),
            row.instance.as_deref().unwrap_or("-"),
        ));
        if let Some(hint) = unmet_hint(row) {
            out.push_str(&format!("  {}\n", paint(YELLOW, &hint)));
        }
    }
    out
}

/// The informational line for a service whose declared needs nothing here
/// meets. Purely advisory — see [`ServiceRow::unmet_needs`].
fn unmet_hint(row: &ServiceRow) -> Option<String> {
    if row.unmet_needs.is_empty() {
        return None;
    }
    Some(format!(
        "wants {} — not enabled on this node (informational; {} still serves)",
        row.unmet_needs.join(", "),
        row.kind
    ))
}

/// `service status` — a readable block per service rather than a flat dump.
fn render_status(rows: &[ServiceRow]) -> String {
    if rows.is_empty() {
        return "no services signaling and none enabled\n".into();
    }
    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{} {}  {}\n",
            paint(row.state.style(), row.state.glyph()),
            paint(BOLD, &row.kind),
            paint(row.state.style(), row.state.label()),
        ));
        let fields = [
            ("instance", row.instance.as_deref().unwrap_or("-").to_string()),
            ("version", row.version.as_deref().unwrap_or("-").to_string()),
            ("offers", join_or_dash(&row.capabilities)),
            ("scopes", join_or_dash(&row.scopes)),
            ("needs", join_or_dash(&row.needs)),
        ];
        for (name, value) in fields {
            out.push_str(&format!("    {} {value}\n", column(name, 10, DIM)));
        }
        if row.state == ServiceState::EnabledAbsent {
            // a daemon refused for build skew never reaches the catalog, so it
            // looks identical to one that is simply down. Name both causes
            // here — it is the only place an operator would look.
            out.push_str(&format!(
                "    {}\n",
                paint(
                    YELLOW,
                    &format!(
                        "enabled but not signaling — is its daemon running, and on build {}? \
                         (a different build is refused: reason build_mismatch)",
                        noded::services::build_identity()
                    )
                )
            ));
        }
        if let Some(hint) = unmet_hint(row) {
            out.push_str(&format!("    {}\n", paint(YELLOW, &hint)));
        }
    }
    out
}

/// The consent summary shown before a grant is minted: what is being enabled,
/// on which node, and exactly which offers/scopes the reviewed hello declared.
fn render_enable_summary(plan: &EnablePlan) -> String {
    let signaling = match &plan.offered {
        Some(_) => paint(GREEN, "signaling"),
        None => paint(YELLOW, "not signaling yet"),
    };
    let offered_list = |pick: fn(&noded::services::Signaling) -> &Vec<String>| {
        plan.offered
            .as_ref()
            .map(|offer| join_or_dash(pick(offer)))
            .unwrap_or_else(|| "-".into())
    };
    let rows = [
        ("service", paint(BOLD, &plan.kind)),
        ("node", format!("{} · {}", plan.chain_id, plan.node_hex8())),
        ("status", signaling),
        ("offers", offered_list(|offer| &offer.capabilities)),
        ("grant scopes", paint(RED, &offered_list(|offer| &offer.scopes))),
    ];
    let mut out = String::new();
    for (name, value) in rows {
        out.push_str(&format!("  {} {value}\n", column(name, 13, DIM)));
    }
    out
}

/// Print rendered output to stdout THROUGH `anstream`, which strips every
/// escape when the destination is not a terminal, when `NO_COLOR` is set, or
/// under `TERM=dumb`. This is the single place non-tty correctness is
/// enforced — the renderers above always style, and never ask.
fn write_out(rendered: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write as _;
    let mut out = anstream::stdout().lock();
    out.write_all(rendered.as_bytes())?;
    out.flush()?;
    Ok(())
}

/// the stderr twin of [`write_out`], for prose and the consent summary.
fn write_err(rendered: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write as _;
    let mut err = anstream::stderr().lock();
    err.write_all(rendered.as_bytes())?;
    err.flush()?;
    Ok(())
}

// ============================================================================
// the `ducktape service` family
// ============================================================================

/// the `ducktape service` verbs — the consent boundary for offchain service
/// daemons. `run` arrives with daemon mode; today a service is hosted in the
/// node process and `enable` is what turns it on.
#[derive(Debug, clap::Subcommand)]
pub(crate) enum ServiceCmd {
    /// show services signaling to this node and those enabled in config
    List(ReadArgs),
    /// grant a service standing on this node (mints its instance id)
    Enable(EnableArgs),
    /// revoke a service's grant and retire its instance id
    Disable(KindArgs),
    /// per-service state, instance id, offered tags and requested scopes
    Status(ReadArgs),
}

/// which workspace's `services.toml` a verb reads or edits: an explicit
/// `--workspace` wins, else `-n/--network` resolves through the registry.
#[derive(Debug, clap::Args)]
pub(crate) struct WorkspaceArgs {
    /// explicit workspace dir (wins over -n)
    #[arg(long, value_name = "DIR")]
    workspace: Option<PathBuf>,
    /// a registered workspace's chain id (`ducktape node list`)
    #[arg(short = 'n', long = "network", value_name = "CHAIN-ID")]
    network: Option<String>,
}

impl WorkspaceArgs {
    fn dir(&self) -> Result<PathBuf, String> {
        if let Some(dir) = &self.workspace {
            return Ok(dir.clone());
        }
        if let Some(needle) = &self.network {
            let (dir, _http) = config::resolve_network(needle)?;
            return Ok(dir);
        }
        Err("service command needs --workspace <dir> or -n/--network <id>".into())
    }
}

#[derive(Debug, clap::Args)]
pub(crate) struct ReadArgs {
    #[command(flatten)]
    workspace: WorkspaceArgs,
    /// emit one machine-readable JSON array instead of a table
    #[arg(long)]
    json: bool,
}

#[derive(Debug, clap::Args)]
pub(crate) struct KindArgs {
    /// the service kind (`compute`)
    #[arg(value_name = "KIND")]
    kind: String,
    #[command(flatten)]
    workspace: WorkspaceArgs,
}

#[derive(Debug, clap::Args)]
pub(crate) struct EnableArgs {
    /// the service kind (`compute`)
    #[arg(value_name = "KIND")]
    kind: String,
    #[command(flatten)]
    workspace: WorkspaceArgs,
    /// grant without the interactive confirmation (for scripts and unit files)
    #[arg(long, short = 'y')]
    yes: bool,
}

/// Run one verb of the `ducktape service` family.
pub(super) fn run(cmd: ServiceCmd) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ServiceCmd::List(args) => list(args),
        ServiceCmd::Enable(args) => enable(args),
        ServiceCmd::Disable(args) => disable(args),
        ServiceCmd::Status(args) => status(args),
    }
}

/// The services signaling to the workspace's own node. A node that is not
/// running is NOT an error here: nothing signaling is exactly what `list` must
/// render, and the grants still come off disk.
fn signaling_now(workspace: &Path) -> Vec<noded::services::Signaling> {
    let Ok(base) = config::http_base_in(workspace) else {
        return Vec::new();
    };
    let Ok(body) = crate::node_http::get_json(&base, "/v1/services") else {
        return Vec::new();
    };
    serde_json::from_value(body["signaling"].clone()).unwrap_or_default()
}


fn view(args: &ReadArgs) -> Result<Vec<ServiceRow>, Box<dyn std::error::Error>> {
    let workspace = args.workspace.dir()?;
    let grants = load(&workspace)?;
    Ok(rows(&signaling_now(&workspace), &grants.grants))
}

fn list(args: ReadArgs) -> Result<(), Box<dyn std::error::Error>> {
    let rows = view(&args)?;
    if args.json {
        println!("{}", serde_json::to_string(&rows)?);
        return Ok(());
    }
    write_out(&render_list(&rows))?;
    Ok(())
}

fn status(args: ReadArgs) -> Result<(), Box<dyn std::error::Error>> {
    let rows = view(&args)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    write_out(&render_status(&rows))?;
    Ok(())
}

fn join_or_dash(items: &[String]) -> String {
    match items.is_empty() {
        true => "-".into(),
        false => items.join(", "),
    }
}

/// Everything `enable` needs to decide, gathered without writing anything.
///
/// Split from the commit deliberately: the CLI verb renders this, asks the
/// user, and only then commits — and `service run`'s inline "enable now?"
/// prompt (step 2) drives the same two calls, so there is exactly one grant
/// mint and one consent record no matter which surface the user came through.
pub(crate) struct EnablePlan {
    pub kind: String,
    pub chain_id: String,
    pub node_id: Vec<u8>,
    /// the reviewed hello, when the daemon is currently signaling.
    pub offered: Option<noded::services::Signaling>,
}

impl EnablePlan {
    /// the node's own short form, matching the `#hex8` display convention.
    fn node_hex8(&self) -> String {
        config::hex_bytes(&self.node_id[..4.min(self.node_id.len())])
    }
}

/// Decide what enabling `kind` would mean. Writes nothing.
pub(crate) fn plan_enable(workspace: &Path, kind: &str) -> Result<EnablePlan, String> {
    if !kind_is_well_formed(kind) {
        return Err(format!(
            "{kind:?} is not a service kind (1..32 chars of [a-z0-9-])"
        ));
    }
    // re-enabling would mint a SECOND id for the same kind while the first is
    // still recorded as live consent. One grant per kind: disable first.
    if let Some(existing) = load(workspace)?.grant(kind) {
        return Err(format!(
            "{kind} is already enabled as {} — `ducktape service disable {kind}` first",
            existing.display_id()
        ));
    }
    // the node's own key scopes the id, so the mint reads the workspace
    // identity exactly the way every other node-scoped verb does.
    let resolved = config::resolve(&workspace.join("node.toml"))?;
    // the grant is minted FROM the reviewed hello when the daemon is
    // signaling. It is not required to be: at this step compute is still
    // hosted in the node process, so there is no daemon to signal — enabling
    // an absent kind is the documented `enabled-but-absent` state, which
    // `status` surfaces as a warning rather than an error.
    let offered = signaling_now(workspace)
        .into_iter()
        .find(|entry| entry.kind == kind);
    Ok(EnablePlan {
        kind: kind.to_string(),
        chain_id: resolved.chain_id,
        node_id: resolved.signer.public_key().as_ref().to_vec(),
        offered,
    })
}

/// Mint the grant the plan describes and persist it. THE enable code path.
pub(crate) fn commit_enable(
    workspace: &Path,
    plan: &EnablePlan,
) -> Result<ServiceGrant, String> {
    let mut services = load(workspace)?;
    let mut nonce = [0u8; GRANT_NONCE_LEN];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let instance = mint_instance(&plan.node_id, &plan.kind, &nonce);
    let grant = ServiceGrant {
        kind: plan.kind.clone(),
        instance: config::hex_bytes(&instance),
        nonce: config::hex_bytes(&nonce),
        granted_unix: now_unix(),
        capabilities: plan
            .offered
            .as_ref()
            .map(|offer| offer.capabilities.clone())
            .unwrap_or_default(),
        scopes: plan
            .offered
            .as_ref()
            .map(|offer| offer.scopes.clone())
            .unwrap_or_default(),
    };
    let position = services
        .grants
        .binary_search_by(|existing| existing.kind.as_str().cmp(&plan.kind))
        .unwrap_or_else(|position| position);
    services.grants.insert(position, grant.clone());
    save(workspace, &services)?;
    set_capability_announce(workspace, &plan.kind, true)?;
    tracing::info!(
        target: "ducktape::service",
        kind = %plan.kind,
        instance = %grant.display_id(),
        "service enabled"
    );
    Ok(grant)
}

fn enable(args: EnableArgs) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = args.workspace.dir()?;
    let plan = plan_enable(&workspace, &args.kind)?;

    write_err(&render_enable_summary(&plan))?;
    let question = format!("Enable {} on this node?", plan.kind);
    if !crate::tty::confirm(&question, crate::tty::stdin_is_tty(), args.yes)? {
        eprintln!("not enabled");
        return Ok(());
    }

    let grant = commit_enable(&workspace, &plan)?;
    // stdout is the id alone, so `$(ducktape service enable compute)` is the
    // instance id and nothing else; the prose goes to stderr.
    println!("{}", grant.display_id());
    write_err(&format!(
        "{} enabled {}\n",
        paint(GREEN, ServiceState::Enabled.glyph()),
        grant.display_id()
    ))?;
    if plan.offered.is_none() {
        eprintln!("  it is not signaling yet — status will show enabled-but-absent");
    }
    if plan.kind == COMPUTE_KIND {
        eprintln!("  restart the node to start its compute plane: ducktape node run");
    }
    Ok(())
}

fn disable(args: KindArgs) -> Result<(), Box<dyn std::error::Error>> {
    let kind = args.kind;
    let workspace = args.workspace.dir()?;
    let mut services = load(&workspace)?;
    let position = services
        .grants
        .iter()
        .position(|grant| grant.kind == kind)
        .ok_or_else(|| format!("{kind} is not enabled in {}", workspace.display()))?;
    let retired = services.grants.remove(position);
    save(&workspace, &services)?;
    set_capability_announce(&workspace, &kind, false)?;
    tracing::info!(
        target: "ducktape::service",
        %kind,
        instance = %retired.display_id(),
        "service disabled"
    );
    println!("{}", retired.display_id());
    eprintln!(
        "disabled {kind}; {} is retired (a re-enable mints a fresh id)",
        retired.display_id()
    );
    Ok(())
}

/// The capability announce follows the compute grant.
///
/// `node init --compute` used to set `announce_capabilities` at founding time;
/// with the flag gone, enabling compute is what turns the announce on and
/// disabling it is what turns it off — otherwise the flag day would leave an
/// operator no way to advertise capacity short of hand-editing node.toml.
/// Every other kind leaves node.toml alone. The rewrite goes through the same
/// merge+render the `init`/`join` verbs use, so every other key keeps its
/// current value.
fn set_capability_announce(workspace: &Path, kind: &str, announce: bool) -> Result<(), String> {
    let drives_the_compute_plane = kind == COMPUTE_KIND;
    if !drives_the_compute_plane {
        return Ok(());
    }
    let mut plumbing = config::merged_plumbing(
        workspace, None, None, None, None, None, None, None, None, None,
    )?;
    if plumbing.announce_capabilities == announce {
        return Ok(());
    }
    plumbing.announce_capabilities = announce;
    config::write_node_toml(workspace, &plumbing)?;
    Ok(())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE_A: [u8; 32] = [7u8; 32];
    const NODE_B: [u8; 32] = [9u8; 32];
    const NONCE: [u8; GRANT_NONCE_LEN] = [3u8; GRANT_NONCE_LEN];

    fn grant(kind: &str, instance: [u8; 32]) -> ServiceGrant {
        ServiceGrant {
            kind: kind.into(),
            instance: config::hex_bytes(&instance),
            nonce: config::hex_bytes(&NONCE),
            granted_unix: 1_700_000_000,
            capabilities: vec!["agent.claude".into()],
            scopes: vec!["cred:read".into()],
        }
    }

    fn signaling(kind: &str) -> noded::services::Signaling {
        noded::services::Signaling {
            kind: kind.into(),
            version: "1.2.3".into(),
            capabilities: vec!["agent.codex".into()],
            scopes: vec![],
            needs: vec![],
        }
    }

    #[test]
    fn the_instance_id_is_deterministic_and_domain_separated() {
        let mine = mint_instance(&NODE_A, "compute", &NONCE);
        // determinism: the same three inputs always mint the same id, so a
        // daemon restart under one grant keeps its identity.
        assert_eq!(mine, mint_instance(&NODE_A, "compute", &NONCE));

        // every input separates: node, kind and nonce each change the id.
        assert_ne!(mine, mint_instance(&NODE_B, "compute", &NONCE));
        assert_ne!(mine, mint_instance(&NODE_A, "storage", &NONCE));
        assert_ne!(mine, mint_instance(&NODE_A, "compute", &[4u8; GRANT_NONCE_LEN]));

        // the domain prefix is real: the id is NOT a bare sha256 of the parts.
        let mut undomained = Sha256::new();
        undomained.update(NODE_A);
        undomained.update(b"compute");
        undomained.update(NONCE);
        let undomained: [u8; 32] = undomained.finalize().into();
        assert_ne!(mine, undomained);

        // and the separators make the preimage unambiguous: a kind that
        // borrows a byte from the nonce must not collide with the honest
        // split. (`kind ‖ nonce` concatenated blindly would collide here.)
        let split_a = mint_instance(&NODE_A, "ab", &[0xcd; GRANT_NONCE_LEN]);
        let split_b = mint_instance(&NODE_A, "abc", &[0xcd; GRANT_NONCE_LEN]);
        assert_ne!(split_a, split_b);
    }

    #[test]
    fn the_display_id_is_the_chain_id_convention() {
        let id = mint_instance(&NODE_A, "compute", &NONCE);
        let display = grant("compute", id).display_id();
        let (kind, head) = display.split_once('#').expect("kind#hex8");
        assert_eq!(kind, "compute");
        assert_eq!(head.len(), 8, "the display tail is the first 4 bytes");
        assert_eq!(head, config::hex_bytes(&id[..4]));
    }

    #[test]
    fn list_state_derives_all_three_standings() {
        let enabled = grant("compute", mint_instance(&NODE_A, "compute", &NONCE));
        let absent = grant("storage", mint_instance(&NODE_A, "storage", &NONCE));
        // compute: granted AND signaling. airlock: signaling, no grant.
        // storage: granted, nothing signaling.
        let live = vec![signaling("compute"), signaling("airlock")];
        let rows = rows(&live, &[enabled.clone(), absent.clone()]);

        let states: Vec<(&str, ServiceState)> = rows
            .iter()
            .map(|row| (row.kind.as_str(), row.state))
            .collect();
        assert_eq!(
            states,
            vec![
                ("airlock", ServiceState::Signaling),
                ("compute", ServiceState::Enabled),
                ("storage", ServiceState::EnabledAbsent),
            ],
            "rows are kind-sorted and each standing is derived independently"
        );

        // a signaling-only row has no id; a granted one always does.
        assert_eq!(rows[0].instance, None);
        assert_eq!(rows[1].instance, Some(enabled.display_id()));
        assert_eq!(rows[2].instance, Some(absent.display_id()));

        // the LIVE hello wins for offered tags; an absent service falls back
        // to what its grant recorded at consent time.
        assert_eq!(rows[1].capabilities, vec!["agent.codex".to_string()]);
        assert_eq!(rows[2].capabilities, vec!["agent.claude".to_string()]);
        assert_eq!(rows[2].version, None);
    }

    #[test]
    fn grants_round_trip_through_the_file_and_an_empty_set_leaves_none() {
        let dir = tempfile::tempdir().unwrap();
        // an absent file is an empty grant set, never an error.
        assert_eq!(load(dir.path()).unwrap(), Services::default());
        assert!(grant_for(dir.path(), COMPUTE_KIND).unwrap().is_none());

        let services = Services {
            version: FORMAT_VERSION,
            grants: vec![
                grant("compute", mint_instance(&NODE_A, "compute", &NONCE)),
                grant("storage", mint_instance(&NODE_A, "storage", &NONCE)),
            ],
        };
        save(dir.path(), &services).unwrap();
        assert_eq!(load(dir.path()).unwrap(), services);
        assert_eq!(
            grant_for(dir.path(), COMPUTE_KIND).unwrap().unwrap().kind,
            "compute"
        );

        // removing the last grant removes the file rather than leaving a husk.
        save(dir.path(), &Services::default()).unwrap();
        assert!(!dir.path().join(FILE_NAME).exists());
        assert!(grant_for(dir.path(), COMPUTE_KIND).unwrap().is_none());
    }

    /// the three-state fixture every renderer test shares.
    fn every_state() -> Vec<ServiceRow> {
        rows(
            &[signaling("compute"), signaling("airlock")],
            &[
                grant("compute", mint_instance(&NODE_A, "compute", &NONCE)),
                grant("storage", mint_instance(&NODE_A, "storage", &NONCE)),
            ],
        )
    }

    /// Render `text` the way a real verb would, to a destination with the
    /// given color choice — i.e. through the SAME `anstream` adapter
    /// `write_out` uses. `Never` is what a pipe, a file, `NO_COLOR` and
    /// `TERM=dumb` all resolve to.
    fn through_anstream(text: &str, choice: anstream::ColorChoice) -> String {
        use std::io::Write as _;
        let mut stream = anstream::AutoStream::new(Vec::new(), choice);
        stream.write_all(text.as_bytes()).expect("write");
        String::from_utf8(stream.into_inner()).expect("utf8")
    }

    #[test]
    fn a_non_terminal_destination_receives_no_escape_sequences() {
        let rows = every_state();
        let plan = EnablePlan {
            kind: "compute".into(),
            chain_id: "dukenet#03f6df3d".into(),
            node_id: NODE_A.to_vec(),
            offered: Some(signaling("compute")),
        };
        let rendered = [
            render_list(&rows),
            render_status(&rows),
            render_enable_summary(&plan),
            render_list(&[]),
            render_status(&[]),
        ];
        for one in &rendered {
            let piped = through_anstream(one, anstream::ColorChoice::Never);
            assert!(
                !piped.contains('\x1b'),
                "a non-terminal must never receive ANSI:\n{piped:?}"
            );
        }
        // and the facts a script or a human greps for survive the stripping.
        let list = through_anstream(&rendered[0], anstream::ColorChoice::Never);
        assert!(list.contains("enabled-but-absent"));
        assert!(list.contains("compute"));
        assert!(
            through_anstream(&rendered[1], anstream::ColorChoice::Never).contains("agent.codex")
        );
        assert!(
            through_anstream(&rendered[2], anstream::ColorChoice::Never)
                .contains("dukenet#03f6df3d")
        );
    }

    #[test]
    fn a_terminal_destination_keeps_the_state_colors_and_stays_aligned() {
        let rows = every_state();
        let painted = through_anstream(&render_list(&rows), anstream::ColorChoice::Always);
        assert!(painted.contains('\x1b'), "a terminal gets color");

        // alignment is computed on the TEXT, never on escape bytes, so the
        // colored rendering and the piped one differ ONLY by escapes.
        let piped = through_anstream(&render_list(&rows), anstream::ColorChoice::Never);
        assert_eq!(strip_ansi(&painted), piped);

        // every state carries its own distinct styling.
        let styles: Vec<String> = rows
            .iter()
            .map(|row| paint(row.state.style(), row.state.glyph()))
            .collect();
        assert_eq!(
            styles.iter().collect::<std::collections::BTreeSet<_>>().len(),
            3,
            "signaling / enabled / enabled-but-absent must be visually distinct"
        );
    }

    /// drop SGR sequences (`ESC [ ... m`) so a styled string can be compared
    /// against its stripped twin.
    fn strip_ansi(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '\x1b' {
                out.push(c);
                continue;
            }
            for skipped in chars.by_ref() {
                if skipped == 'm' {
                    break;
                }
            }
        }
        out
    }

    #[test]
    fn the_consent_prompt_is_skipped_where_no_one_can_answer() {
        // automation: a pipe has nobody to ask, and --yes says so explicitly.
        // Neither may block on stdin, so both proceed without reading it.
        assert!(crate::tty::confirm("Enable compute?", false, false).unwrap());
        assert!(crate::tty::confirm("Enable compute?", false, true).unwrap());
        assert!(crate::tty::confirm("Enable compute?", true, true).unwrap());
    }

    #[test]
    fn state_tokens_match_the_rendered_labels() {
        // one state, one name: a script keying on the `--json` token and a
        // human reading the table must never see two different spellings.
        for state in [
            ServiceState::Signaling,
            ServiceState::Enabled,
            ServiceState::EnabledAbsent,
        ] {
            let json = serde_json::to_string(&state).expect("serialize state");
            assert_eq!(
                json,
                format!("\"{}\"", state.label()),
                "the json token and the printed label must agree"
            );
        }
    }

    #[test]
    fn the_instance_id_survives_a_daemon_restart() {
        let dir = tempfile::tempdir().unwrap();
        // enabling mints the id ONCE and writes it down ...
        let minted = grant("compute", mint_instance(&NODE_A, "compute", &NONCE));
        save(
            dir.path(),
            &Services {
                version: FORMAT_VERSION,
                grants: vec![minted.clone()],
            },
        )
        .unwrap();
        let first = minted.display_id();

        // ... so every later read — a daemon restart, a node restart, a fresh
        // CLI process — resolves the SAME kind#hex8. The id's lifetime is the
        // grant's, not the process's.
        for _ in 0..3 {
            let reloaded = grant_for(dir.path(), COMPUTE_KIND).unwrap().unwrap();
            assert_eq!(reloaded.display_id(), first);
            assert_eq!(reloaded, minted, "the whole grant round-trips unchanged");
            // and it is reproducible from the record alone.
            let nonce = (0..GRANT_NONCE_LEN)
                .map(|i| {
                    u8::from_str_radix(&reloaded.nonce[i * 2..i * 2 + 2], 16).expect("hex nonce")
                })
                .collect::<Vec<u8>>();
            assert_eq!(
                config::hex_bytes(&mint_instance(&NODE_A, "compute", &nonce)),
                reloaded.instance,
                "the id re-derives from node + kind + the recorded nonce"
            );
        }

        // disable retires it; a re-enable must NOT resurrect the same id.
        save(dir.path(), &Services::default()).unwrap();
        assert!(grant_for(dir.path(), COMPUTE_KIND).unwrap().is_none());
        let fresh = mint_instance(&NODE_A, "compute", &[0xab; GRANT_NONCE_LEN]);
        assert_ne!(
            config::hex_bytes(&fresh),
            minted.instance,
            "a fresh grant nonce means a fresh consent epoch"
        );
    }

    #[test]
    fn an_unmet_declared_need_is_a_warning_and_nothing_more() {
        let mut agent = signaling("agent");
        agent.needs = vec!["compute".into()];
        let agent_grant = grant("agent", mint_instance(&NODE_A, "agent", &NONCE));

        // nothing named `compute` is enabled here, so the need is unmet ...
        let unmet_rows = rows(&[agent.clone()], std::slice::from_ref(&agent_grant));
        assert_eq!(unmet_rows[0].unmet_needs, vec!["compute".to_string()]);
        // ... and yet the service is fully enabled: a need gates NOTHING.
        assert_eq!(unmet_rows[0].state, ServiceState::Enabled);
        assert!(unmet_rows[0].instance.is_some());

        let hint = unmet_hint(&unmet_rows[0]).expect("an unmet need is surfaced");
        assert!(hint.contains("compute"));
        assert!(hint.contains("informational"));
        assert!(
            through_anstream(&render_list(&unmet_rows), anstream::ColorChoice::Never)
                .contains("compute"),
            "the warning reaches the rendered list"
        );

        // enable a compute service and the need is met — same states, no hint.
        let with_compute = rows(
            &[agent],
            &[
                agent_grant,
                grant("compute", mint_instance(&NODE_A, "compute", &NONCE)),
            ],
        );
        let agent_row = with_compute
            .iter()
            .find(|row| row.kind == "agent")
            .expect("agent row");
        assert!(agent_row.unmet_needs.is_empty());
        assert_eq!(unmet_hint(agent_row), None);
    }

    #[test]
    fn a_malformed_grant_file_is_refused_loudly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);

        // unsorted / duplicate kinds
        let duplicate = Services {
            version: FORMAT_VERSION,
            grants: vec![
                grant("compute", [1u8; 32]),
                grant("compute", [2u8; 32]),
            ],
        };
        assert!(duplicate.validate().is_err());

        // a short instance id
        let mut short = grant("compute", [1u8; 32]);
        short.instance.truncate(10);
        assert!(
            Services {
                version: FORMAT_VERSION,
                grants: vec![short],
            }
            .validate()
            .is_err()
        );

        // an unknown key never parses silently
        std::fs::write(&path, "version = 1\nsurprise = true\n").unwrap();
        assert!(load(dir.path()).is_err());

        // a future format version is a loud refusal, not a shrug
        std::fs::write(&path, "version = 2\n").unwrap();
        assert!(load(dir.path()).is_err());
    }
}
