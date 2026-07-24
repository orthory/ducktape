//! Executing-node credential resolution: turn a run's named gateway credential
//! into a self-host airlock broker source, gated on the run's committed saga
//! origin.
//!
//! A `ducktape agent sched --cred <name>` run carries the credential NAME in its
//! envelope, never any secret. On the node that executes the run, this resolver
//! reads three pieces of COMMITTED state and refuses anything that does not line
//! up:
//!
//! 1. the gateway credential record for `<name>` (routing metadata + seal_pk),
//! 2. the run's saga origin (the submitting node key — cryptographic, stamped by
//!    the frameless `/v1/submit` lane, never user-supplied envelope bytes),
//! 3. the account that node key is bound to (the grant subject).
//!
//! The grant is checked locally with [`gateway::credential_use_allowed`] — a
//! fast refusal before any provider spawns. The owner's gateway remains the
//! final word, since the traffic terminates there.
//!
//! The resolver reads state over the same committed-query lane
//! (`NodeCommand::Query`) the agent provisioner uses, so it sees exactly what
//! consensus committed.

use capability_host::{AirlockConfig, CredentialKind, ResolvedCredential};
use dispatch_host::{CredentialResolver, Resolved};
use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};
use gateway::{CredentialRecord, GatewayQuery, GatewayReply, HandleRegistration, credential_use_allowed};
use identity::{AccountView, IdentityQuery, IdentityReply};
use noded::{NodeCommand, NodeHandle};
use saga::{SagaOrigin, SagaQuery, SagaReply};

/// The gateway route label the co-hosted airlock gateway registers itself under
/// (`bin/node/src/boot/surfaces.rs`). A resolved credential's traffic is routed
/// to `<AIRLOCK_ROUTE>.<owner-handle>.duck`.
const AIRLOCK_ROUTE: &str = "airlock";

/// Resolves a run's named credential against committed state on the executing
/// node. Cheap to clone (holds a command-lane sender and the local browser
/// gateway URL); built once at boot from the node's `http_handle`.
pub(crate) struct NodeCredentialResolver {
    commands: mpsc::Sender<NodeCommand>,
    /// this node's browser-gateway base URL — the `via` a self-host airlock
    /// config routes through onto the overlay gateway plane. `None` on a node
    /// with no browser gateway, which then cannot host a lent-credential run.
    via: Option<String>,
}

impl NodeCredentialResolver {
    pub(crate) fn new(handle: &NodeHandle) -> Self {
        Self {
            commands: handle.command_sender(),
            via: handle.browser_gateway_url(),
        }
    }

    /// Run one committed query over the actor lane and return its raw reply
    /// bytes. The three module reads below each decode their own reply.
    async fn query(&self, target: &str, req: Vec<u8>) -> Result<Vec<u8>, String> {
        let (reply, rx) = oneshot::channel();
        let mut commands = self.commands.clone();
        commands
            .send(NodeCommand::Query {
                target: target.to_string(),
                req,
                reply,
            })
            .await
            .map_err(|_| "node actor is gone".to_string())?;
        rx.await
            .map_err(|_| "node actor dropped the query reply".to_string())?
    }

    async fn credential_record(&self, name: &str) -> Result<Option<CredentialRecord>, String> {
        let bytes = self
            .query(
                "gateway",
                gateway::encode_query(&GatewayQuery::Credential { name: name.to_string() }),
            )
            .await?;
        match gateway::decode_reply(&bytes)? {
            GatewayReply::Credential(record) => Ok(record),
            other => Err(format!("gateway returned an unexpected reply: {other:?}")),
        }
    }

    async fn saga_origin(&self, saga_id: &str) -> Result<SagaOrigin, String> {
        let bytes = self
            .query(
                "saga",
                saga::encode_query(&SagaQuery::Get { saga_id: saga_id.to_string() }),
            )
            .await?;
        match saga::decode_reply(&bytes)? {
            SagaReply::Saga(Some(view)) => Ok(view.origin),
            SagaReply::Saga(None) => Err("scheduled run has no committed saga record".into()),
            other => Err(format!("saga returned an unexpected reply: {other:?}")),
        }
    }

