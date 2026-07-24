//! qmdb-backed ACCOUNT registry: an umbrella over a person's keys and nodes.
//!
//! an ACCOUNT is keyed by its FOUNDING key (`account_id` = the first member
//! key). it collects many MEMBER KEYS of different schemes (an ed25519 seed
//! key, a WebAuthn passkey, a native P-256 key -- see [`KeyKind`]), shares one
//! display name across them, and owns many NODES (each a workspace's
//! mesh/valset identity). every state-changing op is authorized by a MEMBER
//! KEY, captured as a [`MemberAuth`]: the account it speaks for is resolved
//! from that key's membership, never a spoofable payload field. proofs are
//! verified over chain-and-nonce-scoped preimages, so a certificate can never
//! replay across networks or after the shared nonce advances.
//!
//! - [`IdentityMsg::BindNode`] binds the SUBMITTING NODE (origin) to the
//!   authorizer's account, CREATING that account if the authorizer is a
//!   brand-new founding key (the desktop's auto-bind path).
//! - [`IdentityMsg::UnbindNode`] evicts a node, authorized by ANY member --
//!   the "surviving device evicts a lost one" recovery path.
//! - [`IdentityMsg::AddMemberKey`] admits a new key: an existing member
//!   consents AND the new key proves possession, both over one preimage.
//! - [`IdentityMsg::RemoveMemberKey`] drops a key (any member may drop any,
//!   except the last -- an account always keeps at least one live key).
//! - [`IdentityMsg::SetAccountName`] renames, origin-gated to a bound node.
//! - [`IdentityMsg::SetNodeLabel`] labels a bound node, origin-gated the same
//!   way (a bound node labels its account's own devices; the label is a
//!   per-network on-chain fact visible to the user's other devices).
//!
//! ## State model
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today -- `statesync::qmdb::QmdbStore`) and hands
//! it to [`Identity::new`], so this crate never names a storage crate. one
//! logical record per account (`acct\0{account_id}`), plus the two OWNERSHIP
//! INDEXES as write-path-maintained point records -- `node\0{node_key}` and
//! `member\0{member_key}`, each valued by the owning account id -- and two
//! aggregate records:
//!
//! - the account ROSTER (the sorted account-id list, bounded by
//!   [`MAX_ACCOUNTS`]) -- the ONE enumeration read. it stays canonical
//!   because identity's reads are consumed in-consensus (governance resolves
//!   actors/shares through them at execute time) and by the operator CLIs;
//!   the id-length bound is structural ([`KeyKind::pubkey_wellformed`] admits
//!   nothing past a 65-byte SEC1 point);
//! - the CLIENT set (the submit-door ACL, bounded by [`MAX_CLIENTS`]) -- the
//!   node's submit door consumes it whole between drains, and governance's
//!   redeem dedup reads it, so it lives as one sorted aggregate of fixed
//!   32-byte ed25519 keys. an empty set deletes the record, so a fully
//!   revoked plane is byte-identical to one that never granted.
//!
//! `OfNode`/`OfMember` are canonical POINT READS over the index records
//! (dispatch-consumed: the join/settle paths resolve node standing through
//! them), so no derived read model exists to rebuild -- the execute paths
//! maintain the indexes as they stage.
//!
//! writes are staged during a block and flushed to the store in one batch at
//! `commit_block`; the module root IS the store's merkle root. sync belongs
//! to the store, not this module: a joiner rebuilds the concrete store from a
//! peer (`QmdbStore::sync_from`) and wraps a fresh `Identity` around it.
//!
//! oversized values never reach the store (the poison-value lesson -- the
//! qmdb wire codec bounds a value at decode, so an over-cap committed value
//! would wedge every syncing peer): EVERY path that grows an account record
//! (bind, add-member, the profile setters) restages the whole record through
//! the [`MAX_ACCOUNT_RECORD_BYTES`] gate, so no growth can bypass it and no
//! half-cap headroom is needed; the roster is byte-gated at
//! [`MAX_ROSTER_RECORD_BYTES`] on top of its count cap; the client set is
//! bounded by construction (fixed 32-byte keys under [`MAX_CLIENTS`]).
//!
//! ## Genesis config (the chain id)
//!
//! the per-network chain id reaches the NATIVE module through
//! [`Identity::new`]. the wasm tenant is fixed bytes, so there the id rides
//! GENESIS CONFIG: the host seeds the reserved `__config` entry
//! ([`sdk::genesis_config`]) into this module's store at genesis construction
//! -- under [`sdk::store_key`], the same logical→store mapping every record
//! here uses -- and the guest decodes it per dispatch. the config is
//! consensus state in the store's merkle root from genesis and rides
//! state-sync like any other record. this module never writes that key.
//!
//! ## client standing (the submit-door ACL, a facet of the account plane)
//!
//! identity also carries the CLIENT set: ed25519 keys that hold SUBMIT
//! authorization at a validator's door and nothing else — no consensus seat,
//! no mesh, no statesync (the sync/mesh planes read valset, never this set, so
//! a client can never leak into standing). governance's `role=Client` invite
//! redemption emits [`IdentityMsg::GrantClient`] as a MODULE-origin follow-up;
//! [`IdentityMsg::RevokeClient`] drops a key. the set is one store record in
//! identity's ONE root, so a joiner restores it with the rest of the account
//! plane.
//!
// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the pluggable member-key verifier: "(kind, pubkey, proof) -> valid?" for
// every scheme an account can collect. flattened at the crate root so the wire
// types (`KeyKind`, `MemberProof`) and the account logic share one vocabulary.
mod scheme;
pub use scheme::{KeyKind, MemberProof, verify_authority, webauthn_challenge, webauthn_rp_id_hash};

