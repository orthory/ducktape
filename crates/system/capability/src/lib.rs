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

use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

/// most tags a single node may announce. a bound, not a schema: it exists so
/// one announcement cannot bloat replicated state, while staying far above
/// any real host's executor count.
const MAX_CAPABILITIES: usize = 64;

pub struct CapabilityRegistry {
    id: ModuleId,
    /// the valset module consulted to gate announcements to current members;
    /// `None` runs ungated (the single-node daemon carries no valset).
    valset_id: Option<ModuleId>,
    /// committed registry — what `root()` and the app-hash commit to. a node
    /// key never maps to an empty set: empty means absent.
    announced: BTreeMap<Vec<u8>, BTreeSet<String>>,
    /// per-block staged replacements: the set stages a full declarative
    /// replace, an EMPTY set stages a removal. read ahead of `announced`
    /// (read-your-writes), merged into committed state only on `commit_block`.
    pending: BTreeMap<Vec<u8>, BTreeSet<String>>,
}

impl CapabilityRegistry {
    pub fn new(id: impl Into<ModuleId>, valset_id: Option<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset_id,
            announced: BTreeMap::new(),
            pending: BTreeMap::new(),
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
    /// the valset module's staged-over-committed projection — exactly
    /// identity's `BindNode` gate: an announce is admitted for either
    /// standing, so a joined (not-yet-promoted) node can publish the
    /// executors it really provides.
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

    /// the committed registry with this block's staged replacements applied —
    /// read-your-writes; an empty staged set reads as absent.
    fn effective(&self) -> BTreeMap<Vec<u8>, BTreeSet<String>> {
        let mut map = self.announced.clone();
        for (key, set) in &self.pending {
            if set.is_empty() {
                map.remove(key);
            } else {
                map.insert(key.clone(), set.clone());
            }
        }
        map
    }

    // ---- state-sync ---------------------------------------------------------
    // ship the committed registry as its root preimage; adopt a peer's bytes
    // only after re-deriving the root consensus expects — the root, not the
    // peer, is the trust anchor.

    /// canonical bytes of the COMMITTED registry — exactly the byte stream
    /// `root()` hashes: node count u64-le, then per sorted node key its len
    /// u64-le + key bytes + tag count u64-le, then per sorted tag its len
    /// u64-le + utf-8 bytes. for a non-empty registry
    /// `sha256(snapshot()) == root()`; an empty registry snapshots to a lone
    /// zero count (whose root is still `ZERO`, unhashed). pending is
    /// deliberately excluded — a snapshot ships what consensus committed to.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::snapshot_of(&self.announced)
    }

    fn snapshot_of(map: &BTreeMap<Vec<u8>, BTreeSet<String>>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(map.len() as u64).to_le_bytes());
        for (key, tags) in map {
            out.extend_from_slice(&(key.len() as u64).to_le_bytes());
            out.extend_from_slice(key);
            out.extend_from_slice(&(tags.len() as u64).to_le_bytes());
            for tag in tags {
                out.extend_from_slice(&(tag.len() as u64).to_le_bytes());
                out.extend_from_slice(tag.as_bytes());
            }
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
        let decoded = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&decoded);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.announced = decoded;
        self.pending.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves
    /// them). every count and length is checked against the remaining buffer
    /// BEFORE any allocation, truncation and trailing bytes both reject, node
    /// keys and tags must arrive strictly increasing, and a node with zero
    /// tags rejects (empty means absent, so it has no encoding) — a given
    /// registry has exactly one valid byte stream, so a peer cannot mint
    /// alternative encodings for one state.
    fn decode_snapshot(bytes: &[u8]) -> Result<BTreeMap<Vec<u8>, BTreeSet<String>>, Error> {
        fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
            let Some((head, rest)) = (*buf).split_first_chunk::<8>() else {
                return Err(Error::Module("snapshot truncated".into()));
            };
            *buf = rest;
            Ok(u64::from_le_bytes(*head))
        }

