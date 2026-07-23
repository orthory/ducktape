//! the ed25519 membership registry as replicated state: validators + residents.
//!
//! a validator is a 32-byte ed25519 public key. anyone holding a WELL-FORMED
//! ed25519 key may [`ValsetMsg::Join`] the set — no authorization, no gating,
//! no stake weighting. this is deliberately permissionless: per the design,
//! "permissionless joining suffices; don't concern with proper shares." real
//! governance (who may join) and stake-weighted shares (voting power) are
//! DEFERRED — this module only replicates *membership*.
//!
//! ## residents (staged admission)
//!
//! a RESIDENT holds mesh + statesync standing but no quorum seat — the tier
//! a joiner syncs in before promotion, so the consensus set only ever gains a
//! caught-up validator. [`ValsetMsg::Grant`] / [`ValsetMsg::Revoke`] manage
//! the set; a [`ValsetMsg::Join`] on a current resident PROMOTES it (adds the
//! validator, removes the resident, one block).
//!
//! ## root/snapshot encoding
//!
//! the root preimage and snapshot always carry both sections — validators,
//! then residents — each a count-prefixed sorted key list, so a given state
//! has exactly one encoding.
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

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519::PublicKey;
use sdk::codec;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

/// a 32-byte ed25519 public key encoding.
const KEY_LEN: usize = 32;
type SnapshotMembership = (BTreeSet<Vec<u8>>, BTreeSet<Vec<u8>>);

pub struct Valset {
    id: ModuleId,
    /// committed membership — what `root()` and the root-hash commit to.
    validators: BTreeSet<Vec<u8>>,
    /// membership changes staged during the current block: `true` == staged add,
    /// `false` == staged remove. read ahead of `validators` (read-your-writes),
    /// merged into committed state (and `root()`) only on `commit_block`.
    pending: BTreeMap<Vec<u8>, bool>,
    /// committed RESIDENT standing (mesh + statesync, no quorum seat). folded
    /// into `root()`/`snapshot()` as the second section.
    residents: BTreeSet<Vec<u8>>,
    /// resident changes staged during the current block, same discipline as
    /// `pending`.
    pending_residents: BTreeMap<Vec<u8>, bool>,
}

impl Valset {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            validators: BTreeSet::new(),
            pending: BTreeMap::new(),
            residents: BTreeSet::new(),
            pending_residents: BTreeMap::new(),
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

    /// the COMMITTED membership pair — `(validators, residents)`, sorted.
    /// the post-install/rebuild witness (both classes must survive a sync
    /// byte-for-byte); reads inside a block use the staged projections.
    pub fn membership(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        (
            self.validators.iter().cloned().collect(),
            self.residents.iter().cloned().collect(),
        )
    }

    /// the committed resident set with this block's staged changes applied.
    fn effective_residents(&self) -> Vec<Vec<u8>> {
        Self::overlay(&self.residents, &self.pending_residents)
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
    /// u64-le + key bytes), then the resident section in the same shape. so for
    /// non-empty state `sha256(snapshot()) == root()`; fully empty state
    /// snapshots to two zero counts (root still `ZERO`, unhashed). pending is
    /// deliberately excluded — a snapshot ships what consensus committed to,
    /// and staged-but-uncommitted changes are not that.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_membership(&self.validators, &self.residents)
    }

