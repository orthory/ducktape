//! the network-wide registry of node host capabilities as replicated state.
//!
//! a "capability" is an open-set string tag ("codex", "claude", ...) naming
//! something a node's HOST can execute for the network — the consensus-side
//! half of the provider seam: this module replicates *who provides what*;
//! actually spawning a provider is host code and never happens here (no I/O,
//! no provider-specific logic). every node holds an identical view at every
//! height, which is exactly the property future work assignment needs (all
//! nodes must agree deterministically on who could serve a job).
//!
//! announcements are declarative, self-scoped, and truthful by construction:
//! [`CapabilityMsg::Announce`] replaces the SUBMITTER's full tag set — the
//! announced identity is the verified external submit origin, never payload
//! data, so a node can only speak for itself. an empty set removes the node.
//! when constructed with a valset id, announcements are additionally gated to
//! current validators UNION residents (mirroring identity's `BindNode` gate):
//! a joined-but-not-promoted node provides real executors too, and its
//! announce is what lets dispatch route work to it. without a valset (the
//! single-node daemon) any external key may self-announce.
//!
//! an `Announce` may additionally carry `resources`: announced numeric
//! capacity per open-set dimension ("cores" -> 8, "mem_gb" -> 32), riding the
//! same declarative replace as tags. capacity with nothing to execute is
//! meaningless, so resources without at least one tag is rejected; a
//! tags-only node stays valid (direct-spawn mode) but never satisfies a
//! demands-carrying query — absent is never infinite.
//! [`CapabilityQuery::CapableProviders`] is the read future work
//! assignment filters on: providers of a capability whose announced
//! resources cover every demanded dimension.
//!
//! ## capability classes
//!
//! the registry additionally routes capability CLASSES — namespace tokens
//! (`agent`, `ai`, ...) a MODULE claims via [`CapabilityMsg::ClaimClass`].
//! the wire form of a classed capability is `<class>:<rest>` and dispatch
//! addresses classed work as `<node_id>/<class>:<rest>` (see
//! [`parse_classed_address`]); the class map (class -> claimant module) is
//! the primary router for that space. the claimant is the verified MODULE
//! origin — a class is claimed by the module that serves it, so External and
//! System origins reject. first claim wins deterministically; a re-claim by
//! the owner is an idempotent no-op. there is deliberately NO unclaim op:
//! dropping a claim would dangle every `<class>:` route already minted, so
//! claims are permanent.
//! class claims ride the current snapshot as their own count-prefixed
//! section, appended after the node section.
//!
//! state model mirrors valset's host-lent staging seam: `execute` STAGES into
//! a `pending` overlay (committed state untouched); `query` reads
//! pending-over-committed (read-your-writes); `commit_block` merges pending
//! into committed; `abort_block` drops pending; `root()` reflects COMMITTED
//! state only — a state-based (sorted, length-prefixed) sha256 over the
//! registry, so it is order-independent and idempotent.
//!
//! ## state-sync
//!
//! a joiner rebuilds this module from a peer via [`CapabilityRegistry::snapshot`]
//! / [`CapabilityRegistry::install`]. the snapshot is the exact preimage of
//! `root()`, so the joiner needs no trust in the serving peer: install
//! recomputes the root of whatever bytes arrived and refuses to adopt them
//! unless it matches the expected root consensus already agreed on.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use sdk::codec;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

/// most tags a single node may announce. a bound, not a schema: it exists so
/// one announcement cannot bloat replicated state, while staying far above
/// any real host's executor count.
const MAX_CAPABILITIES: usize = 64;

/// one node's registry entry (committed or staged): the tag set it announced
/// plus the numeric capacity it announced per dimension. tags empty means the
/// node is absent — `announced` never holds an entry that way, and a staged
/// entry with empty tags stages a removal. resources alone is never a valid
/// entry: `execute` rejects an announce that carries resources without at
/// least one tag, so every stored entry with non-empty resources also has
/// non-empty tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NodeEntry {
    tags: BTreeSet<String>,
    resources: BTreeMap<String, u64>,
}

pub struct CapabilityRegistry {
    id: ModuleId,
    /// the valset module consulted to gate announcements to current members;
    /// `None` runs ungated (the single-node daemon carries no valset).
    valset_id: Option<ModuleId>,
    /// committed registry — what `root()` and the app-hash commit to. a node
    /// key never maps to an entry with empty tags: empty tags means absent.
    announced: BTreeMap<Vec<u8>, NodeEntry>,
    /// per-block staged replacements: the entry stages a full declarative
    /// replace, an entry with EMPTY tags stages a removal. read ahead of
    /// `announced` (read-your-writes), merged into committed state only on
    /// `commit_block`.
    pending: BTreeMap<Vec<u8>, NodeEntry>,
    /// committed class claims: class -> the module that serves it. first
    /// claim wins and claims are never removed (no unclaim op — see the
    /// module doc), so this map only grows.
    class_claims: BTreeMap<String, ModuleId>,
    /// per-block staged class claims — an insert-only overlay read ahead of
    /// `class_claims` (read-your-writes: a rival claim in the same block sees
    /// the earlier stage), merged on `commit_block`, dropped on `abort_block`.
    pending_class_claims: BTreeMap<String, ModuleId>,
}