// test-only member-auth builders (beside `IDENTITY_BIND_NS`). dev-only: gated so
// a shipping build never compiles the ed25519 signing helpers into itself.
#[cfg(feature = "testkit")]
pub mod testkit;

use std::collections::BTreeMap;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519::PublicKey;
use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// accounts retained over the network's life (an account is never deleted --
/// an unbind can empty its node set, but the record and its roster entry
/// persist). founding past this refuses loudly at execute.
pub const MAX_ACCOUNTS: usize = 1024;
/// serialized roster-record byte bound, enforced at founding. the count cap
/// alone does not bound the roster's SERIALIZED form: account ids are keys of
/// up to 65 bytes ([`KeyKind::pubkey_wellformed`]) rendered as JSON byte
/// arrays, so the byte gate keeps the committed record far under the qmdb
/// value-decode ceiling (the poison-value lesson).
pub const MAX_ROSTER_RECORD_BYTES: usize = 512 * 1024;
/// serialized account-record ceiling, enforced on EVERY staged account write.
/// unlike governance's ballots there is no growth path that bypasses the
/// gate -- bind/add-member/profile ops all restage the whole record through
/// it -- so an op that would push a record past the cap is refused loudly and
/// deterministically instead of poisoning the sync wire.
pub const MAX_ACCOUNT_RECORD_BYTES: usize = 512 * 1024;
/// client-ACL count cap. clients enter one governance-redeemed invite at a
/// time and every key is a fixed 32 bytes, so the record stays bounded by
/// construction under this count.
pub const MAX_CLIENTS: usize = 1024;

/// per-account record key: prefix + 0 + account id (the single-component
/// shape chat uses). safe because every key literal below is fixed and none
/// is another followed by a 0 byte.
fn acct_key(account_id: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + account_id.len());
    key.extend_from_slice(b"acct");
    key.push(0);
    key.extend_from_slice(account_id);
    key
}

/// node-ownership index key: prefix + 0 + node key. valued by the owning
/// account id; maintained by the execute paths as they stage.
fn node_owner_key(node: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + node.len());
    key.extend_from_slice(b"node");
    key.push(0);
    key.extend_from_slice(node);
    key
}

/// member-ownership index key: prefix + 0 + member public key.
fn member_owner_key(member: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(6 + 1 + member.len());
    key.extend_from_slice(b"member");
    key.push(0);
    key.extend_from_slice(member);
    key
}

/// the account roster's whole key. collides with no `acct\0...` /
/// `node\0...` / `member\0...` key (nor the host-seeded `__config`
/// genesis-config record).
const ACCOUNT_ROSTER_KEY: &[u8] = b"accounts";

/// the client set's whole key. absent = no client holds standing.
const CLIENTS_KEY: &[u8] = b"clients";

/// per-member metadata; the public key is the pair key, so it is not
/// repeated. serialized verbatim inside [`AccountStored`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MemberMeta {
    kind: KeyKind,
    label: Option<String>,
    /// `SHA-256(rp_id)` a WebAuthn member's later assertions must carry in
    /// authenticatorData -- pins the passkey to the RP it enrolled under.
    /// `Some` iff `kind == WebauthnP256`.
    rp_id_hash: Option<[u8; 32]>,
    added_at: u64,
}

