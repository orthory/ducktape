//! the client ACL as replicated state: a single ed25519 key set.
//!
//! a CLIENT holds SUBMIT authorization at a validator's door and nothing else —
//! no consensus seat, no mesh, no statesync standing. this is the thin-client
//! (Design 4) foundation: a `role=Client` invite, when redeemed, records a key
//! here (governance emits [`ClientsMsg::Grant`]); the submit door then admits a
//! client's own-signed frame. it is DELIBERATELY a separate module from valset:
//! PR6 keyed statesync fail-closed off `members ∪ residents`, so keeping clients
//! out of valset makes it structurally impossible for a client to ever obtain
//! statesync standing — the sync door reads valset, never this set.
//!
//! ## state model (cloned from valset's resident-set discipline, ONE set)
//!
//! `execute` STAGES into a `pending` overlay (committed state untouched);
//! `query` reads pending-over-committed (read-your-writes); `commit_block`
//! merges pending into committed; `abort_block` drops pending; `root()`
//! reflects COMMITTED state only — a state-based (sorted, length-prefixed)
//! sha256 over the client set, so it is order-independent and idempotent.
//!
//! ## authorization
//!
//! Grant/Revoke are MODULE-ORIGIN-GATED exactly like valset membership: only a
//! module origin (governance's redeem follow-up) or a system origin (genesis)
//! may stage a change. an external key cannot self-grant client standing.

use std::collections::{BTreeMap, BTreeSet};

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519::PublicKey;
use sdk::codec;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// a 32-byte ed25519 public key encoding.
const KEY_LEN: usize = 32;

// ---- wire surface -----------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientsMsg {
    /// grant CLIENT standing (submit authorization only). `key` MUST be a
    /// 32-byte ed25519 public key; the impl rejects a malformed key.
    Grant { key: Vec<u8> },
    /// revoke client standing by key. a no-op if the key is not a client.
    Revoke { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientsQuery {
    /// the full committed client set.
    Clients,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientsReply {
    /// the committed clients, sorted (order-independent).
    Clients(Vec<Vec<u8>>),
}

pub fn encode_msg(m: &ClientsMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<ClientsMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &ClientsQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<ClientsQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &ClientsReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<ClientsReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

// ---- module -----------------------------------------------------------------

pub struct Clients {
    id: ModuleId,
    /// committed client standing — what `root()`/`snapshot()` commit to.
    clients: BTreeSet<Vec<u8>>,
    /// this block's staged changes: `true` == staged add, `false` == staged
    /// remove. read ahead of committed (read-your-writes), merged on `commit_block`.
    pending: BTreeMap<Vec<u8>, bool>,
}

impl Clients {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            clients: BTreeSet::new(),
            pending: BTreeMap::new(),
        }
    }

    /// validate that `key` is a well-formed 32-byte ed25519 public key — the
    /// explicit length guard makes the 32-byte invariant independent of decode's
    /// trailing-byte behavior; `PublicKey::decode` then checks the curve point.
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

    /// the committed set with this block's staged changes applied — sorted.
    fn effective(&self) -> Vec<Vec<u8>> {
        let mut set: BTreeSet<Vec<u8>> = self.clients.clone();
        for (k, present) in &self.pending {
            if *present {
                set.insert(k.clone());
            } else {
                set.remove(k);
            }
        }
        set.into_iter().collect()
    }

    /// canonical bytes of the COMMITTED state — exactly what `root()` hashes:
    /// count (u64-le), then each sorted key's len (u64-le) + bytes. so for a
    /// non-empty set `sha256(snapshot()) == root()`; empty snapshots to a zero
    /// count (root still `ZERO`, unhashed). pending is excluded — a snapshot
    /// ships what consensus committed to.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_set(&self.clients)
    }

    fn encode_set(set: &BTreeSet<Vec<u8>>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(set.len() as u64).to_le_bytes());
        for k in set {
            codec::push_bytes(&mut out, k);
        }
        out
    }

    /// replace committed state with a decoded snapshot, iff the decoded state's
    /// recomputed root equals `expected`. self is mutated only after both decode
    /// and root-check pass, so on any `Err` committed state and `root()` are
    /// byte-identical to before. success clears pending.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let clients = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&clients);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.clients = clients;
        self.pending.clear();
        Ok(())
    }

    /// strict decode of UNTRUSTED snapshot bytes: count checked against the
    /// remaining buffer BEFORE allocation, truncation and trailing bytes both
    /// reject, keys must arrive strictly increasing — one state, one encoding.
    fn decode_snapshot(bytes: &[u8]) -> Result<BTreeSet<Vec<u8>>, Error> {
        let mut cur = codec::Cursor::new(bytes);
        let count = cur.u64("snapshot clients")?;
        cur.bound(count, 8, "snapshot clients")?;
        let mut set = BTreeSet::new();
        let mut prev: Option<Vec<u8>> = None;
        for _ in 0..count {
            let key = cur.bytes("snapshot clients")?;
            if prev.as_deref().is_some_and(|p| p >= key) {
                return Err(Error::Module(
                    "snapshot keys must be strictly increasing".into(),
                ));
            }
            prev = Some(key.to_vec());
            set.insert(key.to_vec());
        }
        cur.finish("snapshot")?;
        Ok(set)
    }

    /// the state-based commitment: `ZERO` when empty, else sha256 over exactly
    /// the bytes `snapshot` emits. shared by `root()` and `install`.
    fn root_of(clients: &BTreeSet<Vec<u8>>) -> StateRoot {
        if clients.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::encode_set(clients)).into())
    }
}

