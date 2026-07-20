//! the network lifecycle module: ONE app-hashed commitment covering both
//! height-gated coordination classes that used to live in the mirror-image
//! `upgrade` and `modreg` crates.
//!
//! it holds, folded into a single `root()`:
//!
//!   * the agreed node `current_version`, the SINGLE pending `ScheduledUpgrade`,
//!     and the per-validator upgrade-readiness set (the R=n binary roll); and
//!   * per hot-swappable module, the ACTIVE 32-byte code hash plus at most one
//!     pending `ScheduledSwap` with its own byte-receipt readiness latch (the
//!     height-gated wasm code swap).
//!
//! governance authorizes a schedule/cancel/register by emitting a host-drained
//! follow-up (origin `Module("governance")`); validators self-submit the
//! `UpgradeReady` / `SwapReady` signals; and the ONE system-injected `Advance`
//! boundary tick reconciles BOTH halves in a single dispatch — arming or
//! aborting the pending upgrade AND activating every armed code swap.
//!
//! ## activation is a pure derivation over FROZEN committed state
//!
//! both halves arm exactly once at their `activation_height`, gated on full
//! readiness (R=n). the `Advance` handler decides the arm set over the FROZEN
//! COMMITTED end-of-(H-1) state — never staged-over-committed — the SAME snapshot
//! the orchestrator `arm_verdict`, the host `effective_version(H)` stamp, and the
//! host `realize_module_swaps` boundary read all use, so live, recovery-replay,
//! and state-sync nodes all reconstruct the activation identically. the upgrade
//! half routes through the shared `effective_version` predicate; the swap half
//! applies the same `ready && activation_height <= height` gate the host's
//! `ArmedAt` read realizes.
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
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the minimum lead (in blocks) between the scheduling block and a pending
/// upgrade's `activation_height`, so `H` is strictly in every node's future and
/// never lands inside an armed epoch-cutover window. this is at least the
/// orchestrator's `cutover_delay` (=3, `bin/node/src/main.rs`).
pub const MIN_UPGRADE_LEAD: u64 = 3;

/// the minimum lead (in blocks) between the scheduling block and a swap's
/// `activation_height`, so `H` is strictly in every node's future — long enough
/// to fetch + verify the out-of-band bytes before the boundary.
pub const MIN_SWAP_LEAD: u64 = 3;

/// one validator's upgrade-readiness signal. idempotent per pubkey,
/// last-write-wins. the optional `commitment` (a fingerprint of the new logic)
/// is folded into `root()`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadySignal {
    commitment: Option<Vec<u8>>,
}

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
    // ---- protocol-version path ----
    /// monotonic agreed protocol version.
    current_version: u32,
    /// the single pending upgrade — AT MOST ONE ever exists.
    pending_upgrade: Option<ScheduledUpgrade>,
    /// upgrade-readiness signals, keyed by 32-byte member pubkey.
    upgrade_readiness: BTreeMap<Vec<u8>, ReadySignal>,
    // ---- module-code path ----
    /// per hot-swappable module, its active code hash + at most one pending swap.
    modules: BTreeMap<String, ModuleEntry>,
}