    async fn account_of_node(&self, node_key: &[u8]) -> Result<Option<AccountView>, String> {
        let bytes = self
            .query(
                "identity",
                identity::encode_query(&IdentityQuery::OfNode { node_key: node_key.to_vec() }),
            )
            .await?;
        match identity::decode_reply(&bytes)? {
            IdentityReply::Account(account) => Ok(account),
            other => Err(format!("identity returned an unexpected reply: {other:?}")),
        }
    }

    /// The `.duck` handle registered for `account_id`, if any. Scans the handle
    /// registration listing. ponytail: single page (MAX_QUERY_LIMIT handles); a
    /// network past that needs pagination here.
    async fn handle_of_account(&self, account_id: &[u8]) -> Result<Option<String>, String> {
        let bytes = self
            .query(
                "gateway",
                gateway::encode_query(&GatewayQuery::Registrations {
                    from: 0,
                    limit: gateway::MAX_QUERY_LIMIT,
                }),
            )
            .await?;
        let registrations = match gateway::decode_reply(&bytes)? {
            GatewayReply::Registrations(list) => list,
            other => return Err(format!("gateway returned an unexpected reply: {other:?}")),
        };
        Ok(registrations
            .into_iter()
            .find(|r: &HandleRegistration| r.account_id == account_id)
            .map(|r| r.handle))
    }

    /// Build the broker's self-host config from a granted record: reach the
    /// owner's gateway at `<airlock>.<owner-handle>.duck` through this node's
    /// browser gateway, pinning the on-chain seal_pk (no TEE quote in self-host).
    async fn build_airlock(
        &self,
        record: &CredentialRecord,
        on_behalf: Vec<u8>,
    ) -> Result<AirlockConfig, String> {
        let Some(via) = self.via.clone() else {
            return Err("this node has no browser gateway to route credential traffic".into());
        };
        let Some(owner) = self.account_of_node(&record.publisher_node).await? else {
            return Err("credential publisher node is not bound to an account".into());
        };
        let Some(handle) = self.handle_of_account(&owner.account_id).await? else {
            return Err("credential publisher has no registered duck handle".into());
        };
        let resolved = ResolvedCredential {
            name: record.name.clone(),
            kind: map_kind(record.kind),
            authority: format!("{AIRLOCK_ROUTE}.{handle}.duck"),
            via,
            seal_pk: record.seal_pk,
            account: on_behalf,
        };
        Ok(AirlockConfig::self_host(&resolved))
    }
}

#[async_trait::async_trait]
impl CredentialResolver for NodeCredentialResolver {
    async fn resolve(&self, credential: &str, saga_id: &str) -> Result<Resolved, String> {
        let record = self.credential_record(credential).await?;
        let origin = self.saga_origin(saga_id).await?;
        // only an EXTERNAL origin (a submitting node key) maps to an account; a
        // module/system-triggered saga gets no identity lookup at all.
        let account = match &origin {
            SagaOrigin::External(node_key) => self.account_of_node(node_key).await?,
            SagaOrigin::Module(_) | SagaOrigin::System => None,
        };
        let (record, on_behalf) = authorize(credential, record, &origin, account)?;
        let airlock = self.build_airlock(&record, on_behalf).await?;
        Ok(Resolved { airlock })
    }
}

/// The pure grant decision — the whole security gate in one testable place. The
/// three committed reads are done by the caller; this decides. Returns the
/// granted record and the account to act on behalf of, or a named refusal.
fn authorize(
    credential: &str,
    record: Option<CredentialRecord>,
    origin: &SagaOrigin,
    account: Option<AccountView>,
) -> Result<(CredentialRecord, Vec<u8>), String> {
    let Some(record) = record else {
        return Err(format!("unknown credential: {credential}"));
    };
    let SagaOrigin::External(_) = origin else {
        return Err("scheduled run has no account origin".into());
    };
    let Some(account) = account else {
        return Err("submitting node is not bound to an account".into());
    };
    let granted = credential_use_allowed(&record, &account.account_id);
    if !granted {
        return Err("credential_not_granted".into());
    }
    Ok((record, account.account_id))
}

