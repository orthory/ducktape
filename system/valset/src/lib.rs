//! the ed25519 permissionless validator set as replicated state.
//!
//! a validator is a 32-byte ed25519 public key. anyone holding a WELL-FORMED
//! ed25519 key may [`ValsetMsg::Join`] the set — no authorization, no gating,
//! no stake weighting. this is deliberately permissionless: per the design,
//! "permissionless joining suffices; don't concern with proper shares." real
//! governance (who may join) and stake-weighted shares (voting power) are
//! DEFERRED — this module only replicates *membership*.
//!
//! state model mirrors the directory module's host-lent staging seam:
//! `execute` STAGES into a `pending` overlay (committed state untouched);
//! `query` reads pending-over-committed (read-your-writes); `commit_block`
//! merges pending into committed; `abort_block` drops pending; `root()`
//! reflects COMMITTED state only — a state-based (sorted, length-prefixed)
//! sha256 over the validator set, so it is order-independent and idempotent.

use std::collections::{BTreeMap, BTreeSet};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519::PublicKey;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};
use valset_interface::{decode_msg, decode_query, encode_reply, ValsetMsg, ValsetQuery, ValsetReply};

/// a 32-byte ed25519 public key encoding.
const KEY_LEN: usize = 32;

pub struct Valset {
    id: ModuleId,
    /// committed membership — what `root()` and the app-hash commit to.
    validators: BTreeSet<Vec<u8>>,
    /// membership changes staged during the current block: `true` == staged add,
    /// `false` == staged remove. read ahead of `validators` (read-your-writes),
    /// merged into committed state (and `root()`) only on `commit_block`.
    pending: BTreeMap<Vec<u8>, bool>,
}

impl Valset {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self { id: id.into(), validators: BTreeSet::new(), pending: BTreeMap::new() }
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
        let mut set: BTreeSet<Vec<u8>> = self.validators.clone();
        for (k, present) in &self.pending {
            if *present {
                set.insert(k.clone());
            } else {
                set.remove(k);
            }
        }
        set.into_iter().collect()
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Valset {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED set: a length-prefixed sha256
    /// over the sorted validators. order-independent (BTreeSet) and idempotent.
    /// an empty set reports `ZERO` — an empty/uninitialized module (matching the
    /// sdk `StateRoot::ZERO` doc and forge's unborn-repo root).
    fn root(&self) -> StateRoot {
        if self.validators.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update((self.validators.len() as u64).to_le_bytes());
        for k in &self.validators {
            h.update((k.len() as u64).to_le_bytes());
            h.update(k);
        }
        StateRoot(h.finalize().into())
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            // permissionless: any VALID ed25519 key may join — no authorization.
            ValsetMsg::Join { key } => {
                Self::validate_key(&key)?;
                self.stage_add(key);
            }
            ValsetMsg::Leave { key } => self.stage_remove(key),
        }
        Ok(())
    }

    /// read projection — the committed set plus this block's staged changes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ValsetQuery::Validators => {
                Ok(encode_reply(&ValsetReply::Validators(self.effective())))
            }
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
        Ok(())
    }

    /// discard the block's staged changes — committed state (and `root()`) is
    /// unchanged, so a failed block leaves no trace.
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;
    use valset_interface::{encode_msg, encode_query};

    // a minimal Ctx — valset's execute never touches ctx, but the trait needs one.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new() -> Self {
            Self {
                env: sdk::Env {
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
        fn env(&self) -> &sdk::Env { &self.env }
        fn module_root(&self, _t: &str) -> Option<StateRoot> { None }
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> { Err(Error::QueryUnsupported) }
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
        Msg { target: "valset".into(), payload: encode_msg(&ValsetMsg::Join { key: key.to_vec() }) }
    }
    fn leave(key: &[u8]) -> Msg {
        Msg { target: "valset".into(), payload: encode_msg(&ValsetMsg::Leave { key: key.to_vec() }) }
    }
    fn validators(v: &Valset) -> Vec<Vec<u8>> {
        let reply = futures::executor::block_on(v.query(&encode_query(&ValsetQuery::Validators))).unwrap();
        match valset_interface::decode_reply(&reply).unwrap() {
            ValsetReply::Validators(list) => list,
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
        assert_eq!(validators(&v), vec![k.clone()], "read-your-writes sees the stage");

        futures::executor::block_on(v.commit_block()).unwrap();
        assert_ne!(v.root(), StateRoot::ZERO, "a committed join moves the root off ZERO");
        assert_eq!(validators(&v), vec![k]);
    }

    #[test]
    fn leave_removes_a_validator() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let k = valid_key(2);
        futures::executor::block_on(v.execute(&mut ctx, &join(&k))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        let joined_root = v.root();

        futures::executor::block_on(v.execute(&mut ctx, &leave(&k))).unwrap();
        futures::executor::block_on(v.commit_block()).unwrap();
        assert!(validators(&v).is_empty(), "leave removes the validator");
        assert_eq!(v.root(), StateRoot::ZERO, "an empty set is back to ZERO");
        assert_ne!(v.root(), joined_root);
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
        assert!(matches!(err, Error::Module(_)), "malformed key errs with Module");
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
        assert_eq!(validators(&v).len(), 3, "any valid key joins, permissionlessly");
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
        assert_eq!(v.root(), before, "root is unchanged after a rolled-back block");
    }
}
