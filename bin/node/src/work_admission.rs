//! Work admission — whose work this node will execute.
//!
//! ## the premise this whole module rests on
//!
//! A saga's committed [`SagaOrigin::External`] key is DERIVED, never asserted:
//! it is the key whose verified frame signature carried the op. There are two
//! signers. `POST /v1/submit` on a real node DISCARDS the caller's claimed
//! submitter id and re-signs with the node's own signer
//! (`validator/run/ingress.rs`: *"this lane signs frames, and the signed origin
//! IS this node's pubkey"*), so a node-authored op carries the SUBMITTING
//! NODE's key. `POST /v1/submit/frame` relays a frame the USER signed with an
//! account key, so a user-authored op (`ducktape agent run`, a scheduled run)
//! carries that key. The mesh [`PeerId`](data_plane::PeerId) the terminal
//! plane hands us is the same kind of fact, proven by the WireGuard transport.
//!
//! **If `/v1/submit` ever stamped a caller-supplied origin, this admission
//! would become decorative.** `the_submit_lane_still_resigns_with_the_node_key`
//! pins it.
//!
//! ## what it decides, and what it deliberately does not
//!
//! Identity never binds a node to an account. Attribution comes from the
//! signing key alone: a user key resolves to its account through
//! [`identity::IdentityQuery::OfKey`]; a node key resolves to nothing, because
//! no account is ever keyed by a node. So a grant lends to an ACCOUNT, and the
//! question this module answers is not *"who is this run acting for"* (a
//! claim about a third party, which this layer must never make) but *"will
//! this host run this party's workload at all"* — a host deciding its own
//! policy about a party it identified itself.
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
//! - A mesh PEER (the terminal plane's control stream, a peer node's own
//!   `/v1/submit`) is a node, not an account, and the default policy names
//!   only accounts — so a peer's work runs only under [`WorkAdmission::Anyone`]
//!   until the terminal plane carries a user proof of its own.
//! - The guarantee is bounded by `/v1`'s exposure. `POST /v1/submit` re-signs
//!   as THIS node, so anything that can reach the node's HTTP or RPC port takes
//!   the [`WorkCaller::ThisNode`] path by construction. Making un-tokened `/v1`
//!   callers refused is its own campaign
//!   (`docs/superpowers/plans/2026-07-26-wave3-scope-enforcement.md`); keeping
//!   those ports loopback-bound is what makes this module mean anything.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use saga::SagaOrigin;

/// the policy file, beside `node.toml` in the workspace. Deliberately its own
/// file rather than a `node.toml` table: `write_node_toml` REWRITES the whole
/// config on every `init`/`join` merge, so a list living there would need a
/// `Plumbing` field to survive — five touch points for one list. Absent = the
/// default, exactly as an absent `services.toml` means "no grants".
pub(crate) const FILE_NAME: &str = "work-admit.toml";

/// the one `admit` entry that is not an account number — and the one word the
/// CLI takes for it, deliberately the SAME token in both places rather than a
/// config spelling and a CLI spelling that must be kept in sync. It is a
/// statement, not an entry, so it may not be mixed with account numbers.
pub(crate) const ANYONE: &str = "anyone";

// ============================================================================
// the policy
// ============================================================================

/// Whose work this node will run. ONE discriminant; the file's `admit` list
/// decodes into exactly one arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkAdmission {
    /// exactly these accounts (plus this node's own submissions, always). The
    /// default is the EMPTY set: a node runs nobody's work but its own until
    /// its operator admits an account — the same shape as the record it
    /// protects, `gateway::credential_use_allowed`, which admits explicit
    /// grantees and nobody else.
    Accounts(BTreeSet<u64>),
    /// any node the mesh admitted. Opt-in, and it re-opens what this module
    /// exists to close — the CLI says so on the way in.
    Anyone,
}

impl Default for WorkAdmission {
    fn default() -> Self {
        WorkAdmission::Accounts(BTreeSet::new())
    }
}

