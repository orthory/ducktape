//! deterministic ACCOUNT registry: an umbrella over a person's keys and nodes.
//!
//! an ACCOUNT is keyed by its FOUNDING key (`account_id` = the first member
//! key). it collects many MEMBER KEYS of different schemes (an ed25519 seed
//! key, a WebAuthn passkey, a native P-256 key -- see [`scheme`]), shares one
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
//!
//! state model mirrors capability's host-lent staging seam: `execute`
//! STAGES into a `pending` overlay (committed state untouched); `query` reads
//! pending-over-committed (read-your-writes) via the `merged_*` helpers;
//! `commit_block` folds pending into committed state AND rebuilds the derived
//! `node_index` + `member_index`; `abort_block` drops pending; `root()`
//! reflects COMMITTED `accounts` only (both indexes are derived, so excluded).
//!
//! `BindNode` is additionally member-gated when constructed with a valset id:
//! the submitting node must be a current validator OR resident (queried live
//! via [`Ctx::query`]). the member-key and unbind ops carry NO valset gate
//! beyond "external origin": their authority is the member signature.
//!
//! state-sync: a joiner rebuilds this module from a peer via
//! [`Identity::snapshot`]/[`Identity::install`]. the snapshot is the exact
//! preimage of `root()`, so `install` needs no trust in the serving peer -- it
//! recomputes the root of whatever bytes arrived and adopts them only on a
//! match, rebuilding both indexes from the adopted map and clearing `pending`.
//! an account with an EMPTY node set is a legal encoding (an unbind can leave a
//! record with no nodes but surviving members/name/nonce); an account with an
//! EMPTY member set is NOT (every live account keeps at least one key).
//!
//! MIGRATION NOTE: this is the v2 account format. the wire shape of the ops and
//! the canonical state encoding both changed from the v1 single-user-key
//! registry, so it is a coordinated consensus change -- a mixed old/new network
//! would fork. dev networks re-establish binds automatically (auto-bind runs on
//! every desktop connect); a versioned rollout uses the node upgrade path.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the pluggable member-key verifier: "(kind, pubkey, proof) -> valid?" for
// every scheme an account can collect. flattened at the crate root so the wire
// types (`KeyKind`, `MemberProof`) and the account logic share one vocabulary.
mod scheme;
pub use scheme::{
    KeyKind, MemberProof, verify_authority, webauthn_challenge, webauthn_rp_id_hash,
};

use std::collections::{BTreeMap, BTreeSet};

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

/// per-member metadata; the public key is the map key, so it is not repeated.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MemberMeta {
    kind: KeyKind,
    label: Option<String>,
    /// `SHA-256(rp_id)` a WebAuthn member's later assertions must carry in
    /// authenticatorData -- pins the passkey to the RP it enrolled under.
    /// `Some` iff `kind == WebauthnP256`, enforced by the canonical codec.
    rp_id_hash: Option<[u8; 32]>,
    added_at: u64,
}

/// one stored account: display name, shared replay nonce, the member-key set,
/// the bound node set, and the last-write block timestamp. `account_id` is the
/// map key, so it is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountRecord {
    display_name: Option<String>,
    nonce: u64,
    member_keys: BTreeMap<Vec<u8>, MemberMeta>,
    nodes: BTreeSet<Vec<u8>>,
    updated_at: u64,
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
    /// committed registry -- what `root()` commits to.
    accounts: BTreeMap<Vec<u8>, AccountRecord>,
    /// committed, DERIVED index: node key -> owning account id. rebuilt from
    /// `accounts` at every `commit_block`; excluded from `root()`.
    node_index: BTreeMap<Vec<u8>, Vec<u8>>,
    /// committed, DERIVED index: member public key -> owning account id.
    /// rebuilt alongside `node_index`; excluded from `root()`.
    member_index: BTreeMap<Vec<u8>, Vec<u8>>,
    /// this block's staged per-account upserts (`Some`) / clears (`None`).
    /// read ahead of `accounts` (read-your-writes), merged in on `commit_block`.
    pending: BTreeMap<Vec<u8>, Option<AccountRecord>>,
}

