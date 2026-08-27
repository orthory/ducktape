//! qmdb-backed ACCOUNT registry: numbered principals over key associations.
//!
//! an ACCOUNT is identified by a NUMBER (monotonic from 1) and owns an
//! ASSOCIATION of keys of mixed schemes (an ed25519 device key, an Ethereum
//! wallet, a WebAuthn passkey -- see [`KeyScheme`]). the frame ORIGIN is the
//! acting key for every op; a node key is never bound to an account, and no
//! consumer ever asks "whose node is this" -- attribution comes only from a
//! user-signed origin, resolved through [`IdentityQuery::OfKey`].
//!
//! - [`IdentityMsg::Create`] founds an account for the origin key.
//! - [`IdentityMsg::AddKey`] admits the origin key into an existing member's
//!   account: the member consents over [`add_key_preimage`] at the origin's
//!   CURRENT generation; acceptance advances that generation, so the consent
//!   is single-use -- including after a compromised key is removed. a removed
//!   key is never burned (a wallet or an SSH key cannot be re-minted): it can
//!   be re-admitted anywhere by a consent signed at its next generation.
//! - [`IdentityMsg::RemoveKey`] drops a key (any member may drop any, except
//!   the last -- an account always keeps at least one live key).
//! - [`IdentityMsg::SetName`] / [`IdentityMsg::SetProfile`] are member-gated
//!   by the origin alone.
//!
//! ## State model
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today -- `statesync::qmdb::QmdbStore`) and hands
//! it to [`Identity::new`], so this crate never names a storage crate. one
//! record per account (`acct\0{number}`), the KEY INDEX (`key\0{pubkey}` ->
//! number), the per-key ADMISSION COUNTER (`gen\0{pubkey}` -> how many times
//! that key has been admitted anywhere; absent = 0), and `next` (the next
//! account number; absent = 1). no roster: accounts are never deleted, so
//! `All` walks `acct\0{from..next}` directly, and [`MAX_ACCOUNTS`] caps `next`.
//!
//! writes are staged during a block and flushed to the store in one batch at
//! `commit_block`; the module root IS the store's merkle root. sync belongs
//! to the store, not this module: a joiner rebuilds the concrete store from a
//! peer (`QmdbStore::sync_from`) and wraps a fresh `Identity` around it.
//!
//! oversized values never reach the store (the poison-value lesson -- the
//! qmdb wire codec bounds a value at decode, so an over-cap committed value
//! would wedge every syncing peer): EVERY path that grows an account record
//! restages the whole record through the [`MAX_ACCOUNT_RECORD_BYTES`] gate.
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
// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the one verifier every consent rides — shared with the kernel frame codec,
// so an account key and a frame origin are verified identically.
pub use keyscheme::KeyScheme;

// test-only consent builders. dev-only: gated so a shipping build never
// compiles the ed25519 signing helpers into itself.
#[cfg(feature = "testkit")]
pub mod testkit;

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};

/// accounts retained over the network's life (an account is never deleted).
/// founding past this refuses loudly at execute.
pub const MAX_ACCOUNTS: u64 = 65_536;
/// serialized account-record ceiling, enforced on EVERY staged account write.
/// there is no growth path that bypasses the gate -- create/add-key/profile
/// ops all restage the whole record through it -- so an op that would push a
/// record past the cap is refused loudly and deterministically instead of
/// poisoning the sync wire.
pub const MAX_ACCOUNT_RECORD_BYTES: usize = 512 * 1024;

/// per-account record key: prefix + 0 + the number, little-endian.
fn acct_key(number: AccountNumber) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + 8);
    key.extend_from_slice(b"acct");
    key.push(0);
    key.extend_from_slice(&number.to_le_bytes());
    key
}

/// key-index key: prefix + 0 + public key. valued by the owning account
/// number; maintained by the execute paths as they stage.
fn key_owner_key(pubkey: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + 1 + pubkey.len());
    key.extend_from_slice(b"key");
    key.push(0);
    key.extend_from_slice(pubkey);
    key
}

/// admission-counter key: prefix + 0 + public key. valued by how many times
/// the key has been admitted anywhere; absent = 0.
fn key_gen_key(pubkey: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + 1 + pubkey.len());
    key.extend_from_slice(b"gen");
    key.push(0);
    key.extend_from_slice(pubkey);
    key
}

