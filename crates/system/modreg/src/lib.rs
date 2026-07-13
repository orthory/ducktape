//! the module code registry: a height-gated, governance-authorized commitment
//! to WHICH wasm code each hot-swappable module runs.
//!
//! a small dedicated system module composed into the app-hash, mirroring
//! `upgrade`. it holds, per module, the ACTIVE 32-byte code hash and AT MOST ONE
//! pending `ScheduledSwap { name, activation_height, code_hash }` — all folded
//! into `root()`. governance authorizes a schedule/cancel by emitting a
//! host-drained follow-up (origin `Module("governance")`); the system-injected
//! `Advance` boundary tick activates every swap whose `activation_height` has
//! been reached.
//!
//! ## activation = height floor AND full byte receipt
//!
//! a swap arms only when `ready && activation_height <= env.height`. `ready`
//! mirrors `upgrade`'s R=n readiness gate for the BYTES: each validator
//! verifies the target component locally and self-submits a `SignalReady`;
//! the signal that completes coverage of the boundary member set LATCHES the
//! committed `ready` flag (in-block, where the valset is queryable). the arm
//! set thus stays a pure, total function of COMMITTED modreg state + height —
//! identical on every node (live, replay, state-sync) — while guaranteeing
//! no swap ever activates onto a validator set that has not demonstrably
//! received the code. the `handle_cancel` guard (`height <
//! activation_height`) forbids a same-block cancel of an arming swap, so
//! deciding the arm set over COMMITTED (frozen end-of-(H-1)) state can never
//! diverge from the host's out-of-block `ArmedAt` read, which realizes the
//! registry swap.
//!
//! ## code bytes are out-of-band
//!
//! modreg commits only the 32-byte hash. the component BYTES are content-
//! addressed and distributed out-of-band (blobstore / state-sync); the host
//! verifies `sha256(bytes) == code_hash` before swapping registry code, and a
//! node lacking the bytes at the boundary fails closed (never forks).
//!
//! ## state model
//!
//! mirrors `upgrade`: `execute` STAGES the whole state into an overlay
//! (committed untouched), `read()` sees staged-over-committed, `commit_block`
//! publishes, `abort_block` discards; `root()` is sha256 over the canonical
//! encoding of COMMITTED state (`StateRoot::ZERO` when empty), and
//! `snapshot`/`install` ship exactly that preimage (verify-then-adopt).

mod interface;
pub use interface::*;

use std::collections::BTreeMap;

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the minimum lead (in blocks) between the scheduling block and a swap's
/// `activation_height`, so `H` is strictly in every node's future — long enough
/// to fetch + verify the out-of-band bytes before the boundary.
pub const MIN_SWAP_LEAD: u64 = 3;

/// one registered module's code state.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ModuleEntry {
    active_code_hash: Vec<u8>,
    pending: Option<ScheduledSwap>,
}

/// the whole committed (or staged) registry state — small enough to overlay
/// wholesale, exactly like `upgrade` stages its whole state.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct ModregState {
    modules: BTreeMap<String, ModuleEntry>,
}

pub struct Modreg {
    id: ModuleId,
    /// the valset module the readiness denominator (boundary member set)
    /// comes from, via host-routed queries — exactly like `upgrade`.
    valset_id: ModuleId,
    committed: ModregState,
    staged: Option<ModregState>,
}