        let mut buf = bytes;
        let count = take_u64(&mut buf)?;
        // each node entry costs at least its 8-byte key-length prefix plus an
        // 8-byte tag count, so a count the remaining bytes cannot possibly
        // hold is rejected up front — a forged count never drives allocation.
        if count > (buf.len() / 16) as u64 {
            return Err(Error::Module(format!(
                "snapshot node count {count} exceeds the {} remaining bytes",
                buf.len()
            )));
        }
        let mut map = BTreeMap::new();
        let mut prev_key: Option<&[u8]> = None;
        for _ in 0..count {
            let key_len = take_u64(&mut buf)?;
            if key_len > buf.len() as u64 {
                return Err(Error::Module(format!(
                    "snapshot key length {key_len} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let (key, rest) = buf.split_at(key_len as usize);
            buf = rest;
            if prev_key.is_some_and(|p| p >= key) {
                return Err(Error::Module(
                    "snapshot node keys must be strictly increasing".into(),
                ));
            }
            prev_key = Some(key);

            let tag_count = take_u64(&mut buf)?;
            if tag_count == 0 {
                return Err(Error::Module(
                    "snapshot node with zero capabilities (empty means absent)".into(),
                ));
            }
            // each tag costs at least its 8-byte length prefix.
            if tag_count > (buf.len() / 8) as u64 {
                return Err(Error::Module(format!(
                    "snapshot tag count {tag_count} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut tags = BTreeSet::new();
            let mut prev_tag: Option<&[u8]> = None;
            for _ in 0..tag_count {
                let tag_len = take_u64(&mut buf)?;
                if tag_len > buf.len() as u64 {
                    return Err(Error::Module(format!(
                        "snapshot tag length {tag_len} exceeds the {} remaining bytes",
                        buf.len()
                    )));
                }
                let (tag, rest) = buf.split_at(tag_len as usize);
                buf = rest;
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
            map.insert(key.to_vec(), tags);
        }
        if !buf.is_empty() {
            return Err(Error::Module(format!(
                "snapshot carries {} trailing bytes",
                buf.len()
            )));
        }
        Ok(map)
    }

    /// the state-based commitment for `map`: `ZERO` when empty, else sha256
    /// over exactly the bytes `snapshot` emits. shared by `root()` (committed
    /// state) and `install` (a decoded candidate), so the two can never drift.
    fn root_of(map: &BTreeMap<Vec<u8>, BTreeSet<String>>) -> StateRoot {
        if map.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::snapshot_of(map)).into())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for CapabilityRegistry {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED registry: a length-prefixed
    /// sha256 over the sorted node -> tags map. order-independent (BTreeMap /
    /// BTreeSet) and idempotent. an empty registry reports `ZERO`.
    fn root(&self) -> StateRoot {
        Self::root_of(&self.announced)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // identity comes from the verified submit origin, never the payload —
        // a node can only announce for itself, which keeps the registry
        // truthful by construction. module/system origins have no host of
        // their own to speak for.
        let node = match &ctx.env().origin {
            sdk::Origin::External(key) => key.clone(),
            other => {
                return Err(Error::Module(format!(
                    "capability announcements require an external submitter, got {other:?}"
                )));
            }
        };
        if let Some(valset_id) = self.valset_id.clone() {
            // member-gated like identity's BindNode: validators UNION
            // residents populate the registry, so lookups resolve to known
            // peers — including a joined node that has not been promoted yet.
            if !self.members(ctx, &valset_id).await?.contains(&node) {
                return Err(Error::Module(
                    "capability announcer holds no current standing (validator or resident)".into(),
                ));
            }
        }
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            CapabilityMsg::Announce { capabilities } => {
                let tags = Self::validate_tags(capabilities)?;
                // declarative replace: the last announcement staged in a block
                // wins, and an empty set stages a removal.
                self.pending.insert(node, tags);
            }
        }
        Ok(())
    }

    /// read projection — the committed registry plus this block's staged
    /// replacements.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let view = self.effective();
        Ok(match decode_query(req).map_err(Error::Module)? {
            CapabilityQuery::Providers { capability } => {
                let providers = view
                    .iter()
                    .filter(|(_, tags)| tags.contains(&capability))
                    .map(|(key, _)| key.clone())
                    .collect();
                encode_reply(&CapabilityReply::Providers(providers))
            }
            CapabilityQuery::Node { node } => {
                let tags = view
                    .get(&node)
                    .map(|t| t.iter().cloned().collect())
                    .unwrap_or_default();
                encode_reply(&CapabilityReply::Node(tags))
            }
            CapabilityQuery::All => {
                let all = view
                    .into_iter()
                    .map(|(key, tags)| (key, tags.into_iter().collect()))
                    .collect();
                encode_reply(&CapabilityReply::All(all))
            }
        })
    }

    /// merge the block's staged replacements into committed state — `root()`
    /// now reflects them. no-op if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        for (key, tags) in std::mem::take(&mut self.pending) {
            if tags.is_empty() {
                self.announced.remove(&key);
            } else {
                self.announced.insert(key, tags);
            }
        }
        Ok(())
    }

    /// discard the block's staged replacements — committed state (and
    /// `root()`) is unchanged, so a failed block leaves no trace.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_TAG_LEN, encode_msg, encode_query};
    use valset::encode_reply as valset_encode_reply;

    /// a minimal Ctx: origin-configurable, and (optionally) answers BOTH the
    /// valset Validators and Residents queries so the member gate is testable
    /// for either standing (mirrors identity's test stub).
    struct TestCtx {
        env: sdk::Env,
        members: Option<Vec<Vec<u8>>>,
        residents: Option<Vec<Vec<u8>>>,
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
                    me: "capability".into(),
                },
                members: None,
                residents: None,
            }
        }
        fn gated(key: &[u8], validators: Vec<Vec<u8>>, residents: Vec<Vec<u8>>) -> Self {
            let mut ctx = Self::external(key);
            ctx.members = Some(validators);
            ctx.residents = Some(residents);
            ctx
        }
        fn with_members(key: &[u8], members: Vec<Vec<u8>>) -> Self {
            Self::gated(key, members, Vec::new())
        }
        fn with_residents(key: &[u8], residents: Vec<Vec<u8>>) -> Self {
            Self::gated(key, Vec::new(), residents)
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
            match (q, &self.members, &self.residents) {
                (ValsetQuery::Validators, Some(m), _) => {
                    Ok(valset_encode_reply(&ValsetReply::Validators(m.clone())))
                }
                (ValsetQuery::Residents, _, Some(o)) => {
                    Ok(valset_encode_reply(&ValsetReply::Residents(o.clone())))
                }
                _ => Err(Error::QueryUnsupported),
            }
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }

    fn announce(tags: &[&str]) -> Msg {
        Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::Announce {
                capabilities: tags.iter().map(|t| t.to_string()).collect(),
            }),
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
    fn announce_registers_and_moves_root_off_zero() {
        let mut c = ungated();
        let me = vec![1u8; 32];
        let mut ctx = TestCtx::external(&me);
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
        let mut ctx = TestCtx::external(&me);
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
        let mut ctx = TestCtx::external(&me);
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
        let mut ctx = TestCtx::external(&me);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex", "codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(node_tags(&c, &me), vec!["codex"], "set semantics");
    }

    #[test]
    fn non_external_origins_are_rejected() {
        let mut c = ungated();
        for origin in [sdk::Origin::Module("agent".into()), sdk::Origin::System] {
            let mut ctx = TestCtx::with_origin(origin);
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
    fn member_gate_rejects_non_members_and_admits_members() {
        let member = vec![5u8; 32];
        let outsider = vec![6u8; 32];
        let mut c = CapabilityRegistry::new("capability", Some("valset".into()));

        let mut ctx = TestCtx::with_members(&outsider, vec![member.clone()]);
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("no current standing")),
            "got {err:?}"
        );

        let mut ctx = TestCtx::with_members(&member, vec![member.clone()]);
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
        let mut ctx = TestCtx::gated(&resident, vec![validator.clone()], vec![resident.clone()]);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["codex"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(providers(&c, "codex"), vec![resident.clone()]);

        // a key with NEITHER standing is still rejected, even alongside a
        // populated resident set.
        let mut ctx = TestCtx::gated(&outsider, vec![validator.clone()], vec![resident.clone()]);
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &announce(&["claude"]))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("no current standing")),
            "got {err:?}"
        );

        // resident-only standing (no validator overlap) also admits — the
        // gate is a true union, not an intersection.
        let mut ctx = TestCtx::with_residents(&resident, vec![resident.clone()]);
        futures::executor::block_on(c.execute(&mut ctx, &announce(&["claude"]))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(providers(&c, "claude"), vec![resident]);
    }

    #[test]
    fn malformed_tags_are_rejected() {
        let mut c = ungated();
        let me = vec![7u8; 32];
        let mut ctx = TestCtx::external(&me);
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
            c1.execute(&mut TestCtx::external(&a), &announce(&["codex"]))
                .await
                .unwrap();
            c1.execute(&mut TestCtx::external(&b), &announce(&["claude"]))
                .await
                .unwrap();
            c1.commit_block().await.unwrap();
        });