    /// the shared canonical encoding behind `snapshot` and `root_of`.
    fn encode_membership(
        validators: &BTreeSet<Vec<u8>>,
        residents: &BTreeSet<Vec<u8>>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        for set in [validators, residents] {
            out.extend_from_slice(&(set.len() as u64).to_le_bytes());
            for k in set {
                codec::push_bytes(&mut out, k);
            }
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
        let (validators, residents) = Self::decode_snapshot(bytes)?;
        sdk::verify_snapshot_root(Self::root_of(&validators, &residents), expected)?;
        self.validators = validators;
        self.residents = residents;
        self.pending.clear();
        self.pending_residents.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves them):
    /// the validator section, then the resident section. the count and every
    /// key length are checked against the remaining buffer BEFORE any
    /// allocation, truncation and trailing bytes both reject, and keys must
    /// arrive strictly increasing per section — a peer cannot mint alternative
    /// byte streams for one state.
    fn decode_snapshot(bytes: &[u8]) -> Result<SnapshotMembership, Error> {
        fn take_section(cur: &mut codec::Cursor, what: &str) -> Result<BTreeSet<Vec<u8>>, Error> {
            let count = cur.u64(what)?;
            // each entry costs at least its 8-byte length prefix, so a count the
            // remaining bytes cannot possibly hold is rejected up front — a forged
            // count never drives allocation.
            cur.bound(count, 8, what)?;
            let mut set = BTreeSet::new();
            let mut prev: Option<Vec<u8>> = None;
            for _ in 0..count {
                let key = cur.bytes(what)?;
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

        let mut cur = codec::Cursor::new(bytes);
        let validators = take_section(&mut cur, "snapshot validator")?;
        let residents = take_section(&mut cur, "snapshot resident")?;
        cur.finish("snapshot")?;
        Ok((validators, residents))
    }

    /// the state-based commitment: `ZERO` when both sets are empty, else sha256
    /// over exactly the bytes `snapshot` emits. shared by `root()` (committed
    /// state) and `install` (a decoded candidate), so the two can never drift.
    fn root_of(validators: &BTreeSet<Vec<u8>>, residents: &BTreeSet<Vec<u8>>) -> StateRoot {
        if validators.is_empty() && residents.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::encode_membership(validators, residents)).into())
    }
}

/// the CURRENT member set of the valset module at `valset`: its
/// staged-over-committed Validators projection, via the host-routed read lane.
/// the one shared read every membership-gated module (governance, upgrade, …)
/// funnels through.
pub async fn members(ctx: &dyn Ctx, valset: &str) -> Result<Vec<Vec<u8>>, Error> {
    let reply = ctx
        .query(valset, &encode_query(&ValsetQuery::Validators))
        .await?;
    match decode_reply(&reply).map_err(Error::Module)? {
        ValsetReply::Validators(members) => Ok(members),
        other => Err(Error::Module(format!(
            "valset answered a Validators query with {other:?}"
        ))),
    }
}

/// the CURRENT validator set UNION resident set of the valset module at
/// `valset`, both queried live from its staged-over-committed projection — an
/// op is admitted for EITHER standing, so a joined (not-yet-promoted) resident
/// still passes. the shared read behind identity's and capability's bind gates.
pub async fn members_and_residents(
    ctx: &dyn Ctx,
    valset: &str,
) -> Result<BTreeSet<Vec<u8>>, Error> {
    let validators = match decode_reply(
        &ctx.query(valset, &encode_query(&ValsetQuery::Validators))
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
    let residents = match decode_reply(
        &ctx.query(valset, &encode_query(&ValsetQuery::Residents))
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

#[async_trait::async_trait(?Send)]
impl Module for Valset {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED state: a length-prefixed
    /// sha256 over the sorted validators, then the sorted residents.
    /// order-independent (BTreeSet) and idempotent. fully empty
    /// state reports `ZERO` — an empty/uninitialized module (matching the sdk
    /// `StateRoot::ZERO` doc and forge's unborn-repo root).
    fn root(&self) -> StateRoot {
        Self::root_of(&self.validators, &self.residents)
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
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
                // PROMOTION: a joining resident leaves the resident tier in
                // the same block — one boundary carries the whole transition,
                // and the transport union never double-counts the key.
                if self.effective_residents().contains(&key) {
                    self.pending_residents.insert(key.clone(), false);
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
                Self::validate_key(&key)?;
                // a validator already holds every resident capability; a
                // second standing would only smear the promote/demote edges.
                if self.effective().contains(&key) {
                    return Err(Error::Module(
                        "key is a current validator — resident standing is the pre-promotion tier"
                            .into(),
                    ));
                }
                self.pending_residents.insert(key, true);
            }
            ValsetMsg::Revoke { key } => {
                self.pending_residents.insert(key, false);
            }
        }
        Ok(())
    }

    /// read projection — the committed sets plus this block's staged changes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ValsetQuery::Validators => Ok(encode_reply(&ValsetReply::Validators(self.effective()))),
            ValsetQuery::Residents => Ok(encode_reply(&ValsetReply::Residents(
                self.effective_residents(),
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
        for (k, present) in std::mem::take(&mut self.pending_residents) {
            if present {
                self.residents.insert(k);
            } else {
                self.residents.remove(&k);
            }
        }
        Ok(())
    }

    /// discard the block's staged changes — committed state (and `root()`) is
    /// unchanged, so a failed block leaves no trace.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        self.pending_residents.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_msg, encode_query};
    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;

    use sdk_testkit::TestCtx;

    // valset's execute reads only env (origin); me/height are cosmetic, so
    // the shared TestCtx's defaults stand in.
    fn sys_ctx() -> TestCtx {
        TestCtx::at_height(0)
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
        match crate::decode_reply(&reply).unwrap() {
            ValsetReply::Validators(list) => list,
            other => panic!("expected Validators, got {other:?}"),
        }
    }
    fn residents(v: &Valset) -> Vec<Vec<u8>> {
        let reply =
            futures::executor::block_on(v.query(&encode_query(&ValsetQuery::Residents))).unwrap();
        match crate::decode_reply(&reply).unwrap() {
            ValsetReply::Residents(list) => list,
            other => panic!("expected Residents, got {other:?}"),
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
        let mut ctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut c1 = sys_ctx();
        futures::executor::block_on(v1.execute(&mut c1, &join(&a))).unwrap();
        futures::executor::block_on(v1.execute(&mut c1, &join(&b))).unwrap();
        futures::executor::block_on(v1.commit_block()).unwrap();

        // same two validators, joined in the OPPOSITE order.
        let mut v2 = Valset::new("valset");
        let mut c2 = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut dctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut dctx = sys_ctx();
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
        let mut ctx = sys_ctx();
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
        let mut dctx = sys_ctx();
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
            [0u8; 16].to_vec(),
            "an empty state is two zero section counts"
        );

        let mut dst = Valset::new("valset");
        dst.install(&bytes, StateRoot::ZERO).unwrap();
        assert_eq!(dst.root(), StateRoot::ZERO);
        assert!(validators(&dst).is_empty());
    }

    // ---- residents (staged admission) --------------------------------------

    #[test]
    fn resident_ops_apply_from_genesis() {
        let mut v = Valset::new("valset");
        let mut ctx = sys_ctx();
        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();

        // grant confers resident standing on a freshly founded network.
        let obs = valid_key(2);
        futures::executor::block_on(v.execute(&mut ctx, &grant(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(residents(&v), vec![obs.clone()], "grant applied");

        // revoke clears it.
        futures::executor::block_on(v.execute(&mut ctx, &revoke(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert!(residents(&v).is_empty(), "revoke applied");
    }

    #[test]
    fn grant_folds_the_root_only_while_residents_exist() {
        // deterministic encoding end to end: granting moves the root; revoking
        // the last resident returns it BYTE-IDENTICAL to the validators-only
        // root and snapshot.
        let mut v = Valset::new("valset");
        let mut ctx = sys_ctx();
        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        let validators_only = v.root();
        let validators_only_snapshot = v.snapshot();

        let obs = valid_key(2);
        futures::executor::block_on(v.execute(&mut ctx, &grant(&obs))).unwrap();
        assert_eq!(v.root(), validators_only, "root reflects committed only");
        assert_eq!(residents(&v), vec![obs.clone()], "read-your-writes");
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_ne!(
            v.root(),
            validators_only,
            "a committed grant moves the root"
        );

        futures::executor::block_on(v.execute(&mut ctx, &revoke(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(
            v.root(),
            validators_only,
            "revoking the last resident restores the exact validators-only root"
        );
        assert_eq!(
            v.snapshot(),
            validators_only_snapshot,
            "and the exact validators-only snapshot bytes"
        );
    }

    #[test]
    fn join_promotes_a_resident_out_of_the_tier() {
        let mut v = Valset::new("valset");
        let mut ctx = sys_ctx();
        futures::executor::block_on(v.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        let obs = valid_key(2);
        futures::executor::block_on(v.execute(&mut ctx, &grant(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert_eq!(residents(&v), vec![obs.clone()]);

        // the promotion: ONE Join both seats the validator and clears the
        // resident standing, in the same block.
        futures::executor::block_on(v.execute(&mut ctx, &join(&obs))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert!(validators(&v).contains(&obs), "promoted into the quorum");
        assert!(residents(&v).is_empty(), "and out of the resident tier");
    }

    #[test]
    fn granting_a_current_validator_is_refused() {
        let mut v = Valset::new("valset");
        let mut ctx = sys_ctx();
        let k = valid_key(1);
        futures::executor::block_on(v.execute(&mut ctx, &join(&k))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();

        let err = futures::executor::block_on(v.execute(&mut ctx, &grant(&k))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("current validator")),
            "got {err:?}"
        );
        assert!(residents(&v).is_empty());
    }

    #[test]
    fn snapshot_with_residents_round_trips_and_rejects_forgeries() {
        let mut src = Valset::new("valset");
        let mut ctx = sys_ctx();
        futures::executor::block_on(src.execute(&mut ctx, &join(&valid_key(1)))).unwrap();
        futures::executor::block_on(src.execute(&mut ctx, &grant(&valid_key(2)))).unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();

        // the snapshot stays the exact root preimage with residents present.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        let mut dst = Valset::new("valset");
        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root);
        assert_eq!(validators(&dst), validators(&src));
        assert_eq!(residents(&dst), residents(&src));

        // bytes past the resident section are trailing garbage — reject them
        // (single-encoding invariant).
        let mut two_encodings = Valset::new("valset");
        let mut c2 = sys_ctx();
        futures::executor::block_on(two_encodings.execute(&mut c2, &join(&valid_key(3)))).unwrap();
        futures::executor::block_on(two_encodings.commit_block()).unwrap();
        let mut padded = two_encodings.snapshot();
        padded.extend_from_slice(&0u64.to_le_bytes());
        let err = dst.install(&padded, two_encodings.root()).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("trailing bytes")),
            "got {err:?}"
        );
    }
}