impl Modreg {
    pub fn new(id: impl Into<ModuleId>, valset_id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            committed: ModregState::default(),
            staged: None,
        }
    }

    /// the CURRENT boundary member set: the valset module's staged-over-
    /// committed projection, via the shared `valset::members` read.
    async fn members(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        valset::members(ctx, &self.valset_id).await
    }

    /// GENESIS seeding: install a module's initial active code hash directly
    /// into committed state, BEFORE the host registers this instance — the same
    /// pattern as `Valset::insert`. deterministic and identical on every node
    /// (a different seed set composes a different genesis app-hash and the
    /// network forks at genesis). never valid after genesis: live changes go
    /// through `Register`/`Schedule` ops.
    pub fn seed(&mut self, module_id: impl Into<String>, code_hash: Vec<u8>) {
        assert_eq!(
            code_hash.len(),
            CODE_HASH_LEN,
            "genesis code hash must be {CODE_HASH_LEN} bytes"
        );
        self.committed.modules.insert(
            module_id.into(),
            ModuleEntry {
                active_code_hash: code_hash,
                pending: None,
            },
        );
    }

    /// staged-over-committed read (read-your-writes within a block).
    fn read(&self) -> &ModregState {
        self.staged.as_ref().unwrap_or(&self.committed)
    }

    /// register/schedule/cancel are governance/system-authored, never external.
    fn require_module_or_system(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::Module(_) | Origin::System => Ok(()),
            Origin::External(_) => Err(Error::Module(
                "modreg register/schedule/cancel only via governance (module) or system origin"
                    .into(),
            )),
        }
    }

    /// the boundary tick is system-injected only.
    fn require_system(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::System => Ok(()),
            other => Err(Error::Module(format!(
                "modreg Advance is a system boundary tick, got {other:?}"
            ))),
        }
    }

    fn require_hash_len(code_hash: &[u8]) -> Result<(), Error> {
        if code_hash.len() != CODE_HASH_LEN {
            return Err(Error::Module(format!(
                "code_hash must be {CODE_HASH_LEN} bytes, got {}",
                code_hash.len()
            )));
        }
        Ok(())
    }

    // ---- op handlers --------------------------------------------------------

    async fn handle_register(
        &mut self,
        ctx: &mut dyn Ctx,
        module_id: String,
        code_hash: Vec<u8>,
    ) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        Self::require_hash_len(&code_hash)?;
        let mut next = self.read().clone();
        if next.modules.contains_key(&module_id) {
            return Err(Error::Module(format!(
                "module {module_id} is already registered (code changes go through Schedule)"
            )));
        }
        next.modules.insert(
            module_id,
            ModuleEntry {
                active_code_hash: code_hash,
                pending: None,
            },
        );
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_schedule(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: Vec<u8>,
    ) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        Self::require_hash_len(&code_hash)?;
        let mut next = self.read().clone();
        let entry = next.modules.get_mut(&module_id).ok_or_else(|| {
            Error::Module(format!("cannot schedule a swap for unregistered module {module_id}"))
        })?;
        // minimum lead: activation is strictly in the future, never retroactive.
        let floor = ctx.env().height.saturating_add(MIN_SWAP_LEAD);
        if activation_height <= floor {
            return Err(Error::Module(format!(
                "activation_height {activation_height} must exceed height+MIN_SWAP_LEAD ({floor})"
            )));
        }
        // a swap to the currently-active code is a no-op — reject it.
        if code_hash == entry.active_code_hash {
            return Err(Error::Module(
                "scheduled code_hash equals the active code (no-op swap)".into(),
            ));
        }
        // at most one pending swap per module.
        if entry.pending.is_some() {
            return Err(Error::Module(format!(
                "module {module_id} already has a pending swap (cancel it first)"
            )));
        }
        entry.pending = Some(ScheduledSwap {
            name,
            activation_height,
            code_hash,
            readiness: Vec::new(),
            ready: false,
        });
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_cancel(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        module_id: String,
    ) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        let height = ctx.env().height;
        let mut next = self.read().clone();
        let entry = next
            .modules
            .get_mut(&module_id)
            .ok_or_else(|| Error::Module(format!("no such module {module_id}")))?;
        match &entry.pending {
            // can only cancel BEFORE the boundary — never race an arming swap.
            Some(swap) if swap.name == name && height < swap.activation_height => {}
            Some(swap) if swap.name == name => {
                return Err(Error::Module(
                    "cannot cancel: activation height already reached".into(),
                ));
            }
            _ => {
                return Err(Error::Module(
                    "no matching pending swap to cancel".into(),
                ));
            }
        }
        entry.pending = None;
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_signal_ready(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        module_id: String,
    ) -> Result<(), Error> {
        // validator-origin only: the authenticated frame origin attributes
        // the signal to exactly one member key (mirrors upgrade).
        let pubkey = match &ctx.env().origin {
            Origin::External(key) => key.clone(),
            other => {
                return Err(Error::Module(format!(
                    "SignalReady requires an external validator submitter, got {other:?}"
                )));
            }
        };
        let members = self.members(ctx).await?;
        if !members.iter().any(|m| m == &pubkey) {
            return Err(Error::Module(
                "SignalReady submitter is not a current validator-set member".into(),
            ));
        }
        let mut next = self.read().clone();
        let entry = next
            .modules
            .get_mut(&module_id)
            .ok_or_else(|| Error::Module(format!("no such module {module_id}")))?;
        let swap = match &mut entry.pending {
            Some(swap) if swap.name == name => swap,
            _ => {
                return Err(Error::Module(
                    "SignalReady does not match the pending swap (name/module)".into(),
                ));
            }
        };
        // idempotent per pubkey; the readiness list stays strictly increasing
        // (one committed state has exactly one encoding).
        if let Err(at) = swap.readiness.binary_search(&pubkey) {
            swap.readiness.insert(at, pubkey);
        }
        // the covering signal LATCHES ready (R = n at this instant). a member
        // admitted later heals through the fetch lane, never un-arms a swap.
        if !members.is_empty() && members.iter().all(|m| swap.readiness.binary_search(m).is_ok())
        {
            swap.ready = true;
        }
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_advance(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        Self::require_system(ctx)?;
        let height = ctx.env().height;
        // decide the arm set over FROZEN committed (end-of-(H-1)) state — the SAME
        // snapshot the host's out-of-block `ArmedAt` read realizes into registry
        // swaps. handle_cancel forbids a same-block cancel of an arming swap, so
        // committed and staged agree on the arm set; we still apply over
        // staged-over-committed for read-your-writes on any same-block register.
        let armed: Vec<String> = self
            .committed
            .modules
            .iter()
            .filter(|(_, e)| {
                e.pending
                    .as_ref()
                    .is_some_and(|p| p.ready && height >= p.activation_height)
            })
            .map(|(id, _)| id.clone())
            .collect();
        if armed.is_empty() {
            return Ok(());
        }
        let mut next = self.read().clone();
        for id in armed {
            if let Some(entry) = next.modules.get_mut(&id)
                && let Some(swap) = entry.pending.take()
            {
                entry.active_code_hash = swap.code_hash;
            }
        }
        self.staged = Some(next);
        Ok(())
    }

    // ---- queries ------------------------------------------------------------

    fn status(&self) -> ModregReply {
        let modules = self
            .read()
            .modules
            .iter()
            .map(|(id, e)| ModuleCode {
                module_id: id.clone(),
                active_code_hash: e.active_code_hash.clone(),
                pending: e.pending.clone(),
            })
            .collect();
        ModregReply::Status { modules }
    }

    fn armed_at(&self, height: u64) -> ModregReply {
        let swaps = self
            .read()
            .modules
            .iter()
            .filter_map(|(id, e)| {
                e.pending.as_ref().and_then(|p| {
                    (p.ready && height >= p.activation_height).then(|| ArmedSwap {
                        module_id: id.clone(),
                        code_hash: p.code_hash.clone(),
                    })
                })
            })
            .collect();
        ModregReply::ArmedAt { swaps }
    }

    // ---- canonical state bytes (root preimage + snapshot format) ------------

    fn encode_state(s: &ModregState) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(s.modules.len() as u64).to_le_bytes());
        for (id, entry) in &s.modules {
            push_bytes(&mut out, id.as_bytes());
            push_bytes(&mut out, &entry.active_code_hash);
            match &entry.pending {
                None => out.push(0),
                Some(swap) => {
                    out.push(1);
                    push_bytes(&mut out, swap.name.as_bytes());
                    out.extend_from_slice(&swap.activation_height.to_le_bytes());
                    push_bytes(&mut out, &swap.code_hash);
                    out.push(u8::from(swap.ready));
                    out.extend_from_slice(&(swap.readiness.len() as u64).to_le_bytes());
                    for key in &swap.readiness {
                        push_bytes(&mut out, key);
                    }
                }
            }
        }
        out
    }

    /// sha256 over the canonical encoding; `ZERO` iff no module is registered.
    fn root_of(s: &ModregState) -> StateRoot {
        if s.modules.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update(Self::encode_state(s));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state — the exact preimage of `root()`.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.committed)
    }

    /// verify-then-adopt a peer snapshot: decode, recompute the root, refuse on
    /// mismatch — committed state and stage untouched on any error.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let decoded = decode_state(bytes)?;
        if Self::root_of(&decoded) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.committed = decoded;
        self.staged = None;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Modreg {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ModregMsg::Register {
                module_id,
                code_hash,
            } => self.handle_register(ctx, module_id, code_hash).await,
            ModregMsg::Schedule {
                name,
                module_id,
                activation_height,
                code_hash,
            } => {
                self.handle_schedule(ctx, name, module_id, activation_height, code_hash)
                    .await
            }
            ModregMsg::Cancel { name, module_id } => self.handle_cancel(ctx, name, module_id).await,
            ModregMsg::SignalReady { name, module_id } => {
                self.handle_signal_ready(ctx, name, module_id).await
            }
            ModregMsg::Advance => self.handle_advance(ctx).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ModregQuery::Status => Ok(encode_reply(&self.status())),
            ModregQuery::ArmedAt { height } => Ok(encode_reply(&self.armed_at(height))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(s) = self.staged.take() {
            self.committed = s;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged = None;
        Ok(())
    }
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------

fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
    let Some((head, rest)) = buf.split_first_chunk::<8>() else {
        return Err(Error::Module("snapshot truncated".into()));
    };
    *buf = rest;
    Ok(u64::from_le_bytes(*head))
}

fn take_u8(buf: &mut &[u8]) -> Result<u8, Error> {
    let Some((head, rest)) = buf.split_first() else {
        return Err(Error::Module("snapshot truncated".into()));
    };
    let v = *head;
    *buf = rest;
    Ok(v)
}

fn take_vec(buf: &mut &[u8]) -> Result<Vec<u8>, Error> {
    let len = take_u64(buf)?;
    if len > buf.len() as u64 {
        return Err(Error::Module("snapshot length exceeds buffer".into()));
    }
    let (head, rest) = buf.split_at(len as usize);
    *buf = rest;
    Ok(head.to_vec())
}

fn take_string(buf: &mut &[u8]) -> Result<String, Error> {
    String::from_utf8(take_vec(buf)?).map_err(|_| Error::Module("snapshot: bad utf-8".into()))
}

fn decode_state(bytes: &[u8]) -> Result<ModregState, Error> {
    let mut buf = bytes;
    let count = take_u64(&mut buf)?;
    // each module costs at least an 8-byte id-length prefix + a 40-byte hash
    // field + a 1-byte pending tag — a forged count can never over-allocate.
    if count > (buf.len() / 9) as u64 {
        return Err(Error::Module("snapshot module count exceeds buffer".into()));
    }
    let mut modules = BTreeMap::new();
    let mut prev: Option<String> = None;
    for _ in 0..count {
        let id = take_string(&mut buf)?;
        // strictly increasing ids: one state has exactly one encoding.
        if prev.as_deref().is_some_and(|p| p >= id.as_str()) {
            return Err(Error::Module(
                "snapshot module ids must be strictly increasing".into(),
            ));
        }
        let active_code_hash = take_vec(&mut buf)?;
        if active_code_hash.len() != CODE_HASH_LEN {
            return Err(Error::Module("snapshot: bad active hash length".into()));
        }
        let pending = match take_u8(&mut buf)? {
            0 => None,
            1 => {
                let name = take_string(&mut buf)?;
                let activation_height = take_u64(&mut buf)?;
                let code_hash = take_vec(&mut buf)?;
                if code_hash.len() != CODE_HASH_LEN {
                    return Err(Error::Module("snapshot: bad pending hash length".into()));
                }
                let ready = match take_u8(&mut buf)? {
                    0 => false,
                    1 => true,
                    other => {
                        return Err(Error::Module(format!("snapshot: bad ready tag {other}")));
                    }
                };
                let signals = take_u64(&mut buf)?;
                // each key costs at least its 8-byte length prefix.
                if signals > (buf.len() / 8) as u64 {
                    return Err(Error::Module(
                        "snapshot readiness count exceeds buffer".into(),
                    ));
                }
                let mut readiness = Vec::with_capacity(signals as usize);
                let mut prev_key: Option<Vec<u8>> = None;
                for _ in 0..signals {
                    let key = take_vec(&mut buf)?;
                    // strictly increasing keys: one state, one encoding.
                    if prev_key.as_ref().is_some_and(|p| p >= &key) {
                        return Err(Error::Module(
                            "snapshot readiness keys must be strictly increasing".into(),
                        ));
                    }
                    prev_key = Some(key.clone());
                    readiness.push(key);
                }
                Some(ScheduledSwap {
                    name,
                    activation_height,
                    code_hash,
                    readiness,
                    ready,
                })
            }
            other => return Err(Error::Module(format!("snapshot: bad pending tag {other}"))),
        };
        prev = Some(id.clone());
        modules.insert(id, ModuleEntry {
            active_code_hash,
            pending,
        });
    }
    if !buf.is_empty() {
        return Err(Error::Module("snapshot carries trailing bytes".into()));
    }
    Ok(ModregState { modules })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCtx {
        env: sdk::Env,
        /// what the stubbed valset answers a Validators query with.
        members: Vec<Vec<u8>>,
    }
    impl TestCtx {
        fn new(origin: Origin, height: u64) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height,
                    consensus_time: 0,
                    origin,
                    me: "modreg".into(),
                },
                members: vec![member(1)],
            }
        }
        fn with_members(mut self, members: Vec<Vec<u8>>) -> Self {
            self.members = members;
            self
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
        async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
            if target == "valset"
                && matches!(
                    valset::decode_query(req),
                    Ok(valset::ValsetQuery::Validators)
                )
            {
                return Ok(valset::encode_reply(&valset::ValsetReply::Validators(
                    self.members.clone(),
                )));
            }
            Err(Error::QueryUnsupported)
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
    }

    fn member(seed: u8) -> Vec<u8> {
        vec![seed; 32]
    }

    fn hash(seed: u8) -> Vec<u8> {
        vec![seed; CODE_HASH_LEN]
    }

    fn mr() -> Modreg {
        Modreg::new("modreg", "valset")
    }

    fn msg(m: ModregMsg) -> Msg {
        Msg {
            target: "modreg".into(),
            payload: encode_msg(&m),
        }
    }

    fn run(mr: &mut Modreg, ctx: &mut TestCtx, m: &Msg) -> Result<(), Error> {
        futures::executor::block_on(mr.execute(ctx, m))
    }
    fn commit(mr: &mut Modreg) {
        futures::executor::block_on(mr.commit_block()).unwrap();
    }
    fn status(mr: &Modreg) -> Vec<ModuleCode> {
        let bytes =
            futures::executor::block_on(mr.query(&encode_query(&ModregQuery::Status))).unwrap();
        match decode_reply(&bytes).unwrap() {
            ModregReply::Status { modules } => modules,
            other => panic!("expected Status, got {other:?}"),
        }
    }
    fn armed_at(mr: &Modreg, height: u64) -> Vec<ArmedSwap> {
        let bytes =
            futures::executor::block_on(mr.query(&encode_query(&ModregQuery::ArmedAt { height })))
                .unwrap();
        match decode_reply(&bytes).unwrap() {
            ModregReply::ArmedAt { swaps } => swaps,
            other => panic!("expected ArmedAt, got {other:?}"),
        }
    }

    fn register(mr: &mut Modreg, module_id: &str, code: u8) {
        let mut sys = TestCtx::new(Origin::System, 0);
        run(
            mr,
            &mut sys,
            &msg(ModregMsg::Register {
                module_id: module_id.into(),
                code_hash: hash(code),
            }),
        )
        .unwrap();
        commit(mr);
    }

    fn schedule(module_id: &str, name: &str, ah: u64, code: u8) -> Msg {
        msg(ModregMsg::Schedule {
            name: name.into(),
            module_id: module_id.into(),
            activation_height: ah,
            code_hash: hash(code),
        })
    }
    fn cancel(module_id: &str, name: &str) -> Msg {
        msg(ModregMsg::Cancel {
            name: name.into(),
            module_id: module_id.into(),
        })
    }
    fn signal(module_id: &str, name: &str) -> Msg {
        msg(ModregMsg::SignalReady {
            name: name.into(),
            module_id: module_id.into(),
        })
    }
    /// drive the full single-member readiness latch for a pending swap.
    fn make_ready(mr: &mut Modreg, module_id: &str, name: &str) {
        let mut ext = TestCtx::new(Origin::External(member(1)), 0);
        run(mr, &mut ext, &signal(module_id, name)).unwrap();
        commit(mr);
    }
    fn advance() -> Msg {
        msg(ModregMsg::Advance)
    }

    // ---- origin gates -------------------------------------------------------

    #[test]
    fn register_and_schedule_origin_gate() {
        let mut mr = mr();
        // External register rejected.
        let mut ext = TestCtx::new(Origin::External(vec![1; 32]), 0);
        assert!(matches!(
            run(
                &mut mr,
                &mut ext,
                &msg(ModregMsg::Register {
                    module_id: "hello".into(),
                    code_hash: hash(1)
                })
            ),
            Err(Error::Module(_))
        ));
        // governance (Module) accepted.
        let mut gov = TestCtx::new(Origin::Module("governance".into()), 0);
        run(
            &mut mr,
            &mut gov,
            &msg(ModregMsg::Register {
                module_id: "hello".into(),
                code_hash: hash(1),
            }),
        )
        .unwrap();
        commit(&mut mr);
        assert_eq!(status(&mr)[0].active_code_hash, hash(1));

        // External schedule rejected; Module accepted.
        let mut ext2 = TestCtx::new(Origin::External(vec![1; 32]), 0);
        assert!(matches!(
            run(&mut mr, &mut ext2, &schedule("hello", "v2", 10, 2)),
            Err(Error::Module(_))
        ));
        run(&mut mr, &mut gov, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);
        assert!(status(&mr)[0].pending.is_some());
    }

    #[test]
    fn advance_is_system_only() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut gov = TestCtx::new(Origin::Module("governance".into()), 10);
        assert!(matches!(
            run(&mut mr, &mut gov, &advance()),
            Err(Error::Module(_))
        ));
    }

    // ---- register discipline ------------------------------------------------

    #[test]
    fn register_rejects_reregistration() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        assert!(
            matches!(
                run(
                    &mut mr,
                    &mut sys,
                    &msg(ModregMsg::Register {
                        module_id: "hello".into(),
                        code_hash: hash(9)
                    })
                ),
                Err(Error::Module(_))
            ),
            "re-register must go through Schedule"
        );
    }

    #[test]
    fn register_rejects_bad_hash_len() {
        let mut mr = mr();
        let mut sys = TestCtx::new(Origin::System, 0);
        assert!(matches!(
            run(
                &mut mr,
                &mut sys,
                &msg(ModregMsg::Register {
                    module_id: "hello".into(),
                    code_hash: vec![1, 2, 3]
                })
            ),
            Err(Error::Module(_))
        ));
    }

    // ---- schedule validation ------------------------------------------------

    #[test]
    fn schedule_requires_registered_module() {
        let mut mr = mr();
        let mut sys = TestCtx::new(Origin::System, 0);
        assert!(matches!(
            run(&mut mr, &mut sys, &schedule("ghost", "v2", 10, 2)),
            Err(Error::Module(_))
        ));
    }

    #[test]
    fn schedule_min_lead_and_noop_and_one_pending() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        // min lead: height 0 + 3 -> activation must be > 3.
        assert!(matches!(
            run(&mut mr, &mut sys, &schedule("hello", "v2", 3, 2)),
            Err(Error::Module(_))
        ));
        // no-op swap (same as active) rejected.
        assert!(matches!(
            run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 1)),
            Err(Error::Module(_))
        ));
        // valid schedule accepted.
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);
        // a second pending swap rejected.
        assert!(matches!(
            run(&mut mr, &mut sys, &schedule("hello", "v3", 20, 3)),
            Err(Error::Module(_))
        ));
    }

    // ---- advance arms at height ---------------------------------------------

    #[test]
    fn advance_activates_at_height_and_frees_slot() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);
        make_ready(&mut mr, "hello", "v2");

        // below activation: no-op.
        let root_before = mr.root();
        let mut below = TestCtx::new(Origin::System, 9);
        run(&mut mr, &mut below, &advance()).unwrap();
        commit(&mut mr);
        assert_eq!(mr.root(), root_before, "below activation is a no-op");
        assert_eq!(status(&mr)[0].active_code_hash, hash(1));

        // at activation: swap active -> pending code, slot freed.
        let mut at = TestCtx::new(Origin::System, 10);
        run(&mut mr, &mut at, &advance()).unwrap();
        commit(&mut mr);
        assert_eq!(status(&mr)[0].active_code_hash, hash(2), "active swapped");
        assert!(status(&mr)[0].pending.is_none(), "slot freed");

        // a second, higher schedule now succeeds.
        run(&mut mr, &mut sys, &schedule("hello", "v3", 20, 3)).unwrap();
        commit(&mut mr);
        assert!(status(&mr)[0].pending.is_some());
    }

    #[test]
    fn armed_at_reports_the_swaps_the_host_realizes() {
        let mut mr = mr();
        register(&mut mr, "a", 1);
        register(&mut mr, "b", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("a", "v2", 10, 2)).unwrap();
        commit(&mut mr);
        run(&mut mr, &mut sys, &schedule("b", "v2", 20, 2)).unwrap();
        commit(&mut mr);
        make_ready(&mut mr, "a", "v2");
        make_ready(&mut mr, "b", "v2");

        assert!(armed_at(&mr, 9).is_empty());
        let at10 = armed_at(&mr, 10);
        assert_eq!(at10.len(), 1);
        assert_eq!(at10[0].module_id, "a");
        assert_eq!(at10[0].code_hash, hash(2));
        assert_eq!(armed_at(&mr, 20).len(), 2, "both armed by height 20");
    }

    // ---- the readiness gate ---------------------------------------------------

    #[test]
    fn no_readiness_means_no_arm_however_high_the_height() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);

        assert!(armed_at(&mr, u64::MAX).is_empty(), "unready never arms");
        let mut late = TestCtx::new(Origin::System, 999);
        run(&mut mr, &mut late, &advance()).unwrap();
        commit(&mut mr);
        assert_eq!(status(&mr)[0].active_code_hash, hash(1), "no swap");
        assert!(status(&mr)[0].pending.is_some(), "pending survives");
    }

    #[test]
    fn partial_coverage_does_not_latch_full_coverage_does() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);

        let two = vec![member(1), member(2)];
        // first of two members: recorded, not latched.
        let mut m1 =
            TestCtx::new(Origin::External(member(1)), 0).with_members(two.clone());
        run(&mut mr, &mut m1, &signal("hello", "v2")).unwrap();
        commit(&mut mr);
        let pending = status(&mr)[0].pending.clone().unwrap();
        assert_eq!(pending.readiness, vec![member(1)]);
        assert!(!pending.ready);
        assert!(armed_at(&mr, 10).is_empty());

        // re-signal is idempotent.
        let mut again =
            TestCtx::new(Origin::External(member(1)), 0).with_members(two.clone());
        run(&mut mr, &mut again, &signal("hello", "v2")).unwrap();
        commit(&mut mr);
        assert_eq!(status(&mr)[0].pending.clone().unwrap().readiness.len(), 1);

        // the covering signal latches.
        let mut m2 = TestCtx::new(Origin::External(member(2)), 0).with_members(two);
        run(&mut mr, &mut m2, &signal("hello", "v2")).unwrap();
        commit(&mut mr);
        assert!(status(&mr)[0].pending.clone().unwrap().ready);
        assert_eq!(armed_at(&mr, 10).len(), 1, "ready + height arms");
    }

    #[test]
    fn signal_gates_origin_membership_and_identity() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);

        // system/module origins rejected.
        assert!(matches!(
            run(&mut mr, &mut sys, &signal("hello", "v2")),
            Err(Error::Module(_))
        ));
        // a non-member key rejected.
        let mut stranger = TestCtx::new(Origin::External(member(9)), 0);
        assert!(matches!(
            run(&mut mr, &mut stranger, &signal("hello", "v2")),
            Err(Error::Module(_))
        ));
        // a wrong swap name rejected.
        let mut m1 = TestCtx::new(Origin::External(member(1)), 0);
        assert!(matches!(
            run(&mut mr, &mut m1, &signal("hello", "vX")),
            Err(Error::Module(_))
        ));
        // and readiness survives snapshot/install byte-for-byte.
        make_ready(&mut mr, "hello", "v2");
        let bytes = mr.snapshot();
        let mut dst = Modreg::new("modreg", "valset");
        dst.install(&bytes, mr.root()).unwrap();
        assert_eq!(dst.committed, mr.committed);
        assert!(status(&dst)[0].pending.clone().unwrap().ready);
    }

    // ---- cancel -------------------------------------------------------------

    #[test]
    fn cancel_guards_and_clears() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut mr);

        // wrong name rejected.
        assert!(matches!(
            run(&mut mr, &mut sys, &cancel("hello", "vX")),
            Err(Error::Module(_))
        ));
        // at/after activation rejected (never race an arming swap).
        let mut late = TestCtx::new(Origin::System, 10);
        assert!(matches!(
            run(&mut mr, &mut late, &cancel("hello", "v2")),
            Err(Error::Module(_))
        ));
        // valid cancel clears the pending.
        run(&mut mr, &mut sys, &cancel("hello", "v2")).unwrap();
        commit(&mut mr);
        assert!(status(&mr)[0].pending.is_none());
        // and the active code is untouched.
        assert_eq!(status(&mr)[0].active_code_hash, hash(1));
    }

    // ---- root discipline + snapshot/install ---------------------------------

    #[test]
    fn root_zero_fresh_nonzero_after_register() {
        let mut mr = mr();
        assert_eq!(mr.root(), StateRoot::ZERO, "fresh module is ZERO");
        let mut sys = TestCtx::new(Origin::System, 0);
        run(
            &mut mr,
            &mut sys,
            &msg(ModregMsg::Register {
                module_id: "hello".into(),
                code_hash: hash(1),
            }),
        )
        .unwrap();
        assert_eq!(mr.root(), StateRoot::ZERO, "staged only: committed still ZERO");
        commit(&mut mr);
        assert_ne!(mr.root(), StateRoot::ZERO, "committed register moves off ZERO");
    }

    #[test]
    fn swap_activation_advances_root_then_reproduces_via_snapshot() {
        let mut src = mr();
        register(&mut src, "hello", 1);
        let root_active1 = src.root();
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut src, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        commit(&mut src);
        make_ready(&mut src, "hello", "v2");
        let mut at = TestCtx::new(Origin::System, 10);
        run(&mut src, &mut at, &advance()).unwrap();
        commit(&mut src);
        let root_active2 = src.root();
        assert_ne!(root_active1, root_active2, "activating a swap changes the root");

        // snapshot IS the root preimage; install reconstructs.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), root_active2, "sha256(snapshot()) == root()");
        let mut dst = mr();
        dst.install(&bytes, root_active2).unwrap();
        assert_eq!(dst.root(), root_active2);
        assert_eq!(dst.committed, src.committed);
    }

    #[test]
    fn install_rejects_tamper_and_leaves_target_untouched() {
        let mut src = mr();
        register(&mut src, "hello", 1);
        let root = src.root();
        let bytes = src.snapshot();

        let mut dst = mr();
        register(&mut dst, "other", 7);
        let pre = dst.root();

        // flip a byte in the MIDDLE of the buffer (inside the fixed-width active
        // hash) so the bytes still decode cleanly and it is the verify-then-adopt
        // root check — not a decode error — that rejects them.
        let mut flipped = bytes.clone();
        let mid = flipped.len() / 2;
        flipped[mid] ^= 0x01;
        assert!(matches!(
            dst.install(&flipped, root).unwrap_err(),
            Error::Module(m) if m.contains("root mismatch")
        ));
        // truncation + trailing byte rejected.
        assert!(dst.install(&bytes[..bytes.len() - 1], root).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(dst.install(&trailing, root).is_err());

        assert_eq!(dst.root(), pre, "failed install left the target untouched");
    }

    #[test]
    fn abort_discards_staged() {
        let mut mr = mr();
        register(&mut mr, "hello", 1);
        let before = mr.root();
        let mut sys = TestCtx::new(Origin::System, 0);
        run(&mut mr, &mut sys, &schedule("hello", "v2", 10, 2)).unwrap();
        futures::executor::block_on(mr.abort_block()).unwrap();
        assert_eq!(mr.root(), before, "aborted schedule leaves root untouched");
        assert!(status(&mr)[0].pending.is_none());
    }
}