impl WorkAdmission {
    /// the file's `admit` entries, in the canonical order the file is written
    /// in. The default is the empty list.
    pub(crate) fn entries(&self) -> Vec<String> {
        match self {
            WorkAdmission::Anyone => vec![ANYONE.to_string()],
            WorkAdmission::Accounts(accounts) => {
                accounts.iter().map(|number| number.to_string()).collect()
            }
        }
    }

    /// admit one more account — or widen to [`Self::Anyone`]. `Anyone` absorbs.
    pub(crate) fn with(self, target: AdmitTarget) -> Self {
        match target {
            AdmitTarget::Anyone => WorkAdmission::Anyone,
            AdmitTarget::Account(number) => match self {
                WorkAdmission::Anyone => WorkAdmission::Anyone,
                WorkAdmission::Accounts(mut accounts) => {
                    accounts.insert(number);
                    WorkAdmission::Accounts(accounts)
                }
            },
        }
    }

    /// stop admitting one account — or narrow back from [`Self::Anyone`].
    pub(crate) fn without(self, target: AdmitTarget) -> Self {
        match target {
            AdmitTarget::Anyone => WorkAdmission::default(),
            AdmitTarget::Account(number) => match self {
                // revoking one account from a wildcard is meaningless and
                // silently doing nothing would read as success: the CLI refuses
                // it before this is reached.
                WorkAdmission::Anyone => WorkAdmission::Anyone,
                WorkAdmission::Accounts(mut accounts) => {
                    accounts.remove(&number);
                    WorkAdmission::Accounts(accounts)
                }
            },
        }
    }
}

/// what one `node work admit|revoke` names. ONE discriminant — the literal
/// `anyone`, or an account number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmitTarget {
    Anyone,
    Account(u64),
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
            return Ok(WorkAdmission::default());
        }
        Err(error) => return Err(format!("read {path:?}: {error}")),
    };
    let file: AdmitFile = toml::from_str(&text).map_err(|error| format!("{path:?}: {error}"))?;
    parse(&file.admit)
}

/// Write the workspace's policy. The default REMOVES the file: an empty policy
/// and a missing one mean the same thing, and leaving the husk behind invites
/// a stale read (`services::save`'s rule, for the same reason).
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
        "# whose work this node will execute — the account numbers this node admits,\n\
         # on top of its own submissions (always admitted).\n\
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
pub(crate) fn admit_account_fixture(workspace: &Path, account: u64) -> Result<(), String> {
    save(
        workspace,
        &WorkAdmission::default().with(AdmitTarget::Account(account)),
    )
}

/// Test fixture: the `anyone` policy — the only one under which a mesh PEER's
/// work runs (a peer is a node, never an account). Same rationale as
/// [`admit_account_fixture`].
#[cfg(test)]
pub(crate) fn admit_anyone_fixture(workspace: &Path) -> Result<(), String> {
    save(workspace, &WorkAdmission::Anyone)
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
            "admit lists {ANYONE:?} alongside account numbers: a wildcard is a statement, not \
             an entry — use either {ANYONE:?} alone or only account numbers"
        ));
    }
    if wildcards > 0 {
        return Ok(WorkAdmission::Anyone);
    }
    let mut accounts = BTreeSet::new();
    for entry in entries {
        let number: u64 = entry
            .parse()
            .map_err(|_| format!("admit entry {entry:?} is not an account number"))?;
        if number == 0 {
            return Err("admit carries account number 0, which is no account".into());
        }
        accounts.insert(number);
    }
    Ok(WorkAdmission::Accounts(accounts))
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

