//! deterministic user->nodes binding registry.
//!
//! a USER (an ed25519 keypair held by the person, app-side) owns many NODES
//! (each a mesh/valset identity). [`IdentityMsg::BindNode`] binds the
//! SUBMITTING NODE (the verified origin) to a user, consented to by a
//! USER-KEY SIGNATURE over a chain-and-nonce-scoped preimage --
//! [`bind_preimage`]/[`unbind_preimage`] -- so a certificate can never replay
//! across networks or after an unbind bumps the nonce.
//!
//! state model mirrors profiles/capability's host-lent staging seam:
//! `execute` STAGES into a `pending` overlay (committed state untouched);
//! `query` reads pending-over-committed (read-your-writes) via the
//! `merged_*` helpers; `commit_block` folds pending into committed state AND
//! rebuilds the derived `node_index`; `abort_block` drops pending; `root()`
//! reflects COMMITTED `users` only (the index is derived, so it is excluded).
//!
//! `BindNode` is additionally member-gated when constructed with a valset id:
//! the submitting node must be a current validator OR observer (queried live
//! via [`Ctx::query`]) -- without a valset (the single-node daemon has none)
//! any external key may bind. `UnbindNode` carries NO member gate and NO
//! origin restriction beyond "external": it is the recovery path a surviving
//! device uses to evict a lost one, authorized purely by the user signature.
//!
//! state-sync: a joiner rebuilds this module from a peer via
//! [`Identity::snapshot`]/[`Identity::install`] (wired through
//! [`Module::state_sync_handle`]). the snapshot is the exact preimage of
//! `root()`, so `install` needs no trust in the serving peer -- it recomputes
//! the root of whatever bytes arrived and adopts them only on a match,
//! rebuilding `node_index` from the adopted map and clearing `pending`. a
//! user with an EMPTY node set is a legal encoding (unlike capability, where
//! an empty tag set means absent): an unbind can leave a record with no
//! nodes but a surviving name/nonce.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{
    Verifier as _,
    ed25519::{PublicKey, Signature},
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

/// one stored user: display name, replay-guard nonce, bound node set, and the
/// last-write block timestamp. the user key is the map key, so it is not
/// repeated here.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UserRecord {
    display_name: Option<String>,
    nonce: u64,
    nodes: BTreeSet<Vec<u8>>,
    updated_at: u64,
}

pub struct Identity {
    id: ModuleId,
    /// the valset module consulted to gate `BindNode` to current members
    /// (validators UNION observers); `None` runs ungated (the single-node
    /// daemon carries no valset).
    valset_id: Option<ModuleId>,
    /// this network's chain id -- folded into every signed preimage so a
    /// certificate minted for one network can never bind/unbind on another.
    chain_id: String,
    /// committed registry -- what `root()` commits to.
    users: BTreeMap<Vec<u8>, UserRecord>,
    /// committed, DERIVED index: node key -> owning user key. rebuilt from
    /// `users` at every `commit_block`; excluded from `root()`.
    node_index: BTreeMap<Vec<u8>, Vec<u8>>,
    /// this block's staged per-user upserts (`Some`) / clears (`None`). read
    /// ahead of `users` (read-your-writes), merged into committed state only
    /// on `commit_block`.
    pending: BTreeMap<Vec<u8>, Option<UserRecord>>,
}