/// per-node metadata; the node key is the map key, so it is not repeated. the
/// label is the human name a bound device set (`SetNodeLabel`) and is dropped
/// with the node on unbind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct NodeMeta {
    label: Option<String>,
}

/// one account: display name, avatar ref + bio (the account's global profile,
/// propagated per-network by the app), shared replay nonce, the member-key
/// set, the labeled bound-node map, and the last-write block timestamp.
/// `account_id` is the record key, so it is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountRecord {
    display_name: Option<String>,
    /// duckfs path the app resolves the avatar image against (`None` unset).
    avatar: Option<String>,
    /// short bio/status line (`None` unset).
    bio: Option<String>,
    nonce: u64,
    member_keys: BTreeMap<Vec<u8>, MemberMeta>,
    nodes: BTreeMap<Vec<u8>, NodeMeta>,
    updated_at: u64,
}

/// the stored form of an [`AccountRecord`]: maps flatten to sorted pair lists
/// because the record codec (JSON) cannot key an object by raw bytes.
/// `BTreeMap` iteration writes them sorted; `collect` rebuilds the maps.
#[derive(Serialize, Deserialize)]
struct AccountStored {
    display_name: Option<String>,
    avatar: Option<String>,
    bio: Option<String>,
    nonce: u64,
    member_keys: Vec<(Vec<u8>, MemberMeta)>,
    nodes: Vec<(Vec<u8>, Option<String>)>,
    updated_at: u64,
}

impl From<&AccountRecord> for AccountStored {
    fn from(r: &AccountRecord) -> Self {
        Self {
            display_name: r.display_name.clone(),
            avatar: r.avatar.clone(),
            bio: r.bio.clone(),
            nonce: r.nonce,
            member_keys: r
                .member_keys
                .iter()
                .map(|(k, meta)| (k.clone(), meta.clone()))
                .collect(),
            nodes: r
                .nodes
                .iter()
                .map(|(k, meta)| (k.clone(), meta.label.clone()))
                .collect(),
            updated_at: r.updated_at,
        }
    }
}

impl From<AccountStored> for AccountRecord {
    fn from(s: AccountStored) -> Self {
        Self {
            display_name: s.display_name,
            avatar: s.avatar,
            bio: s.bio,
            nonce: s.nonce,
            member_keys: s.member_keys.into_iter().collect(),
            nodes: s
                .nodes
                .into_iter()
                .map(|(k, label)| (k, NodeMeta { label }))
                .collect(),
            updated_at: s.updated_at,
        }
    }
}

pub struct Identity {
    id: ModuleId,
    /// the valset module consulted to gate `BindNode` to current members
    /// (validators UNION residents); `None` runs ungated (the single-node
    /// daemon carries no valset).
    valset_id: Option<ModuleId>,
    /// this network's chain id -- folded into every signed preimage so a
    /// certificate minted for one network can never act on another.
    chain_id: String,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes, folded into `root()` at `commit_block`). store key
    /// is `sha256(logical_key)`, owned by [`StagedStore`].
    staged: StagedStore,
}

impl Identity {
    /// wrap the host-constructed store under module identity `id`.
    pub fn new(
        id: impl Into<ModuleId>,
        store: Box<dyn MerkleStore>,
        valset_id: Option<ModuleId>,
        chain_id: String,
    ) -> Self {
        Self {
            id: id.into(),
            valset_id,
            chain_id,
            staged: StagedStore::new(store),
        }
    }

