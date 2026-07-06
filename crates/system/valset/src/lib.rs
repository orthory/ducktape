//! the ed25519 membership registry as replicated state: validators + observers.
//!
//! a validator is a 32-byte ed25519 public key. anyone holding a WELL-FORMED
//! ed25519 key may [`ValsetMsg::Join`] the set — no authorization, no gating,
//! no stake weighting. this is deliberately permissionless: per the design,
//! "permissionless joining suffices; don't concern with proper shares." real
//! governance (who may join) and stake-weighted shares (voting power) are
//! DEFERRED — this module only replicates *membership*.
//!
//! ## observers (staged admission, protocol >= 3)
//!
//! an OBSERVER holds mesh + statesync standing but no quorum seat — the tier
//! a joiner syncs in before promotion, so the consensus set only ever gains a
//! caught-up validator. [`ValsetMsg::Grant`] / [`ValsetMsg::Revoke`] manage
//! the set; a [`ValsetMsg::Join`] on a current observer PROMOTES it (adds the
//! validator, removes the observer, one block). both new ops REJECT below
//! protocol version 3 — byte-for-byte the outcome an older binary's
//! unknown-variant decode produces, so a mixed-binary net cannot fork on them.
//!
//! ## root/snapshot compatibility
//!
//! the root preimage and snapshot append the observer section ONLY when the
//! observer set is non-empty: every pre-observer state (and every state that
//! never grants) keeps its exact historical root and snapshot bytes — no
//! migration, no dual-path root.
//!
//! state model mirrors the directory module's host-lent staging seam:
//! `execute` STAGES into a `pending` overlay (committed state untouched);
//! `query` reads pending-over-committed (read-your-writes); `commit_block`
//! merges pending into committed; `abort_block` drops pending; `root()`
//! reflects COMMITTED state only — a state-based (sorted, length-prefixed)
//! sha256 over the validator set, so it is order-independent and idempotent.
//!
//! ## state-sync
//!
//! a joiner rebuilds this module from a peer via [`Valset::snapshot`] /
//! [`Valset::install`]. the snapshot is the exact preimage of `root()`, so the
//! joiner needs no trust in the serving peer: install recomputes the root of
//! whatever bytes arrived and refuses to adopt them unless it matches the
//! expected root consensus already agreed on.

use std::collections::{BTreeMap, BTreeSet};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519::PublicKey;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset_interface::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_msg, decode_query, encode_reply,
};

/// a 32-byte ed25519 public key encoding.
const KEY_LEN: usize = 32;

/// the protocol version that introduces the observer tier. `Grant`/`Revoke`
/// reject below it — the same deterministic reject an older binary's
/// unknown-variant decode produces, so mixed-binary nets cannot fork.
const OBSERVER_VERSION: u32 = 3;

pub struct Valset {
    id: ModuleId,
    /// committed membership — what `root()` and the app-hash commit to.
    validators: BTreeSet<Vec<u8>>,
    /// membership changes staged during the current block: `true` == staged add,
    /// `false` == staged remove. read ahead of `validators` (read-your-writes),
    /// merged into committed state (and `root()`) only on `commit_block`.
    pending: BTreeMap<Vec<u8>, bool>,
    /// committed OBSERVER standing (mesh + statesync, no quorum seat). folded
    /// into `root()`/`snapshot()` only when non-empty — see the module doc's
    /// compatibility note.
    observers: BTreeSet<Vec<u8>>,
    /// observer changes staged during the current block, same discipline as
    /// `pending`.
    pending_observers: BTreeMap<Vec<u8>, bool>,
}