/// Who is asking, as far as committed state can say. SIX states, because
/// "could not ask", "a key on no account" and "a peer node" are different
/// operator problems — the lesson `airlock::server::GrantAnswer` already paid
/// for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WorkCaller {
    /// the asking key IS this node's key: our own submission, or the terminal
    /// plane's own-node loopback. Zero queries, and it is what keeps a
    /// single-node workspace and an account-less node working.
    ThisNode,
    /// the signing key is a member of this account.
    Account(u64),
    /// identity answered, and the signing key belongs to no account. A peer
    /// node's own `/v1/submit` lands here too: no account is keyed by a node.
    KeyWithoutAccount,
    /// a mesh peer on the terminal plane — a node, which is never an account.
    PeerNode,
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
                "this node does not run work for that party — its operator admits an account \
                 with `ducktape node work admit <account>`, or anyone with \
                 `ducktape node work admit anyone`"
            }
            WorkRefusal::CallerUnbound => {
                "the submitting key is on no Identity account, so no admission can name it — \
                 submit as a user (`ducktape account create` there, then run user-signed)"
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
    verdict(&policy, &caller)
}

/// Pure. Policy first, caller second — so an `Anyone` node keeps running work
/// when the identity module hiccups, which is the only answer that policy can
/// mean.
fn verdict(policy: &WorkAdmission, caller: &WorkCaller) -> WorkVerdict {
    match policy {
        WorkAdmission::Anyone => WorkVerdict::Admitted,
        WorkAdmission::Accounts(accounts) => admits(accounts, caller),
    }
}

fn admits(accounts: &BTreeSet<u64>, caller: &WorkCaller) -> WorkVerdict {
    match caller {
        WorkCaller::ThisNode => WorkVerdict::Admitted,
        WorkCaller::NotAnAccountOrigin => WorkVerdict::Admitted,
        WorkCaller::Unresolved => WorkVerdict::AuthorityUnavailable,
        WorkCaller::KeyWithoutAccount => WorkVerdict::Refused(WorkRefusal::CallerUnbound),
        WorkCaller::PeerNode => WorkVerdict::Refused(WorkRefusal::NotAdmitted),
        WorkCaller::Account(number) => match accounts.contains(number) {
            true => WorkVerdict::Admitted,
            false => WorkVerdict::Refused(WorkRefusal::NotAdmitted),
        },
    }
}

/// Attribute a source. Exhaustive on purpose: a new `SagaOrigin` variant must
/// fail the build here rather than default to admitted. `me` is this node's
/// own key and every compared key came from a verified signature or the mesh
/// transport — nothing a caller sends can reach these comparisons.
async fn resolve_caller(
    reader: &dyn CommittedReader,
    me: &[u8],
    source: WorkSource<'_>,
) -> WorkCaller {
    match source {
        WorkSource::Peer(node) => match node == me {
            true => WorkCaller::ThisNode,
            false => WorkCaller::PeerNode,
        },
        WorkSource::Saga(SagaOrigin::Module(_)) | WorkSource::Saga(SagaOrigin::System) => {
            WorkCaller::NotAnAccountOrigin
        }
        WorkSource::Saga(SagaOrigin::External(key)) => {
            if key == me {
                return WorkCaller::ThisNode;
            }
            match account_of_key(reader, key).await {
                Ok(Some(number)) => WorkCaller::Account(number),
                Ok(None) => WorkCaller::KeyWithoutAccount,
                Err(_) => WorkCaller::Unresolved,
            }
        }
    }
}

/// The committed key→account resolution. `pub(crate)` because the credential
/// lender's delegation gate (`crate::airlock`) asks the same question of the
/// same module over the same seam — it borrows this READ and nothing else, and
/// in particular never touches the admission policy above: whose work a node
/// runs and whose credential a run draws on are two separate consents.
pub(crate) async fn account_of_key(
    reader: &dyn CommittedReader,
    key: &[u8],
) -> Result<Option<u64>, String> {
    let request = identity::encode_query(&identity::IdentityQuery::OfKey { key: key.to_vec() });
    let bytes = reader.read("identity", request).await?;
    match identity::decode_reply(&bytes)? {
        identity::IdentityReply::Account(account) => Ok(account.map(|view| view.number)),
        other => Err(format!("identity returned an unexpected reply: {other:?}")),
    }
}

#[cfg(test)]
mod tests;