    // ---- staged-over-committed reads ----------------------------------------

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: DeserializeOwned,
    {
        match self.staged.get(key).await? {
            Some(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage a value whose serialized size is bounded by construction (an
    /// ownership index entry, the client set) -- see the module doc's
    /// poison-value paragraph. account records and the roster go through
    /// [`Self::store_bounded`].
    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: Serialize,
    {
        self.staged.stage(
            key,
            serde_json::to_vec(value).expect("identity value is serializable"),
        );
    }

    /// stage a value only if its serialized size fits `cap` -- the write-time
    /// guard against poison values (the qmdb codec cap is decode-only).
    fn store_bounded<T>(
        &mut self,
        key: Vec<u8>,
        value: &T,
        cap: usize,
        what: &str,
    ) -> Result<(), Error>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec(value).expect("identity value is serializable");
        if bytes.len() > cap {
            return Err(Error::Module(format!(
                "{what} record too large: {} > {cap} bytes",
                bytes.len()
            )));
        }
        self.staged.stage(key, bytes);
        Ok(())
    }

    async fn account(&self, account_id: &[u8]) -> Result<Option<AccountRecord>, Error> {
        Ok(self
            .load::<AccountStored>(&acct_key(account_id))
            .await?
            .map(AccountRecord::from))
    }

    /// an account a roster entry or ownership index points at. a reference key
    /// without its record is a store bug -- loud, never skipped.
    async fn stored_account(&self, account_id: &[u8]) -> Result<AccountRecord, Error> {
        self.account(account_id)
            .await?
            .ok_or_else(|| Error::Module("missing account record".into()))
    }

    /// the account owning `node`, if bound -- the node-ownership index read.
    async fn owner_of_node(&self, node: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.load(&node_owner_key(node)).await
    }

    /// the account `member` belongs to, if any -- the member-ownership index.
    async fn owner_of_member(&self, member: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.load(&member_owner_key(member)).await
    }

    /// the account roster -- every account id, sorted. record, roster, and
    /// index entries are staged (and commit or abort) together, so membership
    /// in one is membership in all.
    async fn roster(&self) -> Result<Vec<Vec<u8>>, Error> {
        Ok(self.load(ACCOUNT_ROSTER_KEY).await?.unwrap_or_default())
    }

    /// the client set, sorted. absent record = empty set.
    async fn client_set(&self) -> Result<Vec<Vec<u8>>, Error> {
        Ok(self.load(CLIENTS_KEY).await?.unwrap_or_default())
    }

    /// stage an updated account record under the byte cap -- the ONE write
    /// every account mutation funnels through (see [`MAX_ACCOUNT_RECORD_BYTES`]).
    fn store_account(&mut self, account_id: &[u8], record: &AccountRecord) -> Result<(), Error> {
        self.store_bounded(
            acct_key(account_id),
            &AccountStored::from(record),
            MAX_ACCOUNT_RECORD_BYTES,
            "account",
        )
    }

    // ---- gates ---------------------------------------------------------------

    /// validate that `key` is a well-formed 32-byte ed25519 public key — the
    /// explicit length guard keeps the 32-byte invariant independent of decode's
    /// trailing-byte behavior; `PublicKey::decode` then checks the curve point.
    fn validate_client_key(key: &[u8]) -> Result<(), Error> {
        if key.len() != 32 {
            return Err(Error::Module(format!(
                "invalid ed25519 client key: expected 32 bytes, got {}",
                key.len()
            )));
        }
        PublicKey::decode(key)
            .map_err(|e| Error::Module(format!("invalid ed25519 client key: {e}")))?;
        Ok(())
    }

    /// the AUTHENTICATED submitter key -- a non-empty external origin, or a
    /// deterministic rejection.
    fn origin_key(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(bytes) if bytes.is_empty() => Err(Error::Module(
                "external origin must carry a non-empty submitter id".into(),
            )),
            Origin::External(bytes) => Ok(bytes.clone()),
            other => Err(Error::Module(format!(
                "identity operations are origin-gated to external submitters, got {other:?}"
            ))),
        }
    }

    /// verify that `auth` is a current member of `record` and that its proof
    /// authorizes `preimage` under `namespace`. the one gate every member-signed
    /// op funnels through, so scheme dispatch lives in exactly one place.
    fn authorize(
        record: &AccountRecord,
        namespace: &[u8],
        preimage: &[u8],
        auth: &MemberAuth,
    ) -> Result<(), Error> {
        let meta = record
            .member_keys
            .get(&auth.key)
            .ok_or_else(|| Error::Module("authorizer is not a member of this account".into()))?;
        if meta.kind != auth.kind {
            return Err(Error::Module(
                "authorizer kind does not match its registered kind".into(),
            ));
        }
        if !verify_authority(
            auth.kind,
            &auth.key,
            meta.rp_id_hash.as_ref(),
            namespace,
            preimage,
            &auth.proof,
        ) {
            return Err(Error::Module(
                "authorizer certificate does not verify".into(),
            ));
        }
        Ok(())
    }

    /// client standing changes only via governance: a module origin (its redeem
    /// follow-up) or a system origin (genesis). part of the deterministic Env,
    /// enforced identically on every node.
    fn require_module_origin(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::Module(_) | Origin::System => Ok(()),
            Origin::External(_) => Err(Error::Module(
                "client standing changes only via governance".into(),
            )),
        }
    }

    fn account_view(account_id: &[u8], record: &AccountRecord) -> AccountView {
        AccountView {
            account_id: account_id.to_vec(),
            display_name: record.display_name.clone(),
            avatar: record.avatar.clone(),
            bio: record.bio.clone(),
            nonce: record.nonce,
            member_keys: record
                .member_keys
                .iter()
                .map(|(pubkey, meta)| MemberKeyView {
                    pubkey: pubkey.clone(),
                    kind: meta.kind,
                    label: meta.label.clone(),
                    added_at: meta.added_at,
                })
                .collect(),
            nodes: record
                .nodes
                .iter()
                .map(|(node_key, meta)| NodeView {
                    node_key: node_key.clone(),
                    label: meta.label.clone(),
                })
                .collect(),
            updated_at: record.updated_at,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Identity {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the store's merkle root over all committed records, verbatim — the
    /// staged overlay is invisible here until `commit_block`.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            IdentityMsg::BindNode { authorizer } => self.bind_node(ctx, authorizer).await,
            IdentityMsg::UnbindNode {
                node_key,
                authorizer,
            } => self.unbind_node(ctx, node_key, authorizer).await,
            IdentityMsg::AddMemberKey {
                new_key,
                new_kind,
                new_label,
                possession,
                authorizer,
            } => {
                self.add_member_key(ctx, new_key, new_kind, new_label, possession, authorizer)
                    .await
            }
            IdentityMsg::RemoveMemberKey {
                target_key,
                authorizer,
            } => self.remove_member_key(ctx, target_key, authorizer).await,
            IdentityMsg::SetAccountName { display_name } => {
                self.set_account_name(ctx, display_name).await
            }
            IdentityMsg::SetProfile { avatar, bio } => self.set_profile(ctx, avatar, bio).await,
            IdentityMsg::SetNodeLabel { node_key, label } => {
                self.set_node_label(ctx, node_key, label).await
            }
            IdentityMsg::GrantClient { key } => self.grant_client(ctx, key).await,
            IdentityMsg::RevokeClient { key } => self.revoke_client(ctx, key).await,
        }
    }

    /// read projection — committed plus this block's staged changes (the
    /// staged-over-committed store view).
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            IdentityQuery::All { from, limit } => {
                // walk the roster window by derived key (≤ MAX_QUERY_LIMIT
                // point reads).
                let roster = self.roster().await?;
                let limit = limit.min(MAX_QUERY_LIMIT) as usize;
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let mut accounts = Vec::new();
                for account_id in roster.iter().skip(from).take(limit) {
                    let record = self.stored_account(account_id).await?;
                    accounts.push(Self::account_view(account_id, &record));
                }
                Ok(encode_reply(&IdentityReply::Accounts(accounts)))
            }
            IdentityQuery::Get { account_id } => Ok(encode_reply(&IdentityReply::Account(
                self.account(&account_id)
                    .await?
                    .map(|record| Self::account_view(&account_id, &record)),
            ))),
            IdentityQuery::OfNode { node_key } => {
                let account = match self.owner_of_node(&node_key).await? {
                    Some(account_id) => Some(Self::account_view(
                        &account_id,
                        &self.stored_account(&account_id).await?,
                    )),
                    None => None,
                };
                Ok(encode_reply(&IdentityReply::Account(account)))
            }
            IdentityQuery::OfMember { member_key } => {
                let account = match self.owner_of_member(&member_key).await? {
                    Some(account_id) => Some(Self::account_view(
                        &account_id,
                        &self.stored_account(&account_id).await?,
                    )),
                    None => None,
                };
                Ok(encode_reply(&IdentityReply::Account(account)))
            }
            IdentityQuery::Clients => Ok(encode_reply(&IdentityReply::Clients(
                self.client_set().await?,
            ))),
        }
    }

