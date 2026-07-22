//! The merged gateway module: the whole `.duck` **name → AccountId → route**
//! pipeline as ONE consensus tenant.
//!
//! Two planes share this module, both gated the same way (external 32-byte node
//! origin, valset standing = validators ∪ residents, identity `OfNode` account
//! derivation):
//! * the **handle plane** (absorbed from duckdns) — an optional human `.duck`
//!   name for one Identity account. Resolution stops at the stable AccountId;
//!   this module stores no node address.
//! * the **route plane** — an Identity account signs one monotonic route from
//!   its apex or a service label to a typed upstream plus an invocation policy.
//!
//! The committed state is the two pure registries composed under one root: a
//! length-framed snapshot `[names_len][names_snapshot][routes_snapshot]` and a
//! combined root that is the ZERO sentinel while BOTH planes are empty and a
//! domain-tagged digest of the two sub-roots otherwise.

use std::collections::BTreeSet;

use duckdns::Registry as NamesRegistry;
use identity::{
    AccountView, IdentityQuery, IdentityReply, KeyKind, MemberProof,
    decode_reply as identity_decode_reply, encode_query as identity_encode_query, verify_authority,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

use crate::registry::Registry as RoutesRegistry;
use crate::{
    GATEWAY_ROUTE_NS, GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization, RouteRecord,
    RouteStatement, decode_msg, decode_query, encode_reply, route_signing_preimage,
};

/// domain tag separating the merged root digest from either sub-root and from
/// any other digest.
const MERGED_ROOT_DOMAIN: &[u8] = b"ducktape-gateway-merged-v1";

pub struct Gateway {
    id: ModuleId,
    identity_id: ModuleId,
    valset_id: Option<ModuleId>,
    chain_id: String,
    /// the `.duck` handle plane (human name → AccountId).
    names: NamesRegistry,
    /// the route plane (AccountId + name → upstream + policy).
    routes: RoutesRegistry,
}

impl Gateway {
    pub fn new(
        id: impl Into<ModuleId>,
        identity_id: impl Into<ModuleId>,
        valset_id: Option<ModuleId>,
        chain_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            identity_id: identity_id.into(),
            valset_id,
            chain_id: chain_id.into(),
            names: NamesRegistry::new(),
            routes: RoutesRegistry::new(),
        }
    }

    fn combined_root(&self) -> [u8; 32] {
        let names = self.names.root_bytes();
        let routes = self.routes.root_bytes();
        if names == [0; 32] && routes == [0; 32] {
            return [0; 32];
        }
        let mut hasher = Sha256::new();
        hasher.update(MERGED_ROOT_DOMAIN);
        hasher.update(names);
        hasher.update(routes);
        hasher.finalize().into()
    }

    pub fn snapshot(&self) -> Vec<u8> {
        let names = self.names.snapshot();
        let routes = self.routes.snapshot();
        let mut out = Vec::with_capacity(8 + names.len() + routes.len());
        out.extend_from_slice(&(names.len() as u64).to_le_bytes());
        out.extend_from_slice(&names);
        out.extend_from_slice(&routes);
        out
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        self.install_bytes(bytes)
            .and_then(|()| {
                let actual = self.combined_root();
                if actual == expected.0 {
                    Ok(())
                } else {
                    Err(format!(
                        "gateway: merged snapshot root mismatch: decoded {}, expected {}",
                        hex(&actual),
                        hex(&expected.0)
                    ))
                }
            })
            .map_err(Error::Module)
    }

    fn install_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let header: [u8; 8] = bytes
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .ok_or_else(|| "gateway: merged snapshot truncated header".to_string())?;
        let names_len = usize::try_from(u64::from_le_bytes(header))
            .map_err(|_| "gateway: merged snapshot names length overflows usize".to_string())?;
        let names_end = 8usize
            .checked_add(names_len)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| "gateway: merged snapshot names length out of bounds".to_string())?;
        self.names.adopt(&bytes[8..names_end])?;
        self.routes.adopt(&bytes[names_end..])
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
            ValsetReply::Validators(nodes) => nodes,
            other => {
                return Err(Error::Module(format!(
                    "gateway: valset answered Validators with {other:?}"
                )));
            }
        };
        let residents = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Residents))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Residents(nodes) => nodes,
            other => {
                return Err(Error::Module(format!(
                    "gateway: valset answered Residents with {other:?}"
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
                "gateway: origin is not a validator or admitted resident".into(),
            ));
        }
        Ok(())
    }

    async fn account_of_node(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<AccountView, Error> {
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
            IdentityReply::Account(Some(account)) => Ok(account),
            IdentityReply::Account(None) => Err(Error::Module(
                "gateway: origin is not bound to an Identity account".into(),
            )),
            other => Err(Error::Module(format!(
                "gateway: identity answered OfNode with {other:?}"
            ))),
        }
    }

    fn origin_node(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(node) if node.len() == crate::NODE_KEY_BYTES => Ok(node.clone()),
            Origin::External(node) => Err(Error::Module(format!(
                "gateway: origin must be a {}-byte node key, got {} bytes",
                crate::NODE_KEY_BYTES,
                node.len()
            ))),
            other => Err(Error::Module(format!(
                "gateway: mutation requires an external node origin, got {other:?}"
            ))),
        }
    }

    /// the handle plane: bind the authenticated node's account to (or free) one
    /// optional `.duck` name. AccountId is authority; the handle is a mutable
    /// presentation alias.
    async fn set_handle(
        &mut self,
        ctx: &dyn Ctx,
        origin: &[u8],
        handle: Option<String>,
    ) -> Result<(), Error> {
        let account = self.account_of_node(ctx, origin).await?;
        self.names
            .set_handle(&account.account_id, handle)
            .map_err(Error::Module)
    }

    async fn set_route(
        &mut self,
        ctx: &dyn Ctx,
        origin: &[u8],
        statement: RouteStatement,
        authorization: MemberAuthorization,
    ) -> Result<(), Error> {
        if statement.chain_id != self.chain_id {
            return Err(Error::Module(
                "gateway: route belongs to another chain".into(),
            ));
        }
        if statement.publisher_node != origin {
            return Err(Error::Module(
                "gateway: publisher does not match the authenticated origin".into(),
            ));
        }
        let account = self.account_of_node(ctx, origin).await?;
        if statement.account_id != account.account_id {
            return Err(Error::Module(
                "gateway: route account does not own the publisher node".into(),
            ));
        }
        let signer_is_current = account
            .member_keys
            .iter()
            .any(|member| member.pubkey == authorization.signer && member.kind == KeyKind::Ed25519);
        if !signer_is_current {
            return Err(Error::Module(
                "gateway: signer is not a current Ed25519 account member".into(),
            ));
        }
        let preimage = route_signing_preimage(&statement).map_err(Error::Module)?;
        let proof = MemberProof::Signature {
            sig: authorization.signature.clone(),
        };
        if !verify_authority(
            KeyKind::Ed25519,
            &authorization.signer,
            None,
            GATEWAY_ROUTE_NS,
            &preimage,
            &proof,
        ) {
            return Err(Error::Module(
                "gateway: route signature does not verify".into(),
            ));
        }
        self.routes
            .set_route(RouteRecord {
                statement,
                authorization,
            })
            .map_err(Error::Module)
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Gateway {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot(self.combined_root())
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let origin = Self::origin_node(ctx)?;
        self.require_standing(ctx, &origin).await?;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            GatewayMsg::SetHandle { handle } => self.set_handle(ctx, &origin, handle).await,
            GatewayMsg::SetRoute {
                statement,
                authorization,
            } => self.set_route(ctx, &origin, statement, authorization).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            GatewayQuery::Resolve { name } => Ok(encode_reply(&GatewayReply::Resolved(
                self.names.resolve(&name).map_err(Error::Module)?,
            ))),
            GatewayQuery::Registrations { from, limit } => {
                Ok(encode_reply(&GatewayReply::Registrations(
                    self.names
                        .registrations(from, limit)
                        .map_err(Error::Module)?,
                )))
            }
            GatewayQuery::Get { account_id, name } => Ok(encode_reply(&GatewayReply::Route(
                Box::new(self.routes.route(&account_id, &name).map_err(Error::Module)?),
            ))),
            GatewayQuery::List { account_id } => Ok(encode_reply(&GatewayReply::Routes(
                self.routes.routes(&account_id).map_err(Error::Module)?,
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.names.commit();
        self.routes.commit();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.names.abort();
        self.routes.abort();
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_round_trips_to_zero_root() {
        let module = Gateway::new("gateway", "identity", None, "test");
        let snapshot = module.snapshot();
        let mut restored = Gateway::new("gateway", "identity", None, "test");
        restored
            .install(&snapshot, StateRoot([0; 32]))
            .expect("empty install");
        assert_eq!(restored.root(), StateRoot([0; 32]));
    }

    #[test]
    fn handle_write_moves_the_merged_root_and_survives_a_snapshot() {
        let mut module = Gateway::new("gateway", "identity", None, "test");
        module
            .names
            .set_handle(&[9u8; 32], Some("orthory".into()))
            .expect("set handle");
        module.names.commit();
        let root = module.root();
        assert_ne!(root, StateRoot([0; 32]), "a handle write moves the root");
        let snapshot = module.snapshot();
        let mut restored = Gateway::new("gateway", "identity", None, "test");
        restored.install(&snapshot, root).expect("install");
        assert_eq!(restored.root(), root);
        // a trailing byte breaks the frame's routes tail decode.
        let mut trailing = snapshot;
        trailing.push(0);
        assert!(restored.install(&trailing, root).is_err());
    }
}