impl Valset {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            validators: BTreeSet::new(),
            pending: BTreeMap::new(),
            observers: BTreeSet::new(),
            pending_observers: BTreeMap::new(),
        }
    }

    /// direct sync add (handy for genesis seeding / tests). does NOT validate —
    /// callers seeding genesis are trusted; the `execute(Join)` path validates.
    pub fn insert(&mut self, key: Vec<u8>) {
        self.validators.insert(key);
    }

    /// validate that `key` is a well-formed 32-byte ed25519 public key. the
    /// explicit length guard makes the 32-byte invariant independent of decode's
    /// trailing-byte behavior; `PublicKey::decode` then checks the curve point
    /// (ZIP215: must decompress to a point on the twisted Edwards curve).
    fn validate_key(key: &[u8]) -> Result<(), Error> {
        if key.len() != KEY_LEN {
            return Err(Error::Module(format!(
                "invalid ed25519 public key: expected {KEY_LEN} bytes, got {}",
                key.len()
            )));
        }
        PublicKey::decode(key)
            .map_err(|e| Error::Module(format!("invalid ed25519 public key: {e}")))?;
        Ok(())
    }

    /// stage an add for this block (read-your-writes; committed on `commit_block`).
    fn stage_add(&mut self, key: Vec<u8>) {
        self.pending.insert(key, true);
    }

    /// stage a remove for this block.
    fn stage_remove(&mut self, key: Vec<u8>) {
        self.pending.insert(key, false);
    }

    /// the committed validator set with this block's staged changes applied —
    /// read-your-writes, sorted (order-independent).
    fn effective(&self) -> Vec<Vec<u8>> {
        Self::overlay(&self.validators, &self.pending)
    }

    /// the COMMITTED membership pair — `(validators, observers)`, sorted.
    /// the post-install/rebuild witness (both classes must survive a sync
    /// byte-for-byte); reads inside a block use the staged projections.
    pub fn membership(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        (
            self.validators.iter().cloned().collect(),
            self.observers.iter().cloned().collect(),
        )
    }

    /// the committed observer set with this block's staged changes applied.
    fn effective_observers(&self) -> Vec<Vec<u8>> {
        Self::overlay(&self.observers, &self.pending_observers)
    }

    /// committed-plus-staged projection shared by both sets.
    fn overlay(committed: &BTreeSet<Vec<u8>>, staged: &BTreeMap<Vec<u8>, bool>) -> Vec<Vec<u8>> {
        let mut set: BTreeSet<Vec<u8>> = committed.clone();
        for (k, present) in staged {
            if *present {
                set.insert(k.clone());
            } else {
                set.remove(k);
            }
        }
        set.into_iter().collect()
    }

    // ---- state-sync ---------------------------------------------------------
    // ship the committed set as its root preimage; adopt a peer's bytes only
    // after re-deriving the root consensus expects — the root, not the peer, is
    // the trust anchor.

    /// canonical bytes of the COMMITTED state — exactly the byte stream `root()`
    /// hashes: the validator section (count u64-le, then per sorted key its len
    /// u64-le + key bytes), then — ONLY when the observer set is non-empty — the
    /// observer section in the same shape. the conditional append is the
    /// compatibility invariant: with no observers this is byte-for-byte the
    /// historical validators-only stream, so every pre-observer state keeps its
    /// exact root. so for non-empty state `sha256(snapshot()) == root()`; fully
    /// empty state snapshots to a lone zero count (root still `ZERO`, unhashed).
    /// pending is deliberately excluded — a snapshot ships what consensus
    /// committed to, and staged-but-uncommitted changes are not that.
    pub fn snapshot(&self) -> Vec<u8> {
        let sized = |set: &BTreeSet<Vec<u8>>| 8 + set.iter().map(|k| 8 + k.len()).sum::<usize>();
        let mut out = Vec::with_capacity(
            sized(&self.validators) + if self.observers.is_empty() { 0 } else { sized(&self.observers) },
        );
        let section = |out: &mut Vec<u8>, set: &BTreeSet<Vec<u8>>| {
            out.extend_from_slice(&(set.len() as u64).to_le_bytes());
            for k in set {
                out.extend_from_slice(&(k.len() as u64).to_le_bytes());
                out.extend_from_slice(k);
            }
        };
        section(&mut out, &self.validators);
        if !self.observers.is_empty() {
            section(&mut out, &self.observers);
        }
        out
    }

    /// replace committed state with a decoded snapshot, iff the decoded state's
    /// recomputed root equals `expected`. decode and verification land in a
    /// temporary: self is mutated only after both pass, so on any `Err` committed
    /// state, pending, and `root()` are byte-identical to before the call.
    /// success clears pending — staged changes belong to the state being
    /// replaced, not the state being adopted.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (validators, observers) = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&validators, &observers);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.validators = validators;
        self.observers = observers;
        self.pending.clear();
        self.pending_observers.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves them):
    /// the validator section, then an OPTIONAL observer section (present iff
    /// bytes remain — the encoder omits it when empty, and an explicit empty
    /// section is rejected so a given state has exactly one valid encoding).
    /// the count and every key length are checked against the remaining buffer
    /// BEFORE any allocation, truncation and trailing bytes both reject, and
    /// keys must arrive strictly increasing per section — a peer cannot mint
    /// alternative byte streams for one state.
    #[allow(clippy::type_complexity)]
    fn decode_snapshot(bytes: &[u8]) -> Result<(BTreeSet<Vec<u8>>, BTreeSet<Vec<u8>>), Error> {
        fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
            let Some((head, rest)) = (*buf).split_first_chunk::<8>() else {
                return Err(Error::Module("snapshot truncated".into()));
            };
            *buf = rest;
            Ok(u64::from_le_bytes(*head))
        }

        fn take_section(buf: &mut &[u8]) -> Result<BTreeSet<Vec<u8>>, Error> {
            let count = take_u64(buf)?;
            // each entry costs at least its 8-byte length prefix, so a count the
            // remaining bytes cannot possibly hold is rejected up front — a forged
            // count never drives allocation.
            if count > (buf.len() / 8) as u64 {
                return Err(Error::Module(format!(
                    "snapshot count {count} exceeds the {} remaining bytes",
                    buf.len()
                )));
            }
            let mut set = BTreeSet::new();
            let mut prev: Option<Vec<u8>> = None;
            for _ in 0..count {
                let len = take_u64(buf)?;
                if len > buf.len() as u64 {
                    return Err(Error::Module(format!(
                        "snapshot key length {len} exceeds the {} remaining bytes",
                        buf.len()
                    )));
                }
                let (key, rest) = buf.split_at(len as usize);
                *buf = rest;
                if prev.as_deref().is_some_and(|p| p >= key) {
                    return Err(Error::Module(
                        "snapshot keys must be strictly increasing".into(),
                    ));
                }
                prev = Some(key.to_vec());
                set.insert(key.to_vec());
            }
            Ok(set)
        }

        let mut buf = bytes;
        let validators = take_section(&mut buf)?;
        let observers = if buf.is_empty() {
            BTreeSet::new()
        } else {
            let observers = take_section(&mut buf)?;
            if observers.is_empty() {
                // the encoder omits an empty observer section — an explicit
                // zero count would be a second encoding of the same state.
                return Err(Error::Module(
                    "snapshot carries an explicit empty observer section".into(),
                ));
            }
            observers
        };
        if !buf.is_empty() {
            return Err(Error::Module(format!(
                "snapshot carries {} trailing bytes",
                buf.len()
            )));
        }
        Ok((validators, observers))
    }

    /// the state-based commitment: `ZERO` when both sets are empty, else sha256
    /// over exactly the bytes `snapshot` emits (observer section only when
    /// non-empty — the compatibility invariant). shared by `root()` (committed
    /// state) and `install` (a decoded candidate), so the two can never drift.
    fn root_of(validators: &BTreeSet<Vec<u8>>, observers: &BTreeSet<Vec<u8>>) -> StateRoot {
        if validators.is_empty() && observers.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        let mut section = |set: &BTreeSet<Vec<u8>>| {
            h.update((set.len() as u64).to_le_bytes());
            for k in set {
                h.update((k.len() as u64).to_le_bytes());
                h.update(k);
            }
        };
        section(validators);
        if !observers.is_empty() {
            section(observers);
        }
        StateRoot(h.finalize().into())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Valset {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED state: a length-prefixed
    /// sha256 over the sorted validators (plus the observer section only when
    /// non-empty). order-independent (BTreeSet) and idempotent. fully empty
    /// state reports `ZERO` — an empty/uninitialized module (matching the sdk
    /// `StateRoot::ZERO` doc and forge's unborn-repo root).
    fn root(&self) -> StateRoot {
        Self::root_of(&self.validators, &self.observers)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // membership changes are GOVERNANCE-GATED: only a module origin (the
        // governance module's follow-up after a passing proposal) or a system
        // origin (genesis orchestration) may stage them. an unauthenticated
        // external Leave was a one-message liveness kill on a private network;
        // origin is part of the deterministic Env, so every validator enforces
        // this identically.
        match &ctx.env().origin {
            sdk::Origin::Module(_) | sdk::Origin::System => {}
            sdk::Origin::External(_) => {
                return Err(Error::Module(
                    "valset membership changes only via governance".into(),
                ));
            }
        }
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ValsetMsg::Join { key } => {
                Self::validate_key(&key)?;
                // PROMOTION: a joining observer leaves the observer tier in
                // the same block — one boundary carries the whole transition,
                // and the transport union never double-counts the key.
                if self.effective_observers().contains(&key) {
                    self.pending_observers.insert(key.clone(), false);
                }
                self.stage_add(key);
            }
            ValsetMsg::Leave { key } => {
                // the validator set must NEVER go empty. a downstream orderer
                // reconfigured to zero validators hits commonware `quorum(0)`,
                // which panics ("n must not be zero") and halts the node. refuse
                // a removal that would drop the LAST validator. authoritative
                // here: every membership removal (a governance-passed
                // RemoveValidator or genesis orchestration) funnels through this
                // arm, so the invariant holds no matter who staged it — the set
                // is closed under this rule regardless of the caller.
                let mut after: BTreeSet<Vec<u8>> = self.effective().into_iter().collect();
                after.remove(&key);
                if after.is_empty() {
                    return Err(Error::Module(
                        "refusing to remove the last validator: the set must never be empty".into(),
                    ));
                }
                self.stage_remove(key);
            }
            ValsetMsg::Grant { key } => {
                // the observer tier exists from protocol 3: rejecting below it
                // is byte-for-byte what an older binary's unknown-variant
                // decode does, so a mixed-binary net cannot fork on a Grant.
                if ctx.env().protocol_version < OBSERVER_VERSION {
                    return Err(Error::Module(format!(
                        "observer grants require protocol version {OBSERVER_VERSION}"
                    )));
                }
                Self::validate_key(&key)?;
                // a validator already holds every observer capability; a
                // second standing would only smear the promote/demote edges.
                if self.effective().contains(&key) {
                    return Err(Error::Module(
                        "key is a current validator — observer standing is the pre-promotion tier"
                            .into(),
                    ));
                }
                self.pending_observers.insert(key, true);
            }
            ValsetMsg::Revoke { key } => {
                if ctx.env().protocol_version < OBSERVER_VERSION {
                    return Err(Error::Module(format!(
                        "observer revokes require protocol version {OBSERVER_VERSION}"
                    )));
                }
                self.pending_observers.insert(key, false);
            }
        }
        Ok(())
    }

    /// read projection — the committed sets plus this block's staged changes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ValsetQuery::Validators => Ok(encode_reply(&ValsetReply::Validators(self.effective()))),
            ValsetQuery::Observers => Ok(encode_reply(&ValsetReply::Observers(
                self.effective_observers(),
            ))),
        }
    }

    /// merge the block's staged membership changes into committed state —
    /// `root()` now reflects them. no-op if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        for (k, present) in std::mem::take(&mut self.pending) {
            if present {
                self.validators.insert(k);
            } else {
                self.validators.remove(&k);
            }
        }
        for (k, present) in std::mem::take(&mut self.pending_observers) {
            if present {
                self.observers.insert(k);
            } else {
                self.observers.remove(&k);
            }
        }
        Ok(())
    }

    /// discard the block's staged changes — committed state (and `root()`) is
    /// unchanged, so a failed block leaves no trace.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        self.pending_observers.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;
    use valset_interface::{encode_msg, encode_query};

    // a minimal Ctx — valset's execute reads only env (origin + protocol_version).
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new() -> Self {
            Self::at_version(0)
        }
        /// a ctx whose block runs at `protocol_version` — the observer ops
        /// gate on it, so their tests need a v3 block.
        fn at_version(protocol_version: u32) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version,
                    height: 0,
                    consensus_time: 0,
                    origin: sdk::Origin::System,
                    me: "valset".into(),
                },
            }
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
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::QueryUnsupported)
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }

    // a deterministic, VALID 32-byte ed25519 public key: any 32 bytes is a valid
    // ed25519 seed, and the derived public key is always a valid curve point.
    fn valid_key(seed_byte: u8) -> Vec<u8> {
        let seed = [seed_byte; 32];
        let sk = PrivateKey::decode(&seed[..]).expect("any 32 bytes is a valid seed");
        sk.public_key().as_ref().to_vec()
    }

    fn join(key: &[u8]) -> Msg {
        Msg {
            target: "valset".into(),
            payload: encode_msg(&ValsetMsg::Join { key: key.to_vec() }),
        }
    }
    fn leave(key: &[u8]) -> Msg {
        Msg {
            target: "valset".into(),
            payload: encode_msg(&ValsetMsg::Leave { key: key.to_vec() }),
        }
    }
    fn validators(v: &Valset) -> Vec<Vec<u8>> {
        let reply =
            futures::executor::block_on(v.query(&encode_query(&ValsetQuery::Validators))).unwrap();
        match valset_interface::decode_reply(&reply).unwrap() {
            ValsetReply::Validators(list) => list,
            other => panic!("expected Validators, got {other:?}"),
        }
    }
    fn observers(v: &Valset) -> Vec<Vec<u8>> {
        let reply =
            futures::executor::block_on(v.query(&encode_query(&ValsetQuery::Observers))).unwrap();
        match valset_interface::decode_reply(&reply).unwrap() {
            ValsetReply::Observers(list) => list,
            other => panic!("expected Observers, got {other:?}"),
        }
    }
    fn grant(key: &[u8]) -> Msg {
        Msg {
            target: "valset".into(),
            payload: encode_msg(&ValsetMsg::Grant { key: key.to_vec() }),
        }
    }
    fn revoke(key: &[u8]) -> Msg {
        Msg {
            target: "valset".into(),
            payload: encode_msg(&ValsetMsg::Revoke { key: key.to_vec() }),
        }
    }

    #[test]
    fn join_adds_a_validator_and_moves_root_off_zero() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        assert_eq!(v.root(), StateRoot::ZERO, "genesis set is empty -> ZERO");

        let k = valid_key(1);
        futures::executor::block_on(v.execute(&mut ctx, &join(&k))).unwrap();
        // staged, not yet committed: root still ZERO, but read-your-writes sees it.
        assert_eq!(v.root(), StateRoot::ZERO, "root reflects committed only");
        assert_eq!(
            validators(&v),
            vec![k.clone()],
            "read-your-writes sees the stage"
        );

        futures::executor::block_on(v.commit_block()).unwrap();
        assert_ne!(
            v.root(),
            StateRoot::ZERO,
            "a committed join moves the root off ZERO"
        );
        assert_eq!(validators(&v), vec![k]);
    }

    #[test]
    fn leave_removes_a_validator() {
        // remove ONE of two validators — the set stays non-empty, so the
        // last-validator guard never fires. (removing the last validator is a
        // refused no-op; see `leaving_the_last_validator_is_refused`.)
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let (keep, drop) = (valid_key(2), valid_key(3));
        futures::executor::block_on(v.execute(&mut ctx, &join(&keep))).unwrap();
        futures::executor::block_on(v.execute(&mut ctx, &join(&drop))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        let joined_root = v.root();

        futures::executor::block_on(v.execute(&mut ctx, &leave(&drop))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(validators(&v), vec![keep], "leave removes exactly that key");
        assert_ne!(v.root(), joined_root, "the committed root moved");
        assert_ne!(v.root(), StateRoot::ZERO, "a non-empty set is not ZERO");
    }

    #[test]
    fn leaving_the_last_validator_is_refused() {
        // the set must never go empty: an orderer reconfigured to zero
        // validators hits commonware `quorum(0)`, which panics. removing the
        // SOLE validator is refused deterministically, and the set is untouched.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let solo = valid_key(7);
        futures::executor::block_on(v.execute(&mut ctx, &join(&solo))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        let before = v.root();

        let err = futures::executor::block_on(v.execute(&mut ctx, &leave(&solo))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("last validator")),
            "got {err:?}"
        );
        // read-your-writes: nothing was staged, so the sole validator remains.
        assert_eq!(validators(&v), vec![solo], "the last validator stays");
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(v.root(), before, "committed set is byte-identical");
        assert_ne!(v.root(), StateRoot::ZERO, "the set never went empty");
    }

    #[test]
    fn leaving_the_last_of_a_shrinking_set_is_refused() {
        // stage two leaves in one block: the first (of two) is fine, the second
        // would empty the set within the same block's read-your-writes view and
        // is refused — the guard reads the EFFECTIVE (staged-over-committed) set.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let (a, b) = (valid_key(4), valid_key(5));
        futures::executor::block_on(v.execute(&mut ctx, &join(&a))).unwrap();
        futures::executor::block_on(v.execute(&mut ctx, &join(&b))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();

        futures::executor::block_on(v.execute(&mut ctx, &leave(&a))).unwrap();
        let err = futures::executor::block_on(v.execute(&mut ctx, &leave(&b))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("last validator")),
            "got {err:?}"
        );
    }

    #[test]
    fn malformed_key_is_rejected() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        // wrong length is the deterministic malformed input: ~half of all 32-byte
        // strings are valid curve points (ZIP215 accepts non-canonical), so a
        // wrong-LENGTH key is the reliable reject path.
        let bad = vec![0u8; 16];
        let err = futures::executor::block_on(v.execute(&mut ctx, &join(&bad))).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "malformed key errs with Module"
        );
        futures::executor::block_on(v.commit_block()).unwrap();
        assert!(validators(&v).is_empty(), "a rejected join adds nothing");
        assert_eq!(v.root(), StateRoot::ZERO);
    }

    #[test]
    fn permissionless_any_valid_key_joins() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        // no authorization, no gating: three unrelated valid keys all join.
        for b in [10u8, 20, 30] {
            futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(b)))).unwrap();
        }
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(
            validators(&v).len(),
            3,
            "any valid key joins, permissionlessly"
        );
    }

    #[test]
    fn root_is_state_based_order_independent() {
        let a = valid_key(3);
        let b = valid_key(4);

        let mut v1 = Valset::new("valset");
        let mut c1 = TestCtx::new();
        futures::executor::block_on(v1.execute(&mut c1, &join(&a))).unwrap();
        futures::executor::block_on(v1.execute(&mut c1, &join(&b))).unwrap();
        futures::executor::block_on(v1.commit_block()).unwrap();

        // same two validators, joined in the OPPOSITE order.
        let mut v2 = Valset::new("valset");
        let mut c2 = TestCtx::new();
        futures::executor::block_on(v2.execute(&mut c2, &join(&b))).unwrap();
        futures::executor::block_on(v2.execute(&mut c2, &join(&a))).unwrap();
        futures::executor::block_on(v2.commit_block()).unwrap();

        assert_eq!(v1.root(), v2.root(), "root is f(state), order-independent");
    }

    #[test]
    fn atomicity_a_failed_block_rolls_back_the_join() {
        // reuse the host-lent staging seam directly: stage a join, then the block
        // fails -> abort_block drops the stage. no validator is added, root is
        // byte-identical to its pre-block value.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let before = v.root();

        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(5)))).unwrap();
        // ... a later dispatch in the same block errors, so the host aborts:
        futures::executor::block_on(v.abort_block()).unwrap();

        assert!(validators(&v).is_empty(), "aborted join added no validator");
        assert_eq!(
            v.root(),
            before,
            "root is unchanged after a rolled-back block"
        );
    }

    #[test]
    fn snapshot_install_round_trip_reconstructs_root_and_membership() {
        // SOURCE: three validators through the real execute(Join)+commit_block
        // path, so the snapshot ships state that consensus actually committed.
        let mut src = Valset::new("valset");
        let mut ctx = TestCtx::new();
        for b in [1u8, 2, 3] {
            futures::executor::block_on(src.execute(&mut ctx, &join(&valid_key(b)))).unwrap();
        }
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        // the snapshot IS the root preimage: hashing it reproduces root(), so
        // snapshot and root can never encode two different states.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        // TARGET: a fresh set with an unrelated join STAGED — install must drop
        // it, or the stale stage would leak into the post-install view.
        let mut dst = Valset::new("valset");
        let mut dctx = TestCtx::new();
        futures::executor::block_on(dst.execute(&mut dctx, &join(&valid_key(9)))).unwrap();

        dst.install(&bytes, src_root).unwrap();
        assert_eq!(
            dst.root(),
            src_root,
            "installed root equals the source root"
        );
        assert_eq!(
            validators(&dst),
            validators(&src),
            "query parity after install"
        );
    }

    #[test]
    fn tampered_snapshot_is_rejected_and_the_target_is_untouched() {
        let mut src = Valset::new("valset");
        let mut ctx = TestCtx::new();
        for b in [4u8, 5] {
            futures::executor::block_on(src.execute(&mut ctx, &join(&valid_key(b)))).unwrap();
        }
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();

        // flip one bit inside the LAST key: count, lengths, and sort order all
        // still hold, so structural decode alone cannot catch it — only the
        // recomputed-root check can. exactly the byzantine-payload case.
        let mut bytes = src.snapshot();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;

        // the TARGET already holds committed membership AND a staged join: a
        // failed install must leave every layer untouched, not merely "still
        // empty" — a cleared-on-err bug is invisible against an empty target.
        let mut dst = Valset::new("valset");
        let mut dctx = TestCtx::new();
        futures::executor::block_on(dst.execute(&mut dctx, &join(&valid_key(8)))).unwrap();
        futures::executor::block_on(dst.commit_block()).unwrap();
        futures::executor::block_on(dst.execute(&mut dctx, &join(&valid_key(9)))).unwrap();
        let pre_root = dst.root();
        let pre_view = validators(&dst);

        let err = dst.install(&bytes, src_root).unwrap_err();
        assert!(
            matches!(err, Error::Module(_)),
            "tampered snapshot errs with Module"
        );
        assert_eq!(
            dst.root(),
            pre_root,
            "failed install leaves the committed root untouched"
        );
        assert_eq!(
            validators(&dst),
            pre_view,
            "failed install leaves membership and stage untouched"
        );
    }

    #[test]
    fn truncated_or_trailing_bytes_are_rejected_and_leave_state_intact() {
        let mut src = Valset::new("valset");
        let mut ctx = TestCtx::new();
        for b in [6u8, 7] {
            futures::executor::block_on(src.execute(&mut ctx, &join(&valid_key(b)))).unwrap();
        }
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();
        let bytes = src.snapshot();

        // the target has COMMITTED state of its own plus a STAGED join — every
        // failed install must leave both exactly as they were, a stronger claim
        // than "empty stays empty".
        let mut dst = Valset::new("valset");
        let mut dctx = TestCtx::new();
        futures::executor::block_on(dst.execute(&mut dctx, &join(&valid_key(8)))).unwrap();
        futures::executor::block_on(dst.commit_block()).unwrap();
        futures::executor::block_on(dst.execute(&mut dctx, &join(&valid_key(9)))).unwrap();
        let before_root = dst.root();
        let before_view = validators(&dst);

        // truncation: the final key byte is missing.
        assert!(dst.install(&bytes[..bytes.len() - 1], src_root).is_err());
        // trailing garbage: one byte past a well-formed stream.
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(dst.install(&trailing, src_root).is_err());
        // forged count: claims more entries than the buffer could possibly hold —
        // rejected before any allocation.
        let mut forged = bytes.clone();
        forged[..8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(dst.install(&forged, src_root).is_err());

        assert_eq!(dst.root(), before_root, "a failed install moved the root");
        assert_eq!(
            validators(&dst),
            before_view,
            "a failed install changed membership or dropped the stage"
        );
    }

    #[test]
    fn empty_snapshot_installs_onto_an_empty_set() {
        let src = Valset::new("valset");
        assert_eq!(src.root(), StateRoot::ZERO);
        let bytes = src.snapshot();
        assert_eq!(
            bytes,
            0u64.to_le_bytes().to_vec(),
            "an empty set is a lone zero count"
        );

        let mut dst = Valset::new("valset");
        dst.install(&bytes, StateRoot::ZERO).unwrap();
        assert_eq!(dst.root(), StateRoot::ZERO);
        assert!(validators(&dst).is_empty());
    }

    // ---- observers (staged admission) --------------------------------------

    #[test]
    fn observer_ops_reject_below_protocol_v3() {
        // the version gate IS the mixed-binary fork guard: below v3 a new
        // binary must land exactly where an old binary's unknown-variant
        // decode lands — deterministic reject, no state, root untouched.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new(); // protocol_version 0
        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        let before = v.root();

        for msg in [grant(&valid_key(2)), revoke(&valid_key(2))] {
            let err = futures::executor::block_on(v.execute(&mut ctx, &msg)).unwrap_err();
            assert!(
                matches!(err, Error::Module(ref m) if m.contains("protocol version 3")),
                "got {err:?}"
            );
        }
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(v.root(), before, "a gated op leaves the root byte-identical");
        assert!(observers(&v).is_empty());
    }

    #[test]
    fn grant_folds_the_root_only_while_observers_exist() {
        // the compatibility invariant end to end: a validators-only state
        // keeps its exact historical root; granting moves it; revoking the
        // last observer returns it BYTE-IDENTICAL to the validators-only root.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::at_version(3);
        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        let validators_only = v.root();
        let validators_only_snapshot = v.snapshot();

        let obs = valid_key(2);
        futures::executor::block_on(v.execute(&mut ctx, &grant(&obs))).unwrap();
        assert_eq!(v.root(), validators_only, "root reflects committed only");
        assert_eq!(observers(&v), vec![obs.clone()], "read-your-writes");
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_ne!(v.root(), validators_only, "a committed grant moves the root");

        futures::executor::block_on(v.execute(&mut ctx, &revoke(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(
            v.root(),
            validators_only,
            "revoking the last observer restores the exact validators-only root"
        );
        assert_eq!(
            v.snapshot(),
            validators_only_snapshot,
            "and the exact validators-only snapshot bytes"
        );
    }

    #[test]
    fn join_promotes_an_observer_out_of_the_tier() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::at_version(3);
        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        let obs = valid_key(2);
        futures::executor::block_on(v.execute(&mut ctx, &grant(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(observers(&v), vec![obs.clone()]);

        // the promotion: ONE Join both seats the validator and clears the
        // observer standing, in the same block.
        futures::executor::block_on(v.execute(&mut ctx, &join(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert!(validators(&v).contains(&obs), "promoted into the quorum");
        assert!(observers(&v).is_empty(), "and out of the observer tier");
    }

    #[test]
    fn granting_a_current_validator_is_refused() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::at_version(3);
        let k = valid_key(1);
        futures::executor::block_on(v.execute(&mut ctx, &join(&k))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();

        let err = futures::executor::block_on(v.execute(&mut ctx, &grant(&k))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("current validator")),
            "got {err:?}"
        );
        assert!(observers(&v).is_empty());
    }

    #[test]
    fn snapshot_with_observers_round_trips_and_rejects_forgeries() {
        let mut src = Valset::new("valset");
        let mut ctx = TestCtx::at_version(3);
        futures::executor::block_on(src.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        futures::executor::block_on(src.execute(&mut ctx, &grant(&valid_key(2)))).unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();

        // the snapshot stays the exact root preimage with observers present.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        let mut dst = Valset::new("valset");
        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root);
        assert_eq!(validators(&dst), validators(&src));
        assert_eq!(observers(&dst), observers(&src));

        // an EXPLICIT empty observer section is a second encoding of a
        // validators-only state — reject it (single-encoding invariant).
        let mut two_encodings = Valset::new("valset");
        let mut c2 = TestCtx::new();
        futures::executor::block_on(two_encodings.execute(&mut c2, &join(&valid_key(3)))).unwrap();
        futures::executor::block_on(two_encodings.commit_block()).unwrap();
        let mut padded = two_encodings.snapshot();
        padded.extend_from_slice(&0u64.to_le_bytes());
        let err = dst.install(&padded, two_encodings.root()).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("empty observer section")),
            "got {err:?}"
        );
    }
}
