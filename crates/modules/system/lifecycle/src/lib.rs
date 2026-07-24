//! the network lifecycle module: the root-hashed commitment to WHICH code every
//! hot-swappable module runs.
//!
//! it holds, folded into a single `root()`: per hot-swappable module, the
//! ACTIVE 32-byte code hash plus at most one pending `ScheduledSwap` with its
//! byte-receipt readiness latch (the height-gated wasm code swap).
//!
//! governance authorizes a schedule/cancel/register by emitting a host-drained
//! follow-up (origin `Module("governance")`); validators self-submit the
//! `SwapReady` signals; and the ONE system-injected `Advance` boundary tick
//! activates every armed code swap in a single dispatch.
//!
//! ## activation is a pure derivation over FROZEN committed state
//!
//! a swap arms exactly once at its `activation_height`, gated on full readiness
//! (R=n). the `Advance` handler decides the arm set over the FROZEN COMMITTED
//! end-of-(H-1) state — never staged-over-committed — the SAME snapshot the
//! host `realize_module_swaps` boundary read uses, so live, recovery-replay,
//! and state-sync nodes all reconstruct the activation identically, applying
//! the `ready && activation_height <= height` gate the host's `ArmedAt` read
//! realizes.
//!
//! ## code bytes are out-of-band
//!
//! the module commits only the 32-byte code hash. component BYTES are
//! content-addressed and distributed out-of-band (blobstore / state-sync); the
//! host verifies `sha256(bytes) == code_hash` before swapping registry code, and
//! a node lacking the bytes at the boundary fails closed (never forks).
//!
//! ## state model
//!
//! a whole-state overlay (like governance): `execute` STAGES the whole
//! `LifecycleState` into an overlay (committed untouched), `read()` sees
//! staged-over-committed, `commit_block` publishes, `abort_block` discards;
//! `root()` is sha256 over the canonical encoding of COMMITTED state
//! (`StateRoot::ZERO` when fully uninitialized), and `snapshot`/`install` ship
//! exactly that root preimage (verify-then-adopt).

mod interface;
pub use interface::*;

use std::collections::BTreeMap;

use sdk::codec::{Cursor, push_bytes};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
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

/// the whole committed (or staged) lifecycle state — small enough to overlay
/// wholesale.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct LifecycleState {
    /// per hot-swappable module, its active code hash + at most one pending swap.
    modules: BTreeMap<String, ModuleEntry>,
}

impl LifecycleState {
    /// the fully-uninitialized sentinel: no registered module. `root()` is
    /// `ZERO` exactly here.
    fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

pub struct Lifecycle {
    id: ModuleId,
    /// the valset module the readiness denominator (boundary member set) comes
    /// from, via host-routed queries. genesis wiring — identical on every node.
    valset_id: ModuleId,
    committed: LifecycleState,
    staged: Option<LifecycleState>,
}

impl Lifecycle {
    pub fn new(id: impl Into<ModuleId>, valset_id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            committed: LifecycleState::default(),
            staged: None,
        }
    }

    /// whether any registered module carries a pending swap in committed state.
    pub fn has_pending_swaps(&self) -> bool {
        self.committed
            .modules
            .values()
            .any(|entry| entry.pending.is_some())
    }

    /// GENESIS seeding: install a module's initial active code hash directly into
    /// committed state, BEFORE the host registers this instance. deterministic
    /// and identical on every node (a different seed set composes a different
    /// genesis root-hash and the network forks at genesis). never valid after
    /// genesis: live changes go through `RegisterModule`/`ScheduleSwap` ops.
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
    fn read(&self) -> &LifecycleState {
        self.staged.as_ref().unwrap_or(&self.committed)
    }

    /// the CURRENT boundary member set: the valset module's staged-over-committed
    /// projection, via the shared `valset::members` read.
    async fn members(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        valset::members(ctx, &self.valset_id).await
    }