/// The node owns the gateway↔capability-host credential-kind mapping, because
/// capability-host does not depend on the gateway crate.
fn map_kind(kind: gateway::CredentialKind) -> CredentialKind {
    match kind {
        gateway::CredentialKind::Claude => CredentialKind::Claude,
        gateway::CredentialKind::Codex => CredentialKind::Codex,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn record(owner: &[u8], grants: &[&[u8]]) -> CredentialRecord {
        CredentialRecord {
            name: "owner-claude-1".into(),
            owner_account: owner.to_vec(),
            publisher_node: b"pub-node".to_vec(),
            kind: gateway::CredentialKind::Claude,
            seal_pk: [3u8; 32],
            grants: grants.iter().map(|g| g.to_vec()).collect::<BTreeSet<_>>(),
        }
    }

    fn account(id: &[u8]) -> AccountView {
        AccountView {
            account_id: id.to_vec(),
            display_name: None,
            avatar: None,
            bio: None,
            nonce: 0,
            member_keys: Vec::new(),
            nodes: Vec::new(),
            updated_at: 0,
        }
    }

    fn external(node_key: &[u8]) -> SagaOrigin {
        SagaOrigin::External(node_key.to_vec())
    }

    #[test]
    fn the_owner_account_resolves_and_acts_for_itself() {
        let (record, on_behalf) = authorize(
            "owner-claude-1",
            Some(record(b"owner", &[])),
            &external(b"owner-node"),
            Some(account(b"owner")),
        )
        .expect("the owner may always use its own credential");
        assert_eq!(record.name, "owner-claude-1");
        assert_eq!(on_behalf, b"owner");
    }

    #[test]
    fn a_granted_account_resolves() {
        let (_record, on_behalf) = authorize(
            "owner-claude-1",
            Some(record(b"owner", &[b"guest"])),
            &external(b"guest-node"),
            Some(account(b"guest")),
        )
        .expect("a granted account may use the credential");
        assert_eq!(on_behalf, b"guest");
    }

    #[test]
    fn an_ungranted_account_is_refused() {
        let err = authorize(
            "owner-claude-1",
            Some(record(b"owner", &[b"someone-else"])),
            &external(b"stranger-node"),
            Some(account(b"stranger")),
        )
        .unwrap_err();
        assert_eq!(err, "credential_not_granted");
    }

    #[test]
    fn an_unknown_credential_is_refused_first() {
        let err = authorize("ghost", None, &external(b"node"), Some(account(b"acct"))).unwrap_err();
        assert!(err.contains("unknown credential: ghost"), "got {err:?}");
    }

    #[test]
    fn a_non_external_origin_has_no_account() {
        for origin in [SagaOrigin::Module("runs".into()), SagaOrigin::System] {
            let err = authorize(
                "owner-claude-1",
                Some(record(b"owner", &[])),
                &origin,
                Some(account(b"owner")),
            )
            .unwrap_err();
            assert_eq!(err, "scheduled run has no account origin");
        }
    }

    #[test]
    fn an_unbound_submitting_node_is_refused() {
        let err = authorize(
            "owner-claude-1",
            Some(record(b"owner", &[])),
            &external(b"orphan-node"),
            None,
        )
        .unwrap_err();
        assert_eq!(err, "submitting node is not bound to an account");
    }

    #[test]
    fn kinds_map_across_the_boundary() {
        assert_eq!(map_kind(gateway::CredentialKind::Claude), CredentialKind::Claude);
        assert_eq!(map_kind(gateway::CredentialKind::Codex), CredentialKind::Codex);
    }
}