/// the CURRENT client set at `clients`: its staged-over-committed projection,
/// via the host-routed read lane. mirrors [`valset::members`] — the one shared
/// read the redeem path (and the submit door's caller) funnels through.
pub async fn clients(ctx: &dyn Ctx, clients_id: &str) -> Result<Vec<Vec<u8>>, Error> {
    let reply = ctx
        .query(clients_id, &encode_query(&ClientsQuery::Clients))
        .await?;
    match decode_reply(&reply).map_err(Error::Module)? {
        ClientsReply::Clients(list) => Ok(list),
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Clients {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment over the COMMITTED set: a length-prefixed sha256
    /// over the sorted clients. order-independent and idempotent; empty -> ZERO.
    fn root(&self) -> StateRoot {
        Self::root_of(&self.clients)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // client standing changes are GOVERNANCE-GATED, same discipline as
        // valset: only a module origin (governance's redeem follow-up) or a
        // system origin (genesis) may stage them. an external key cannot
        // self-grant. origin is part of the deterministic Env, enforced
        // identically on every node.
        match &ctx.env().origin {
            sdk::Origin::Module(_) | sdk::Origin::System => {}
            sdk::Origin::External(_) => {
                return Err(Error::Module(
                    "client standing changes only via governance".into(),
                ));
            }
        }
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ClientsMsg::Grant { key } => {
                Self::validate_key(&key)?;
                self.pending.insert(key, true);
            }
            ClientsMsg::Revoke { key } => {
                self.pending.insert(key, false);
            }
        }
        Ok(())
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ClientsQuery::Clients => Ok(encode_reply(&ClientsReply::Clients(self.effective()))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (k, present) in std::mem::take(&mut self.pending) {
            if present {
                self.clients.insert(k);
            } else {
                self.clients.remove(&k);
            }
        }
        Ok(())
    }

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

    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn new(origin: sdk::Origin) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin,
                    me: "clients".into(),
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
    }

    fn valid_key(seed_byte: u8) -> Vec<u8> {
        let seed = [seed_byte; 32];
        let sk = PrivateKey::decode(&seed[..]).expect("any 32 bytes is a valid seed");
        sk.public_key().as_ref().to_vec()
    }

    fn grant(key: &[u8]) -> Msg {
        Msg {
            target: "clients".into(),
            payload: encode_msg(&ClientsMsg::Grant { key: key.to_vec() }),
        }
    }
    fn revoke(key: &[u8]) -> Msg {
        Msg {
            target: "clients".into(),
            payload: encode_msg(&ClientsMsg::Revoke { key: key.to_vec() }),
        }
    }
    fn clients_of(c: &Clients) -> Vec<Vec<u8>> {
        let reply =
            futures::executor::block_on(c.query(&encode_query(&ClientsQuery::Clients))).unwrap();
        match decode_reply(&reply).unwrap() {
            ClientsReply::Clients(list) => list,
        }
    }

    #[test]
    fn grant_from_a_module_origin_inserts_and_moves_root_off_zero() {
        let mut c = Clients::new("clients");
        let mut ctx = TestCtx::new(sdk::Origin::Module("governance".into()));
        assert_eq!(c.root(), StateRoot::ZERO, "empty set -> ZERO");

        let k = valid_key(1);
        futures::executor::block_on(c.execute(&mut ctx, &grant(&k))).unwrap();
        // staged, not committed: root still ZERO, but read-your-writes sees it.
        assert_eq!(c.root(), StateRoot::ZERO, "root reflects committed only");
        assert_eq!(clients_of(&c), vec![k.clone()], "read-your-writes");

        futures::executor::block_on(c.commit_block()).unwrap();
        assert_ne!(c.root(), StateRoot::ZERO, "a committed grant moves the root");
        assert_eq!(clients_of(&c), vec![k]);
    }

    #[test]
    fn grant_from_an_external_origin_is_refused() {
        let mut c = Clients::new("clients");
        let mut ctx = TestCtx::new(sdk::Origin::External(valid_key(9)));
        let err = futures::executor::block_on(c.execute(&mut ctx, &grant(&valid_key(1)))).unwrap_err();
        assert!(
            matches!(err, Error::Module(ref m) if m.contains("only via governance")),
            "got {err:?}"
        );
        futures::executor::block_on(c.commit_block()).unwrap();
        assert!(clients_of(&c).is_empty(), "a refused grant adds nothing");
    }

    #[test]
    fn revoke_removes_and_restores_the_exact_empty_root() {
        let mut c = Clients::new("clients");
        let mut ctx = TestCtx::new(sdk::Origin::System);
        let empty_snapshot = c.snapshot();
        let k = valid_key(2);
        futures::executor::block_on(c.execute(&mut ctx, &grant(&k))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_eq!(clients_of(&c), vec![k.clone()]);

        futures::executor::block_on(c.execute(&mut ctx, &revoke(&k))).unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert!(clients_of(&c).is_empty(), "revoke removed it");
        assert_eq!(c.root(), StateRoot::ZERO);
        assert_eq!(
            c.snapshot(),
            empty_snapshot,
            "revoking the last client restores the exact empty snapshot bytes"
        );
    }

    #[test]
    fn malformed_key_is_rejected() {
        let mut c = Clients::new("clients");
        let mut ctx = TestCtx::new(sdk::Origin::System);
        let err =
            futures::executor::block_on(c.execute(&mut ctx, &grant(&[0u8; 16]))).unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        futures::executor::block_on(c.commit_block()).unwrap();
        assert!(clients_of(&c).is_empty());
    }

    #[test]
    fn root_is_state_based_order_independent() {
        let (a, b) = (valid_key(3), valid_key(4));
        let build = |first: &[u8], second: &[u8]| {
            let mut c = Clients::new("clients");
            let mut ctx = TestCtx::new(sdk::Origin::System);
            futures::executor::block_on(c.execute(&mut ctx, &grant(first))).unwrap();
            futures::executor::block_on(c.execute(&mut ctx, &grant(second))).unwrap();
            futures::executor::block_on(c.commit_block()).unwrap();
            c
        };
        assert_eq!(build(&a, &b).root(), build(&b, &a).root(), "root is f(state)");
    }

    #[test]
    fn snapshot_install_round_trips_and_rejects_forgeries() {
        let mut src = Clients::new("clients");
        let mut ctx = TestCtx::new(sdk::Origin::System);
        for seed in [1u8, 2, 3] {
            futures::executor::block_on(src.execute(&mut ctx, &grant(&valid_key(seed)))).unwrap();
        }
        futures::executor::block_on(src.commit_block()).unwrap();
        let src_root = src.root();
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        // TARGET with an unrelated staged grant — install must drop it.
        let mut dst = Clients::new("clients");
        let mut dctx = TestCtx::new(sdk::Origin::System);
        futures::executor::block_on(dst.execute(&mut dctx, &grant(&valid_key(9)))).unwrap();
        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root);
        assert_eq!(clients_of(&dst), clients_of(&src));

        // a flipped bit inside a key is caught only by the recomputed-root check.
        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(dst.install(&tampered, src_root).is_err());
        // truncation and trailing garbage both reject.
        assert!(dst.install(&bytes[..bytes.len() - 1], src_root).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(dst.install(&trailing, src_root).is_err());
        // a failed install left the target's committed state untouched.
        assert_eq!(dst.root(), src_root);
        assert_eq!(clients_of(&dst), clients_of(&src));
    }

    #[test]
    fn empty_snapshot_installs_onto_an_empty_set() {
        let src = Clients::new("clients");
        assert_eq!(src.root(), StateRoot::ZERO);
        let bytes = src.snapshot();
        assert_eq!(bytes, [0u8; 8].to_vec(), "empty is a single zero count");
        let mut dst = Clients::new("clients");
        dst.install(&bytes, StateRoot::ZERO).unwrap();
        assert_eq!(dst.root(), StateRoot::ZERO);
        assert!(clients_of(&dst).is_empty());
    }
}