    /// publish the block's staged writes in ONE store batch. no-op (and no
    /// root movement) if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}

impl Identity {
    /// bind the origin node to the authorizer's account, creating that account
    /// when the authorizer is a brand-new founding key.
    async fn bind_node(&mut self, ctx: &mut dyn Ctx, authorizer: MemberAuth) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;

        // member gate: validators UNION residents, only when configured.
        if let Some(valset_id) = self.valset_id.clone() {
            let members = valset::members_and_residents(&*ctx, &valset_id).await?;
            if !members.contains(&origin) {
                return Err(Error::Module(
                    "bind origin is not a network member or resident".into(),
                ));
            }
        }

        // which account does this authorizer speak for -- an existing
        // membership, or a brand-new account it founds?
        let resolved = self.owner_of_member(&authorizer.key).await?;
        let founding = resolved.is_none();
        let (account_id, mut record) = match resolved {
            Some(account_id) => {
                let record = self.stored_account(&account_id).await?;
                (account_id, record)
            }
            None => {
                if !authorizer.kind.pubkey_wellformed(&authorizer.key) {
                    return Err(Error::Module(
                        "founding key is malformed for its kind".into(),
                    ));
                }
                if self.account(&authorizer.key).await?.is_some() {
                    return Err(Error::Module(
                        "account id already exists but its founding key is not a member".into(),
                    ));
                }
                let rp_id_hash = if authorizer.kind.expects_rp_id_hash() {
                    webauthn_rp_id_hash(&authorizer.proof)
                } else {
                    None
                };
                let mut member_keys = BTreeMap::new();
                member_keys.insert(
                    authorizer.key.clone(),
                    MemberMeta {
                        kind: authorizer.kind,
                        label: None,
                        rp_id_hash,
                        added_at: ctx.env().consensus_time,
                    },
                );
                let record = AccountRecord {
                    display_name: None,
                    avatar: None,
                    bio: None,
                    nonce: 0,
                    member_keys,
                    nodes: BTreeMap::new(),
                    updated_at: 0,
                };
                (authorizer.key.clone(), record)
            }
        };