impl CapabilityRegistry {
    pub fn new(id: impl Into<ModuleId>, valset_id: Option<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset_id,
            announced: BTreeMap::new(),
            pending: BTreeMap::new(),
            class_claims: BTreeMap::new(),
            pending_class_claims: BTreeMap::new(),
        }
    }

    /// validate and canonicalize an announced tag list. duplicates collapse
    /// (set semantics — announcing ["codex", "codex"] is not an error), shape
    /// violations reject deterministically: every validator sees the same
    /// bytes, so every validator rejects identically.
    fn validate_tags(tags: Vec<String>) -> Result<BTreeSet<String>, Error> {
        if tags.len() > MAX_CAPABILITIES {
            return Err(Error::Module(format!(
                "too many capabilities: {} exceeds the {MAX_CAPABILITIES} cap",
                tags.len()
            )));
        }
        let mut set = BTreeSet::new();
        for tag in tags {
            validate_tag(&tag).map_err(Error::Module)?;
            set.insert(tag);
        }
        Ok(set)
    }

    /// the CURRENT validator set UNION resident set, both queried live from
    /// the committed registry with this block's staged replacements applied —
    /// read-your-writes; a staged entry with empty tags reads as absent.
    fn effective(&self) -> BTreeMap<Vec<u8>, NodeEntry> {
        let mut map = self.announced.clone();
        for (key, entry) in &self.pending {
            if entry.tags.is_empty() {
                map.remove(key);
            } else {
                map.insert(key.clone(), entry.clone());
            }
        }
        map
    }

    /// the committed class map with this block's staged claims applied —
    /// read-your-writes; claims are insert-only, so the overlay never removes.
    fn effective_classes(&self) -> BTreeMap<String, ModuleId> {
        let mut map = self.class_claims.clone();
        for (class, module) in &self.pending_class_claims {
            map.insert(class.clone(), module.clone());
        }
        map
    }

    /// the module owning `class` in the pending-over-committed view — the
    /// first-claim-wins check reads THIS, so a claim staged earlier in the
    /// block already blocks a rival in the same block.
    fn effective_class_owner(&self, class: &str) -> Option<&ModuleId> {
        self.pending_class_claims
            .get(class)
            .or_else(|| self.class_claims.get(class))
    }

    // ---- state-sync ---------------------------------------------------------
    // ship the committed registry as its root preimage; adopt a peer's bytes
    // only after re-deriving the root consensus expects — the root, not the
    // peer, is the trust anchor.

    /// canonical bytes of the COMMITTED registry — exactly the byte stream
    /// `root()` hashes, two count-prefixed sections back to back:
    ///
    /// 1. announcements — node count u64-le, then per sorted node key its len
    ///    u64-le + key bytes + tag count u64-le, then per sorted tag its len
    ///    u64-le + utf-8 bytes, then resource count u64-le, then per sorted
    ///    dimension its key len u64-le + utf-8 bytes + value u64-le. a
    ///    tags-only node still emits a trailing zero resource count, so it
    ///    round-trips;
    /// 2. class claims — class count u64-le, then per sorted class its
    ///    len-prefixed utf-8 name + the claimant module id's len-prefixed
    ///    utf-8 bytes. an empty class map encodes as a lone zero count.
    ///
    /// for a non-empty registry `sha256(snapshot()) == root()`; an empty
    /// registry snapshots to two zero counts (whose root is still `ZERO`,
    /// unhashed). pending is deliberately excluded — a snapshot ships what
    /// consensus committed to.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::snapshot_of(&self.announced, &self.class_claims)
    }

    fn snapshot_announcements(map: &BTreeMap<Vec<u8>, NodeEntry>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(map.len() as u64).to_le_bytes());
        for (key, entry) in map {
            codec::push_bytes(&mut out, key);
            out.extend_from_slice(&(entry.tags.len() as u64).to_le_bytes());
            for tag in &entry.tags {
                codec::push_str(&mut out, tag);
            }
            out.extend_from_slice(&(entry.resources.len() as u64).to_le_bytes());
            for (dim, value) in &entry.resources {
                codec::push_str(&mut out, dim);
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }

    fn snapshot_of(
        map: &BTreeMap<Vec<u8>, NodeEntry>,
        classes: &BTreeMap<String, ModuleId>,
    ) -> Vec<u8> {
        let mut out = Self::snapshot_announcements(map);
        out.extend_from_slice(&(classes.len() as u64).to_le_bytes());
        for (class, module) in classes {
            codec::push_str(&mut out, class);
            codec::push_str(&mut out, module);
        }
        out
    }

    /// replace committed state with a decoded snapshot, iff the decoded
    /// registry's recomputed root equals `expected`. decode and verification
    /// land in a temporary: self is mutated only after both pass, so on any
    /// `Err` committed state, pending, and `root()` are byte-identical to
    /// before the call. success clears pending — staged changes belong to the
    /// state being replaced, not the state being adopted.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (announced, class_claims) = Self::decode_snapshot(bytes)?;
        sdk::verify_snapshot_root(Self::root_of(&announced, &class_claims), expected)?;
        self.announced = announced;
        self.class_claims = class_claims;
        self.pending.clear();
        self.pending_class_claims.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves
    /// them). every count and length is checked against the remaining buffer
    /// BEFORE any allocation, truncation and trailing bytes both reject, node
    /// keys, tags, resource dimensions, and class names must each arrive
    /// strictly increasing, a node with zero tags rejects (empty means
    /// absent, so it has no encoding), and a zero-valued resource rejects too
    /// (the same `validate_resources` invariant, held at decode time so a
    /// byzantine peer cannot mint a dimension no honest announce could
    /// produce) — a given registry has exactly one valid byte stream, so a
    /// peer cannot mint alternative encodings for one state.
    #[allow(clippy::type_complexity)]
    fn decode_snapshot(
        bytes: &[u8],
    ) -> Result<(BTreeMap<Vec<u8>, NodeEntry>, BTreeMap<String, ModuleId>), Error> {
        let mut cur = codec::Cursor::new(bytes);
        let count = cur.u64("snapshot node count")?;
        // each node entry costs at least its 8-byte key-length prefix plus an
        // 8-byte tag count, so a count the remaining bytes cannot possibly
        // hold is rejected up front — a forged count never drives allocation.
        cur.bound(count, 16, "snapshot node")?;
        let mut map = BTreeMap::new();
        let mut prev_key: Option<&[u8]> = None;
        for _ in 0..count {
            let key = cur.bytes("snapshot node key")?;
            if prev_key.is_some_and(|p| p >= key) {
                return Err(Error::Module(
                    "snapshot node keys must be strictly increasing".into(),
                ));
            }
            prev_key = Some(key);

            let tag_count = cur.u64("snapshot tag count")?;
            if tag_count == 0 {
                return Err(Error::Module(
                    "snapshot node with zero capabilities (empty means absent)".into(),
                ));
            }
            // each tag costs at least its 8-byte length prefix.
            cur.bound(tag_count, 8, "snapshot tag")?;
            let mut tags = BTreeSet::new();
            let mut prev_tag: Option<&[u8]> = None;
            for _ in 0..tag_count {
                let tag = cur.bytes("snapshot tag")?;
                if prev_tag.is_some_and(|p| p >= tag) {
                    return Err(Error::Module(
                        "snapshot tags must be strictly increasing".into(),
                    ));
                }
                prev_tag = Some(tag);
                let tag = std::str::from_utf8(tag)
                    .map_err(|e| Error::Module(format!("snapshot tag is not utf-8: {e}")))?;
                tags.insert(tag.to_string());
            }

            let resource_count = cur.u64("snapshot resource count")?;
            // each dimension costs at least its 8-byte key-length prefix plus
            // an 8-byte value.
            cur.bound(resource_count, 16, "snapshot resource")?;
            let mut resources = BTreeMap::new();
            let mut prev_dim: Option<&[u8]> = None;
            for _ in 0..resource_count {
                let dim = cur.bytes("snapshot resource key")?;
                if prev_dim.is_some_and(|p| p >= dim) {
                    return Err(Error::Module(
                        "snapshot resource keys must be strictly increasing".into(),
                    ));
                }
                prev_dim = Some(dim);
                let dim = std::str::from_utf8(dim).map_err(|e| {
                    Error::Module(format!("snapshot resource key is not utf-8: {e}"))
                })?;
                let value = cur.u64("snapshot resource value")?;
                if value == 0 {
                    return Err(Error::Module(
                        "snapshot resource value is zero (omit the dimension instead)".into(),
                    ));
                }
                resources.insert(dim.to_string(), value);
            }

            map.insert(key.to_vec(), NodeEntry { tags, resources });
        }

        let class_count = cur.u64("snapshot class count")?;
        // each class entry costs at least its two 8-byte length prefixes.
        cur.bound(class_count, 16, "snapshot class")?;
        let mut classes = BTreeMap::new();
        let mut prev_class: Option<&[u8]> = None;
        for _ in 0..class_count {
            let class = cur.bytes("snapshot class name")?;
            if prev_class.is_some_and(|p| p >= class) {
                return Err(Error::Module(
                    "snapshot classes must be strictly increasing".into(),
                ));
            }
            prev_class = Some(class);
            let class = std::str::from_utf8(class)
                .map_err(|e| Error::Module(format!("snapshot class name is not utf-8: {e}")))?;
            let module = cur.string("snapshot class claimant")?;
            classes.insert(class.to_string(), module);
        }
        cur.finish("snapshot")?;
        Ok((map, classes))
    }

    /// the state-based commitment for the registry: `ZERO` when BOTH sections
    /// are empty, else sha256 over exactly the bytes `snapshot` emits. shared
    /// by `root()` (committed state) and `install` (a decoded candidate), so
    /// the two can never drift.
    fn root_of(
        map: &BTreeMap<Vec<u8>, NodeEntry>,
        classes: &BTreeMap<String, ModuleId>,
    ) -> StateRoot {
        if map.is_empty() && classes.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::snapshot_of(map, classes)).into())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for CapabilityRegistry {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED registry: a length-prefixed
    /// sha256 over the sorted node -> tags map plus the sorted class -> module
    /// map. order-independent (BTreeMap / BTreeSet) and idempotent. an empty
    /// registry reports `ZERO`.
    fn root(&self) -> StateRoot {
        Self::root_of(&self.announced, &self.class_claims)
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            CapabilityMsg::Announce {
                capabilities,
                resources,
            } => {
                // identity comes from the verified submit origin, never the
                // payload — a node can only announce for itself, which keeps
                // the registry truthful by construction. module/system
                // origins have no host of their own to speak for, and an
                // empty key is a malformed origin (the same reject dispatch
                // applies), never a registry entry.
                let node = match &ctx.env().origin {
                    sdk::Origin::External(key) if key.is_empty() => {
                        return Err(Error::Module("external origin key is empty".into()));
                    }
                    sdk::Origin::External(key) => key.clone(),
                    other => {
                        return Err(Error::Module(format!(
                            "capability announcements require an external submitter, got {other:?}"
                        )));
                    }
                };
                if let Some(valset_id) = self.valset_id.clone() {
                    // member-gated like identity's BindNode: validators UNION
                    // residents populate the registry, so lookups resolve to
                    // known peers — including a joined node that has not been
                    // promoted yet.
                    if !valset::members_and_residents(ctx, &valset_id)
                        .await?
                        .contains(&node)
                    {
                        return Err(Error::Module(
                            "capability announcer holds no current standing (validator or resident)"
                                .into(),
                        ));
                    }
                }
                let tags = Self::validate_tags(capabilities)?;
                validate_resources(&resources).map_err(Error::Module)?;
                if tags.is_empty() && !resources.is_empty() {
                    return Err(Error::Module(
                        "resources without capabilities (announce at least one tag)".into(),
                    ));
                }
                // declarative replace: the last announcement staged in a block
                // wins, and an empty-tags entry stages a removal.
                self.pending.insert(node, NodeEntry { tags, resources });
            }
            CapabilityMsg::ClaimClass { class } => {
                // a class is claimed by the module that serves it: the
                // claimant is the verified MODULE origin, never payload data.
                // external submitters and the system have no module to route
                // classed work to, so both reject.
                let module = match &ctx.env().origin {
                    sdk::Origin::Module(id) => id.clone(),
                    other => {
                        return Err(Error::Module(format!(
                            "a class is claimed by the module that serves it \
                             (module origin required), got {other:?}"
                        )));
                    }
                };
                validate_class(&class).map_err(Error::Module)?;
                // first claim wins, read against the pending-over-committed
                // view so a claim staged earlier in this block already binds.
                match self.effective_class_owner(&class) {
                    Some(owner) if *owner != module => {
                        return Err(Error::Module(format!(
                            "class {class:?} is already claimed by module {owner:?} \
                             (first claim wins)"
                        )));
                    }
                    // a re-claim by the owning module is an idempotent no-op:
                    // nothing is staged, so the root cannot move.
                    Some(_) => {}
                    None => {
                        self.pending_class_claims.insert(class, module);
                    }
                }
            }
        }
        Ok(())
    }

    /// read projection — the committed registry plus this block's staged
    /// replacements and claims.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let view = self.effective();
        Ok(match decode_query(req).map_err(Error::Module)? {
            CapabilityQuery::Providers { capability } => {
                let providers = view
                    .iter()
                    .filter(|(_, e)| e.tags.contains(&capability))
                    .map(|(key, _)| key.clone())
                    .collect();
                encode_reply(&CapabilityReply::Providers(providers))
            }
            CapabilityQuery::Node { node } => {
                let tags = view
                    .get(&node)
                    .map(|e| e.tags.iter().cloned().collect())
                    .unwrap_or_default();
                encode_reply(&CapabilityReply::Node(tags))
            }
            CapabilityQuery::CapableProviders {
                capability,
                demands,
            } => {
                let providers = view
                    .iter()
                    .filter(|(_, e)| e.tags.contains(&capability))
                    .filter(|(_, e)| {
                        demands
                            .iter()
                            .all(|(k, v)| e.resources.get(k).is_some_and(|have| have >= v))
                    })
                    .map(|(key, _)| key.clone())
                    .collect();
                encode_reply(&CapabilityReply::Providers(providers))
            }
            CapabilityQuery::Resources { node } => {
                let resources = view
                    .get(&node)
                    .map(|e| e.resources.clone())
                    .unwrap_or_default();
                encode_reply(&CapabilityReply::Resources(resources))
            }
            CapabilityQuery::All => {
                let all = view
                    .into_iter()
                    .map(|(key, e)| (key, e.tags.into_iter().collect()))
                    .collect();
                encode_reply(&CapabilityReply::All(all))
            }
            CapabilityQuery::ResolveClass { class } => {
                let owner = self.effective_class_owner(&class).cloned();
                encode_reply(&CapabilityReply::ClassOwner(owner))
            }
            CapabilityQuery::Classes => {
                let classes = self.effective_classes().into_iter().collect();
                encode_reply(&CapabilityReply::Classes(classes))
            }
        })
    }

    /// merge the block's staged replacements and claims into committed state
    /// — `root()` now reflects them. no-op if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        for (key, entry) in std::mem::take(&mut self.pending) {
            if entry.tags.is_empty() {
                self.announced.remove(&key);
            } else {
                self.announced.insert(key, entry);
            }
        }
        self.class_claims.append(&mut self.pending_class_claims);
        Ok(())
    }

    /// discard the block's staged replacements and claims — committed state
    /// (and `root()`) is unchanged, so a failed block leaves no trace.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        self.pending_class_claims.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_CLASS_LEN, MAX_TAG_LEN, encode_msg, encode_query};
    use valset::{ValsetQuery, ValsetReply, encode_reply as valset_encode_reply};

    use sdk_testkit::TestCtx;

    /// a valset-query responder over an optional member/resident set — answers
    /// BOTH Validators and Residents so the member gate is testable for either
    /// standing (mirrors identity's test stub).
    fn valset_reads(
        members: Option<Vec<Vec<u8>>>,
        residents: Option<Vec<Vec<u8>>>,
    ) -> impl FnMut(&[u8]) -> Result<Vec<u8>, Error> {
        move |req| {
            let q = valset::decode_query(req).map_err(Error::Module)?;
            match (q, &members, &residents) {
                (ValsetQuery::Validators, Some(m), _) => {
                    Ok(valset_encode_reply(&ValsetReply::Validators(m.clone())))
                }
                (ValsetQuery::Residents, _, Some(o)) => {
                    Ok(valset_encode_reply(&ValsetReply::Residents(o.clone())))
                }
                _ => Err(Error::QueryUnsupported),
            }
        }
    }

    fn ctx_with(
        origin: sdk::Origin,
        members: Option<Vec<Vec<u8>>>,
        residents: Option<Vec<Vec<u8>>>,
    ) -> TestCtx {
        TestCtx::with_env(sdk::Env {
            height: 0,
            consensus_time: 0,
            origin,
            me: "capability".into(),
        })
        .on_query("valset", valset_reads(members, residents))
    }

    fn ctx_origin(origin: sdk::Origin) -> TestCtx {
        ctx_with(origin, None, None)
    }
    fn ctx_external(key: &[u8]) -> TestCtx {
        ctx_origin(sdk::Origin::External(key.to_vec()))
    }
    fn ctx_gated(key: &[u8], validators: Vec<Vec<u8>>, residents: Vec<Vec<u8>>) -> TestCtx {
        ctx_with(
            sdk::Origin::External(key.to_vec()),
            Some(validators),
            Some(residents),
        )
    }
    fn ctx_with_members(key: &[u8], members: Vec<Vec<u8>>) -> TestCtx {
        ctx_gated(key, members, Vec::new())
    }
    fn ctx_with_residents(key: &[u8], residents: Vec<Vec<u8>>) -> TestCtx {
        ctx_gated(key, Vec::new(), residents)
    }

    fn announce_with(tags: &[&str], resources: &[(&str, u64)]) -> Msg {
        Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::Announce {
                capabilities: tags.iter().map(|t| t.to_string()).collect(),
                resources: resources.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            }),
        }
    }
    fn announce(tags: &[&str]) -> Msg {
        announce_with(tags, &[])
    }
    fn capable(c: &CapabilityRegistry, capability: &str, demands: &[(&str, u64)]) -> Vec<Vec<u8>> {
        let reply = futures::executor::block_on(c.query(&encode_query(
            &CapabilityQuery::CapableProviders {
                capability: capability.into(),
                demands: demands.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            },
        )))
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Providers(p) => p,
            other => panic!("expected Providers reply, got {other:?}"),
        }
    }
    fn node_tags(c: &CapabilityRegistry, node: &[u8]) -> Vec<String> {
        let reply = futures::executor::block_on(c.query(&encode_query(&CapabilityQuery::Node {
            node: node.to_vec(),
        })))
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Node(tags) => tags,
            other => panic!("expected Node reply, got {other:?}"),
        }
    }
    fn providers(c: &CapabilityRegistry, capability: &str) -> Vec<Vec<u8>> {
        let reply =
            futures::executor::block_on(c.query(&encode_query(&CapabilityQuery::Providers {
                capability: capability.into(),
            })))
            .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Providers(p) => p,
            other => panic!("expected Providers reply, got {other:?}"),
        }
    }
    /// an ungated registry (no valset) — most tests exercise state mechanics,
    /// not the member gate.
    fn ungated() -> CapabilityRegistry {
        CapabilityRegistry::new("capability", None)
    }

    #[test]
    fn a_snapshot_without_the_class_section_is_rejected() {
        let mut src = ungated();
        let node = vec![42u8; 32];
        futures::executor::block_on(src.execute(&mut ctx_external(&node), &announce(&["codex"])))
        .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();

        let snapshot = src.snapshot();
        let truncated = &snapshot[..snapshot.len() - 8];
        assert!(
            ungated().install(truncated, src.root()).is_err(),
            "a snapshot without its class count must not decode"
        );
    }

    #[test]
    fn announce_registers_and_moves_root_off_zero() {
        let mut c = ungated();
        let me = vec![1u8; 32];
        let mut ctx = ctx_external(&me);
        assert_eq!(c.root(), StateRoot::ZERO, "genesis registry is empty");

        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex", "claude"]))).unwrap();
        // staged, not committed: root still ZERO, but read-your-writes sees it.
        assert_eq!(c.root(), StateRoot::ZERO, "root reflects committed only");
        assert_eq!(
            node_tags(&c, &me),
            vec!["claude", "codex"],
            "ryw sees stage"
        );

        futures::executor::block_on(c.commit_block()).unwrap();
        assert_ne!(c.root(), StateRoot::ZERO, "a committed announce moves root");
        assert_eq!(providers(&c, "codex"), vec![me.clone()]);
        assert_eq!(providers(&c, "claude"), vec![me]);
    }

    #[test]
    fn announce_is_a_declarative_replace() {
        let mut c = ungated();
        let me = vec![2u8; 32];
        let mut ctx = ctx_external(&me);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();

        // the second announcement REPLACES the set — "codex" is gone, not kept.
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["claude"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(node_tags(&c, &me), vec!["claude"]);
        assert!(providers(&c, "codex").is_empty(), "replaced tag is gone");
    }

    #[test]
    fn empty_announce_removes_the_node() {
        let mut c = ungated();
        let me = vec![3u8; 32];
        let mut ctx = ctx_external(&me);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_ne!(c.root(), StateRoot::ZERO);

        futures::executor::block_on(c.execute(&mut ctx, &announce(&[]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert!(node_tags(&c, &me).is_empty(), "the node is gone");
        assert_eq!(c.root(), StateRoot::ZERO, "an emptied registry is ZERO");
    }

    #[test]
    fn duplicate_tags_collapse() {
        let mut c = ungated();
        let me = vec![4u8; 32];
        let mut ctx = ctx_external(&me);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex", "codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(node_tags(&c, &me), vec!["codex"], "set semantics");
    }

    #[test]
    fn non_external_origins_are_rejected() {
        let mut c = ungated();
        for origin in [sdk::Origin::Module("agent".into()), sdk::Origin::System] {
            let mut ctx = ctx_origin(origin);
            let err = futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"])))
                .unwrap_err();
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("external submitter")),
                "got {err:?}"
            );
        }
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(c.root(), StateRoot::ZERO, "nothing was staged");
    }

    #[test]
    fn empty_external_keys_are_rejected() {
        // the same malformed-origin reject dispatch applies: an empty key is
        // never a registry entry.
        let mut c = ungated();
        let mut ctx = ctx_external(&[]);
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("key is empty")),
            "got {err:?}"
        );
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(c.root(), StateRoot::ZERO, "nothing was staged");
    }

    #[test]
    fn member_gate_rejects_non_members_and_admits_members() {
        let member = vec![5u8; 32];
        let outsider = vec![6u8; 32];
        let mut c = CapabilityRegistry::new("capability", Some("valset".into()));

        let mut ctx = ctx_with_members(&outsider, vec![member.clone()]);
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("no current standing")),
            "got {err:?}"
        );

        let mut ctx = ctx_with_members(&member, vec![member.clone()]);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(providers(&c, "codex"), vec![member]);
    }

    #[test]
    fn member_gate_admits_residents_and_still_rejects_outsiders() {
        let validator = vec![20u8; 32];
        let resident = vec![21u8; 32];
        let outsider = vec![22u8; 32];
        let mut c = CapabilityRegistry::new("capability", Some("valset".into()));

        // a RESIDENT (joined, admitted, not promoted) announces: admitted —
        // the whole point of the resident-announce path.
        let mut ctx = ctx_gated(&resident, vec![validator.clone()], vec![resident.clone()]);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(providers(&c, "codex"), vec![resident.clone()]);

        // a key with NEITHER standing is still rejected, even alongside a
        // populated resident set.
        let mut ctx = ctx_gated(&outsider, vec![validator.clone()], vec![resident.clone()]);
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &announce(&["claude"]))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("no current standing")),
            "got {err:?}"
        );

        // resident-only standing (no validator overlap) also admits — the
        // gate is a true union, not an intersection.
        let mut ctx = ctx_with_residents(&resident, vec![resident.clone()]);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["claude"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(providers(&c, "claude"), vec![resident]);
    }

    #[test]
    fn malformed_tags_are_rejected() {
        let mut c = ungated();
        let me = vec![7u8; 32];
        let mut ctx = ctx_external(&me);
        let too_long = "x".repeat(MAX_TAG_LEN + 1);
        let too_many: Vec<String> = (0..=MAX_CAPABILITIES).map(|i| format!("cap{i}")).collect();
        let too_many: Vec<&str> = too_many.iter().map(String::as_str).collect();
        for bad in [
            vec![""],
            vec![too_long.as_str()],
            vec!["Codex"],
            vec!["co dex"],
            too_many,
        ] {
            let err =
                futures::executor::block_on(c.execute(&mut ctx, &announce(&bad))).unwrap_err();
            assert!(matches!(err, Error::Module(_)), "got {err:?} for {bad:?}");
        }
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(
            c.root(),
            StateRoot::ZERO,
            "rejected announces staged nothing"
        );
    }

    #[test]
    fn root_is_state_based_order_independent() {
        let (a, b) = (vec![8u8; 32], vec![9u8; 32]);

        let mut c1 = ungated();
        futures::executor::block_on(async {
            c1.execute(&mut ctx_external(&a), &announce(&["codex"]))
                .await
                .unwrap();
            c1.execute(&mut ctx_external(&b), &announce(&["claude"]))
                .await
                .unwrap();
            c1.commit_block().await.unwrap();
        });

        // same registry contents, announced in the opposite order.
        let mut c2 = ungated();
        futures::executor::block_on(async {
            c2.execute(&mut ctx_external(&b), &announce(&["claude"]))
                .await
                .unwrap();
            c2.execute(&mut ctx_external(&a), &announce(&["codex"]))
                .await
                .unwrap();
            c2.commit_block().await.unwrap();
        });

        assert_eq!(c1.root(), c2.root(), "root is f(state), order-independent");
    }

    #[test]
    fn atomicity_a_failed_block_rolls_back_the_stage() {
        let mut c = ungated();
        let me = vec![10u8; 32];
        let mut ctx = ctx_external(&me);
        let before = c.root();

        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap();
        // ... a later dispatch in the same block errors, so the host aborts:
        futures::executor::block_on(c.abort_block()).unwrap();

        assert!(
            node_tags(&c, &me).is_empty(),
            "aborted announce left nothing"
        );
        assert_eq!(c.root(), before, "root unchanged after a rolled-back block");
    }

    #[test]
    fn snapshot_install_round_trip_reconstructs_root_and_registry() {
        // SOURCE: two nodes through the real execute+commit path, so the
        // snapshot ships state consensus actually committed.
        let mut src = ungated();
        let (a, b) = (vec![11u8; 32], vec![12u8; 32]);
        futures::executor::block_on(async {
            src.execute(&mut ctx_external(&a), &announce(&["codex", "claude"]))
                .await
                .unwrap();
            src.execute(&mut ctx_external(&b), &announce(&["codex"]))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
        });
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);

        // the snapshot IS the root preimage.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        // TARGET: a fresh registry with an unrelated announce STAGED — install
        // must drop it, or the stale stage would leak into the new view.
        let mut dst = ungated();
        futures::executor::block_on(
            dst.execute(&mut ctx_external(&[13u8; 32]), &announce(&["other"])),
        )
        .unwrap();

        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root, "installed root equals source root");
        assert_eq!(providers(&dst, "codex"), providers(&src, "codex"));
        assert!(
            node_tags(&dst, &[13u8; 32]).is_empty(),
            "stale stage dropped"
        );
    }

    #[test]
    fn tampered_snapshot_is_rejected_and_the_target_is_untouched() {
        let mut src = ungated();
        let a = vec![14u8; 32];
        futures::executor::block_on(src.execute(&mut ctx_external(&a), &announce(&["codex"])))
            .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();

        // flip one bit inside the tag: counts, lengths, and sort order still
        // hold, so structural decode alone cannot catch it — only the
        // recomputed-root check can. exactly the byzantine-payload case.
        let mut bytes = src.snapshot();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;

        // the target holds committed state AND a stage: a failed install must
        // leave every layer untouched.
        let mut dst = ungated();
        let b = vec![15u8; 32];
        futures::executor::block_on(dst.execute(&mut ctx_external(&b), &announce(&["claude"])))
        .unwrap();
        futures::executor::block_on(dst.commit_block()).unwrap();
        futures::executor::block_on(dst.execute(&mut ctx_external(&b), &announce(&["other"])))
            .unwrap();
        let pre_root = dst.root();
        let pre_view = node_tags(&dst, &b);

        let err = dst.install(&bytes, src_root).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "got {err:?}");
        assert_eq!(dst.root(), pre_root, "committed root untouched");
        assert_eq!(node_tags(&dst, &b), pre_view, "view and stage untouched");
    }

    #[test]
    fn truncated_trailing_or_forged_snapshots_are_rejected() {
        let mut src = ungated();
        futures::executor::block_on(
            src.execute(&mut ctx_external(&[16u8; 32]), &announce(&["codex"])),
        )
        .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();
        let bytes = src.snapshot();

        let mut dst = ungated();
        let before = dst.root();
        // truncation: the final byte (of the trailing class count) is missing.
        assert!(dst.install(&bytes[..bytes.len() - 1], src_root).is_err());
        // trailing garbage: one byte past a well-formed stream.
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(dst.install(&trailing, src_root).is_err());
        // forged node count: more entries than the buffer could hold —
        // rejected before any allocation.
        let mut forged = bytes.clone();
        forged[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(dst.install(&forged, src_root).is_err());
        // a zero-tag node has no valid encoding (empty means absent).
        let mut zero_tags = bytes.clone();
        let tag_count_at = 8 + 8 + 32; // node count + key len + key bytes
        zero_tags[tag_count_at..tag_count_at + 8].copy_from_slice(&0u64.to_le_bytes());
        assert!(dst.install(&zero_tags, src_root).is_err());

        assert_eq!(dst.root(), before, "failed installs left the target as-is");
    }

    #[test]
    fn empty_snapshot_installs_onto_an_empty_registry() {
        let src = ungated();
        assert_eq!(src.root(), StateRoot::ZERO);
        let bytes = src.snapshot();
        assert_eq!(
            bytes,
            [0u64.to_le_bytes(), 0u64.to_le_bytes()].concat(),
            "an empty registry is two zero counts (announcements, classes)"
        );

        let mut dst = ungated();
        dst.install(&bytes, StateRoot::ZERO).unwrap();
        assert_eq!(dst.root(), StateRoot::ZERO);
    }

    #[test]
    fn resources_are_stored_queryable_and_move_the_root() {
        let mut c = ungated();
        let me = vec![30u8; 32];
        let mut ctx = ctx_external(&me);
        futures::executor::block_on(c.execute(&mut ctx, &announce_with(&["codex"], &[]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        let tags_only_root = c.root();

        futures::executor::block_on(c.execute(
            &mut ctx,
            &announce_with(&["codex"], &[("cores", 8), ("mem_gb", 32)]),
        ))
        .unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_ne!(
            c.root(),
            tags_only_root,
            "resources are part of the commitment"
        );

        let reply =
            futures::executor::block_on(c.query(&encode_query(&CapabilityQuery::Resources {
                node: me.clone(),
            })))
            .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Resources(r) => {
                assert_eq!(r.get("cores"), Some(&8));
                assert_eq!(r.get("mem_gb"), Some(&32));
            }
            other => panic!("expected Resources reply, got {other:?}"),
        }
    }

    // ---- capability classes -------------------------------------------------

    fn claim(class: &str) -> Msg {
        Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::ClaimClass {
                class: class.into(),
            }),
        }
    }
    fn module_ctx(id: &str) -> TestCtx {
        ctx_origin(sdk::Origin::Module(id.into()))
    }
    fn class_owner(c: &CapabilityRegistry, class: &str) -> Option<ModuleId> {
        let reply =
            futures::executor::block_on(c.query(&encode_query(&CapabilityQuery::ResolveClass {
                class: class.into(),
            })))
            .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::ClassOwner(owner) => owner,
            other => panic!("expected ClassOwner reply, got {other:?}"),
        }
    }
    fn classes(c: &CapabilityRegistry) -> Vec<(String, ModuleId)> {
        let reply =
            futures::executor::block_on(c.query(&encode_query(&CapabilityQuery::Classes))).unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Classes(classes) => classes,
            other => panic!("expected Classes reply, got {other:?}"),
        }
    }

    #[test]
    fn claim_class_stages_then_commits_and_moves_root() {
        let mut c = ungated();
        assert_eq!(class_owner(&c, "agent"), None);

        futures::executor::block_on(c.execute(&mut module_ctx("dispatch"), &claim("agent")))
            .unwrap();
        // staged, not committed: root still ZERO, but read-your-writes sees it.
        assert_eq!(c.root(), StateRoot::ZERO, "root reflects committed only");
        assert_eq!(class_owner(&c, "agent"), Some("dispatch".into()));

        futures::executor::block_on(c.commit_block()).unwrap();
        assert_ne!(c.root(), StateRoot::ZERO, "a committed claim moves root");
        assert_eq!(class_owner(&c, "agent"), Some("dispatch".into()));

        // a second class from another module; Classes enumerates both, sorted.
        futures::executor::block_on(c.execute(&mut module_ctx("saga"), &claim("ai"))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(
            classes(&c),
            vec![
                ("agent".to_string(), "dispatch".to_string()),
                ("ai".to_string(), "saga".to_string()),
            ]
        );
    }

    #[test]
    fn first_claim_wins_reclaim_is_idempotent_rival_rejects() {
        let mut c = ungated();
        futures::executor::block_on(c.execute(&mut module_ctx("dispatch"), &claim("agent")))
            .unwrap();

        // a rival claim in the SAME block reads the stage (read-your-writes)
        // and rejects — first claim wins at stage time, not commit time.
        let err = futures::executor::block_on(c.execute(&mut module_ctx("saga"), &claim("agent")))
            .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("already claimed")),
            "got {err:?}"
        );

        futures::executor::block_on(c.commit_block()).unwrap();
        let committed = c.root();

        // a rival claim on the COMMITTED class rejects identically...
        let err = futures::executor::block_on(c.execute(&mut module_ctx("saga"), &claim("agent")))
            .unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("already claimed")),
            "got {err:?}"
        );
        // ...while a re-claim by the OWNER is an idempotent no-op.
        futures::executor::block_on(c.execute(&mut module_ctx("dispatch"), &claim("agent")))
            .unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(c.root(), committed, "an idempotent re-claim holds the root");
        assert_eq!(class_owner(&c, "agent"), Some("dispatch".into()));
    }

    #[test]
    fn claim_class_rejects_external_and_system_origins() {
        let mut c = ungated();
        for origin in [
            sdk::Origin::External(vec![1u8; 32]),
            sdk::Origin::External(Vec::new()),
            sdk::Origin::System,
        ] {
            let mut ctx = ctx_origin(origin);
            let err =
                futures::executor::block_on(c.execute(&mut ctx, &claim("agent"))).unwrap_err();
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("module that serves it")),
                "got {err:?}"
            );
        }
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(c.root(), StateRoot::ZERO, "nothing was staged");
    }

    #[test]
    fn malformed_classes_are_rejected() {
        let mut c = ungated();
        let too_long = "c".repeat(MAX_CLASS_LEN + 1);
        for bad in [
            "",
            too_long.as_str(),
            "Agent",
            "ag:ent",
            "ag/ent",
            "ag.ent",
            "ag_ent",
        ] {
            let err =
                futures::executor::block_on(c.execute(&mut module_ctx("dispatch"), &claim(bad)))
                    .unwrap_err();
            assert!(matches!(err, Error::Module(_)), "got {err:?} for {bad:?}");
        }
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(c.root(), StateRoot::ZERO, "rejected claims staged nothing");
    }

    #[test]
    fn abort_drops_staged_claims() {
        let mut c = ungated();
        futures::executor::block_on(c.execute(&mut module_ctx("dispatch"), &claim("agent")))
            .unwrap();
        futures::executor::block_on(c.abort_block()).unwrap();
        assert_eq!(class_owner(&c, "agent"), None, "the stage is gone");
        assert_eq!(c.root(), StateRoot::ZERO, "root unchanged after abort");

        // the class is claimable again — the aborted claim never bound it.
        futures::executor::block_on(c.execute(&mut module_ctx("saga"), &claim("agent"))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(class_owner(&c, "agent"), Some("saga".into()));
    }

    #[test]
    fn class_snapshot_round_trip_reconstructs_root_claims_and_announcements() {
        let mut src = ungated();
        let node = vec![30u8; 32];
        futures::executor::block_on(async {
            src.execute(&mut ctx_external(&node), &announce(&["codex"]))
                .await
                .unwrap();
            src.execute(&mut module_ctx("dispatch"), &claim("agent"))
                .await
                .unwrap();
            src.execute(&mut module_ctx("saga"), &claim("ai"))
                .await
                .unwrap();
            src.commit_block().await.unwrap();
        });
        let src_root = src.root();

        // the snapshot IS the root preimage, classes included.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        // install must adopt BOTH sections and drop a stale staged claim.
        let mut dst = ungated();
        futures::executor::block_on(dst.execute(&mut module_ctx("other"), &claim("stale")))
            .unwrap();
        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root);
        assert_eq!(node_tags(&dst, &node), vec!["codex"]);
        assert_eq!(classes(&dst), classes(&src));
        assert_eq!(class_owner(&dst, "stale"), None, "stale stage dropped");
    }

    #[test]
    fn class_only_state_has_a_nonzero_root_and_round_trips() {
        // classes alone (no announcements) must be committed to by the root —
        // an empty node section with claims is NOT the ZERO sentinel.
        let mut src = ungated();
        futures::executor::block_on(src.execute(&mut module_ctx("dispatch"), &claim("agent")))
            .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        assert_ne!(src.root(), StateRoot::ZERO);

        let mut dst = ungated();
        dst.install(&src.snapshot(), src.root()).unwrap();
        assert_eq!(dst.root(), src.root());
        assert_eq!(class_owner(&dst, "agent"), Some("dispatch".into()));
    }

    #[test]
    fn unsorted_or_duplicate_class_sections_are_rejected() {
        // hand-build a snapshot whose class section violates the strict
        // ordering: 0 nodes, then "b" before "a" (and the duplicate case) —
        // a peer cannot mint alternative encodings for one state.
        let build = |names: [&str; 2]| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&0u64.to_le_bytes()); // node count
            bytes.extend_from_slice(&2u64.to_le_bytes()); // class count
            for name in names {
                codec::push_str(&mut bytes, name);
                codec::push_str(&mut bytes, "dispatch");
            }
            bytes
        };
        let mut dst = ungated();
        for bad in [build(["b", "a"]), build(["a", "a"])] {
            let err = dst.install(&bad, StateRoot::ZERO).unwrap_err();
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("strictly increasing")),
                "got {err:?}"
            );
        }
        assert_eq!(dst.root(), StateRoot::ZERO, "failed installs left no trace");
    }

    #[test]
    fn capable_providers_filters_per_dimension_and_absent_is_not_infinite() {
        let mut c = ungated();
        let big = vec![31u8; 32];
        let small = vec![32u8; 32];
        let bare = vec![33u8; 32];
        futures::executor::block_on(async {
            c.execute(
                &mut ctx_external(&big),
                &announce_with(&["codex"], &[("cores", 16), ("mem_gb", 64)]),
            )
            .await
            .unwrap();
            c.execute(
                &mut ctx_external(&small),
                &announce_with(&["codex"], &[("cores", 4), ("mem_gb", 8)]),
            )
            .await
            .unwrap();
            // tags-only node (direct mode): never matches ANY demand.
            c.execute(&mut ctx_external(&bare), &announce_with(&["codex"], &[]))
            .await
            .unwrap();
            c.commit_block().await.unwrap();
        });

        assert_eq!(capable(&c, "codex", &[("cores", 8)]), vec![big.clone()]);
        // empty demands degrade to plain Providers (all three).
        assert_eq!(capable(&c, "codex", &[]).len(), 3);
        // a dimension nobody announced matches nobody.
        assert!(capable(&c, "codex", &[("gpu", 1)]).is_empty());
        // both dimensions must hold.
        assert_eq!(
            capable(&c, "codex", &[("cores", 4), ("mem_gb", 32)]),
            vec![big]
        );
    }

    #[test]
    fn resources_without_capabilities_reject_and_malformed_resources_reject() {
        let mut c = ungated();
        let me = vec![34u8; 32];
        let mut ctx = ctx_external(&me);
        // capacity with nothing to execute is meaningless — reject loudly.
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &announce_with(&[], &[("cores", 8)])))
                .unwrap_err();
        assert!(matches!(err, Error::Module(_)), "got {err:?}");
        // zero value / bad key reject via validate_resources.
        assert!(
            futures::executor::block_on(
                c.execute(&mut ctx, &announce_with(&["codex"], &[("cores", 0)]))
            )
            .is_err()
        );
    }

    #[test]
    fn snapshot_round_trip_carries_resources() {
        let mut src = ungated();
        let a = vec![35u8; 32];
        futures::executor::block_on(src.execute(
            &mut ctx_external(&a),
            &announce_with(&["codex"], &[("cores", 8)]),
        ))
        .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src.root());

        let mut dst = ungated();
        dst.install(&bytes, src.root()).unwrap();
        assert_eq!(dst.root(), src.root());
        assert_eq!(capable(&dst, "codex", &[("cores", 8)]), vec![a]);
    }
}

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