impl LifecycleState {
    /// the fully-uninitialized sentinel: no version, no pending, no readiness,
    /// no registered module. `root()` is `ZERO` exactly here.
    fn is_empty(&self) -> bool {
        self.current_version == 0
            && self.pending_upgrade.is_none()
            && self.upgrade_readiness.is_empty()
            && self.modules.is_empty()
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

    /// Raw committed upgrade coordinates for recovery preflight. Unlike
    /// `UpgradeStatus`, this needs no valset query and can validate a checkpoint
    /// before disk-backed modules are opened.
    pub fn committed_coordinates(&self) -> (u32, Option<ScheduledUpgrade>) {
        (
            self.committed.current_version,
            self.committed.pending_upgrade.clone(),
        )
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
    /// genesis app-hash and the network forks at genesis). never valid after
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

    // ---- protocol-version op handlers ---------------------------------------

    fn handle_schedule_upgrade(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        activation_height: u64,
        to_version: u32,
    ) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        let mut next = self.read().clone();
        // monotonic: never downgrade.
        if to_version <= next.current_version {
            return Err(Error::Module(format!(
                "upgrade to_version {to_version} must exceed current_version {} (monotonic)",
                next.current_version
            )));
        }
        // minimum lead: activation is strictly in the future, never retroactive.
        let floor = ctx.env().height.saturating_add(MIN_UPGRADE_LEAD);
        if activation_height <= floor {
            return Err(Error::Module(format!(
                "activation_height {activation_height} must exceed height+MIN_UPGRADE_LEAD ({floor})"
            )));
        }
        // at most one pending: cancel the current one first.
        if next.pending_upgrade.is_some() {
            return Err(Error::Module(
                "an upgrade is already pending (cancel it first)".into(),
            ));
        }
        next.pending_upgrade = Some(ScheduledUpgrade {
            name,
            activation_height,
            to_version,
        });
        // a fresh schedule clears any residual readiness.
        next.upgrade_readiness.clear();
        self.staged = Some(next);
        Ok(())
    }

    fn handle_cancel_upgrade(&mut self, ctx: &mut dyn Ctx, name: String) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        let height = ctx.env().height;
        let mut next = self.read().clone();
        match &next.pending_upgrade {
            Some(up) if up.name == name && height < up.activation_height => {}
            Some(up) if up.name == name => {
                return Err(Error::Module(
                    "cannot cancel: activation height already reached".into(),
                ));
            }
            _ => {
                return Err(Error::Module(
                    "no matching pending upgrade to cancel".into(),
                ));
            }
        }
        next.pending_upgrade = None;
        next.upgrade_readiness.clear();
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_upgrade_ready(
        &mut self,
        ctx: &mut dyn Ctx,
        name: String,
        to_version: u32,
        commitment: Option<Vec<u8>>,
    ) -> Result<(), Error> {
        // validator-origin only: the authenticated frame origin attributes the
        // signal to exactly one member key.
        let pubkey = match &ctx.env().origin {
            Origin::External(key) => key.clone(),
            other => {
                return Err(Error::Module(format!(
                    "UpgradeReady requires an external validator submitter, got {other:?}"
                )));
            }
        };
        // must be a CURRENT boundary member (dead weight otherwise).
        let members = self.members(ctx).await?;
        if !members.iter().any(|m| m == &pubkey) {
            return Err(Error::Module(
                "UpgradeReady submitter is not a current validator-set member".into(),
            ));
        }
        let mut next = self.read().clone();
        // identity scope: only for the currently-pending upgrade.
        match &next.pending_upgrade {
            Some(up) if up.name == name && up.to_version == to_version => {}
            _ => {
                return Err(Error::Module(
                    "UpgradeReady does not match the pending upgrade (name/to_version)".into(),
                ));
            }
        }
        // Preserve the pre-commitment execution contract: older binaries accept
        // and store any commitment bytes. Filtering happens only in the readable
        // ready set / arming predicate below, so mixed binaries keep identical
        // pre-H roots while a wrong artifact still cannot activate.
        next.upgrade_readiness
            .insert(pubkey, ReadySignal { commitment });
        self.staged = Some(next);
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
    /// realizes the initial code at the boundary.
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
        // version gate — see ADMISSION_ACTIVATION_VERSION: closes the
        // mixed-binary window (old binaries cannot decode this op; both sides
        // refuse identically below the activation version).
        if ctx.env().protocol_version < ADMISSION_ACTIVATION_VERSION {
            return Err(Error::Module(format!(
                "module admission activates at protocol v{ADMISSION_ACTIVATION_VERSION} \
                 (network is at v{})",
                ctx.env().protocol_version
            )));
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

    // ---- the shared boundary tick -------------------------------------------

    /// arm/abort the pending upgrade AND activate every armed code swap, in one
    /// dispatch. BOTH arm sets are decided over the FROZEN COMMITTED
    /// end-of-(H-1) state (NOT staged-over-committed) — the SAME snapshot the
    /// orchestrator `arm_verdict`, the host `effective_version(H)` stamp, and the
    /// host `realize_module_swaps` boundary read all use — so a `SignalReady`
    /// finalized as block H's own op cannot flip committed state while the sealed
    /// forge root was composed under the old selector. the reconciliation is then
    /// APPLIED over staged-over-committed (published at `commit_block`).
    async fn handle_advance(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        Self::require_system(ctx)?;
        let height = ctx.env().height;

        // decide over FROZEN committed state.
        let upgrade_pending = self.committed.pending_upgrade.clone();
        let upgrade_reached = upgrade_pending
            .as_ref()
            .is_some_and(|up| height >= up.activation_height);
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

        // nothing to reconcile on EITHER half: a true no-op (root untouched).
        if !upgrade_reached && armed_swaps.is_empty() {
            return Ok(());
        }

        let mut next = self.read().clone();

        // upgrade half: arm (flip current_version) iff every boundary member
        // signaled by end-of-(H-1), else abort — in both cases clear the slot.
        if let Some(up) = upgrade_pending.filter(|_| upgrade_reached) {
            let members = self.members(ctx).await?;
            // the shared predicate over FROZEN committed readiness (plan R4).
            let armed = up.to_version
                == crate::effective_version(
                    height,
                    self.committed.current_version,
                    Some(&up),
                    &members,
                    |member| {
                        self.committed
                            .upgrade_readiness
                            .get(member)
                            .is_some_and(|signal| {
                                crate::readiness_commitment_matches(
                                    &up.name,
                                    signal.commitment.as_deref(),
                                )
                            })
                    },
                );
            if armed {
                next.current_version = up.to_version;
            }
            next.pending_upgrade = None;
            next.upgrade_readiness.clear();
        }

        // swap half: flip every armed swap's active hash into the app-hash.
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

    async fn upgrade_status(&self, ctx: &dyn Ctx) -> Result<UpgradeStatus, Error> {
        let members = self.members(ctx).await?;
        let state = self.read();
        let ready: Vec<Vec<u8>> = state
            .upgrade_readiness
            .iter()
            .filter(|(_, signal)| {
                state.pending_upgrade.as_ref().is_none_or(|pending| {
                    crate::readiness_commitment_matches(&pending.name, signal.commitment.as_deref())
                })
            })
            .map(|(key, _)| key.clone())
            .collect();
        // `armed` (readiness complete) is derived from the ONE shared predicate
        // evaluated AT the activation height (where the height gate always
        // passes), so it can never drift from the arm check the Advance handler
        // and the host stamp use (risk R4).
        let armed = match &state.pending_upgrade {
            Some(up) => {
                up.to_version
                    == crate::effective_version(
                        up.activation_height,
                        state.current_version,
                        Some(up),
                        &members,
                        |member| ready.iter().any(|key| key == member),
                    )
            }
            None => false,
        };
        Ok(UpgradeStatus {
            current_version: state.current_version,
            pending: state.pending_upgrade.clone(),
            member_count: members.len() as u64,
            ready_count: ready.len() as u64,
            members,
            ready,
            armed,
        })
    }

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
        // ---- protocol-version part ----
        out.extend_from_slice(&s.current_version.to_le_bytes());
        match &s.pending_upgrade {
            None => out.push(0),
            Some(up) => {
                out.push(1);
                push_bytes(&mut out, up.name.as_bytes());
                out.extend_from_slice(&up.activation_height.to_le_bytes());
                out.extend_from_slice(&up.to_version.to_le_bytes());
            }
        }
        out.extend_from_slice(&(s.upgrade_readiness.len() as u64).to_le_bytes());
        for (key, sig) in &s.upgrade_readiness {
            push_bytes(&mut out, key);
            match &sig.commitment {
                None => out.push(0),
                Some(c) => {
                    out.push(1);
                    push_bytes(&mut out, c);
                }
            }
        }
        // ---- module-code part ----
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
        if Self::root_of(&decoded) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
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

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            LifecycleMsg::ScheduleUpgrade {
                name,
                activation_height,
                to_version,
            } => self.handle_schedule_upgrade(ctx, name, activation_height, to_version),
            LifecycleMsg::CancelUpgrade { name } => self.handle_cancel_upgrade(ctx, name),
            LifecycleMsg::UpgradeReady {
                name,
                to_version,
                commitment,
            } => {
                self.handle_upgrade_ready(ctx, name, to_version, commitment)
                    .await
            }
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

    /// read projection with host-routed access to the valset — the
    /// `UpgradeStatus` verdict (`member_count`/`ready_count`/`armed`) needs the
    /// boundary set; the module-code projections do not.
    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            LifecycleQuery::UpgradeStatus => {
                let status = self.upgrade_status(ctx).await?;
                Ok(encode_reply(&LifecycleReply::UpgradeStatus(status)))
            }
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

    // ---- protocol-version part ----
    let current_version = cur.u32("snapshot current_version")?;
    let pending_upgrade = match cur.byte("snapshot upgrade pending tag")? {
        0 => None,
        1 => {
            let name = cur.string("snapshot upgrade name")?;
            let activation_height = cur.u64("snapshot upgrade activation height")?;
            let to_version = cur.u32("snapshot upgrade to_version")?;
            Some(ScheduledUpgrade {
                name,
                activation_height,
                to_version,
            })
        }
        other => {
            return Err(Error::Module(format!(
                "snapshot: bad upgrade pending tag {other}"
            )));
        }
    };
    let ready_count = cur.u64("snapshot upgrade readiness count")?;
    // each readiness entry costs at least an 8-byte key-length prefix + a 1-byte
    // commitment tag — a forged count can never drive allocation past the buffer.
    cur.bound(ready_count, 9, "snapshot upgrade readiness")?;
    let mut upgrade_readiness = BTreeMap::new();
    let mut prev_key: Option<Vec<u8>> = None;
    for _ in 0..ready_count {
        let key = cur.bytes("snapshot upgrade readiness key")?.to_vec();
        // strictly increasing keys: one state has exactly one encoding.
        if prev_key.as_deref().is_some_and(|p| p >= key.as_slice()) {
            return Err(Error::Module(
                "snapshot upgrade readiness keys must be strictly increasing".into(),
            ));
        }
        let commitment = match cur.byte("snapshot upgrade commitment tag")? {
            0 => None,
            1 => Some(cur.bytes("snapshot upgrade commitment")?.to_vec()),
            other => {
                return Err(Error::Module(format!(
                    "snapshot: bad upgrade commitment tag {other}"
                )));
            }
        };
        prev_key = Some(key.clone());
        upgrade_readiness.insert(key, ReadySignal { commitment });
    }

    // ---- module-code part ----
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
    Ok(LifecycleState {
        current_version,
        pending_upgrade,
        upgrade_readiness,
        modules,
    })
}

#[cfg(test)]
mod tests;