        // idempotent re-bind: node already bound to THIS account -> no-op, no
        // nonce bump, nothing staged. the proof is deliberately left unverified
        // here (no state change, and origin is already consensus-authenticated).
        if let Some(bound_to) = self.owner_of_node(&origin).await? {
            if bound_to == account_id {
                return Ok(());
            }
            return Err(Error::Module(
                "node is already bound to another account; unbind first".into(),
            ));
        }

        // verify the member's consent at the account's CURRENT nonce.
        let preimage = bind_preimage(&self.chain_id, &origin, record.nonce);
        Self::authorize(&record, IDENTITY_BIND_NS, &preimage, &authorizer)?;

        // a founding bind also claims a roster slot and the founder's
        // member-index entry.
        if founding {
            let mut roster = self.roster().await?;
            let position = match roster.binary_search(&account_id) {
                Ok(_) => {
                    return Err(Error::Module(
                        "account roster carries an id with no record".into(),
                    ));
                }
                Err(position) => position,
            };
            if roster.len() >= MAX_ACCOUNTS {
                return Err(Error::Module(format!("account cap reached ({MAX_ACCOUNTS})")));
            }
            roster.insert(position, account_id.clone());
            self.store_bounded(
                ACCOUNT_ROSTER_KEY.to_vec(),
                &roster,
                MAX_ROSTER_RECORD_BYTES,
                "account roster",
            )?;
            // bounded by construction: the account id is a well-formed key.
            self.store(member_owner_key(&authorizer.key), &account_id);
        }