    /// register/schedule/cancel are governance/system-authored, never external.
    fn require_module_or_system(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::Module(_) | Origin::System => Ok(()),
            Origin::External(_) => Err(Error::Module(
                "lifecycle schedule/cancel/register only via governance (module) or system origin"
                    .into(),
            )),
        }
    }

    /// the boundary tick is system-injected only.
    fn require_system(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::System => Ok(()),
            other => Err(Error::Module(format!(
                "lifecycle Advance is a system boundary tick, got {other:?}"
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

    // ---- module-code op handlers --------------------------------------------

    async fn handle_register_module(
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
                "module {module_id} is already registered (code changes go through ScheduleSwap)"
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

    async fn handle_schedule_swap(
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
            Error::Module(format!(
                "cannot schedule a swap for unregistered module {module_id}"
            ))
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

    /// admission of a brand-new module: like `handle_schedule_swap`, but the entry
    /// must NOT exist yet. the created entry carries an EMPTY active hash —
    /// "registered, not yet running" — and the normal readiness/advance machinery
    /// realizes the initial code at the boundary. KNOWN GAP: the recovery /
    /// state-sync composers still enumerate a fixed module set, so a node
    /// restarting past an admitted module's first checkpoint fails closed until
    /// the admitted-module restore path lands.
    async fn handle_schedule_register(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: Vec<u8>,
    ) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        Self::require_hash_len(&code_hash)?;
        if module_id.is_empty() {
            return Err(Error::Module("module_id must not be empty".into()));
        }
        // an id already LIVE on this host — native, genesis-wasm, or a prior
        // admission — may not be re-admitted. the registry set is consensus
        // state, so this read is identical on every validator. (the map check
        // below still covers admission-pending ids, whose root does not exist
        // yet.)
        if ctx.module_root(&module_id).is_some() {
            return Err(Error::Module(format!(
                "module id {module_id} is already live on this host"
            )));
        }
        let mut next = self.read().clone();
        if next.modules.contains_key(&module_id) {
            return Err(Error::Module(format!(
                "module {module_id} is already registered (code changes go through ScheduleSwap)"
            )));
        }
        let floor = ctx.env().height.saturating_add(MIN_SWAP_LEAD);
        if activation_height <= floor {
            return Err(Error::Module(format!(
                "activation_height {activation_height} must exceed height+MIN_SWAP_LEAD ({floor})"
            )));
        }
        next.modules.insert(
            module_id,
            ModuleEntry {
                active_code_hash: Vec::new(),
                pending: Some(ScheduledSwap {
                    name,
                    activation_height,
                    code_hash,
                    readiness: Vec::new(),
                    ready: false,
                }),
            },
        );
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_cancel_swap(
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
                return Err(Error::Module("no matching pending swap to cancel".into()));
            }
        }
        entry.pending = None;
        // cancelling an ADMISSION (empty active hash: the module never ran)
        // removes the entry entirely — lifecycle must never claim a codeless module.
        if entry.active_code_hash.is_empty() {
            next.modules.remove(&module_id);
        }
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_swap_ready(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        module_id: String,
    ) -> Result<(), Error> {
        // validator-origin only: the authenticated frame origin attributes the
        // signal to exactly one member key.
        let pubkey = match &ctx.env().origin {
            Origin::External(key) => key.clone(),
            other => {
                return Err(Error::Module(format!(
                    "SwapReady requires an external validator submitter, got {other:?}"
                )));
            }
        };
        let members = self.members(ctx).await?;
        if !members.iter().any(|m| m == &pubkey) {
            return Err(Error::Module(
                "SwapReady submitter is not a current validator-set member".into(),
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
                    "SwapReady does not match the pending swap (name/module)".into(),
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
        if !members.is_empty()
            && members
                .iter()
                .all(|m| swap.readiness.binary_search(m).is_ok())
        {
            swap.ready = true;
        }
        self.staged = Some(next);
        Ok(())
    }

    // ---- the boundary tick --------------------------------------------------

    /// activate every armed code swap in one dispatch. the arm set is decided
    /// over the FROZEN COMMITTED end-of-(H-1) state (NOT staged-over-committed)
    /// — the SAME snapshot the host `realize_module_swaps` boundary read uses —
    /// so a `SignalReady` finalized as block H's own op cannot flip committed
    /// state mid-boundary. the reconciliation is then APPLIED over
    /// staged-over-committed (published at `commit_block`).
    async fn handle_advance(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        Self::require_system(ctx)?;
        let height = ctx.env().height;

        // decide over FROZEN committed state.
        let armed_swaps: Vec<String> = self
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

        // nothing to reconcile: a true no-op (root untouched).
        if armed_swaps.is_empty() {
            return Ok(());
        }

        let mut next = self.read().clone();

        // flip every armed swap's active hash into the root-hash.
        for id in armed_swaps {
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

    fn module_status(&self) -> LifecycleReply {
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
        LifecycleReply::ModuleStatus { modules }
    }

    fn armed_at(&self, height: u64) -> LifecycleReply {
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
        LifecycleReply::ArmedAt { swaps }
    }

    // ---- canonical state bytes (root preimage + snapshot format) ------------

    fn encode_state(s: &LifecycleState) -> Vec<u8> {
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
                    for pubkey in &swap.readiness {
                        push_bytes(&mut out, pubkey);
                    }
                }
            }
        }
        out
    }

    /// sha256 over the canonical encoding; `ZERO` iff the state is fully
    /// uninitialized.
    fn root_of(s: &LifecycleState) -> StateRoot {
        if s.is_empty() {
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
        sdk::verify_snapshot_root(Self::root_of(&decoded), expected)?;
        self.committed = decoded;
        self.staged = None;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Lifecycle {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            LifecycleMsg::RegisterModule {
                module_id,
                code_hash,
            } => self.handle_register_module(ctx, module_id, code_hash).await,
            LifecycleMsg::ScheduleSwap {
                name,
                module_id,
                activation_height,
                code_hash,
            } => {
                self.handle_schedule_swap(ctx, name, module_id, activation_height, code_hash)
                    .await
            }
            LifecycleMsg::ScheduleRegister {
                name,
                module_id,
                activation_height,
                code_hash,
            } => {
                self.handle_schedule_register(ctx, name, module_id, activation_height, code_hash)
                    .await
            }
            LifecycleMsg::CancelSwap { name, module_id } => {
                self.handle_cancel_swap(ctx, name, module_id).await
            }
            LifecycleMsg::SwapReady { name, module_id } => {
                self.handle_swap_ready(ctx, name, module_id).await
            }
            LifecycleMsg::Advance => self.handle_advance(ctx).await,
        }
    }

    /// read projection — the module-code projections need no host routing.
    async fn query_with(&self, _ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            LifecycleQuery::ModuleStatus => Ok(encode_reply(&self.module_status())),
            LifecycleQuery::ArmedAt { height } => Ok(encode_reply(&self.armed_at(height))),
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

// ---- strict snapshot decode (untrusted bytes) -------------------------------

fn decode_state(bytes: &[u8]) -> Result<LifecycleState, Error> {
    let mut cur = Cursor::new(bytes);

    let module_count = cur.u64("snapshot module count")?;
    // each module costs at least an 8-byte id-length prefix + an 8-byte active
    // hash-length prefix + a 1-byte pending tag — bound before looping.
    cur.bound(module_count, 17, "snapshot modules")?;
    let mut modules = BTreeMap::new();
    let mut prev_id: Option<String> = None;
    for _ in 0..module_count {
        let id = cur.string("snapshot module id")?;
        // strictly increasing ids: one state has exactly one encoding.
        if prev_id.as_deref().is_some_and(|p| p >= id.as_str()) {
            return Err(Error::Module(
                "snapshot module ids must be strictly increasing".into(),
            ));
        }
        let active_code_hash = cur.bytes("snapshot module active hash")?.to_vec();
        // EMPTY = admission-pending (`ScheduleRegister`): registered, no active
        // code until its boundary realizes the initial hash.
        if active_code_hash.len() != CODE_HASH_LEN && !active_code_hash.is_empty() {
            return Err(Error::Module("snapshot: bad active hash length".into()));
        }
        let pending = match cur.byte("snapshot module pending tag")? {
            0 => None,
            1 => {
                let name = cur.string("snapshot swap name")?;
                let activation_height = cur.u64("snapshot swap activation height")?;
                let code_hash = cur.bytes("snapshot swap code hash")?.to_vec();
                if code_hash.len() != CODE_HASH_LEN {
                    return Err(Error::Module("snapshot: bad pending hash length".into()));
                }
                let ready = match cur.byte("snapshot swap ready tag")? {
                    0 => false,
                    1 => true,
                    other => {
                        return Err(Error::Module(format!("snapshot: bad ready tag {other}")));
                    }
                };
                let signals = cur.u64("snapshot swap readiness count")?;
                cur.bound(signals, 8, "snapshot swap readiness")?;
                let mut readiness = Vec::with_capacity(signals as usize);
                let mut prev_sig: Option<Vec<u8>> = None;
                for _ in 0..signals {
                    let pubkey = cur.bytes("snapshot swap readiness key")?.to_vec();
                    if prev_sig.as_ref().is_some_and(|p| p >= &pubkey) {
                        return Err(Error::Module(
                            "snapshot swap readiness keys must be strictly increasing".into(),
                        ));
                    }
                    prev_sig = Some(pubkey.clone());
                    readiness.push(pubkey);
                }
                Some(ScheduledSwap {
                    name,
                    activation_height,
                    code_hash,
                    readiness,
                    ready,
                })
            }
            other => {
                return Err(Error::Module(format!(
                    "snapshot: bad module pending tag {other}"
                )));
            }
        };
        prev_id = Some(id.clone());
        modules.insert(
            id,
            ModuleEntry {
                active_code_hash,
                pending,
            },
        );
    }

    cur.finish("snapshot")?;
    Ok(LifecycleState { modules })
}

#[cfg(test)]
mod tests;