/// the next account number's whole key. collides with no `acct\0...` /
/// `key\0...` / `gen\0...` key (nor the host-seeded `__config` record).
const NEXT_NUMBER_KEY: &[u8] = b"next";

/// per-key metadata; the public key is the map key, so it is not repeated.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
struct KeyMeta {
    scheme: KeyScheme,
    label: Option<String>,
    added_at: u64,
}

/// one account: name, the association, profile fields, last-write time. the
/// number is the record key, so it is not repeated here. borsh writes the
/// `BTreeMap` length-prefixed in key order, so one record set has exactly one
/// encoding.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
struct AccountRecord {
    name: String,
    keys: BTreeMap<Vec<u8>, KeyMeta>,
    /// duckfs path the app resolves the avatar image against (`None` unset).
    avatar: Option<String>,
    /// short bio/status line (`None` unset).
    bio: Option<String>,
    updated_at: u64,
}

pub struct Identity {
    id: ModuleId,
    /// this network's chain id -- folded into every add-key consent so one
    /// minted for one network can never act on another.
    chain_id: String,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes, folded into `root()` at `commit_block`). store key
    /// is `sha256(logical_key)`, owned by [`StagedStore`].
    staged: StagedStore,
}

impl Identity {
    /// wrap the host-constructed store under module identity `id`.
    pub fn new(id: impl Into<ModuleId>, store: Box<dyn MerkleStore>, chain_id: String) -> Self {
        Self {
            id: id.into(),
            chain_id,
            staged: StagedStore::new(store),
        }
    }