        // same registry contents, announced in the opposite order.
        let mut c2 = ungated();
        futures::executor::block_on(async {
            c2.execute(&mut TestCtx::external(&b), &announce(&["claude"]))
                .await
                .unwrap();
            c2.execute(&mut TestCtx::external(&a), &announce(&["codex"]))
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
        let mut ctx = TestCtx::external(&me);
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
            src.execute(&mut TestCtx::external(&a), &announce(&["codex", "claude"]))
                .await
                .unwrap();
            src.execute(&mut TestCtx::external(&b), &announce(&["codex"]))
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
            dst.execute(&mut TestCtx::external(&[13u8; 32]), &announce(&["other"])),
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
        futures::executor::block_on(src.execute(&mut TestCtx::external(&a), &announce(&["codex"])))
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
        futures::executor::block_on(
            dst.execute(&mut TestCtx::external(&b), &announce(&["claude"])),
        )
        .unwrap();
        futures::executor::block_on(dst.commit_block()).unwrap();
        futures::executor::block_on(dst.execute(&mut TestCtx::external(&b), &announce(&["other"])))
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
            src.execute(&mut TestCtx::external(&[16u8; 32]), &announce(&["codex"])),
        )
        .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();
        let bytes = src.snapshot();

        let mut dst = ungated();
        let before = dst.root();
        // truncation: the final tag byte is missing.
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
            0u64.to_le_bytes().to_vec(),
            "an empty registry is a lone zero count"
        );

        let mut dst = ungated();
        dst.install(&bytes, StateRoot::ZERO).unwrap();
        assert_eq!(dst.root(), StateRoot::ZERO);
    }
}
