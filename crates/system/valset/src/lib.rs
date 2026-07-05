//! the ed25519 validator set as replicated state, in two classes.
//!
//! a validator is a 32-byte ed25519 public key, and membership is a
//! two-step protocol:
//!
//! - [`ValsetMsg::Join`] (governance-gated) registers a key as **standby**:
//!   tracked on the transport mesh, served by statesync, warmed into the
//!   WireGuard reachability mesh — but NOT counted for consensus quorum. an
//!   absent standby key costs the network nothing at the consensus layer.
//! - [`ValsetMsg::Online`] moves a standby key to **active** — the consensus
//!   quorum — once the node itself proves it is up: the op carries the
//!   standby key's own signature (proof of possession, height-windowed), so
//!   a relaying member cannot activate a node that never announced. the
//!   next epoch cutover then respawns the engines with the wider quorum.
//! - [`ValsetMsg::Leave`] (governance-gated) removes a key from either class.
//!
//! genesis seeding ([`Valset::insert`]) lands keys directly in ACTIVE —
//! founding validators never run the online dance.
//!
//! state model mirrors the directory module's host-lent staging seam:
//! `execute` STAGES into a `pending` overlay (committed state untouched);
//! `query` reads pending-over-committed (read-your-writes); `commit_block`
//! merges pending into committed; `abort_block` drops pending; `root()`
//! reflects COMMITTED state only — a state-based (tagged, sorted,
//! length-prefixed) sha256 over both classes, order-independent and
//! idempotent.
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
use commonware_cryptography::Verifier as _;
use commonware_cryptography::ed25519::{PublicKey, Signature};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset_interface::{
    ONLINE_PROOF_NS, ONLINE_PROOF_TTL_BLOCKS, ValsetMsg, ValsetQuery, ValsetReply, decode_msg,
    decode_query, encode_reply, online_proof_message,
};

/// a 32-byte ed25519 public key encoding.
const KEY_LEN: usize = 32;

/// snapshot/root domain tag: the two-class layout can never byte-collide
/// with the v1 single-set encoding.
const SNAPSHOT_TAG: &[u8] = b"valset:v2";

/// one staged membership transition for a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingChange {
    /// register as standby (a Join).
    AddStandby,
    /// move standby -> active (an Online).
    Activate,
    /// remove from whichever class holds the key (a Leave).
    Remove,
}

pub struct Valset {
    id: ModuleId,
    /// committed ACTIVE membership — the consensus-quorum projection.
    active: BTreeSet<Vec<u8>>,
    /// committed STANDBY membership — transport-tracked, quorum-exempt.
    standby: BTreeSet<Vec<u8>>,
    /// transitions staged during the current block: read ahead of committed
    /// state (read-your-writes), merged on `commit_block`.
    pending: BTreeMap<Vec<u8>, PendingChange>,
}

