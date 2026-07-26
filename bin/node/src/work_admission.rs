//! Work admission — whose work this node will execute.
//!
//! ## the premise this whole module rests on
//!
//! `POST /v1/submit` on a real node DISCARDS the caller's claimed submitter id
//! and re-signs the op with the node's own signer
//! (`validator/run/ingress.rs`: *"this lane signs frames, and the signed origin
//! IS this node's pubkey"*). So a saga's committed
//! [`SagaOrigin::External`] carries the SUBMITTING NODE's key, proven by the
//! frame's verified signature — it is derived, never asserted. The mesh
//! [`PeerId`](data_plane::PeerId) the terminal plane hands us is the same kind
//! of fact, proven by the WireGuard transport.
//!
//! **If that ever changes, this admission becomes decorative.**
//! `the_submit_lane_still_resigns_with_the_node_key` pins it.
//!
//! ## what it decides, and what it deliberately does not
//!
//! PR #833 deleted every caller-asserted account from the compute layer:
//! identity enters the flow at exactly one place, the gateway hop, where the
//! LENDER's node stamps the account it vouches for. The consequence is that a
//! grant lends to a node *for whatever workload it runs* — so on a default
//! network any party who can place work on the owner's node spends the owner's
//! subscription without ever touching a token.
//!
//! This is the missing half, and it is a different question: not *"who is this
//! run acting for"* (a claim about a third party, which this layer must never
//! make) but *"will this host run a stranger's workload at all"* — a host
//! deciding its own policy about a party it identified itself.
//!
//! ## one decision, two call sites
//!
//! [`admit`] is the ONLY entry point, and both lanes call it:
//!
//! - `term_plane::serve_create` — a mesh peer asking this host for a pty;
//! - `compute::intake::WorkPump` — a committed saga assigned to or announced
//!   at this node.
//!
//! They are two call sites because they run in two PROCESSES (the wave-2 daemon
//! split), not two policies. `both_lanes_route_through_one_verdict` is a
//! source-parsing lint that keeps it that way: two checks that must agree is
//! the dual-path defect this repo forbids.
//!
//! ## what it does NOT close
//!
//! - A saga triggered by a MODULE (`dispatch` — i.e. the chat/pages/forge/jobs
//!   /`RequestRun` family) has no account origin at this layer and is admitted:
//!   see [`WorkCaller::NotAnAccountOrigin`].
//! - The guarantee is bounded by `/v1`'s exposure. `POST /v1/submit` re-signs
//!   as THIS node, so anything that can reach the node's HTTP or RPC port takes
//!   the [`WorkCaller::ThisNode`] path by construction. Making un-tokened `/v1`
//!   callers refused is its own campaign
//!   (`docs/superpowers/plans/2026-07-26-wave3-scope-enforcement.md`); keeping
//!   those ports loopback-bound is what makes this module mean anything.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use saga::SagaOrigin;

use crate::config;

/// the policy file, beside `node.toml` in the workspace. Deliberately its own
/// file rather than a `node.toml` table: `write_node_toml` REWRITES the whole
/// config on every `init`/`join` merge, so a list living there would need a
/// `Plumbing` field to survive — five touch points for one list. Absent = the
/// default, exactly as an absent `services.toml` means "no grants".
pub(crate) const FILE_NAME: &str = "work-admit.toml";

/// the one `admit` entry that is not an account id — and the one word the CLI
/// takes for it, deliberately the SAME token in both places rather than a
/// config spelling and a CLI spelling that must be kept in sync. It is a
/// statement, not an entry, so it may not be mixed with account ids.
pub(crate) const ANYONE: &str = "anyone";

// ============================================================================
// the policy
// ============================================================================

/// Whose work this node will run. ONE discriminant; the file's `admit` list
/// decodes into exactly one arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkAdmission {
    /// the node's own owner account, and nobody else. The default: it is the
    /// same shape as the record it protects — `gateway::credential_use_allowed`
    /// also admits the owner implicitly and everyone else explicitly.
    Owner,
    /// the owner PLUS these accounts.
    Accounts(BTreeSet<Vec<u8>>),
    /// any node the mesh admitted. Opt-in, and it re-opens what this module
    /// exists to close — the CLI says so on the way in.
    Anyone,
}

impl WorkAdmission {
    /// the file's `admit` entries, in the canonical order the file is written
    /// in. `Owner` is the empty list.
    pub(crate) fn entries(&self) -> Vec<String> {
        match self {
            WorkAdmission::Owner => Vec::new(),
            WorkAdmission::Anyone => vec![ANYONE.to_string()],
            WorkAdmission::Accounts(accounts) => {
                accounts.iter().map(|id| config::hex_bytes(id)).collect()
            }
        }
    }