        record.nodes.insert(origin.clone(), NodeMeta::default());
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)?;
        self.store(node_owner_key(&origin), &account_id);
        Ok(())
    }

    /// evict `node_key`, authorized by any member of its account.
    async fn unbind_node(
        &mut self,
        ctx: &mut dyn Ctx,
        node_key: Vec<u8>,
        authorizer: MemberAuth,
    ) -> Result<(), Error> {
        Self::origin_key(ctx)?;

        let account_id = self
            .owner_of_node(&node_key)
            .await?
            .ok_or_else(|| Error::Module("node is not bound".into()))?;
        let mut record = self.stored_account(&account_id).await?;

        let preimage = unbind_preimage(&self.chain_id, &node_key, record.nonce);
        Self::authorize(&record, IDENTITY_UNBIND_NS, &preimage, &authorizer)?;

        // the record persists even with an empty node set: members + name +
        // nonce survive so a re-bind can still resolve them.
        record.nodes.remove(&node_key);
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)?;
        self.staged.delete(node_owner_key(&node_key));
        Ok(())
    }

    /// admit `new_key` to the authorizer's account: an existing member consents
    /// and the new key proves possession, both over the same add-preimage.
    async fn add_member_key(
        &mut self,
        ctx: &mut dyn Ctx,
        new_key: Vec<u8>,
        new_kind: KeyKind,
        new_label: Option<String>,
        possession: MemberProof,
        authorizer: MemberAuth,
    ) -> Result<(), Error> {
        Self::origin_key(ctx)?;

        let account_id = self
            .owner_of_member(&authorizer.key)
            .await?
            .ok_or_else(|| Error::Module("authorizer belongs to no account".into()))?;
        let mut record = self.stored_account(&account_id).await?;

        if !new_kind.pubkey_wellformed(&new_key) {
            return Err(Error::Module(
                "new member key is malformed for its kind".into(),
            ));
        }
        if record.member_keys.contains_key(&new_key) {
            return Err(Error::Module(
                "key is already a member of this account".into(),
            ));
        }
        if self.owner_of_member(&new_key).await?.is_some() {
            return Err(Error::Module(
                "key already belongs to another account".into(),
            ));
        }
        let label = clean_label(new_label)?;

        let preimage = add_member_preimage(
            &self.chain_id,
            &account_id,
            &new_key,
            new_kind,
            record.nonce,
        );
        // existing member consents ...
        Self::authorize(&record, IDENTITY_ADD_MEMBER_NS, &preimage, &authorizer)?;
        // ... and the new key proves it holds itself (no rp pin yet -- the
        // proof establishes it).
        if !verify_authority(
            new_kind,
            &new_key,
            None,
            IDENTITY_ADD_MEMBER_NS,
            &preimage,
            &possession,
        ) {
            return Err(Error::Module("possession proof does not verify".into()));
        }
        let rp_id_hash = if new_kind.expects_rp_id_hash() {
            Some(webauthn_rp_id_hash(&possession).ok_or_else(|| {
                Error::Module("webauthn possession proof carries no rp id hash".into())
            })?)
        } else {
            None
        };

        record.member_keys.insert(
            new_key.clone(),
            MemberMeta {
                kind: new_kind,
                label,
                rp_id_hash,
                added_at: ctx.env().consensus_time,
            },
        );
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)?;
        self.store(member_owner_key(&new_key), &account_id);
        Ok(())
    }

    /// drop `target_key` from the authorizer's account. any member may drop any
    /// member (including itself), except the last remaining one.
    async fn remove_member_key(
        &mut self,
        ctx: &mut dyn Ctx,
        target_key: Vec<u8>,
        authorizer: MemberAuth,
    ) -> Result<(), Error> {
        Self::origin_key(ctx)?;

        let account_id = self
            .owner_of_member(&authorizer.key)
            .await?
            .ok_or_else(|| Error::Module("authorizer belongs to no account".into()))?;
        let mut record = self.stored_account(&account_id).await?;

        if !record.member_keys.contains_key(&target_key) {
            return Err(Error::Module(
                "target key is not a member of this account".into(),
            ));
        }
        if record.member_keys.len() == 1 {
            return Err(Error::Module(
                "cannot remove the last member of an account".into(),
            ));
        }

        let preimage =
            remove_member_preimage(&self.chain_id, &account_id, &target_key, record.nonce);
        Self::authorize(&record, IDENTITY_REMOVE_MEMBER_NS, &preimage, &authorizer)?;

        record.member_keys.remove(&target_key);
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)?;
        self.staged.delete(member_owner_key(&target_key));
        Ok(())
    }

    /// set the display name of the account the origin node is bound to.
    async fn set_account_name(
        &mut self,
        ctx: &mut dyn Ctx,
        display_name: String,
    ) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;
        let account_id = self
            .owner_of_node(&origin)
            .await?
            .ok_or_else(|| Error::Module("origin node is not bound to an account".into()))?;
        let mut record = self.stored_account(&account_id).await?;

        let trimmed = display_name.trim();
        if trimmed.is_empty() {
            record.display_name = None;
        } else if trimmed.len() > MAX_NAME_LEN {
            return Err(Error::Module(format!(
                "display name exceeds the {MAX_NAME_LEN}-byte limit"
            )));
        } else {
            record.display_name = Some(trimmed.to_string());
        }
        // no signature is consumed here: the nonce is NOT bumped.
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)
    }

    /// set the avatar ref and/or bio of the account the origin node is bound
    /// to. origin-gated exactly like `set_account_name`; each field empty-trims
    /// to cleared, over its byte cap rejects. no signature, no nonce bump.
    async fn set_profile(
        &mut self,
        ctx: &mut dyn Ctx,
        avatar: Option<String>,
        bio: Option<String>,
    ) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;
        let account_id = self
            .owner_of_node(&origin)
            .await?
            .ok_or_else(|| Error::Module("origin node is not bound to an account".into()))?;
        let mut record = self.stored_account(&account_id).await?;

        record.avatar = clean_field(avatar, MAX_AVATAR_REF_LEN, "avatar reference")?;
        record.bio = clean_field(bio, MAX_BIO_LEN, "bio")?;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)
    }

    /// set (or clear) the label of `node_key`. origin-gated exactly like
    /// `set_account_name`: the submitting node must be bound to an account, and
    /// `node_key` must be bound to that SAME account (you label your own
    /// devices). a device label is cosmetic display metadata, so it rides the
    /// bound-node origin gate with no member signature and no nonce bump.
    async fn set_node_label(
        &mut self,
        ctx: &mut dyn Ctx,
        node_key: Vec<u8>,
        label: Option<String>,
    ) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;
        let account_id = self
            .owner_of_node(&origin)
            .await?
            .ok_or_else(|| Error::Module("origin node is not bound to an account".into()))?;
        // the target must be bound to the SAME account -- a bound node labels
        // only its own account's devices, never another account's node.
        if self.owner_of_node(&node_key).await? != Some(account_id.clone()) {
            return Err(Error::Module(
                "target node is not bound to the origin's account".into(),
            ));
        }
        let mut record = self.stored_account(&account_id).await?;

        let meta = record
            .nodes
            .get_mut(&node_key)
            .ok_or_else(|| Error::Module("node index disagrees with its account record".into()))?;
        meta.label = clean_label(label)?;
        // no signature is consumed here: the nonce is NOT bumped.
        record.updated_at = ctx.env().consensus_time;
        self.store_account(&account_id, &record)
    }

    /// grant CLIENT (submit-door) standing to `key`. GOVERNANCE-GATED exactly
    /// like valset membership: only a module origin (governance's redeem
    /// follow-up) or a system origin (genesis) may stage it — an external key
    /// cannot self-grant. a key that already holds standing is a no-op that
    /// stages nothing (no root movement).
    async fn grant_client(&mut self, ctx: &mut dyn Ctx, key: Vec<u8>) -> Result<(), Error> {
        Self::require_module_origin(ctx)?;
        Self::validate_client_key(&key)?;
        let mut clients = self.client_set().await?;
        let Err(position) = clients.binary_search(&key) else {
            return Ok(());
        };
        if clients.len() >= MAX_CLIENTS {
            return Err(Error::Module(format!("client cap reached ({MAX_CLIENTS})")));
        }
        clients.insert(position, key);
        // bounded by construction: ≤ MAX_CLIENTS fixed 32-byte keys.
        self.store(CLIENTS_KEY.to_vec(), &clients);
        Ok(())
    }

    /// revoke client standing by `key`; a no-op (nothing staged) if the key
    /// holds none. same governance origin gate as [`Identity::grant_client`].
    /// revoking the last client deletes the record, so the store returns to
    /// its never-granted shape.
    async fn revoke_client(&mut self, ctx: &mut dyn Ctx, key: Vec<u8>) -> Result<(), Error> {
        Self::require_module_origin(ctx)?;
        let mut clients = self.client_set().await?;
        let Ok(position) = clients.binary_search(&key) else {
            return Ok(());
        };
        clients.remove(position);
        if clients.is_empty() {
            self.staged.delete(CLIENTS_KEY.to_vec());
        } else {
            self.store(CLIENTS_KEY.to_vec(), &clients);
        }
        Ok(())
    }
}

