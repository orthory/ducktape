use std::collections::BTreeSet;

use duckdns_core::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, NODE_KEY_LEN, Registry, ResolvedAccount,
    ResolvedName, ResolvedNode, ResolvedService, ServiceAuthority, ServiceScope, decode_msg,
    decode_query, encode_reply, node_label,
};
use identity::{
    AccountView, IdentityQuery, IdentityReply, decode_reply as identity_decode_reply,
    encode_query as identity_encode_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

pub struct DuckDns {
    id: ModuleId,
    identity_id: ModuleId,
    /// `None` is for the single-node developer daemon. Network-shape nodes
    /// always provide the valset module id.
    valset_id: Option<ModuleId>,
    registry: Registry,
}

impl DuckDns {
    pub fn new(
        id: impl Into<ModuleId>,
        identity_id: impl Into<ModuleId>,
        valset_id: Option<ModuleId>,
        chain_id: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            id: id.into(),
            identity_id: identity_id.into(),
            valset_id,
            registry: Registry::new(chain_id)?,
        })
    }

    pub fn chain_label(&self) -> &str {
        self.registry.chain_label()
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.registry.snapshot()
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        self.registry
            .install(bytes, expected.0)
            .map_err(Error::Module)
    }

    fn origin_node(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(node) if node.len() == NODE_KEY_LEN => Ok(node.clone()),
            Origin::External(node) => Err(Error::Module(format!(
                "duckdns: origin must be a {NODE_KEY_LEN}-byte node key, got {} bytes",
                node.len()
            ))),
            other => Err(Error::Module(format!(
                "duckdns: operation requires an external node origin, got {other:?}"
            ))),
        }
    }

    async fn members(&self, ctx: &dyn Ctx) -> Result<Option<BTreeSet<Vec<u8>>>, Error> {
        let Some(valset_id) = &self.valset_id else {
            return Ok(None);
        };
        let validators = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Validators))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Validators(validators) => validators,
            other => {
                return Err(Error::Module(format!(
                    "duckdns: valset answered Validators with {other:?}"
                )));
            }
        };
        let residents = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Residents))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Residents(residents) => residents,
            other => {
                return Err(Error::Module(format!(
                    "duckdns: valset answered Residents with {other:?}"
                )));
            }
        };
        Ok(Some(validators.into_iter().chain(residents).collect()))
    }

    async fn require_standing(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<(), Error> {
        if self
            .members(ctx)
            .await?
            .is_some_and(|members| !members.contains(node))
        {
            return Err(Error::Module(
                "duckdns: origin is not a validator or admitted resident".into(),
            ));
        }
        Ok(())
    }

    async fn account_of_node(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        match identity_decode_reply(
            &ctx.query(
                &self.identity_id,
                &identity_encode_query(&IdentityQuery::OfNode {
                    node_key: node.to_vec(),
                }),
            )
            .await?,
        )
        .map_err(Error::Module)?
        {
            IdentityReply::Account(account) => Ok(account.map(|account| account.account_id)),
            other => Err(Error::Module(format!(
                "duckdns: identity answered OfNode with {other:?}"
            ))),
        }
    }

    async fn account_by_id(
        &self,
        ctx: &dyn Ctx,
        account_id: &[u8],
    ) -> Result<Option<AccountView>, Error> {
        match identity_decode_reply(
            &ctx.query(
                &self.identity_id,
                &identity_encode_query(&IdentityQuery::Get {
                    account_id: account_id.to_vec(),
                }),
            )
            .await?,
        )
        .map_err(Error::Module)?
        {
            IdentityReply::Account(account) => Ok(account),
            other => Err(Error::Module(format!(
                "duckdns: identity answered Get with {other:?}"
            ))),
        }
    }

    async fn require_account(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<Vec<u8>, Error> {
        self.account_of_node(ctx, node).await?.ok_or_else(|| {
            Error::Module("duckdns: origin is not bound to an identity account".into())
        })
    }

    async fn resolve_account(
        &self,
        ctx: &dyn Ctx,
        handle: &str,
    ) -> Result<Option<ResolvedAccount>, Error> {
        let Some(account_id) = self.registry.handle_owner(handle).map(<[u8]>::to_vec) else {
            return Ok(None);
        };
        let Some(account) = self.account_by_id(ctx, &account_id).await? else {
            return Ok(None);
        };
        if account.account_id != account_id {
            return Err(Error::Module(
                "duckdns: identity returned a mismatched account id".into(),
            ));
        }
        let standing = self.members(ctx).await?;
        let mut nodes = Vec::with_capacity(account.nodes.len());
        for node in account.nodes {
            if standing
                .as_ref()
                .is_some_and(|members| !members.contains(&node))
            {
                continue;
            }
            let label = node_label(&node).map_err(Error::Module)?;
            nodes.push(ResolvedNode {
                node,
                node_label: label,
            });
        }
        nodes.sort_by(|a, b| a.node.cmp(&b.node));
        nodes.dedup_by(|a, b| a.node == b.node);
        Ok(Some(ResolvedAccount { account_id, nodes }))
    }

    async fn filter_service_resolution(
        &self,
        ctx: &dyn Ctx,
        name: &DuckDnsName,
        mut resolved: ResolvedService,
    ) -> Result<Option<ResolvedService>, Error> {
        let standing = self.members(ctx).await?;
        let owner = match &resolved.authority {
            ServiceAuthority::Account { account_id } => Some(account_id),
            ServiceAuthority::Network => None,
        };

        let mut providers = Vec::with_capacity(resolved.providers.len());
        for provider in resolved.providers {
            if standing
                .as_ref()
                .is_some_and(|members| !members.contains(&provider.node))
            {
                continue;
            }
            if let Some(owner) = owner
                && self.account_of_node(ctx, &provider.node).await?.as_ref() != Some(owner)
            {
                continue;
            }
            providers.push(provider);
        }

        // Prefix labels are intentionally short and may theoretically collide.
        // A node-qualified name must resolve to exactly one eligible provider;
        // it never degrades into logical-service failover.
        if matches!(name, DuckDnsName::NodeService { .. }) && providers.len() != 1 {
            return Ok(None);
        }
        if providers.is_empty() {
            return Ok(None);
        }
        resolved.providers = providers;
        Ok(Some(resolved))
    }

    fn query_state(&self, query: DuckDnsQuery) -> Result<Vec<u8>, Error> {
        match query {
            DuckDnsQuery::Resolve { .. } => Err(Error::Module(
                "duckdns: resolution requires a host query context".into(),
            )),
            DuckDnsQuery::Registrations { from, limit } => {
                Ok(encode_reply(&DuckDnsReply::Registrations(
                    self.registry
                        .registrations(from, limit)
                        .map_err(Error::Module)?,
                )))
            }
            DuckDnsQuery::NodeRegistration { .. } => Err(Error::Module(
                "duckdns: node registration requires a host query context".into(),
            )),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for DuckDns {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot(self.registry.root_bytes())
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let node = Self::origin_node(ctx)?;
        self.require_standing(ctx, &node).await?;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            DuckDnsMsg::SetHandle { handle } => {
                let account = self.require_account(ctx, &node).await?;
                self.registry
                    .set_handle(&account, handle)
                    .map_err(Error::Module)
            }
            DuckDnsMsg::ReplaceAnnouncements { announcements } => {
                let needs_account = announcements
                    .iter()
                    .any(|announcement| announcement.scope == ServiceScope::Account);
                let account = if needs_account {
                    Some(self.require_account(ctx, &node).await?)
                } else {
                    None
                };
                self.registry
                    .replace_announcements(&node, account.as_deref(), announcements)
                    .map_err(Error::Module)
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.query_state(decode_query(req).map_err(Error::Module)?)
    }

    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            DuckDnsQuery::Resolve { name } => {
                name.validate().map_err(Error::Module)?;
                let resolved = match &name {
                    DuckDnsName::Account { handle } => self
                        .resolve_account(ctx, handle)
                        .await?
                        .map(ResolvedName::Account),
                    _ => match self
                        .registry
                        .resolve_service(&name)
                        .map_err(Error::Module)?
                    {
                        Some(resolved) => self
                            .filter_service_resolution(ctx, &name, resolved)
                            .await?
                            .map(ResolvedName::Service),
                        None => None,
                    },
                };
                Ok(encode_reply(&DuckDnsReply::Resolved(resolved)))
            }
            DuckDnsQuery::NodeRegistration { node } => {
                if node.len() != NODE_KEY_LEN {
                    return Err(Error::Module(format!(
                        "duckdns: node key must be {NODE_KEY_LEN} bytes, got {}",
                        node.len()
                    )));
                }
                let mut registration = self.registry.node_registration(&node);
                if let Some(stored) = &registration
                    && let Some(account_id) = &stored.account_id
                    && self.account_of_node(ctx, &node).await?.as_ref() != Some(account_id)
                {
                    // A rebinding invalidates the captured account authority.
                    // Presenting no committed registration makes the declarative
                    // announcer refresh it under the node's current AccountId.
                    registration = None;
                }
                Ok(encode_reply(&DuckDnsReply::NodeRegistration(registration)))
            }
            query => self.query_state(query),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.registry.commit();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.registry.abort();
        Ok(())
    }
}