    /// admit one more account — or widen to [`Self::Anyone`]. `Anyone` absorbs.
    pub(crate) fn with(self, target: AdmitTarget) -> Self {
        match target {
            AdmitTarget::Anyone => WorkAdmission::Anyone,
            AdmitTarget::Account(id) => match self {
                WorkAdmission::Anyone => WorkAdmission::Anyone,
                WorkAdmission::Owner => WorkAdmission::Accounts(BTreeSet::from([id])),
                WorkAdmission::Accounts(mut accounts) => {
                    accounts.insert(id);
                    WorkAdmission::Accounts(accounts)
                }
            },
        }
    }

    /// stop admitting one account — or narrow back from [`Self::Anyone`].
    pub(crate) fn without(self, target: AdmitTarget) -> Self {
        match target {
            AdmitTarget::Anyone => WorkAdmission::Owner,
            AdmitTarget::Account(id) => match self {
                // revoking one account from a wildcard is meaningless and
                // silently doing nothing would read as success: the CLI refuses
                // it before this is reached.
                WorkAdmission::Anyone => WorkAdmission::Anyone,
                WorkAdmission::Owner => WorkAdmission::Owner,
                WorkAdmission::Accounts(mut accounts) => {
                    accounts.remove(&id);
                    narrowed(accounts)
                }
            },
        }
    }
}

/// an empty account set IS `Owner` — one representation per policy, so the
/// file never carries `admit = []` meaning something a reader could mistake
/// for a third state.
fn narrowed(accounts: BTreeSet<Vec<u8>>) -> WorkAdmission {
    match accounts.is_empty() {
        true => WorkAdmission::Owner,
        false => WorkAdmission::Accounts(accounts),
    }
}

/// what one `node work admit|revoke` names. ONE discriminant — the literal
/// `anyone`, or an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmitTarget {
    Anyone,
    Account(Vec<u8>),
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmitFile {
    admit: Vec<String>,
}

/// Read the workspace's policy. A MISSING file is the default, not an error —
/// the same convention `services.toml` uses for "no grants".
pub(crate) fn load(workspace: &Path) -> Result<WorkAdmission, String> {
    let path = policy_path(workspace);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkAdmission::Owner);
        }
        Err(error) => return Err(format!("read {path:?}: {error}")),
    };
    let file: AdmitFile = toml::from_str(&text).map_err(|error| format!("{path:?}: {error}"))?;
    parse(&file.admit)
}

/// Write the workspace's policy. `Owner` REMOVES the file: an empty policy and
/// a missing one mean the same thing, and leaving the husk behind invites a
/// stale read (`services::save`'s rule, for the same reason).
pub(crate) fn save(workspace: &Path, policy: &WorkAdmission) -> Result<(), String> {
    let path = policy_path(workspace);
    let entries = policy.entries();
    if entries.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("remove {path:?}: {error}")),
        };
    }
    let listed: String = entries
        .iter()
        .map(|entry| format!("  \"{entry}\",\n"))
        .collect();
    let text = format!(
        "# whose work this node will execute — the accounts this node admits, on top\n\
         # of its own owner (always admitted) and its own submissions.\n\
         # managed by `ducktape node work admit|revoke`; re-read on every decision.\n\
         # [\"{ANYONE}\"] admits any network member: this node then runs a stranger's\n\
         # workload AND lets it draw on every credential this node is granted.\n\
         admit = [\n{listed}]\n"
    );
    let temporary = workspace.join(format!(".{FILE_NAME}.tmp"));
    std::fs::write(&temporary, text).map_err(|error| format!("write {temporary:?}: {error}"))?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("replace {path:?}: {error}"));
    }
    Ok(())
}

/// Test fixture: give `workspace` a policy admitting `account`, through the same
/// writer the CLI uses. It lives HERE so a lane's own tests never need to name a
/// policy type — `both_lanes_route_through_one_verdict` forbids that, and a
/// fixture is not an exception worth carving into a lint that is otherwise
/// absolute.
#[cfg(test)]
pub(crate) fn admit_account_fixture(workspace: &Path, account: &[u8]) -> Result<(), String> {
    save(
        workspace,
        &WorkAdmission::Owner.with(AdmitTarget::Account(account.to_vec())),
    )
}

pub(crate) fn policy_path(workspace: &Path) -> PathBuf {
    workspace.join(FILE_NAME)
}