    // ---- staged-over-committed reads ----------------------------------------

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: BorshDeserialize,
    {
        match self.staged.get(key).await? {
            Some(bytes) => Ok(Some(
                borsh::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage a value whose serialized size is bounded by construction (an
    /// index entry, a counter, the next number) -- see the module doc's
    /// poison-value paragraph. account records go through
    /// [`Self::store_account`].
    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: BorshSerialize,
    {
        self.staged.stage(
            key,
            borsh::to_vec(value).expect("identity value is serializable"),
        );
    }

    async fn account(&self, number: AccountNumber) -> Result<Option<AccountRecord>, Error> {
        self.load(&acct_key(number)).await
    }

    /// an account the key index points at. an index entry without its record
    /// is a store bug -- loud, never skipped.
    async fn stored_account(&self, number: AccountNumber) -> Result<AccountRecord, Error> {
        self.account(number)
            .await?
            .ok_or_else(|| Error::Module("missing account record".into()))
    }

    /// the account `key` belongs to, if any -- the key-index read.
    async fn owner_of_key(&self, key: &[u8]) -> Result<Option<AccountNumber>, Error> {
        self.load(&key_owner_key(key)).await
    }

    /// how many times `key` has been admitted anywhere (absent = 0).
    async fn key_gen(&self, key: &[u8]) -> Result<u64, Error> {
        Ok(self.load(&key_gen_key(key)).await?.unwrap_or(0))
    }

    /// the next account number (absent = 1).
    async fn next_number(&self) -> Result<AccountNumber, Error> {
        Ok(self.load(NEXT_NUMBER_KEY).await?.unwrap_or(1))
    }

    /// stage an updated account record under the byte cap -- the ONE write
    /// every account mutation funnels through (see [`MAX_ACCOUNT_RECORD_BYTES`]).
    fn store_account(&mut self, number: AccountNumber, record: &AccountRecord) -> Result<(), Error> {
        let bytes = borsh::to_vec(record).expect("identity value is serializable");
        if bytes.len() > MAX_ACCOUNT_RECORD_BYTES {
            return Err(Error::Module(format!(
                "account record too large: {} > {MAX_ACCOUNT_RECORD_BYTES} bytes",
                bytes.len()
            )));
        }
        self.staged.stage(acct_key(number), bytes);
        Ok(())
    }

    // ---- gates ---------------------------------------------------------------

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

    /// the origin's own account -- the gate every member-only op (remove,
    /// rename, profile) funnels through.
    async fn account_of_origin(
        &self,
        ctx: &dyn Ctx,
    ) -> Result<(AccountNumber, AccountRecord), Error> {
        let origin = Self::origin_key(ctx)?;
        let number = self
            .owner_of_key(&origin)
            .await?
            .ok_or_else(|| Error::Module("origin key belongs to no account".into()))?;
        let record = self.stored_account(number).await?;
        Ok((number, record))
    }

    fn account_view(number: AccountNumber, record: &AccountRecord) -> AccountView {
        AccountView {
            number,
            name: record.name.clone(),
            keys: record
                .keys
                .iter()
                .map(|(pubkey, meta)| KeyView {
                    scheme: meta.scheme,
                    pubkey: pubkey.clone(),
                    label: meta.label.clone(),
                    added_at: meta.added_at,
                })
                .collect(),
            avatar: record.avatar.clone(),
            bio: record.bio.clone(),
            updated_at: record.updated_at,
        }
    }

    async fn view_of(&self, number: Option<AccountNumber>) -> Result<Option<AccountView>, Error> {
        match number {
            Some(number) => Ok(Some(Self::account_view(
                number,
                &self.stored_account(number).await?,
            ))),
            None => Ok(None),
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
            IdentityMsg::Create { name, scheme } => self.create(ctx, name, scheme).await,
            IdentityMsg::AddKey {
                scheme,
                label,
                authorizer,
            } => self.add_key(ctx, scheme, label, authorizer).await,
            IdentityMsg::RemoveKey { key } => self.remove_key(ctx, key).await,
            IdentityMsg::SetName { name } => self.set_name(ctx, name).await,
            IdentityMsg::SetProfile { avatar, bio } => self.set_profile(ctx, avatar, bio).await,
        }
    }

    /// read projection — committed plus this block's staged changes (the
    /// staged-over-committed store view).
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            IdentityQuery::All { from, limit } => {
                // walk the numbering directly: no deletion ever, so no gaps.
                let first = from.max(1);
                let end = self.next_number().await?;
                let limit = usize::try_from(limit.min(MAX_QUERY_LIMIT)).expect("clamped");
                let mut accounts = Vec::new();
                for number in (first..end).take(limit) {
                    let record = self.stored_account(number).await?;
                    accounts.push(Self::account_view(number, &record));
                }
                Ok(encode_reply(&IdentityReply::Accounts(accounts)))
            }
            IdentityQuery::Get { number } => Ok(encode_reply(&IdentityReply::Account(
                self.account(number)
                    .await?
                    .map(|record| Self::account_view(number, &record)),
            ))),
            IdentityQuery::OfKey { key } => {
                let number = self.owner_of_key(&key).await?;
                Ok(encode_reply(&IdentityReply::Account(
                    self.view_of(number).await?,
                )))
            }
            IdentityQuery::KeyGen { key } => {
                Ok(encode_reply(&IdentityReply::Gen(self.key_gen(&key).await?)))
            }
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
    /// found an account for the origin key.
    async fn create(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        scheme: KeyScheme,
    ) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;
        if !scheme.pubkey_wellformed(&origin) {
            return Err(Error::Module(
                "founding key is malformed for its scheme".into(),
            ));
        }
        if self.owner_of_key(&origin).await?.is_some() {
            return Err(Error::Module("key already belongs to an account".into()));
        }
        let name = clean_name(name)?;
        let number = self.next_number().await?;
        let cap_reached = number > MAX_ACCOUNTS;
        if cap_reached {
            return Err(Error::Module(format!(
                "account cap reached ({MAX_ACCOUNTS})"
            )));
        }

        let now = ctx.env().consensus_time;
        let mut keys = BTreeMap::new();
        keys.insert(
            origin.clone(),
            KeyMeta {
                scheme,
                label: None,
                added_at: now,
            },
        );
        let record = AccountRecord {
            name,
            keys,
            avatar: None,
            bio: None,
            updated_at: now,
        };
        self.store_account(number, &record)?;
        // bounded by construction: a well-formed key and two integers.
        self.store(key_owner_key(&origin), &number);
        self.store(NEXT_NUMBER_KEY.to_vec(), &(number + 1));
        Ok(())
    }

    /// admit the origin key into the authorizer's account.
    async fn add_key(
        &mut self,
        ctx: &mut dyn Ctx,
        scheme: KeyScheme,
        label: Option<String>,
        authorizer: Authorizer,
    ) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;
        if !scheme.pubkey_wellformed(&origin) {
            return Err(Error::Module(
                "joining key is malformed for its scheme".into(),
            ));
        }
        if self.owner_of_key(&origin).await?.is_some() {
            return Err(Error::Module("key already belongs to an account".into()));
        }
        let number = self
            .owner_of_key(&authorizer.key)
            .await?
            .ok_or_else(|| Error::Module("authorizer belongs to no account".into()))?;
        let mut record = self.stored_account(number).await?;
        let authorizer_scheme = record
            .keys
            .get(&authorizer.key)
            .map(|meta| meta.scheme)
            .ok_or_else(|| Error::Module("key index disagrees with its account record".into()))?;

        // the consent names THIS key at ITS current generation -- nothing else.
        let generation = self.key_gen(&origin).await?;
        let preimage = add_key_preimage(&self.chain_id, scheme, &origin, generation);
        if !authorizer_scheme.verify(
            &authorizer.key,
            IDENTITY_ADD_KEY_NS,
            &preimage,
            &authorizer.proof,
        ) {
            return Err(Error::Module(
                "authorizer consent does not verify".into(),
            ));
        }
        let label = clean_label(label)?;

        let now = ctx.env().consensus_time;
        record.keys.insert(
            origin.clone(),
            KeyMeta {
                scheme,
                label,
                added_at: now,
            },
        );
        record.updated_at = now;
        self.store_account(number, &record)?;
        self.store(key_owner_key(&origin), &number);
        // the consent just spent is dead from here on.
        self.store(key_gen_key(&origin), &(generation + 1));
        Ok(())
    }

    /// drop `key` from the origin's account. any member may drop any member
    /// (including itself), except the last remaining one.
    async fn remove_key(&mut self, ctx: &mut dyn Ctx, key: Vec<u8>) -> Result<(), Error> {
        let (number, mut record) = self.account_of_origin(ctx).await?;
        if !record.keys.contains_key(&key) {
            return Err(Error::Module(
                "target key is not a member of this account".into(),
            ));
        }
        let last_key = record.keys.len() == 1;
        if last_key {
            return Err(Error::Module(
                "cannot remove the last key of an account".into(),
            ));
        }
        record.keys.remove(&key);
        record.updated_at = ctx.env().consensus_time;
        self.store_account(number, &record)?;
        // the generation counter stays: a re-admission signs at the next one.
        self.staged.delete(key_owner_key(&key));
        Ok(())
    }

    /// rename the origin's account.
    async fn set_name(&mut self, ctx: &mut dyn Ctx, name: String) -> Result<(), Error> {
        let (number, mut record) = self.account_of_origin(ctx).await?;
        record.name = clean_name(name)?;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(number, &record)
    }

    /// set the avatar ref and/or bio of the origin's account. each field
    /// empty-trims to cleared, over its byte cap rejects.
    async fn set_profile(
        &mut self,
        ctx: &mut dyn Ctx,
        avatar: Option<String>,
        bio: Option<String>,
    ) -> Result<(), Error> {
        let (number, mut record) = self.account_of_origin(ctx).await?;
        record.avatar = clean_field(avatar, MAX_AVATAR_REF_LEN, "avatar reference")?;
        record.bio = clean_field(bio, MAX_BIO_LEN, "bio")?;
        record.updated_at = ctx.env().consensus_time;
        self.store_account(number, &record)
    }
}

/// trim an account name: empty -> reject, over the limit -> reject.
fn clean_name(name: String) -> Result<String, Error> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::Module("account name is empty".into()));
    }
    if trimmed.len() > MAX_NAME_LEN {
        return Err(Error::Module(format!(
            "account name exceeds the {MAX_NAME_LEN}-byte limit"
        )));
    }
    Ok(trimmed.to_string())
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

/// trim a key label: empty -> `None` (no label), over the limit -> reject.
fn clean_label(label: Option<String>) -> Result<Option<String>, Error> {
    clean_field(label, MAX_LABEL_LEN, "key label")
}

#[cfg(test)]
mod tests;

// the wasm-guest port: the store-backed dispatch shell that adapts this module
// to the ducktape:module world. compiled only by the guest-builder's
// synthesized wasm32 cdylib workspace (feature `guest`), never by the native
// build.
#[cfg(feature = "guest")]
mod guest;