impl Identity {
    pub fn new(id: impl Into<ModuleId>, valset_id: Option<ModuleId>, chain_id: String) -> Self {
        Self {
            id: id.into(),
            valset_id,
            chain_id,
            users: BTreeMap::new(),
            node_index: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// the AUTHENTICATED submitter key -- exactly profiles' gate: a non-empty
    /// external origin, or a deterministic rejection.
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

    /// the CURRENT validator set UNION observer set, both queried live from
    /// the valset module's staged-over-committed projection -- a bind is
    /// admitted for either standing.
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
        let observers = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Observers))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Observers(o) => o,
            other => {
                return Err(Error::Module(format!(
                    "valset answered an Observers query with {other:?}"
                )));
            }
        };
        Ok(validators.into_iter().chain(observers).collect())
    }

    // ---- merged (pending-over-committed) view -------------------------------

    /// committed users with this block's staged changes applied -- the read
    /// projection for `All` queries (a `None` overlay entry removes the key).
    fn merged_users(&self) -> BTreeMap<Vec<u8>, UserRecord> {
        let mut merged = self.users.clone();
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

    /// read one user through the staged overlay (read-your-writes): a
    /// pending `None` reads as cleared even if a committed record exists.
    fn merged_record(&self, user_key: &[u8]) -> Option<UserRecord> {
        match self.pending.get(user_key) {
            Some(change) => change.clone(),
            None => self.users.get(user_key).cloned(),
        }
    }

    /// the node -> user index derived from the merged view, so a staged bind
    /// or unbind is visible to `UserOf`/the takeover guard before commit.
    fn merged_index(&self) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let mut index = BTreeMap::new();
        for (user_key, record) in self.merged_users() {
            for node in &record.nodes {
                index.insert(node.clone(), user_key.clone());
            }
        }
        index
    }

    fn user_view(key: &[u8], record: &UserRecord) -> UserView {
        UserView {
            user_key: key.to_vec(),
            display_name: record.display_name.clone(),
            nonce: record.nonce,
            nodes: record.nodes.iter().cloned().collect(),
            updated_at: record.updated_at,
        }
    }

    // ---- canonical state bytes (root() preimage; Task 3 ships them as a
    // snapshot) -----------------------------------------------------------

    /// canonical bytes of `users`: `u64-le` user count, then per sorted user
    /// `len+user_key`, a name-present flag (`u8` + `len+name` if set),
    /// `u64-le` nonce, `u64-le` node count then per sorted node `len+node`,
    /// and `u64-le updated_at`. the index is derived and excluded.
    fn encode_state(users: &BTreeMap<Vec<u8>, UserRecord>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(users.len() as u64).to_le_bytes());
        for (user_key, record) in users {
            push_bytes(&mut out, user_key);
            match &record.display_name {
                Some(name) => {
                    out.push(1u8);
                    push_bytes(&mut out, name.as_bytes());
                }
                None => out.push(0u8),
            }
            out.extend_from_slice(&record.nonce.to_le_bytes());
            out.extend_from_slice(&(record.nodes.len() as u64).to_le_bytes());
            for node in &record.nodes {
                push_bytes(&mut out, node);
            }
            out.extend_from_slice(&record.updated_at.to_le_bytes());
        }
        out
    }

    /// the state-based commitment for `users`: `ZERO` when empty (matching
    /// capability's convention), else sha256 over exactly the bytes
    /// `encode_state` emits.
    fn root_of(users: &BTreeMap<Vec<u8>, UserRecord>) -> StateRoot {
        if users.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::encode_state(users)).into())
    }

    /// the node -> user index derived from `users` -- rebuilt wholesale after
    /// `install` adopts a new registry (mirrors `commit_block`'s per-key
    /// derivation, but over the entire adopted map at once).
    fn index_of(users: &BTreeMap<Vec<u8>, UserRecord>) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let mut index = BTreeMap::new();
        for (user_key, record) in users {
            for node in &record.nodes {
                index.insert(node.clone(), user_key.clone());
            }
        }
        index
    }

    // ---- state-sync (snapshot / install) -----------------------------------
    // a joiner rebuilds this module from a peer via `snapshot`/`install`. the
    // snapshot is the exact preimage of `root()`, so the joiner needs no
    // trust in the serving peer: `install` recomputes the root of whatever
    // bytes arrived and refuses to adopt them unless it matches the expected
    // root consensus already agreed on.

    /// canonical bytes of the COMMITTED registry -- exactly what `root()`
    /// hashes. pending is deliberately excluded: a snapshot ships what
    /// consensus committed to, not this block's in-flight stage.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.users)
    }

    /// replace committed state with a decoded snapshot, iff the decoded
    /// registry's recomputed root equals `expected`. decode and verification
    /// land in a temporary map: `self` is mutated only after both pass, so on
    /// any `Err` committed state, the derived index, and pending are
    /// byte-identical to before the call. success rebuilds `node_index` from
    /// the adopted map and clears `pending` -- staged changes belong to the
    /// state being replaced, not the state being adopted. this is not an
    /// execute path: no `Ctx` is consulted, so a member-gated module installs
    /// exactly like an ungated one.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let decoded = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&decoded);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.node_index = Self::index_of(&decoded);
        self.users = decoded;
        self.pending.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves
    /// them). every count and length is checked against the remaining buffer
    /// BEFORE any allocation, truncation and trailing bytes both reject, user
    /// keys and (within a user) node keys must arrive strictly increasing,
    /// the name-present flag must be exactly 0 or 1 with a non-empty name
    /// when set, and a node claimed by two different users rejects (a state
    /// the execute path can never commit). unlike capability, a user with an
    /// EMPTY node set is a legal encoding -- an unbind can leave a record with
    /// no nodes but a surviving name/nonce, so node count 0 must decode, not
    /// reject.
    fn decode_snapshot(bytes: &[u8]) -> Result<BTreeMap<Vec<u8>, UserRecord>, Error> {
        fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
            let Some((head, rest)) = (*buf).split_first_chunk::<8>() else {
                return Err(Error::Module("snapshot truncated".into()));
            };
            *buf = rest;
            Ok(u64::from_le_bytes(*head))
        }

        let mut buf = bytes;
        let count = take_u64(&mut buf)?;
        // each user entry costs at least its 8-byte key-length prefix, a
        // 1-byte name flag, an 8-byte nonce, an 8-byte node count, and an
        // 8-byte updated_at -- 33 bytes minimum -- so a forged count the
        // remaining bytes could not possibly hold is rejected up front.
        const MIN_USER_BYTES: u64 = 33;
        if count > (buf.len() as u64) / MIN_USER_BYTES {
            return Err(Error::Module(format!(
                "snapshot user count {count} exceeds the {} remaining bytes",
                buf.len()
            )));
        }
        let mut users = BTreeMap::new();
        let mut prev_user: Option<&[u8]> = None;
        for _ in 0..count {
            let key_len = take_u64(&mut buf)?;
            if key_len > buf.len() as u64 {
                return Err(Error::Module(format!(
                    "snapshot user key length {key_len} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let (user_key, rest) = buf.split_at(key_len as usize);
            buf = rest;
            if prev_user.is_some_and(|p| p >= user_key) {
                return Err(Error::Module(
                    "snapshot user keys must be strictly increasing".into(),
                ));
            }
            prev_user = Some(user_key);

            let Some((&name_flag, rest)) = buf.split_first() else {
                return Err(Error::Module("snapshot truncated".into()));
            };
            buf = rest;
            let display_name = match name_flag {
                0 => None,
                1 => {
                    let name_len = take_u64(&mut buf)?;
                    if name_len > buf.len() as u64 {
                        return Err(Error::Module(format!(
                            "snapshot name length {name_len} exceeds the {} remaining bytes",
                            buf.len()
                        )));
                    }
                    let (name_bytes, rest) = buf.split_at(name_len as usize);
                    buf = rest;
                    if name_bytes.is_empty() {
                        return Err(Error::Module(
                            "snapshot name flag set but name is empty".into(),
                        ));
                    }
                    if name_bytes.len() > MAX_NAME_LEN {
                        return Err(Error::Module(format!(
                            "snapshot name exceeds the {MAX_NAME_LEN}-byte limit"
                        )));
                    }
                    let name = std::str::from_utf8(name_bytes).map_err(|e| {
                        Error::Module(format!("snapshot name is not utf-8: {e}"))
                    })?;
                    Some(name.to_string())
                }
                other => {
                    return Err(Error::Module(format!(
                        "snapshot name flag must be 0 or 1, got {other}"
                    )));
                }
            };

            let nonce = take_u64(&mut buf)?;

            let node_count = take_u64(&mut buf)?;
            // each node costs at least its 8-byte length prefix.
            if node_count > (buf.len() as u64) / 8 {
                return Err(Error::Module(format!(
                    "snapshot node count {node_count} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut nodes = BTreeSet::new();
            let mut prev_node: Option<&[u8]> = None;
            for _ in 0..node_count {
                let node_len = take_u64(&mut buf)?;
                if node_len > buf.len() as u64 {
                    return Err(Error::Module(format!(
                        "snapshot node length {node_len} exceeds the {} remaining bytes",
                        buf.len()
                    )));
                }
                let (node, rest) = buf.split_at(node_len as usize);
                buf = rest;
                if prev_node.is_some_and(|p| p >= node) {
                    return Err(Error::Module(
                        "snapshot node keys must be strictly increasing within a user".into(),
                    ));
                }
                prev_node = Some(node);
                nodes.insert(node.to_vec());
            }

            let updated_at = take_u64(&mut buf)?;

            users.insert(
                user_key.to_vec(),
                UserRecord { display_name, nonce, nodes, updated_at },
            );
        }
        if !buf.is_empty() {
            return Err(Error::Module(format!("snapshot carries {} trailing bytes", buf.len())));
        }

        // a node bound to two different users is illegal: the execute path
        // (bind's takeover guard) can never commit that state, so a snapshot
        // claiming it must be a byzantine forgery.
        let mut seen_nodes = BTreeSet::new();
        for record in users.values() {
            for node in &record.nodes {
                if !seen_nodes.insert(node.clone()) {
                    return Err(Error::Module("node bound twice in snapshot".into()));
                }
            }
        }

        Ok(users)
    }
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

#[async_trait::async_trait(?Send)]
impl Module for Identity {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED registry only.
    fn root(&self) -> StateRoot {
        Self::root_of(&self.users)
    }

    /// advertise the snapshot lane: [`Identity::snapshot`] is the exact
    /// preimage of `root()`, and [`Identity::install`] verifies before
    /// adopting.
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            IdentityMsg::BindNode { user_key, user_sig } => {
                let origin = Self::origin_key(ctx)?;

                // 1. user_key must decode as a valid ed25519 point.
                let pubkey = PublicKey::decode(user_key.as_slice())
                    .map_err(|_| Error::Module("bind user_key is not a valid ed25519 key".into()))?;

                // 2. member gate: validators UNION observers, only when configured.
                if let Some(valset_id) = self.valset_id.clone() {
                    let members = self.members(&*ctx, &valset_id).await?;
                    if !members.contains(&origin) {
                        return Err(Error::Module(
                            "bind origin is not a network member or observer".into(),
                        ));
                    }
                }

                // 3. resolve the origin node's current binding via the merged view.
                if let Some(bound_to) = self.merged_index().get(&origin) {
                    if *bound_to == user_key {
                        // idempotent re-bind: no-op, nonce NOT bumped.
                        return Ok(());
                    }
                    return Err(Error::Module(
                        "node is already bound to another user; unbind first".into(),
                    ));
                }

                // 4. verify the user's consent certificate against the CURRENT nonce.
                let mut record = self.merged_record(&user_key).unwrap_or_else(|| UserRecord {
                    display_name: None,
                    nonce: 0,
                    nodes: BTreeSet::new(),
                    updated_at: 0,
                });
                let sig = Signature::decode(user_sig.as_slice())
                    .map_err(|_| Error::Module("bind certificate does not verify".into()))?;
                let preimage = bind_preimage(&self.chain_id, &origin, record.nonce);
                if !pubkey.verify(IDENTITY_BIND_NS, &preimage, &sig) {
                    return Err(Error::Module("bind certificate does not verify".into()));
                }

                // 5. stage.
                record.nodes.insert(origin);
                record.nonce += 1;
                record.updated_at = ctx.env().consensus_time;
                self.pending.insert(user_key, Some(record));
                Ok(())
            }
            IdentityMsg::UnbindNode { node_key, user_sig } => {
                // external, non-empty; NO member gate, NO further origin
                // restriction -- a surviving device can evict a lost one.
                Self::origin_key(ctx)?;

                let user_key = self
                    .merged_index()
                    .get(&node_key)
                    .cloned()
                    .ok_or_else(|| Error::Module("node is not bound".into()))?;
                let mut record = self
                    .merged_record(&user_key)
                    .expect("node_index only ever points at an existing record");

                let pubkey = PublicKey::decode(user_key.as_slice())
                    .map_err(|_| Error::Module("unbind certificate does not verify".into()))?;
                let sig = Signature::decode(user_sig.as_slice())
                    .map_err(|_| Error::Module("unbind certificate does not verify".into()))?;
                let preimage = unbind_preimage(&self.chain_id, &node_key, record.nonce);
                if !pubkey.verify(IDENTITY_UNBIND_NS, &preimage, &sig) {
                    return Err(Error::Module("unbind certificate does not verify".into()));
                }

                // the record persists even with an empty node set: name +
                // nonce survive so a re-bind can still resolve them.
                record.nodes.remove(&node_key);
                record.nonce += 1;
                record.updated_at = ctx.env().consensus_time;
                self.pending.insert(user_key, Some(record));
                Ok(())
            }
            IdentityMsg::SetUserName { display_name } => {
                let origin = Self::origin_key(ctx)?;
                let user_key = self
                    .merged_index()
                    .get(&origin)
                    .cloned()
                    .ok_or_else(|| Error::Module("origin node is not bound to a user".into()))?;
                let mut record = self
                    .merged_record(&user_key)
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
                // no user signature is consumed here: the nonce is NOT bumped.
                record.updated_at = ctx.env().consensus_time;
                self.pending.insert(user_key, Some(record));
                Ok(())
            }
        }
    }

    /// read projection -- the merged (pending-over-committed) view.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            IdentityQuery::All { from, limit } => {
                let merged = self.merged_users();
                let limit = limit.min(MAX_QUERY_LIMIT) as usize;
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let users = merged
                    .iter()
                    .skip(from)
                    .take(limit)
                    .map(|(key, record)| Self::user_view(key, record))
                    .collect();
                Ok(encode_reply(&IdentityReply::Users(users)))
            }
            IdentityQuery::Get { user_key } => Ok(encode_reply(&IdentityReply::User(
                self.merged_record(&user_key)
                    .map(|record| Self::user_view(&user_key, &record)),
            ))),
            IdentityQuery::UserOf { node_key } => {
                let user = self.merged_index().get(&node_key).cloned().and_then(|user_key| {
                    self.merged_record(&user_key)
                        .map(|record| Self::user_view(&user_key, &record))
                });
                Ok(encode_reply(&IdentityReply::User(user)))
            }
        }
    }

    /// merge the block's staged upserts/clears into committed state and
    /// rebuild the affected `node_index` entries.
    async fn commit_block(&mut self) -> Result<(), Error> {
        for (user_key, change) in std::mem::take(&mut self.pending) {
            // drop every index entry currently pointing at this user; the
            // new set (if any) is reinserted below.
            self.node_index.retain(|_, owner| owner != &user_key);
            match change {
                Some(record) => {
                    for node in &record.nodes {
                        self.node_index.insert(node.clone(), user_key.clone());
                    }
                    self.users.insert(user_key, record);
                }
                None => {
                    self.users.remove(&user_key);
                }
            }
        }
        Ok(())
    }

    /// discard the block's staged changes -- committed state (and `root()`)
    /// is unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::{Signer as _, ed25519};

    /// a minimal Ctx: origin-configurable, and (optionally) answers BOTH the
    /// valset Validators and Observers queries so the member gate is
    /// testable for either standing.
    struct TestCtx {
        env: sdk::Env,
        members: Option<Vec<Vec<u8>>>,
        observers: Option<Vec<Vec<u8>>>,
    }
    impl TestCtx {
        fn external(key: &[u8]) -> Self {
            Self::with_origin(sdk::Origin::External(key.to_vec()))
        }
        fn with_origin(origin: sdk::Origin) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin,
                    me: "identity".into(),
                },
                members: None,
                observers: None,
            }
        }
        fn gated(key: &[u8], validators: Vec<Vec<u8>>, observers: Vec<Vec<u8>>) -> Self {
            let mut ctx = Self::external(key);
            ctx.members = Some(validators);
            ctx.observers = Some(observers);
            ctx
        }
        fn with_members(key: &[u8], members: Vec<Vec<u8>>) -> Self {
            Self::gated(key, members, Vec::new())
        }
        fn with_observers(key: &[u8], observers: Vec<Vec<u8>>) -> Self {
            Self::gated(key, Vec::new(), observers)
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &sdk::Env {
            &self.env
        }
        fn module_root(&self, _t: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, t: &str, r: &[u8]) -> Result<Vec<u8>, Error> {
            if t != "valset" {
                return Err(Error::QueryUnsupported);
            }
            let q = valset::decode_query(r).map_err(Error::Module)?;
            match (q, &self.members, &self.observers) {
                (valset::ValsetQuery::Validators, Some(m), _) => {
                    Ok(valset::encode_reply(&valset::ValsetReply::Validators(m.clone())))
                }
                (valset::ValsetQuery::Observers, _, Some(o)) => {
                    Ok(valset::encode_reply(&valset::ValsetReply::Observers(o.clone())))
                }
                _ => Err(Error::QueryUnsupported),
            }
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }

    const CHAIN: &str = "test-chain";

    fn user() -> ed25519::PrivateKey {
        ed25519::PrivateKey::from_seed(7)
    }

    fn bind_msg(user: &ed25519::PrivateKey, chain: &str, node: &[u8], nonce: u64) -> Msg {
        let user_key = user.public_key().as_ref().to_vec();
        let sig = user.sign(IDENTITY_BIND_NS, &bind_preimage(chain, node, nonce));
        Msg {
            target: "identity".into(),
            payload: encode_msg(&IdentityMsg::BindNode {
                user_key,
                user_sig: sig.as_ref().to_vec(),
            }),
        }
    }

    fn unbind_msg(user: &ed25519::PrivateKey, chain: &str, node: &[u8], nonce: u64) -> Msg {
        let sig = user.sign(IDENTITY_UNBIND_NS, &unbind_preimage(chain, node, nonce));
        Msg {
            target: "identity".into(),
            payload: encode_msg(&IdentityMsg::UnbindNode {
                node_key: node.to_vec(),
                user_sig: sig.as_ref().to_vec(),
            }),
        }
    }

    fn setname_msg(display_name: &str) -> Msg {
        Msg {
            target: "identity".into(),
            payload: encode_msg(&IdentityMsg::SetUserName {
                display_name: display_name.to_string(),
            }),
        }
    }

    fn user_of(id: &Identity, node: &[u8]) -> Option<UserView> {
        let reply = futures::executor::block_on(
            id.query(&encode_query(&IdentityQuery::UserOf { node_key: node.to_vec() })),
        )
        .unwrap();
        match decode_reply(&reply).unwrap() {
            IdentityReply::User(u) => u,
            other => panic!("expected User reply, got {other:?}"),
        }
    }

    fn get_user(id: &Identity, user_key: &[u8]) -> Option<UserView> {
        let reply = futures::executor::block_on(
            id.query(&encode_query(&IdentityQuery::Get { user_key: user_key.to_vec() })),
        )
        .unwrap();
        match decode_reply(&reply).unwrap() {
            IdentityReply::User(u) => u,
            other => panic!("expected User reply, got {other:?}"),
        }
    }

    fn all_users(id: &Identity, from: u64, limit: u64) -> Vec<UserView> {
        let reply = futures::executor::block_on(
            id.query(&encode_query(&IdentityQuery::All { from, limit })),
        )
        .unwrap();
        match decode_reply(&reply).unwrap() {
            IdentityReply::Users(u) => u,
            other => panic!("expected Users reply, got {other:?}"),
        }
    }

    #[test]
    fn bind_happy_path_binds_and_bumps_nonce() {
        let mut id = Identity::new("identity", Some("valset".into()), CHAIN.into());
        let u = user();
        let node = vec![1u8; 32];
        let mut ctx = TestCtx::with_members(&node, vec![node.clone()]);

        futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, CHAIN, &node, 0))).unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        let view = user_of(&id, &node).expect("bound");
        assert_eq!(view.nonce, 1);
        assert_eq!(view.nodes, vec![node]);
    }

    #[test]
    fn bind_rejects_wrong_signature() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let wrong = ed25519::PrivateKey::from_seed(8);
        let node = vec![2u8; 32];
        let mut ctx = TestCtx::external(&node);

        // signed by a DIFFERENT key than the claimed user_key.
        let sig = wrong.sign(IDENTITY_BIND_NS, &bind_preimage(CHAIN, &node, 0));
        let msg = Msg {
            target: "identity".into(),
            payload: encode_msg(&IdentityMsg::BindNode {
                user_key: u.public_key().as_ref().to_vec(),
                user_sig: sig.as_ref().to_vec(),
            }),
        };
        let err = futures::executor::block_on(id.execute(&mut ctx, &msg)).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "bind certificate does not verify"),
            "got {err:?}"
        );
    }

    #[test]
    fn bind_rejects_wrong_chain() {
        let mut id = Identity::new("identity", None, "chain-a".into());
        let u = user();
        let node = vec![3u8; 32];
        let mut ctx = TestCtx::external(&node);

        // cert signed over a DIFFERENT chain id than the module is configured with.
        let err =
            futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, "chain-b", &node, 0)))
                .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "bind certificate does not verify"),
            "got {err:?}"
        );
    }

    #[test]
    fn bind_rejects_stale_nonce() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node1 = vec![4u8; 32];
        let node2 = vec![5u8; 32];

        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node1), &bind_msg(&u, CHAIN, &node1, 0)),
        )
        .unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();
        // the user's nonce is now 1.

        // a cert correctly signed for node2 but at the STALE nonce (0) rejects.
        let err = futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node2), &bind_msg(&u, CHAIN, &node2, 0)),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "bind certificate does not verify"),
            "got {err:?}"
        );

        // the fresh cert at the CURRENT nonce (1) is accepted.
        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node2), &bind_msg(&u, CHAIN, &node2, 1)),
        )
        .unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        let view = get_user(&id, u.public_key().as_ref()).expect("bound");
        assert_eq!(view.nonce, 2);
        assert_eq!(view.nodes.len(), 2);
    }

    #[test]
    fn bind_same_user_is_idempotent() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![6u8; 32];
        let mut ctx = TestCtx::external(&node);

        futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, CHAIN, &node, 0))).unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        // second identical bind after commit -> Ok, nonce unchanged.
        futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, CHAIN, &node, 0))).unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        let view = user_of(&id, &node).expect("bound");
        assert_eq!(view.nonce, 1, "idempotent re-bind does not bump the nonce");
        assert_eq!(view.nodes, vec![node]);
    }

    #[test]
    fn bind_rejects_second_user_takeover() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let a = user();
        let b = ed25519::PrivateKey::from_seed(70);
        let node = vec![7u8; 32];
        let mut ctx = TestCtx::external(&node);

        futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&a, CHAIN, &node, 0))).unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        let err =
            futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&b, CHAIN, &node, 0)))
                .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "node is already bound to another user; unbind first"),
            "got {err:?}"
        );
    }

    #[test]
    fn bind_rejects_non_member_when_gated() {
        let mut id = Identity::new("identity", Some("valset".into()), CHAIN.into());
        let u = user();
        let node = vec![8u8; 32];
        let other_member = vec![9u8; 32];
        // `node` is neither a validator nor an observer.
        let mut ctx = TestCtx::with_members(&node, vec![other_member]);

        let err = futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, CHAIN, &node, 0)))
            .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "bind origin is not a network member or observer"),
            "got {err:?}"
        );

        // an OBSERVER-only origin passes the gate too.
        let mut ctx = TestCtx::with_observers(&node, vec![node.clone()]);
        futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, CHAIN, &node, 0))).unwrap();
    }

    #[test]
    fn bind_ungated_without_valset() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![10u8; 32];
        // no members/observers configured at all: an ungated module must
        // never call ctx.query, so this must succeed regardless.
        let mut ctx = TestCtx::external(&node);
        futures::executor::block_on(id.execute(&mut ctx, &bind_msg(&u, CHAIN, &node, 0))).unwrap();
    }

    #[test]
    fn unbind_from_any_origin_with_valid_cert() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node1 = vec![12u8; 32];
        let node2 = vec![13u8; 32];

        futures::executor::block_on(async {
            id.execute(&mut TestCtx::external(&node1), &bind_msg(&u, CHAIN, &node1, 0))
                .await
                .unwrap();
            id.commit_block().await.unwrap();
            id.execute(&mut TestCtx::external(&node2), &bind_msg(&u, CHAIN, &node2, 1))
                .await
                .unwrap();
            id.commit_block().await.unwrap();
        });
        // nonce is now 2; nodes == {node1, node2}.

        // unbind node1, cert at the current nonce (2), submitted from a THIRD origin.
        let third = vec![14u8; 32];
        let unbind = unbind_msg(&u, CHAIN, &node1, 2);
        futures::executor::block_on(id.execute(&mut TestCtx::external(&third), &unbind)).unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        let view = get_user(&id, u.public_key().as_ref()).expect("still bound");
        assert_eq!(view.nodes, vec![node2]);
        assert_eq!(view.nonce, 3);
        assert!(user_of(&id, &node1).is_none(), "node1 no longer bound");

        // the OLD bind cert for node1 (nonce 0) is now stale (current nonce is 3).
        let err = futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node1), &bind_msg(&u, CHAIN, &node1, 0)),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "bind certificate does not verify"),
            "got {err:?}"
        );
    }

    #[test]
    fn unbind_rejects_unbound_node() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![15u8; 32];
        let mut ctx = TestCtx::external(&[16u8; 32]);

        let err =
            futures::executor::block_on(id.execute(&mut ctx, &unbind_msg(&u, CHAIN, &node, 0)))
                .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "node is not bound"),
            "got {err:?}"
        );
    }

    #[test]
    fn unbind_bad_cert() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![17u8; 32];
        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0)),
        )
        .unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        let wrong = ed25519::PrivateKey::from_seed(99);
        let sig = wrong.sign(IDENTITY_UNBIND_NS, &unbind_preimage(CHAIN, &node, 1));
        let msg = Msg {
            target: "identity".into(),
            payload: encode_msg(&IdentityMsg::UnbindNode {
                node_key: node.clone(),
                user_sig: sig.as_ref().to_vec(),
            }),
        };
        let err = futures::executor::block_on(
            id.execute(&mut TestCtx::external(&[18u8; 32]), &msg),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "unbind certificate does not verify"),
            "got {err:?}"
        );
    }

    #[test]
    fn set_user_name_origin_gated() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![19u8; 32];
        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0)),
        )
        .unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();

        // an unbound origin is rejected.
        let unbound = vec![20u8; 32];
        let err = futures::executor::block_on(
            id.execute(&mut TestCtx::external(&unbound), &setname_msg("eddy")),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "origin node is not bound to a user"),
            "got {err:?}"
        );

        // the bound origin sets the name; the nonce does NOT bump.
        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &setname_msg("eddy")),
        )
        .unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();
        let view = get_user(&id, u.public_key().as_ref()).unwrap();
        assert_eq!(view.display_name.as_deref(), Some("eddy"));
        assert_eq!(view.nonce, 1, "setname does not bump the nonce");

        // a 65-byte name is rejected.
        let too_long = "x".repeat(65);
        let err = futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &setname_msg(&too_long)),
        )
        .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m == "display name exceeds the 64-byte limit"),
            "got {err:?}"
        );

        // an empty (whitespace) trim clears the name but the record survives.
        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &setname_msg("   ")),
        )
        .unwrap();
        futures::executor::block_on(id.commit_block()).unwrap();
        let view = get_user(&id, u.public_key().as_ref()).unwrap();
        assert_eq!(view.display_name, None);
        assert_eq!(view.nonce, 1);
        assert_eq!(view.nodes, vec![node]);
    }

    #[test]
    fn queries_read_staged_overlay() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![21u8; 32];

        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0)),
        )
        .unwrap();

        // read-your-writes before commit, across every query shape.
        let view = user_of(&id, &node).expect("staged bind visible before commit");
        assert_eq!(view.nonce, 1);
        assert!(get_user(&id, u.public_key().as_ref()).is_some());
        assert_eq!(all_users(&id, 0, MAX_QUERY_LIMIT).len(), 1);
        assert_eq!(id.root(), StateRoot::ZERO, "root reflects committed only");

        futures::executor::block_on(id.abort_block()).unwrap();
        assert!(user_of(&id, &node).is_none(), "abort_block drops the staged bind");
        assert!(all_users(&id, 0, MAX_QUERY_LIMIT).is_empty());
    }

    #[test]
    fn root_changes_on_commit_only() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        assert_eq!(id.root(), StateRoot::ZERO, "an empty registry roots to ZERO");

        let u = user();
        let node = vec![22u8; 32];
        futures::executor::block_on(
            id.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0)),
        )
        .unwrap();
        assert_eq!(
            id.root(),
            StateRoot::ZERO,
            "root is stable across staged-but-uncommitted writes"
        );

        futures::executor::block_on(id.commit_block()).unwrap();
        assert_ne!(id.root(), StateRoot::ZERO, "root changes after commit");
    }

    // ---- state-sync (snapshot / install) -----------------------------------

    /// one hand-built user entry: key, name, nonce, node keys, updated_at.
    type ManualUser<'a> = (Vec<u8>, Option<&'a str>, u64, Vec<Vec<u8>>, u64);

    /// hand-rolled encoder mirroring `Identity::encode_state`'s layout,
    /// reusing the crate's own `push_bytes` helper. used only to build
    /// deliberately malformed byte streams the private encoder can never
    /// produce itself (it always emits well-formed output from valid state).
    fn manual_encode_users(entries: &[ManualUser]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
        for (key, name, nonce, nodes, updated_at) in entries {
            push_bytes(&mut out, key);
            match name {
                Some(n) => {
                    out.push(1u8);
                    push_bytes(&mut out, n.as_bytes());
                }
                None => out.push(0u8),
            }
            out.extend_from_slice(&nonce.to_le_bytes());
            out.extend_from_slice(&(nodes.len() as u64).to_le_bytes());
            for node in nodes {
                push_bytes(&mut out, node);
            }
            out.extend_from_slice(&updated_at.to_le_bytes());
        }
        out
    }

    #[test]
    fn snapshot_is_root_preimage() {
        // empty registry: the lone zero count, rooting to ZERO.
        let empty = Identity::new("identity", None, CHAIN.into());
        assert_eq!(empty.snapshot(), 0u64.to_le_bytes().to_vec());
        assert_eq!(empty.root(), StateRoot::ZERO);

        // non-empty registry: sha256(snapshot()) == root().
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![30u8; 32];
        futures::executor::block_on(async {
            id.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0))
                .await
                .unwrap();
            id.commit_block().await.unwrap();
        });
        let bytes = id.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), id.root(), "sha256(snapshot()) == root()");
    }

    #[test]
    fn install_roundtrip_rebuilds_index() {
        let mut src = Identity::new("identity", None, CHAIN.into());
        let a = user();
        let b = ed25519::PrivateKey::from_seed(71);
        let n1 = vec![31u8; 32];
        let n2 = vec![32u8; 32];
        let n3 = vec![33u8; 32];
        futures::executor::block_on(async {
            src.execute(&mut TestCtx::external(&n1), &bind_msg(&a, CHAIN, &n1, 0))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
            src.execute(&mut TestCtx::external(&n2), &bind_msg(&a, CHAIN, &n2, 1))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
            src.execute(&mut TestCtx::external(&n3), &bind_msg(&b, CHAIN, &n3, 0))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
        });
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);
        let bytes = src.snapshot();

        let mut dst = Identity::new("identity", None, CHAIN.into());
        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root, "installed root equals source root");
        for node in [&n1, &n2, &n3] {
            assert!(user_of(&dst, node).is_some(), "UserOf resolves {node:?} after install");
        }
    }

    #[test]
    fn install_rejects_root_mismatch() {
        let mut src = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![34u8; 32];
        futures::executor::block_on(async {
            src.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
        });
        let src_root = src.root();
        let bytes = src.snapshot();

        let mut bad_root = src_root;
        bad_root.0[0] ^= 0x01;

        let mut dst = Identity::new("identity", None, CHAIN.into());
        let before_root = dst.root();
        let err = dst.install(&bytes, bad_root).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "got {err:?}");
        assert_eq!(dst.root(), before_root, "a rejected install leaves state untouched");
    }

    #[test]
    fn install_rejects_malformed() {
        let good = vec![
            (vec![1u8; 32], Some("a"), 0u64, vec![vec![10u8; 32]], 100u64),
            (vec![2u8; 32], None, 3u64, vec![vec![20u8; 32], vec![21u8; 32]], 200u64),
        ];
        let bytes = manual_encode_users(&good);
        let root = StateRoot(Sha256::digest(&bytes).into());

        // sanity: the well-formed manual encoding actually installs.
        let mut sane = Identity::new("identity", None, CHAIN.into());
        sane.install(&bytes, root).unwrap();
        assert_eq!(sane.root(), root);

        let mut dst = Identity::new("identity", None, CHAIN.into());
        let before_root = dst.root();
        let assert_rejected = |dst: &mut Identity, bad: &[u8]| {
            let err = dst.install(bad, root).unwrap_err();
            assert!(matches!(err, Error::Module(_)), "got {err:?}");
            assert_eq!(dst.root(), before_root, "state untouched after rejected install");
        };

        // truncated: drop the final byte.
        assert_rejected(&mut dst, &bytes[..bytes.len() - 1]);

        // trailing garbage past a well-formed stream.
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_rejected(&mut dst, &trailing);

        // forged user count exceeding what the remaining buffer could hold.
        let mut forged = bytes.clone();
        forged[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_rejected(&mut dst, &forged);

        // non-increasing user keys.
        let non_increasing_users = manual_encode_users(&[
            (vec![2u8; 32], None, 0, vec![], 0),
            (vec![1u8; 32], None, 0, vec![], 0),
        ]);
        assert_rejected(&mut dst, &non_increasing_users);

        // non-increasing node keys within a single user.
        let non_increasing_nodes = manual_encode_users(&[(
            vec![1u8; 32],
            None,
            0,
            vec![vec![9u8; 32], vec![9u8; 32]],
            0,
        )]);
        assert_rejected(&mut dst, &non_increasing_nodes);

        // name flag 1 with an over-long name.
        let long_name = "x".repeat(MAX_NAME_LEN + 1);
        let over_long_name =
            manual_encode_users(&[(vec![1u8; 32], Some(long_name.as_str()), 0, vec![], 0)]);
        assert_rejected(&mut dst, &over_long_name);
    }

    #[test]
    fn install_rejects_node_bound_twice_across_users() {
        // a node claimed by two different users is a state the execute path
        // can never commit -- decode must reject it outright.
        let bytes = manual_encode_users(&[
            (vec![1u8; 32], None, 0, vec![vec![9u8; 32]], 0),
            (vec![2u8; 32], None, 0, vec![vec![9u8; 32]], 0),
        ]);
        let root = StateRoot(Sha256::digest(&bytes).into());

        let mut dst = Identity::new("identity", None, CHAIN.into());
        let err = dst.install(&bytes, root).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("node bound twice in snapshot")),
            "got {err:?}"
        );
    }

    #[test]
    fn unbind_last_node_keeps_record() {
        let mut id = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![36u8; 32];

        futures::executor::block_on(async {
            id.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0))
                .await
                .unwrap();
            id.commit_block().await.unwrap();
            id.execute(&mut TestCtx::external(&node), &setname_msg("eddy"))
                .await
                .unwrap();
            id.commit_block().await.unwrap();
            id.execute(
                &mut TestCtx::external(&[37u8; 32]),
                &unbind_msg(&u, CHAIN, &node, 1),
            )
            .await
            .unwrap();
            id.commit_block().await.unwrap();
        });

        let view =
            get_user(&id, u.public_key().as_ref()).expect("record survives with zero nodes");
        assert!(view.nodes.is_empty());
        assert_eq!(view.display_name.as_deref(), Some("eddy"));
        assert_eq!(view.nonce, 2);

        // snapshot/install this zero-node state into a fresh Identity.
        let root = id.root();
        assert_ne!(root, StateRoot::ZERO, "a zero-node user record is not the empty registry");
        let bytes = id.snapshot();

        let mut fresh = Identity::new("identity", None, CHAIN.into());
        fresh.install(&bytes, root).unwrap();
        assert_eq!(fresh.root(), root);
        let fresh_view =
            get_user(&fresh, u.public_key().as_ref()).expect("zero-node record round-trips");
        assert_eq!(fresh_view.nodes, Vec::<Vec<u8>>::new());
        assert_eq!(fresh_view.display_name.as_deref(), Some("eddy"));
        assert_eq!(fresh_view.nonce, 2);
    }

    #[test]
    fn install_gate_untouched_by_valset() {
        // build committed state on an ungated source (no member cert needed),
        // then install those bytes into a GATED registry (`valset_id =
        // Some`): install takes no `Ctx` at all, so it must never consult the
        // valset -- this is not an execute path.
        let mut src = Identity::new("identity", None, CHAIN.into());
        let u = user();
        let node = vec![38u8; 32];
        futures::executor::block_on(async {
            src.execute(&mut TestCtx::external(&node), &bind_msg(&u, CHAIN, &node, 0))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
        });
        let root = src.root();
        let bytes = src.snapshot();

        let mut gated = Identity::new("identity", Some("valset".into()), CHAIN.into());
        gated.install(&bytes, root).unwrap();
        assert_eq!(gated.root(), root);
        assert!(user_of(&gated, &node).is_some());
    }
}