/// decode the `admit` list into exactly one policy.
fn parse(entries: &[String]) -> Result<WorkAdmission, String> {
    let wildcards = entries.iter().filter(|entry| *entry == ANYONE).count();
    let mixed = wildcards > 0 && wildcards != entries.len();
    if mixed {
        return Err(format!(
            "admit lists {ANYONE:?} alongside account ids: a wildcard is a statement, not an \
             entry — use either {ANYONE:?} alone or only account ids"
        ));
    }
    if wildcards > 0 {
        return Ok(WorkAdmission::Anyone);
    }
    let mut accounts = BTreeSet::new();
    for entry in entries {
        let id = config::unhex(entry)
            .map_err(|_| format!("admit entry {entry:?} is not a hex account id"))?;
        if id.is_empty() {
            return Err("admit carries an empty account id".into());
        }
        accounts.insert(id);
    }
    Ok(narrowed(accounts))
}

// ============================================================================
// the decision
// ============================================================================

/// What a lane KNOWS about who asked, first-hand. Both arms are derived from a
/// signature or from the mesh transport; neither is anything a caller supplied.
pub(crate) enum WorkSource<'a> {
    /// the mesh-authenticated peer that opened the control stream.
    Peer(&'a [u8]),
    /// a committed saga's origin.
    Saga(&'a SagaOrigin),
}

/// Who is asking, as far as committed state can say. FIVE states, because
/// "could not ask" and "no account bound" are different operator problems —
/// the lesson `airlock::server::GrantAnswer` already paid for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WorkCaller {
    /// the asking key IS this node's key: our own submission, or the terminal
    /// plane's own-node loopback. Zero queries, and it is what keeps a
    /// single-node workspace and the pre-`account-init` window working.
    ThisNode,
    /// committed identity binds the asking node to this account.
    Account(Vec<u8>),
    /// identity answered, and no `BindNode` names that node.
    NodeWithoutAccount,
    /// the saga was triggered by a MODULE, not an external submitter — the
    /// chat/pages/forge/jobs family, whose requester is one hop further back in
    /// `runs`' own state. Not attributable here, and admitted: see the module
    /// header.
    NotAnAccountOrigin,
    /// the identity read did not answer. Nothing is known.
    Unresolved,
}

/// Why work was turned away. The stable snake_case `reason` is DERIVED from the
/// variant, so a typo cannot silently downgrade a refusal
/// (`admin::AdminRefusal`'s shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkRefusal {
    NotAdmitted,
    CallerUnbound,
    PolicyUnreadable,
}

impl WorkRefusal {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            WorkRefusal::NotAdmitted => "work_not_admitted",
            WorkRefusal::CallerUnbound => "work_caller_unbound",
            WorkRefusal::PolicyUnreadable => "work_policy_unreadable",
        }
    }

    /// operator-facing text. Names the verb that fixes it and NEVER echoes the
    /// account that would have been accepted.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            WorkRefusal::NotAdmitted => {
                "this node does not run work for that account — its operator admits one with \
                 `ducktape node work admit <account>`"
            }
            WorkRefusal::CallerUnbound => {
                "the requesting node is bound to no account, so no admission can name it — \
                 run `ducktape user account-init` there"
            }
            WorkRefusal::PolicyUnreadable => {
                "this node's work-admission policy could not be read; it runs nothing until \
                 its operator repairs it"
            }
        }
    }
}

/// THREE states, not two. Folding "I could not ask" into a refusal is the
/// expensive mistake: it tells the caller to go get an admission that may
/// already exist, and on the saga lane it would burn an attempt on a read that
/// simply failed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WorkVerdict {
    Admitted,
    Refused(WorkRefusal),
    AuthorityUnavailable,
}

/// The committed-state read both lanes need, behind the one method they share.
/// Two transports (the daemon's `NodeLink`, the node's `NodeCommand` channel),
/// one resolution.
#[async_trait::async_trait]
pub(crate) trait CommittedReader: Send + Sync {
    async fn read(&self, target: &str, request: Vec<u8>) -> Result<Vec<u8>, String>;
}

/// **The** admission decision. Both lanes call this and nothing else; see
/// `both_lanes_route_through_one_verdict`.
pub(crate) async fn admit(
    reader: &dyn CommittedReader,
    workspace: &Path,
    me: &[u8],
    source: WorkSource<'_>,
) -> WorkVerdict {
    let policy = match load(workspace) {
        Ok(policy) => policy,
        Err(error) => {
            tracing::warn!(
                target: "ducktape::service",
                reason = "work_policy_unreadable",
                %error,
                "work admission cannot read its policy"
            );
            return WorkVerdict::Refused(WorkRefusal::PolicyUnreadable);
        }
    };
    let caller = resolve_caller(reader, me, source).await;
    // The owner is only a QUESTION when the caller is an account the policy has
    // to weigh against it. Reading it unconditionally would make
    // [`WorkCaller::ThisNode`] — the path a single-node workspace and the whole
    // pre-`account-init` window take — depend on an identity module that may not
    // be answering, which is exactly the dependency that path exists to avoid.
    let owner = match &caller {
        WorkCaller::Account(_) => match owner_account(reader, me).await {
            Ok(owner) => owner,
            // The owner is HALF the comparison for an account caller, so a
            // failed owner read is not a refusal either — reporting it as one
            // would tell the operator to admit an account that may already BE
            // the owner. Same three-state lesson, applied to the other read.
            Err(_) => return WorkVerdict::AuthorityUnavailable,
        },
        WorkCaller::ThisNode
        | WorkCaller::NotAnAccountOrigin
        | WorkCaller::NodeWithoutAccount
        | WorkCaller::Unresolved => None,
    };
    verdict(&policy, owner.as_deref(), &caller)
}