impl Valset {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            active: BTreeSet::new(),
            standby: BTreeSet::new(),
            pending: BTreeMap::new(),
        }
    }

    /// direct sync add (genesis seeding / tests): lands ACTIVE — founding
    /// validators never run the online dance. does NOT validate — callers
    /// seeding genesis are trusted; the `execute(Join)` path validates.
    pub fn insert(&mut self, key: Vec<u8>) {
        self.standby.remove(&key);
        self.active.insert(key);
    }

    /// validate that `key` is a well-formed 32-byte ed25519 public key. the
    /// explicit length guard makes the 32-byte invariant independent of decode's
    /// trailing-byte behavior; `PublicKey::decode` then checks the curve point
    /// (ZIP215: must decompress to a point on the twisted Edwards curve).
    fn validate_key(key: &[u8]) -> Result<PublicKey, Error> {
        if key.len() != KEY_LEN {
            return Err(Error::Module(format!(
                "invalid ed25519 public key: expected {KEY_LEN} bytes, got {}",
                key.len()
            )));
        }
        PublicKey::decode(key).map_err(|e| Error::Module(format!("invalid ed25519 public key: {e}")))
    }

    /// the COMMITTED membership picture — `(active, standby)`, both sorted.
    /// what a synced snapshot reader (a parked joiner probing its own
    /// registration) needs without driving the async query surface.
    pub fn membership(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        (
            self.active.iter().cloned().collect(),
            self.standby.iter().cloned().collect(),
        )
    }

    /// the committed sets with this block's staged changes applied —
    /// read-your-writes, both sorted (order-independent).
    fn effective(&self) -> (BTreeSet<Vec<u8>>, BTreeSet<Vec<u8>>) {
        let mut active = self.active.clone();
        let mut standby = self.standby.clone();
        for (k, change) in &self.pending {
            match change {
                PendingChange::AddStandby => {
                    if !active.contains(k) {
                        standby.insert(k.clone());
                    }
                }
                PendingChange::Activate => {
                    standby.remove(k);
                    active.insert(k.clone());
                }
                PendingChange::Remove => {
                    active.remove(k);
                    standby.remove(k);
                }
            }
        }
        (active, standby)
    }

    // ---- state-sync ---------------------------------------------------------
    // ship the committed sets as their root preimage; adopt a peer's bytes only
    // after re-deriving the root consensus expects — the root, not the peer, is
    // the trust anchor.

    /// canonical bytes of the COMMITTED sets — exactly the byte stream `root()`
    /// hashes: the domain tag, then per class its count u64-le followed by each
    /// sorted key as len u64-le + bytes (active first, standby second). so for
    /// non-empty state `sha256(snapshot()) == root()`; empty-empty state
    /// snapshots to tag + two zero counts (whose root is still `ZERO`,
    /// unhashed). pending is deliberately excluded — a snapshot ships what
    /// consensus committed to, and staged-but-uncommitted changes are not that.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            SNAPSHOT_TAG.len()
                + 16
                + self
                    .active
                    .iter()
                    .chain(self.standby.iter())
                    .map(|k| 8 + k.len())
                    .sum::<usize>(),
        );
        out.extend_from_slice(SNAPSHOT_TAG);
        for set in [&self.active, &self.standby] {
            out.extend_from_slice(&(set.len() as u64).to_le_bytes());
            for k in set {
                out.extend_from_slice(&(k.len() as u64).to_le_bytes());
                out.extend_from_slice(k);
            }
        }
        out
    }

    /// replace committed state with a decoded snapshot, iff the decoded sets'
    /// recomputed root equals `expected`. decode and verification land in a
    /// temporary: self is mutated only after both pass, so on any `Err` committed
    /// state, pending, and `root()` are byte-identical to before the call.
    /// success clears pending — staged changes belong to the state being
    /// replaced, not the state being adopted.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (active, standby) = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&active, &standby);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.active = active;
        self.standby = standby;
        self.pending.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes (a byzantine peer serves them).
    /// the tag, the counts, and every key length are checked against the
    /// remaining buffer BEFORE any allocation, truncation and trailing bytes
    /// both reject, keys must arrive strictly increasing within each class, and
    /// no key may appear in both classes — a given state has exactly one valid
    /// encoding, so a peer cannot mint alternative byte streams for one state.
    #[allow(clippy::type_complexity)]
    fn decode_snapshot(bytes: &[u8]) -> Result<(BTreeSet<Vec<u8>>, BTreeSet<Vec<u8>>), Error> {
        fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
            let Some((head, rest)) = (*buf).split_first_chunk::<8>() else {
                return Err(Error::Module("snapshot truncated".into()));
            };
            *buf = rest;
            Ok(u64::from_le_bytes(*head))
        }
        fn take_set(buf: &mut &[u8]) -> Result<BTreeSet<Vec<u8>>, Error> {
            let count = take_u64(buf)?;
            // each entry costs at least its 8-byte length prefix, so a count
            // the remaining bytes cannot possibly hold is rejected up front —
            // a forged count never drives allocation.
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
        let Some(rest) = buf.strip_prefix(SNAPSHOT_TAG) else {
            return Err(Error::Module("snapshot tag mismatch".into()));
        };
        buf = rest;
        let active = take_set(&mut buf)?;
        let standby = take_set(&mut buf)?;
        if !buf.is_empty() {
            return Err(Error::Module(format!(
                "snapshot carries {} trailing bytes",
                buf.len()
            )));
        }
        if let Some(dup) = active.intersection(&standby).next() {
            return Err(Error::Module(format!(
                "snapshot key {} is both active and standby",
                dup.iter().map(|b| format!("{b:02x}")).collect::<String>()
            )));
        }
        Ok((active, standby))
    }

    /// the state-based commitment: `ZERO` when both classes are empty, else
    /// sha256 over exactly the bytes `snapshot` emits. shared by `root()`
    /// (committed state) and `install` (a decoded candidate), so the two can
    /// never drift.
    fn root_of(active: &BTreeSet<Vec<u8>>, standby: &BTreeSet<Vec<u8>>) -> StateRoot {
        if active.is_empty() && standby.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update(SNAPSHOT_TAG);
        for set in [active, standby] {
            h.update((set.len() as u64).to_le_bytes());
            for k in set {
                h.update((k.len() as u64).to_le_bytes());
                h.update(k);
            }
        }
        StateRoot(h.finalize().into())
    }

    /// verify an Online op's proof of possession: the key's own signature over
    /// `key || signed_height` under [`ONLINE_PROOF_NS`], within the height
    /// window. binding to a recent height keeps a proof from a previous
    /// standby term (leave -> re-register) from being replayed much later.
    fn verify_online_proof(
        key: &PublicKey,
        key_bytes: &[u8],
        signed_height: u64,
        signature: &[u8],
        height: u64,
    ) -> Result<(), Error> {
        if signed_height > height {
            return Err(Error::Module(format!(
                "online proof signed at future height {signed_height} (block height {height})"
            )));
        }
        if height - signed_height > ONLINE_PROOF_TTL_BLOCKS {
            return Err(Error::Module(format!(
                "online proof expired: signed at {signed_height}, block height {height}"
            )));
        }
        let sig = Signature::decode(signature)
            .map_err(|e| Error::Module(format!("online proof signature malformed: {e}")))?;
        let msg = online_proof_message(key_bytes, signed_height);
        if !key.verify(ONLINE_PROOF_NS, &msg, &sig) {
            return Err(Error::Module(
                "online proof signature does not verify against the standby key".into(),
            ));
        }
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Valset {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED sets: a tagged, length-prefixed
    /// sha256 over the sorted active then standby validators. order-independent
    /// (BTreeSet) and idempotent. empty-empty reports `ZERO` — an
    /// empty/uninitialized module (matching the sdk `StateRoot::ZERO` doc and
    /// forge's unborn-repo root).
    fn root(&self) -> StateRoot {
        Self::root_of(&self.active, &self.standby)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            // registration and removal are GOVERNANCE-GATED: only a module
            // origin (the governance module's follow-up after a passing
            // proposal) or a system origin (genesis orchestration) may stage
            // them. an unauthenticated external Leave was a one-message
            // liveness kill on a private network; origin is part of the
            // deterministic Env, so every validator enforces this identically.
            ValsetMsg::Join { key } => {
                match &ctx.env().origin {
                    sdk::Origin::Module(_) | sdk::Origin::System => {}
                    sdk::Origin::External(_) => {
                        return Err(Error::Module(
                            "valset membership changes only via governance".into(),
                        ));
                    }
                }
                Self::validate_key(&key)?;
                let (active, standby) = self.effective();
                // registration is idempotent and never demotes: an already
                // active or standby key is left exactly where it is.
                if !active.contains(&key) && !standby.contains(&key) {
                    self.pending.insert(key, PendingChange::AddStandby);
                }
            }
            ValsetMsg::Leave { key } => {
                match &ctx.env().origin {
                    sdk::Origin::Module(_) | sdk::Origin::System => {}
                    sdk::Origin::External(_) => {
                        return Err(Error::Module(
                            "valset membership changes only via governance".into(),
                        ));
                    }
                }
                // the ACTIVE set must NEVER go empty. a downstream orderer
                // reconfigured to zero validators hits commonware `quorum(0)`,
                // which panics ("n must not be zero") and halts the node.
                // refuse a removal that would drop the LAST active validator.
                // authoritative here: every membership removal (a governance-
                // passed RemoveValidator or genesis orchestration) funnels
                // through this arm, so the invariant holds no matter who
                // staged it — the set is closed under this rule regardless of
                // the caller.
                let (mut active, _) = self.effective();
                active.remove(&key);
                if active.is_empty() {
                    return Err(Error::Module(
                        "refusing to remove the last active validator: the quorum must never \
                         be empty"
                            .into(),
                    ));
                }
                self.pending.insert(key, PendingChange::Remove);
            }
            // activation is NODE-ATTESTED, member-relayed: the proof inside
            // the op (the standby key's own signature) is the authorization,
            // so the frame origin only needs to be someone with standing —
            // an active member relaying a lobby announce, the key itself
            // (if it can frame ops), or a module/system orchestration.
            ValsetMsg::Online {
                key,
                signed_height,
                signature,
            } => {
                let pk = Self::validate_key(&key)?;
                let (active, standby) = self.effective();
                match &ctx.env().origin {
                    sdk::Origin::Module(_) | sdk::Origin::System => {}
                    sdk::Origin::External(submitter) => {
                        if !active.contains(submitter) && *submitter != key {
                            return Err(Error::Module(
                                "Online must be submitted by an active validator or the key \
                                 itself"
                                    .into(),
                            ));
                        }
                    }
                }
                if active.contains(&key) {
                    // relaying members race; the second arrival is a no-op,
                    // not a violation.
                    return Ok(());
                }
                if !standby.contains(&key) {
                    return Err(Error::Module(
                        "Online for a key that is not standby: register it via governance \
                         first"
                            .into(),
                    ));
                }
                Self::verify_online_proof(
                    &pk,
                    &key,
                    signed_height,
                    &signature,
                    ctx.env().height,
                )?;
                self.pending.insert(key, PendingChange::Activate);
            }
        }
        Ok(())
    }

    /// read projection — the committed sets plus this block's staged changes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let (active, standby) = self.effective();
        match decode_query(req).map_err(Error::Module)? {
            ValsetQuery::Validators => Ok(encode_reply(&ValsetReply::Validators(
                active.into_iter().collect(),
            ))),
            ValsetQuery::Members => Ok(encode_reply(&ValsetReply::Members {
                active: active.into_iter().collect(),
                standby: standby.into_iter().collect(),
            })),
        }
    }

    /// merge the block's staged membership changes into committed state —
    /// `root()` now reflects them. no-op if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        for (k, change) in std::mem::take(&mut self.pending) {
            match change {
                PendingChange::AddStandby => {
                    if !self.active.contains(&k) {
                        self.standby.insert(k);
                    }
                }
                PendingChange::Activate => {
                    self.standby.remove(&k);
                    self.active.insert(k);
                }
                PendingChange::Remove => {
                    self.active.remove(&k);
                    self.standby.remove(&k);
                }
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

    // a minimal Ctx — valset's execute reads only env; the trait needs the rest.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new() -> Self {
            Self::with(sdk::Origin::System, 0)
        }
        fn with(origin: sdk::Origin, height: u64) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height,
                    consensus_time: 0,
                    origin,
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

    fn sk(seed_byte: u8) -> PrivateKey {
        let seed = [seed_byte; 32];
        PrivateKey::decode(&seed[..]).expect("any 32 bytes is a valid seed")
    }
    fn valid_key(seed_byte: u8) -> Vec<u8> {
        sk(seed_byte).public_key().as_ref().to_vec()
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
    fn online(seed_byte: u8, signed_height: u64) -> Msg {
        let signer = sk(seed_byte);
        let key = signer.public_key().as_ref().to_vec();
        let sig = signer.sign(ONLINE_PROOF_NS, &online_proof_message(&key, signed_height));
        Msg {
            target: "valset".into(),
            payload: encode_msg(&ValsetMsg::Online {
                key,
                signed_height,
                signature: sig.as_ref().to_vec(),
            }),
        }
    }
    fn exec(v: &mut Valset, ctx: &mut TestCtx, msg: &Msg) -> Result<(), Error> {
        futures::executor::block_on(v.execute(ctx, msg))
    }
    fn commit(v: &mut Valset) {
        futures::executor::block_on(v.commit_block()).unwrap();
    }
    fn validators(v: &Valset) -> Vec<Vec<u8>> {
        let reply =
            futures::executor::block_on(v.query(&encode_query(&ValsetQuery::Validators))).unwrap();
        match valset_interface::decode_reply(&reply).unwrap() {
            ValsetReply::Validators(list) => list,
            other => panic!("unexpected reply {other:?}"),
        }
    }
    fn members(v: &Valset) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let reply =
            futures::executor::block_on(v.query(&encode_query(&ValsetQuery::Members))).unwrap();
        match valset_interface::decode_reply(&reply).unwrap() {
            ValsetReply::Members { active, standby } => (active, standby),
            other => panic!("unexpected reply {other:?}"),
        }
    }

    #[test]
    fn join_registers_standby_not_active() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        assert_eq!(v.root(), StateRoot::ZERO, "genesis state is empty -> ZERO");

        let k = valid_key(1);
        exec(&mut v, &mut ctx, &join(&k)).unwrap();
        assert_eq!(v.root(), StateRoot::ZERO, "root reflects committed only");
        let (active, standby) = members(&v);
        assert!(active.is_empty(), "a join lands STANDBY, never active");
        assert_eq!(standby, vec![k.clone()], "read-your-writes sees the stage");
        assert!(
            validators(&v).is_empty(),
            "the consensus projection excludes standby"
        );

        commit(&mut v);
        assert_ne!(v.root(), StateRoot::ZERO, "a committed join moves the root");
        assert_eq!(members(&v).1, vec![k]);
    }

    #[test]
    fn online_moves_standby_to_active_at_commit() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        // a founding active member (the relayer) + a standby registrant.
        v.insert(valid_key(1));
        exec(&mut v, &mut ctx, &join(&valid_key(2))).unwrap();
        commit(&mut v);
        assert_eq!(validators(&v).len(), 1);

        // relayed by the ACTIVE member at height 100, proof signed at 90.
        let mut relay = TestCtx::with(sdk::Origin::External(valid_key(1)), 100);
        exec(&mut v, &mut relay, &online(2, 90)).unwrap();
        commit(&mut v);
        let (active, standby) = members(&v);
        assert_eq!(active.len(), 2, "online moved the key into the quorum");
        assert!(standby.is_empty());
        assert_eq!(validators(&v).len(), 2);

        // a racing second relay is a no-op, not a violation.
        let mut relay2 = TestCtx::with(sdk::Origin::External(valid_key(1)), 101);
        exec(&mut v, &mut relay2, &online(2, 90)).unwrap();
    }

    #[test]
    fn online_refuses_bad_proofs_and_bad_standing() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        v.insert(valid_key(1));
        exec(&mut v, &mut ctx, &join(&valid_key(2))).unwrap();
        commit(&mut v);

        // not standby: never registered.
        let mut relay = TestCtx::with(sdk::Origin::External(valid_key(1)), 100);
        let err = exec(&mut v, &mut relay, &online(3, 90)).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("not standby")), "{err:?}");

        // wrong signer: the proof must be the standby key's OWN signature.
        let forged = {
            let signer = sk(9);
            let key = valid_key(2);
            let sig = signer.sign(ONLINE_PROOF_NS, &online_proof_message(&key, 90));
            Msg {
                target: "valset".into(),
                payload: encode_msg(&ValsetMsg::Online {
                    key,
                    signed_height: 90,
                    signature: sig.as_ref().to_vec(),
                }),
            }
        };
        let err = exec(&mut v, &mut relay, &forged).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("does not verify")), "{err:?}");

        // expired window.
        let mut late = TestCtx::with(
            sdk::Origin::External(valid_key(1)),
            90 + ONLINE_PROOF_TTL_BLOCKS + 1,
        );
        let err = exec(&mut v, &mut late, &online(2, 90)).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("expired")), "{err:?}");

        // future-signed proof.
        let mut early = TestCtx::with(sdk::Origin::External(valid_key(1)), 50);
        let err = exec(&mut v, &mut early, &online(2, 90)).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("future")), "{err:?}");

        // relayed by a non-member stranger (not active, not the key itself).
        let mut stranger = TestCtx::with(sdk::Origin::External(valid_key(9)), 100);
        let err = exec(&mut v, &mut stranger, &online(2, 90)).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("active validator")), "{err:?}");

        // nothing landed.
        commit(&mut v);
        assert_eq!(validators(&v).len(), 1);
        assert_eq!(members(&v).1, vec![valid_key(2)]);
    }

    #[test]
    fn join_never_demotes_and_is_idempotent() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        v.insert(valid_key(1));
        commit(&mut v);
        let before = v.root();

        // re-registering an ACTIVE key leaves it active.
        exec(&mut v, &mut ctx, &join(&valid_key(1))).unwrap();
        commit(&mut v);
        assert_eq!(v.root(), before, "join of an active key is a no-op");
        assert_eq!(validators(&v).len(), 1);
    }

    #[test]
    fn external_origin_cannot_join_or_leave() {
        let mut v = Valset::new("valset");
        v.insert(valid_key(1));
        let mut ext = TestCtx::with(sdk::Origin::External(valid_key(1)), 0);
        let err = exec(&mut v, &mut ext, &join(&valid_key(2))).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("governance")), "{err:?}");
        let err = exec(&mut v, &mut ext, &leave(&valid_key(1))).unwrap_err();
        assert!(matches!(err, Error::Module(ref m) if m.contains("governance")), "{err:?}");
    }

    #[test]
    fn leave_removes_from_either_class() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        v.insert(valid_key(1));
        v.insert(valid_key(2));
        exec(&mut v, &mut ctx, &join(&valid_key(3))).unwrap();
        commit(&mut v);

        exec(&mut v, &mut ctx, &leave(&valid_key(2))).unwrap();
        exec(&mut v, &mut ctx, &leave(&valid_key(3))).unwrap();
        commit(&mut v);
        let (active, standby) = members(&v);
        assert_eq!(active, vec![valid_key(1)]);
        assert!(standby.is_empty());
    }

    #[test]
    fn leaving_the_last_active_validator_is_refused_even_with_standby() {
        // standby members are not quorum-capable: the guard protects the
        // ACTIVE set specifically.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        v.insert(valid_key(1));
        exec(&mut v, &mut ctx, &join(&valid_key(2))).unwrap();
        commit(&mut v);
        let before = v.root();

        let err = exec(&mut v, &mut ctx, &leave(&valid_key(1))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("last active validator")),
            "{err:?}"
        );
        commit(&mut v);
        assert_eq!(v.root(), before, "committed state is byte-identical");
    }

    #[test]
    fn leaving_the_last_of_a_shrinking_set_is_refused() {
        // stage two leaves in one block: the second would empty the active
        // set within the same block's read-your-writes view and is refused —
        // the guard reads the EFFECTIVE (staged-over-committed) set.
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        v.insert(valid_key(4));
        v.insert(valid_key(5));
        commit(&mut v);

        exec(&mut v, &mut ctx, &leave(&valid_key(4))).unwrap();
        let err = exec(&mut v, &mut ctx, &leave(&valid_key(5))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("last active validator")),
            "{err:?}"
        );
    }

    #[test]
    fn malformed_key_is_rejected() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let bad = vec![0u8; 16];
        let err = exec(&mut v, &mut ctx, &join(&bad)).unwrap_err();
        assert!(matches!(err, Error::Module(_)), "malformed key errs with Module");
        commit(&mut v);
        assert!(members(&v).1.is_empty(), "a rejected join adds nothing");
        assert_eq!(v.root(), StateRoot::ZERO);
    }

    #[test]
    fn root_is_state_based_order_independent() {
        let a = valid_key(3);
        let b = valid_key(4);

        let mut v1 = Valset::new("valset");
        let mut c1 = TestCtx::new();
        exec(&mut v1, &mut c1, &join(&a)).unwrap();
        exec(&mut v1, &mut c1, &join(&b)).unwrap();
        commit(&mut v1);

        let mut v2 = Valset::new("valset");
        let mut c2 = TestCtx::new();
        exec(&mut v2, &mut c2, &join(&b)).unwrap();
        exec(&mut v2, &mut c2, &join(&a)).unwrap();
        commit(&mut v2);

        assert_eq!(v1.root(), v2.root(), "root is f(state), order-independent");
    }

    #[test]
    fn root_distinguishes_active_from_standby() {
        // the same key active vs standby MUST commit to different roots —
        // the quorum projection is consensus-relevant state.
        let k = valid_key(6);
        let mut as_active = Valset::new("valset");
        as_active.insert(k.clone());
        let mut as_standby = Valset::new("valset");
        let mut ctx = TestCtx::new();
        exec(&mut as_standby, &mut ctx, &join(&k)).unwrap();
        commit(&mut as_standby);
        assert_ne!(as_active.root(), as_standby.root());
    }

    #[test]
    fn atomicity_a_failed_block_rolls_back_the_stage() {
        let mut v = Valset::new("valset");
        let mut ctx = TestCtx::new();
        let before = v.root();

        exec(&mut v, &mut ctx, &join(&valid_key(5))).unwrap();
        futures::executor::block_on(v.abort_block()).unwrap();

        assert!(members(&v).1.is_empty(), "aborted join added nothing");
        assert_eq!(v.root(), before, "root unchanged after a rolled-back block");
    }

    #[test]
    fn snapshot_install_round_trip_reconstructs_root_and_both_classes() {
        let mut src = Valset::new("valset");
        let mut ctx = TestCtx::new();
        src.insert(valid_key(1));
        src.insert(valid_key(2));
        exec(&mut src, &mut ctx, &join(&valid_key(3))).unwrap();
        commit(&mut src);
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);

        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        let mut dst = Valset::new("valset");
        let mut dctx = TestCtx::new();
        exec(&mut dst, &mut dctx, &join(&valid_key(9))).unwrap();

        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root);
        assert_eq!(members(&dst), members(&src), "both classes survive install");
    }

    #[test]
    fn tampered_snapshot_is_rejected_and_the_target_is_untouched() {
        let mut src = Valset::new("valset");
        src.insert(valid_key(4));
        src.insert(valid_key(5));
        let src_root = src.root();

        let mut bytes = src.snapshot();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;

        let mut dst = Valset::new("valset");
        let mut dctx = TestCtx::new();
        dst.insert(valid_key(8));
        exec(&mut dst, &mut dctx, &join(&valid_key(9))).unwrap();
        let pre_root = dst.root();
        let pre_view = members(&dst);

        let err = dst.install(&bytes, src_root).unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        assert_eq!(dst.root(), pre_root, "failed install leaves the root untouched");
        assert_eq!(members(&dst), pre_view, "membership and stage untouched");
    }

    #[test]
    fn truncated_trailing_forged_or_untagged_bytes_are_rejected() {
        let mut src = Valset::new("valset");
        src.insert(valid_key(6));
        src.insert(valid_key(7));
        let src_root = src.root();
        let bytes = src.snapshot();

        let mut dst = Valset::new("valset");
        dst.insert(valid_key(8));
        let before_root = dst.root();

        assert!(dst.install(&bytes[..bytes.len() - 1], src_root).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(dst.install(&trailing, src_root).is_err());
        let mut forged = bytes.clone();
        let tag = SNAPSHOT_TAG.len();
        forged[tag..tag + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(dst.install(&forged, src_root).is_err());
        // a v1 (untagged) stream can never install into a v2 module.
        assert!(dst.install(&2u64.to_le_bytes(), src_root).is_err());

        assert_eq!(dst.root(), before_root, "a failed install moved the root");
    }

    #[test]
    fn empty_snapshot_installs_onto_an_empty_state() {
        let src = Valset::new("valset");
        assert_eq!(src.root(), StateRoot::ZERO);
        let bytes = src.snapshot();
        let mut expected = SNAPSHOT_TAG.to_vec();
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(bytes, expected, "empty state is the tag plus two zero counts");

        let mut dst = Valset::new("valset");
        dst.install(&bytes, StateRoot::ZERO).unwrap();
        assert_eq!(dst.root(), StateRoot::ZERO);
    }
}