/// the CURRENT client set at `identity_id`: its staged-over-committed
/// projection, via the host-routed read lane. the one shared read the redeem
/// path and the submit door's caller funnel through.
pub async fn clients(ctx: &dyn Ctx, identity_id: &str) -> Result<Vec<Vec<u8>>, Error> {
    let reply = ctx
        .query(identity_id, &encode_query(&IdentityQuery::Clients))
        .await?;
    match decode_reply(&reply).map_err(Error::Module)? {
        IdentityReply::Clients(list) => Ok(list),
        other => Err(Error::Module(format!(
            "identity answered a Clients query with {other:?}"
        ))),
    }
}

/// trim an optional profile field: `None` or empty-after-trim -> cleared
/// (`None`), over `max` bytes -> reject, else the trimmed string.
fn clean_field(value: Option<String>, max: usize, what: &str) -> Result<Option<String>, Error> {
    match value {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if trimmed.len() > max {
                Err(Error::Module(format!(
                    "{what} exceeds the {max}-byte limit"
                )))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
    }
}

/// trim a member label: empty -> `None` (no label), over the limit -> reject.
fn clean_label(label: Option<String>) -> Result<Option<String>, Error> {
    match label {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if trimmed.len() > MAX_LABEL_LEN {
                Err(Error::Module(format!(
                    "member label exceeds the {MAX_LABEL_LEN}-byte limit"
                )))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests;

// the wasm-guest port: the store-backed dispatch shell that adapts this module
// to the ducktape:module world. compiled only by the guest-builder's
// synthesized wasm32 cdylib workspace (feature `guest`), never by the native
// build.
#[cfg(feature = "guest")]
mod guest;