/// Pure. Policy first, caller second — so an `Anyone` node keeps running work
/// when the identity module hiccups, which is the only answer that policy can
/// mean.
fn verdict(policy: &WorkAdmission, owner: Option<&[u8]>, caller: &WorkCaller) -> WorkVerdict {
    match policy {
        WorkAdmission::Anyone => WorkVerdict::Admitted,
        WorkAdmission::Owner => admits(owner, caller, &BTreeSet::new()),
        WorkAdmission::Accounts(accounts) => admits(owner, caller, accounts),
    }
}

/// `Owner` is `Accounts(∅)` — written once here rather than twice above.
fn admits(owner: Option<&[u8]>, caller: &WorkCaller, extra: &BTreeSet<Vec<u8>>) -> WorkVerdict {
    match caller {
        WorkCaller::ThisNode => WorkVerdict::Admitted,
        WorkCaller::NotAnAccountOrigin => WorkVerdict::Admitted,
        WorkCaller::Unresolved => WorkVerdict::AuthorityUnavailable,
        WorkCaller::NodeWithoutAccount => WorkVerdict::Refused(WorkRefusal::CallerUnbound),
        WorkCaller::Account(id) => {
            let is_owner = owner == Some(id.as_slice());
            let is_admitted = extra.contains(id);
            match is_owner || is_admitted {
                true => WorkVerdict::Admitted,
                false => WorkVerdict::Refused(WorkRefusal::NotAdmitted),
            }
        }
    }
}

/// the node key a source is attributable to, if any. Pure and exhaustive: a new
/// `SagaOrigin` variant must fail the build here rather than default to admitted.
enum Attributable<'a> {
    Node(&'a [u8]),
    No,
}

fn attributable<'a>(source: &WorkSource<'a>) -> Attributable<'a> {
    match source {
        WorkSource::Peer(key) => Attributable::Node(key),
        WorkSource::Saga(origin) => match origin {
            SagaOrigin::External(key) => Attributable::Node(key),
            SagaOrigin::Module(_) => Attributable::No,
            SagaOrigin::System => Attributable::No,
        },
    }
}

async fn resolve_caller(
    reader: &dyn CommittedReader,
    me: &[u8],
    source: WorkSource<'_>,
) -> WorkCaller {
    let node_key = match attributable(&source) {
        Attributable::No => return WorkCaller::NotAnAccountOrigin,
        Attributable::Node(key) => key,
    };
    // DERIVED, never asserted: `me` is this node's own key and `node_key` came
    // from a verified signature or the mesh transport. Nothing a caller sends
    // can reach this comparison.
    if node_key == me {
        return WorkCaller::ThisNode;
    }
    match account_of_node(reader, node_key).await {
        Ok(Some(account)) => WorkCaller::Account(account),
        Ok(None) => WorkCaller::NodeWithoutAccount,
        Err(_) => WorkCaller::Unresolved,
    }
}

/// this node's own owner account. `Ok(None)` is a node with no owner YET — the
/// pre-`account-init` window, a real state that matches no account. `Err` is a
/// read that did not answer, which is a different thing and is kept different:
/// see [`admit`].
async fn owner_account(reader: &dyn CommittedReader, me: &[u8]) -> Result<Option<Vec<u8>>, String> {
    account_of_node(reader, me).await
}

async fn account_of_node(
    reader: &dyn CommittedReader,
    node_key: &[u8],
) -> Result<Option<Vec<u8>>, String> {
    let request = identity::encode_query(&identity::IdentityQuery::OfNode {
        node_key: node_key.to_vec(),
    });
    let bytes = reader.read("identity", request).await?;
    match identity::decode_reply(&bytes)? {
        identity::IdentityReply::Account(account) => Ok(account.map(|view| view.account_id)),
        other => Err(format!("identity returned an unexpected reply: {other:?}")),
    }
}

#[cfg(test)]
mod tests;