impl Identity {
    pub fn new(id: impl Into<ModuleId>, valset_id: Option<ModuleId>, chain_id: String) -> Self {
        Self {
            id: id.into(),
            valset_id,
            chain_id,
            accounts: BTreeMap::new(),
            node_index: BTreeMap::new(),
            member_index: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
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

    /// the CURRENT validator set UNION resident set, both queried live from the
    /// valset module's staged-over-committed projection -- a bind is admitted
    /// for either standing.
    async fn members(&self, ctx: &dyn Ctx, valset_id: &str) -> Result<BTreeSet<Vec<u8>>, Error> {
        let validators = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Validators))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Validators(v) => v,
            other => {
                return Err(Error::Module(format!(
                    "valset answered a Validators query with {other:?}"
                )));
            }
        };
        let residents = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Residents))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Residents(o) => o,
            other => {
                return Err(Error::Module(format!(
                    "valset answered a Residents query with {other:?}"
                )));
            }
        };
        Ok(validators.into_iter().chain(residents).collect())
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
        let meta = record.member_keys.get(&auth.key).ok_or_else(|| {
            Error::Module("authorizer is not a member of this account".into())
        })?;
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
            return Err(Error::Module("authorizer certificate does not verify".into()));
        }
        Ok(())
    }

    // ---- merged (pending-over-committed) view -------------------------------

    /// committed accounts with this block's staged changes applied.
    fn merged_accounts(&self) -> BTreeMap<Vec<u8>, AccountRecord> {
        let mut merged = self.accounts.clone();
        for (key, change) in &self.pending {
            match change {
                Some(record) => {
                    merged.insert(key.clone(), record.clone());
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        merged
    }

    /// read one account through the staged overlay (read-your-writes).
    fn merged_record(&self, account_id: &[u8]) -> Option<AccountRecord> {
        match self.pending.get(account_id) {
            Some(change) => change.clone(),
            None => self.accounts.get(account_id).cloned(),
        }
    }

    /// node -> account and member -> account indexes derived from the merged
    /// view, so a staged bind/unbind/add/remove is visible before commit.
    fn merged_node_index(&self) -> BTreeMap<Vec<u8>, Vec<u8>> {
        Self::node_index_of(&self.merged_accounts())
    }
    fn merged_member_index(&self) -> BTreeMap<Vec<u8>, Vec<u8>> {
        Self::member_index_of(&self.merged_accounts())
    }

    fn account_view(account_id: &[u8], record: &AccountRecord) -> AccountView {
        AccountView {
            account_id: account_id.to_vec(),
            display_name: record.display_name.clone(),
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
            nodes: record.nodes.iter().cloned().collect(),
            updated_at: record.updated_at,
        }
    }

    // ---- canonical state bytes (root() preimage / snapshot) -----------------

    /// canonical bytes of `accounts`: `u64-le` account count, then per sorted
    /// account -- `len+account_id`, name (flag `u8` + `len+name` if set),
    /// `u64-le` nonce, `u64-le` member count then per sorted member
    /// (`len+pubkey`, kind tag `u8`, label flag + `len+label` if set,
    /// rp-hash flag + 32 bytes if set, `u64-le added_at`), `u64-le` node count
    /// then per sorted node `len+node`, and `u64-le updated_at`. both indexes
    /// are derived and excluded.
    fn encode_state(accounts: &BTreeMap<Vec<u8>, AccountRecord>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(accounts.len() as u64).to_le_bytes());
        for (account_id, record) in accounts {
            push_bytes(&mut out, account_id);
            push_opt_str(&mut out, record.display_name.as_deref());
            out.extend_from_slice(&record.nonce.to_le_bytes());

            out.extend_from_slice(&(record.member_keys.len() as u64).to_le_bytes());
            for (pubkey, meta) in &record.member_keys {
                push_bytes(&mut out, pubkey);
                out.push(meta.kind.tag());
                push_opt_str(&mut out, meta.label.as_deref());
                match &meta.rp_id_hash {
                    Some(hash) => {
                        out.push(1u8);
                        out.extend_from_slice(hash);
                    }
                    None => out.push(0u8),
                }
                out.extend_from_slice(&meta.added_at.to_le_bytes());
            }

            out.extend_from_slice(&(record.nodes.len() as u64).to_le_bytes());
            for node in &record.nodes {
                push_bytes(&mut out, node);
            }
            out.extend_from_slice(&record.updated_at.to_le_bytes());
        }
        out
    }

    /// the state-based commitment: `ZERO` when empty, else sha256 over exactly
    /// the bytes `encode_state` emits.
    fn root_of(accounts: &BTreeMap<Vec<u8>, AccountRecord>) -> StateRoot {
        if accounts.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::encode_state(accounts)).into())
    }

    fn node_index_of(accounts: &BTreeMap<Vec<u8>, AccountRecord>) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let mut index = BTreeMap::new();
        for (account_id, record) in accounts {
            for node in &record.nodes {
                index.insert(node.clone(), account_id.clone());
            }
        }
        index
    }

    fn member_index_of(accounts: &BTreeMap<Vec<u8>, AccountRecord>) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let mut index = BTreeMap::new();
        for (account_id, record) in accounts {
            for member in record.member_keys.keys() {
                index.insert(member.clone(), account_id.clone());
            }
        }
        index
    }

    // ---- state-sync (snapshot / install) -----------------------------------

    /// canonical bytes of the COMMITTED registry -- exactly what `root()`
    /// hashes. pending is deliberately excluded.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.accounts)
    }

    /// replace committed state with a decoded snapshot iff its recomputed root
    /// equals `expected`. decode/verify land in a temporary map, so on any
    /// `Err` committed state and both indexes are byte-identical to before.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let decoded = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&decoded);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.node_index = Self::node_index_of(&decoded);
        self.member_index = Self::member_index_of(&decoded);
        self.accounts = decoded;
        self.pending.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves them):
    /// every count/length is checked against the remaining buffer before any
    /// allocation; truncation and trailing bytes both reject; account ids,
    /// member keys (within an account) and node keys (within an account) must
    /// each arrive strictly increasing; the kind tag must be known; the rp-hash
    /// flag must be present exactly for WebAuthn members; every account carries
    /// at least one member; and no node or member key may appear in two
    /// accounts (states the execute path can never commit).
    fn decode_snapshot(bytes: &[u8]) -> Result<BTreeMap<Vec<u8>, AccountRecord>, Error> {
        let mut cur = Cursor::new(bytes);
        let count = cur.u64()?;
        // per-account minimum: id-len(8) + name flag(1) + nonce(8) + member
        // count(8) + node count(8) + updated_at(8) = 41 bytes.
        const MIN_ACCOUNT_BYTES: u64 = 41;
        cur.bound(count, MIN_ACCOUNT_BYTES, "account")?;

        let mut accounts = BTreeMap::new();
        let mut prev_account: Option<Vec<u8>> = None;
        for _ in 0..count {
            let account_id = cur.bytes()?.to_vec();
            if prev_account.as_deref().is_some_and(|p| p >= account_id.as_slice()) {
                return Err(Error::Module(
                    "snapshot account ids must be strictly increasing".into(),
                ));
            }
            prev_account = Some(account_id.clone());

            let display_name = cur.opt_str(MAX_NAME_LEN, "account name")?;
            let nonce = cur.u64()?;

            let member_keys = Self::decode_members(&mut cur)?;
            let nodes = Self::decode_nodes(&mut cur)?;
            let updated_at = cur.u64()?;

            accounts.insert(
                account_id,
                AccountRecord { display_name, nonce, member_keys, nodes, updated_at },
            );
        }
        cur.finish()?;

        // no node and no member key may be claimed by two accounts: the execute
        // path enforces single-ownership, so a snapshot claiming otherwise is a
        // forgery.
        let mut seen_nodes = BTreeSet::new();
        let mut seen_members = BTreeSet::new();
        for record in accounts.values() {
            for node in &record.nodes {
                if !seen_nodes.insert(node.clone()) {
                    return Err(Error::Module("node bound twice in snapshot".into()));
                }
            }
            for member in record.member_keys.keys() {
                if !seen_members.insert(member.clone()) {
                    return Err(Error::Module("member key claimed twice in snapshot".into()));
                }
            }
        }

        Ok(accounts)
    }

    fn decode_members(cur: &mut Cursor) -> Result<BTreeMap<Vec<u8>, MemberMeta>, Error> {
        let count = cur.u64()?;
        // per-member minimum: pubkey-len(8) + kind(1) + label flag(1) + rp
        // flag(1) + added_at(8) = 19 bytes.
        const MIN_MEMBER_BYTES: u64 = 19;
        cur.bound(count, MIN_MEMBER_BYTES, "member")?;
        if count == 0 {
            return Err(Error::Module(
                "snapshot account has no member keys (every live account keeps one)".into(),
            ));
        }
        let mut members = BTreeMap::new();
        let mut prev: Option<Vec<u8>> = None;
        for _ in 0..count {
            let pubkey = cur.bytes()?.to_vec();
            if prev.as_deref().is_some_and(|p| p >= pubkey.as_slice()) {
                return Err(Error::Module(
                    "snapshot member keys must be strictly increasing within an account".into(),
                ));
            }
            prev = Some(pubkey.clone());

            let kind = KeyKind::from_tag(cur.byte()?)
                .ok_or_else(|| Error::Module("snapshot member has an unknown key kind".into()))?;
            let label = cur.opt_str(MAX_LABEL_LEN, "member label")?;
            let rp_id_hash = match cur.byte()? {
                0 => None,
                1 => Some(cur.array::<32>()?),
                other => {
                    return Err(Error::Module(format!(
                        "snapshot rp-hash flag must be 0 or 1, got {other}"
                    )));
                }
            };
            // the rp-hash pin is present exactly for WebAuthn members.
            if rp_id_hash.is_some() != kind.expects_rp_id_hash() {
                return Err(Error::Module(
                    "snapshot rp-hash presence does not match the member kind".into(),
                ));
            }
            let added_at = cur.u64()?;
            members.insert(pubkey, MemberMeta { kind, label, rp_id_hash, added_at });
        }
        Ok(members)
    }

    fn decode_nodes(cur: &mut Cursor) -> Result<BTreeSet<Vec<u8>>, Error> {
        let count = cur.u64()?;
        cur.bound(count, 8, "node")?;
        let mut nodes = BTreeSet::new();
        let mut prev: Option<Vec<u8>> = None;
        for _ in 0..count {
            let node = cur.bytes()?.to_vec();
            if prev.as_deref().is_some_and(|p| p >= node.as_slice()) {
                return Err(Error::Module(
                    "snapshot node keys must be strictly increasing within an account".into(),
                ));
            }
            prev = Some(node.clone());
            nodes.insert(node);
        }
        Ok(nodes)
    }
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// encode an optional string as a `0` flag, or `1` + length-prefixed bytes.
fn push_opt_str(out: &mut Vec<u8>, s: Option<&str>) {
    match s {
        Some(s) => {
            out.push(1u8);
            push_bytes(out, s.as_bytes());
        }
        None => out.push(0u8),
    }
}

/// a forward-only reader over untrusted snapshot bytes: every accessor checks
/// the remaining buffer before it reads, so a forged length can never
/// over-read or drive an allocation the bytes could not back.
struct Cursor<'a> {
    buf: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    fn u64(&mut self) -> Result<u64, Error> {
        let Some((head, rest)) = self.buf.split_first_chunk::<8>() else {
            return Err(Error::Module("snapshot truncated".into()));
        };
        self.buf = rest;
        Ok(u64::from_le_bytes(*head))
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let Some((&b, rest)) = self.buf.split_first() else {
            return Err(Error::Module("snapshot truncated".into()));
        };
        self.buf = rest;
        Ok(b)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let Some((head, rest)) = self.buf.split_first_chunk::<N>() else {
            return Err(Error::Module("snapshot truncated".into()));
        };
        self.buf = rest;
        Ok(*head)
    }

    /// a `u64`-length-prefixed byte slice, length-checked before the split.
    fn bytes(&mut self) -> Result<&'a [u8], Error> {
        let len = self.u64()?;
        if len > self.buf.len() as u64 {
            return Err(Error::Module(format!(
                "snapshot length {len} exceeds the {} remaining bytes",
                self.buf.len()
            )));
        }
        let (head, rest) = self.buf.split_at(len as usize);
        self.buf = rest;
        Ok(head)
    }

    /// an optional utf-8 string: flag byte `0` (absent) or `1` + a non-empty,
    /// `max`-bounded, valid-utf8 length-prefixed slice.
    fn opt_str(&mut self, max: usize, what: &str) -> Result<Option<String>, Error> {
        match self.byte()? {
            0 => Ok(None),
            1 => {
                let raw = self.bytes()?;
                if raw.is_empty() {
                    return Err(Error::Module(format!("snapshot {what} flag set but empty")));
                }
                if raw.len() > max {
                    return Err(Error::Module(format!(
                        "snapshot {what} exceeds the {max}-byte limit"
                    )));
                }
                let s = std::str::from_utf8(raw)
                    .map_err(|e| Error::Module(format!("snapshot {what} is not utf-8: {e}")))?;
                Ok(Some(s.to_string()))
            }
            other => Err(Error::Module(format!(
                "snapshot {what} flag must be 0 or 1, got {other}"
            ))),
        }
    }

    /// reject a forged count that the remaining bytes could not possibly hold,
    /// before looping -- `count * min_each` must fit.
    fn bound(&self, count: u64, min_each: u64, what: &str) -> Result<(), Error> {
        if count > (self.buf.len() as u64) / min_each {
            return Err(Error::Module(format!(
                "snapshot {what} count {count} exceeds the {} remaining bytes",
                self.buf.len()
            )));
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), Error> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(Error::Module(format!(
                "snapshot carries {} trailing bytes",
                self.buf.len()
            )))
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Identity {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.accounts)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            IdentityMsg::BindNode { authorizer } => self.bind_node(ctx, authorizer).await,
            IdentityMsg::UnbindNode { node_key, authorizer } => {
                self.unbind_node(ctx, node_key, authorizer)
            }
            IdentityMsg::AddMemberKey {
                new_key,
                new_kind,
                new_label,
                possession,
                authorizer,
            } => self.add_member_key(ctx, new_key, new_kind, new_label, possession, authorizer),
            IdentityMsg::RemoveMemberKey { target_key, authorizer } => {
                self.remove_member_key(ctx, target_key, authorizer)
            }
            IdentityMsg::SetAccountName { display_name } => self.set_account_name(ctx, display_name),
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            IdentityQuery::All { from, limit } => {
                let merged = self.merged_accounts();
                let limit = limit.min(MAX_QUERY_LIMIT) as usize;
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let accounts = merged
                    .iter()
                    .skip(from)
                    .take(limit)
                    .map(|(id, record)| Self::account_view(id, record))
                    .collect();
                Ok(encode_reply(&IdentityReply::Accounts(accounts)))
            }
            IdentityQuery::Get { account_id } => Ok(encode_reply(&IdentityReply::Account(
                self.merged_record(&account_id)
                    .map(|record| Self::account_view(&account_id, &record)),
            ))),
            IdentityQuery::OfNode { node_key } => {
                let account = self.merged_node_index().get(&node_key).cloned().and_then(|id| {
                    self.merged_record(&id).map(|record| Self::account_view(&id, &record))
                });
                Ok(encode_reply(&IdentityReply::Account(account)))
            }
            IdentityQuery::OfMember { member_key } => {
                let account = self.merged_member_index().get(&member_key).cloned().and_then(|id| {
                    self.merged_record(&id).map(|record| Self::account_view(&id, &record))
                });
                Ok(encode_reply(&IdentityReply::Account(account)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (account_id, change) in std::mem::take(&mut self.pending) {
            // drop every index entry currently pointing at this account; the
            // new sets (if any) are reinserted below.
            self.node_index.retain(|_, owner| owner != &account_id);
            self.member_index.retain(|_, owner| owner != &account_id);
            match change {
                Some(record) => {
                    for node in &record.nodes {
                        self.node_index.insert(node.clone(), account_id.clone());
                    }
                    for member in record.member_keys.keys() {
                        self.member_index.insert(member.clone(), account_id.clone());
                    }
                    self.accounts.insert(account_id, record);
                }
                None => {
                    self.accounts.remove(&account_id);
                }
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
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
            let members = self.members(&*ctx, &valset_id).await?;
            if !members.contains(&origin) {
                return Err(Error::Module(
                    "bind origin is not a network member or resident".into(),
                ));
            }
        }

        // which account does this authorizer speak for -- an existing
        // membership, or a brand-new account it founds?
        let (account_id, mut record) = match self.merged_member_index().get(&authorizer.key).cloned()
        {
            Some(account_id) => {
                let record = self
                    .merged_record(&account_id)
                    .expect("member_index only ever points at an existing record");
                (account_id, record)
            }
            None => {
                if !authorizer.kind.pubkey_wellformed(&authorizer.key) {
                    return Err(Error::Module("founding key is malformed for its kind".into()));
                }
                if self.merged_record(&authorizer.key).is_some() {
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
                    nonce: 0,
                    member_keys,
                    nodes: BTreeSet::new(),
                    updated_at: 0,
                };
                (authorizer.key.clone(), record)
            }
        };

        // idempotent re-bind: node already bound to THIS account -> no-op, no
        // nonce bump. the proof is deliberately left unverified here (no state
        // change, and origin is already consensus-authenticated).
        if let Some(bound_to) = self.merged_node_index().get(&origin) {
            if *bound_to == account_id {
                return Ok(());
            }
            return Err(Error::Module(
                "node is already bound to another account; unbind first".into(),
            ));
        }

        // verify the member's consent at the account's CURRENT nonce.
        let preimage = bind_preimage(&self.chain_id, &origin, record.nonce);
        Self::authorize(&record, IDENTITY_BIND_NS, &preimage, &authorizer)?;

        record.nodes.insert(origin);
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.pending.insert(account_id, Some(record));
        Ok(())
    }

    /// evict `node_key`, authorized by any member of its account.
    fn unbind_node(
        &mut self,
        ctx: &mut dyn Ctx,
        node_key: Vec<u8>,
        authorizer: MemberAuth,
    ) -> Result<(), Error> {
        Self::origin_key(ctx)?;

        let account_id = self
            .merged_node_index()
            .get(&node_key)
            .cloned()
            .ok_or_else(|| Error::Module("node is not bound".into()))?;
        let mut record = self
            .merged_record(&account_id)
            .expect("node_index only ever points at an existing record");

        let preimage = unbind_preimage(&self.chain_id, &node_key, record.nonce);
        Self::authorize(&record, IDENTITY_UNBIND_NS, &preimage, &authorizer)?;

        // the record persists even with an empty node set: members + name +
        // nonce survive so a re-bind can still resolve them.
        record.nodes.remove(&node_key);
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.pending.insert(account_id, Some(record));
        Ok(())
    }

    /// admit `new_key` to the authorizer's account: an existing member consents
    /// and the new key proves possession, both over the same add-preimage.
    fn add_member_key(
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
            .merged_member_index()
            .get(&authorizer.key)
            .cloned()
            .ok_or_else(|| Error::Module("authorizer belongs to no account".into()))?;
        let mut record = self
            .merged_record(&account_id)
            .expect("member_index only ever points at an existing record");

        if !new_kind.pubkey_wellformed(&new_key) {
            return Err(Error::Module("new member key is malformed for its kind".into()));
        }
        if record.member_keys.contains_key(&new_key) {
            return Err(Error::Module("key is already a member of this account".into()));
        }
        if self.merged_member_index().contains_key(&new_key) {
            return Err(Error::Module("key already belongs to another account".into()));
        }
        let label = clean_label(new_label)?;

        let preimage =
            add_member_preimage(&self.chain_id, &account_id, &new_key, new_kind, record.nonce);
        // existing member consents ...
        Self::authorize(&record, IDENTITY_ADD_MEMBER_NS, &preimage, &authorizer)?;
        // ... and the new key proves it holds itself (no rp pin yet -- the
        // proof establishes it).
        if !verify_authority(new_kind, &new_key, None, IDENTITY_ADD_MEMBER_NS, &preimage, &possession)
        {
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
            new_key,
            MemberMeta { kind: new_kind, label, rp_id_hash, added_at: ctx.env().consensus_time },
        );
        record.nonce += 1;
        record.updated_at = ctx.env().consensus_time;
        self.pending.insert(account_id, Some(record));
        Ok(())
    }

    /// drop `target_key` from the authorizer's account. any member may drop any
    /// member (including itself), except the last remaining one.
    fn remove_member_key(
        &mut self,
        ctx: &mut dyn Ctx,
        target_key: Vec<u8>,
        authorizer: MemberAuth,
    ) -> Result<(), Error> {
        Self::origin_key(ctx)?;

        let account_id = self
            .merged_member_index()
            .get(&authorizer.key)
            .cloned()
            .ok_or_else(|| Error::Module("authorizer belongs to no account".into()))?;
        let mut record = self
            .merged_record(&account_id)
            .expect("member_index only ever points at an existing record");

        if !record.member_keys.contains_key(&target_key) {
            return Err(Error::Module("target key is not a member of this account".into()));
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
        self.pending.insert(account_id, Some(record));
        Ok(())
    }

    /// set the display name of the account the origin node is bound to.
    fn set_account_name(&mut self, ctx: &mut dyn Ctx, display_name: String) -> Result<(), Error> {
        let origin = Self::origin_key(ctx)?;
        let account_id = self
            .merged_node_index()
            .get(&origin)
            .cloned()
            .ok_or_else(|| Error::Module("origin node is not bound to an account".into()))?;
        let mut record = self
            .merged_record(&account_id)
            .expect("node_index only ever points at an existing record");

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
        self.pending.insert(account_id, Some(record));
        Ok(())
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
