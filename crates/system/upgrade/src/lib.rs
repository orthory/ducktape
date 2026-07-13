//! the height-gated, no-downtime node-upgrade module.
//!
//! a small dedicated system module composed into the app-hash, mirroring
//! valset/governance. it holds the agreed `current_version`, the SINGLE pending
//! `ScheduledUpgrade { name, activation_height, to_version }`, and the per-validator
//! readiness set — all folded into `root()`, so all covered by the global
//! app-hash. governance authorizes a schedule/cancel by emitting a host-drained
//! follow-up (origin `Module("governance")`); each current validator emits a
//! `SignalReady` op once running the new binary; the system-injected `Advance`
//! boundary tick arms (flips `current_version` to `to_version`) iff EVERY current
//! boundary member has signaled, else aborts cleanly — in both cases clearing the
//! pending slot so a second upgrade can be scheduled.
//!
//! ## activation is a pure derivation, never a stored flip
//!
//! `effective_version` (the ONE shared predicate at the crate root) is a pure,
//! total function of the COMMITTED state. the module's
//! `Advance` handler re-evaluates the byte-identical predicate against the FROZEN
//! COMMITTED end-of-(H-1) state (NOT staged-over-committed) — the SAME snapshot the
//! orchestrator `arm_verdict` and the host `effective_version(H)` stamp read — so a
//! `SignalReady` finalized as block H's own op cannot flip committed state while
//! the sealed forge root was composed under the old selector; live, recovery-replay,
//! and state-sync nodes all reconstruct the activation identically. the per-node
//! BUILD constant `MAX_PROTOCOL_VERSION` is deliberately NOT here — it varies per
//! node and would fork consensus state; the module only records signals.
//!
//! ## state model
//!
//! mirrors governance's staged-over-committed seam: `execute` STAGES the whole
//! small `UpgradeState` into an overlay (committed untouched), `read()` sees
//! staged-over-committed, `commit_block` publishes, `abort_block` discards;
//! `root()` is sha256 over the canonical encoding of COMMITTED state,
//! `StateRoot::ZERO` as the uninitialized sentinel, and `snapshot`/`install` ship
//! exactly that root preimage (verify-then-adopt).

// the wire surface: this module's shared types, flattened at the crate root.
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

/// one validator's readiness signal for the pending upgrade. idempotent per
/// pubkey, last-write-wins. the optional `commitment` (a fingerprint of the new
/// logic) is folded into `root()`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadySignal {
    commitment: Option<Vec<u8>>,
}

/// the whole committed (or staged) upgrade state — small enough to overlay
/// wholesale, exactly like governance stages whole proposals.
#[derive(Clone, Debug, PartialEq, Eq)]
struct UpgradeState {
    /// monotonic agreed protocol version.
    current_version: u32,
    /// the single pending upgrade — AT MOST ONE ever exists.
    pending: Option<ScheduledUpgrade>,
    /// readiness signals for the pending upgrade, keyed by 32-byte member pubkey.
    readiness: BTreeMap<Vec<u8>, ReadySignal>,
}

impl UpgradeState {
    fn genesis() -> Self {
        Self {
            current_version: 0,
            pending: None,
            readiness: BTreeMap::new(),
        }
    }
}

pub struct Upgrade {
    id: ModuleId,
    /// the id of the valset module this instance reads the boundary member set
    /// from. genesis wiring — identical on every node.
    valset_id: ModuleId,
    /// committed state — what `root()` and the app-hash commit to.
    committed: UpgradeState,
    /// this block's staged writes (whole-state overwrite granularity), read
    /// ahead of committed state, published at `commit_block`.
    staged: Option<UpgradeState>,
}

impl Upgrade {
    pub fn new(id: impl Into<ModuleId>, valset_id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            committed: UpgradeState::genesis(),
            staged: None,
        }
    }

    /// staged-over-committed read (read-your-writes within a block).
    fn read(&self) -> &UpgradeState {
        self.staged.as_ref().unwrap_or(&self.committed)
    }

    /// the CURRENT boundary member set: the valset module's staged-over-committed
    /// projection, via the shared `valset::members` read (exactly like governance).
    async fn members(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        valset::members(ctx, &self.valset_id).await
    }

    /// schedule/cancel are governance/system-authored, never external.
    fn require_module_or_system(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::Module(_) | Origin::System => Ok(()),
            Origin::External(_) => Err(Error::Module(
                "upgrade schedule/cancel only via governance (module) or system origin".into(),
            )),
        }
    }

    /// the boundary tick is system-injected only.
    fn require_system(ctx: &dyn Ctx) -> Result<(), Error> {
        match &ctx.env().origin {
            Origin::System => Ok(()),
            other => Err(Error::Module(format!(
                "upgrade Advance is a system boundary tick, got {other:?}"
            ))),
        }
    }

    // ---- the op handlers ----------------------------------------------------

    fn handle_schedule(
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
        if next.pending.is_some() {
            return Err(Error::Module(
                "an upgrade is already pending (cancel it first)".into(),
            ));
        }
        next.pending = Some(ScheduledUpgrade {
            name,
            activation_height,
            to_version,
        });
        // a fresh schedule clears any residual readiness.
        next.readiness.clear();
        self.staged = Some(next);
        Ok(())
    }

    fn handle_cancel(&mut self, ctx: &mut dyn Ctx, name: String) -> Result<(), Error> {
        Self::require_module_or_system(ctx)?;
        let height = ctx.env().height;
        let mut next = self.read().clone();
        match &next.pending {
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
        next.pending = None;
        next.readiness.clear();
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_signal_ready(
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
                    "SignalReady requires an external validator submitter, got {other:?}"
                )));
            }
        };
        // must be a CURRENT boundary member (dead weight otherwise).
        let members = self.members(ctx).await?;
        if !members.iter().any(|m| m == &pubkey) {
            return Err(Error::Module(
                "SignalReady submitter is not a current validator-set member".into(),
            ));
        }
        let mut next = self.read().clone();
        // identity scope: only for the currently-pending upgrade.
        match &next.pending {
            Some(up) if up.name == name && up.to_version == to_version => {}
            _ => {
                return Err(Error::Module(
                    "SignalReady does not match the pending upgrade (name/to_version)".into(),
                ));
            }
        }
        // Preserve the pre-commitment execution contract: older binaries accept
        // and store any commitment bytes. Filtering happens only in the readable
        // ready set / arming predicate below, so mixed binaries keep identical
        // pre-H roots while a wrong artifact still cannot activate.
        next.readiness.insert(pubkey, ReadySignal { commitment });
        self.staged = Some(next);
        Ok(())
    }

    async fn handle_advance(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        Self::require_system(ctx)?;
        let height = ctx.env().height;
        // The arm/abort verdict MUST read the FROZEN COMMITTED end-of-(H-1) state,
        // NOT staged-over-committed — the SAME snapshot the orchestrator arm_verdict
        // and the host effective_version(H) stamp read (both query out-of-block,
        // where staged is empty). Deciding over `self.read()` (staged) here would
        // let a SignalReady finalized AS block H's own op — staged earlier in this
        // very drain — flip committed current_version to to_version, while every
        // other site, having frozen readiness at n-1 at the boundary, sealed forge
        // under the OLD selector. That split (committed current_version = new, forge
        // root sealed under old) is invisible live (all nodes agree) yet halts every
        // node that later restarts / state-syncs across H, because they rebuild
        // active_version from the flat committed current_version. So decide over
        // `self.committed`; a late same-block signal then cleanly ABORTS (the
        // boundary quorum was genuinely unmet at H-1) and the operator reschedules.
        let pending = self.committed.pending.clone();
        if let Some(up) = pending
            && height >= up.activation_height
        {
            let members = self.members(ctx).await?;
            // the shared predicate over FROZEN committed readiness (plan R4): armed
            // iff every boundary member had signaled by end-of-(H-1).
            let armed = up.to_version
                == crate::effective_version(
                    height,
                    self.committed.current_version,
                    Some(&up),
                    &members,
                    |member| {
                        self.committed.readiness.get(member).is_some_and(|signal| {
                            crate::readiness_commitment_matches(
                                &up.name,
                                signal.commitment.as_deref(),
                            )
                        })
                    },
                );
            // apply the reconciliation over staged-over-committed (published at
            // commit_block): ARM flips current_version + clears the slot; ABORT
            // clears the slot, leaving current_version unchanged.
            let mut next = self.read().clone();
            if armed {
                next.current_version = up.to_version;
            }
            next.pending = None;
            next.readiness.clear();
            self.staged = Some(next);
        }
        Ok(())
    }

    // ---- canonical state bytes (root preimage + snapshot format) -----------

    fn encode_state(s: &UpgradeState) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&s.current_version.to_le_bytes());
        match &s.pending {
            None => out.push(0),
            Some(up) => {
                out.push(1);
                push_bytes(&mut out, up.name.as_bytes());
                out.extend_from_slice(&up.activation_height.to_le_bytes());
                out.extend_from_slice(&up.to_version.to_le_bytes());
            }
        }
        out.extend_from_slice(&(s.readiness.len() as u64).to_le_bytes());
        for (key, sig) in &s.readiness {
            push_bytes(&mut out, key);
            match &sig.commitment {
                None => out.push(0),
                Some(c) => {
                    out.push(1);
                    push_bytes(&mut out, c);
                }
            }
        }
        out
    }

    /// sha256 over the canonical encoding; `ZERO` iff `current_version == 0 &&
    /// pending.is_none() && readiness.is_empty()` (the uninitialized sentinel).
    fn root_of(s: &UpgradeState) -> StateRoot {
        if s.current_version == 0 && s.pending.is_none() && s.readiness.is_empty() {
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

    /// verify-then-adopt a peer snapshot: decode into a temporary, recompute the
    /// root, refuse on mismatch — committed state and stage untouched on any
    /// error. success drops the stage (it belonged to the replaced state).
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
impl Module for Upgrade {
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
            UpgradeMsg::Schedule {
                name,
                activation_height,
                to_version,
            } => self.handle_schedule(ctx, name, activation_height, to_version),
            UpgradeMsg::Cancel { name } => self.handle_cancel(ctx, name),
            UpgradeMsg::SignalReady {
                name,
                to_version,
                commitment,
            } => {
                self.handle_signal_ready(ctx, name, to_version, commitment)
                    .await
            }
            UpgradeMsg::Advance => self.handle_advance(ctx).await,
        }
    }

    /// read projection with host-routed access to the valset — the `Status`
    /// verdict (`member_count`/`ready_count`/`armed`) needs the boundary set.
    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            UpgradeQuery::Status => {
                let members = self.members(ctx).await?;
                let state = self.read();
                let ready: Vec<Vec<u8>> = state
                    .readiness
                    .iter()
                    .filter(|(_, signal)| {
                        state.pending.as_ref().is_none_or(|pending| {
                            crate::readiness_commitment_matches(
                                &pending.name,
                                signal.commitment.as_deref(),
                            )
                        })
                    })
                    .map(|(key, _)| key.clone())
                    .collect();
                // `armed` (readiness complete) is derived from the ONE shared
                // predicate evaluated AT the activation height (where the height
                // gate always passes), so it can never drift from the arm check
                // the Advance handler and the host stamp use (risk R4).
                let armed = match &state.pending {
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
                Ok(encode_reply(&UpgradeReply::Status(UpgradeStatus {
                    current_version: state.current_version,
                    pending: state.pending.clone(),
                    member_count: members.len() as u64,
                    ready_count: ready.len() as u64,
                    members,
                    ready,
                    armed,
                })))
            }
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

fn decode_state(bytes: &[u8]) -> Result<UpgradeState, Error> {
    let mut cur = Cursor::new(bytes);
    let current_version = cur.u32("snapshot current_version")?;
    let pending = match cur.byte("snapshot pending tag")? {
        0 => None,
        1 => {
            let name = cur.string("snapshot upgrade name")?;
            let activation_height = cur.u64("snapshot activation height")?;
            let to_version = cur.u32("snapshot to_version")?;
            Some(ScheduledUpgrade {
                name,
                activation_height,
                to_version,
            })
        }
        other => return Err(Error::Module(format!("snapshot: bad pending tag {other}"))),
    };
    let count = cur.u64("snapshot readiness count")?;
    // each readiness entry costs at least an 8-byte key-length prefix + a 1-byte
    // commitment tag — a forged count can never drive allocation past the buffer.
    cur.bound(count, 9, "snapshot readiness")?;
    let mut readiness = BTreeMap::new();
    let mut prev: Option<Vec<u8>> = None;
    for _ in 0..count {
        let key = cur.bytes("snapshot readiness key")?.to_vec();
        // strictly increasing keys: one state has exactly one encoding.
        if prev.as_deref().is_some_and(|p| p >= key.as_slice()) {
            return Err(Error::Module(
                "snapshot readiness keys must be strictly increasing".into(),
            ));
        }
        let commitment = match cur.byte("snapshot commitment tag")? {
            0 => None,
            1 => Some(cur.bytes("snapshot commitment")?.to_vec()),
            other => {
                return Err(Error::Module(format!(
                    "snapshot: bad commitment tag {other}"
                )));
            }
        };
        prev = Some(key.clone());
        readiness.insert(key, ReadySignal { commitment });
    }
    cur.finish("snapshot")?;
    Ok(UpgradeState {
        current_version,
        pending,
        readiness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;
    use commonware_codec::DecodeExt as _;
    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;
    use valset::ValsetReply;

    // a Ctx that answers the valset Validators query with a configurable member
    // set (upgrade's only host-routed read) and carries a settable origin/height.
    struct TestCtx {
        env: sdk::Env,
        members: Vec<Vec<u8>>,
    }
    impl TestCtx {
        fn new(origin: Origin, height: u64, members: Vec<Vec<u8>>) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height,
                    consensus_time: 0,
                    origin,
                    me: "upgrade".into(),
                },
                members,
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
            // upgrade only ever queries the valset Validators set.
            Ok(valset::encode_reply(&ValsetReply::Validators(
                self.members.clone(),
            )))
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
    }

    // a deterministic, VALID 32-byte ed25519 public key (like valset's tests).
    fn valid_key(seed_byte: u8) -> Vec<u8> {
        let seed = [seed_byte; 32];
        let sk = PrivateKey::decode(&seed[..]).expect("any 32 bytes is a valid seed");
        sk.public_key().as_ref().to_vec()
    }

    fn up() -> Upgrade {
        Upgrade::new("upgrade", "valset")
    }

    fn schedule(name: &str, ah: u64, tv: u32) -> Msg {
        Msg {
            target: "upgrade".into(),
            payload: encode_msg(&UpgradeMsg::Schedule {
                name: name.into(),
                activation_height: ah,
                to_version: tv,
            }),
        }
    }
    fn cancel(name: &str) -> Msg {
        Msg {
            target: "upgrade".into(),
            payload: encode_msg(&UpgradeMsg::Cancel { name: name.into() }),
        }
    }
    fn signal(name: &str, tv: u32) -> Msg {
        signal_with(name, tv, None)
    }
    fn signal_with(name: &str, tv: u32, commitment: Option<Vec<u8>>) -> Msg {
        Msg {
            target: "upgrade".into(),
            payload: encode_msg(&UpgradeMsg::SignalReady {
                name: name.into(),
                to_version: tv,
                commitment,
            }),
        }
    }
    fn advance() -> Msg {
        Msg {
            target: "upgrade".into(),
            payload: encode_msg(&UpgradeMsg::Advance),
        }
    }

    fn run(u: &mut Upgrade, ctx: &mut TestCtx, m: &Msg) -> Result<(), Error> {
        futures::executor::block_on(u.execute(ctx, m))
    }
    fn commit(u: &mut Upgrade) {
        futures::executor::block_on(u.commit_block()).unwrap();
    }
    fn status(u: &Upgrade, ctx: &TestCtx) -> UpgradeStatus {
        let reply = futures::executor::block_on(
            u.query_with(ctx, &crate::encode_query(&UpgradeQuery::Status)),
        )
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            UpgradeReply::Status(s) => s,
        }
    }

    // ---- origin gates -------------------------------------------------------

    #[test]
    fn schedule_and_cancel_origin_gate() {
        let members = vec![valid_key(1)];
        // External rejected.
        let mut u = up();
        let mut ext = TestCtx::new(Origin::External(valid_key(1)), 0, members.clone());
        assert!(matches!(
            run(&mut u, &mut ext, &schedule("a", 10, 2)),
            Err(Error::Module(_))
        ));

        // Module accepted.
        let mut u = up();
        let mut m = TestCtx::new(Origin::Module("governance".into()), 0, members.clone());
        run(&mut u, &mut m, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        assert!(u.committed.pending.is_some());

        // Cancel from External rejected; from System accepted.
        let mut ext2 = TestCtx::new(Origin::External(valid_key(1)), 0, members.clone());
        assert!(matches!(
            run(&mut u, &mut ext2, &cancel("a")),
            Err(Error::Module(_))
        ));
        let mut sys = TestCtx::new(Origin::System, 0, members);
        run(&mut u, &mut sys, &cancel("a")).unwrap();
        commit(&mut u);
        assert!(u.committed.pending.is_none());
    }

    #[test]
    fn signal_ready_origin_gate() {
        let member = valid_key(1);
        let members = vec![member.clone()];
        let mut u = up();
        // set up a pending upgrade via System.
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);

        // Module/System rejected.
        let mut m = TestCtx::new(Origin::Module("governance".into()), 0, members.clone());
        assert!(matches!(
            run(&mut u, &mut m, &signal("a", 2)),
            Err(Error::Module(_))
        ));
        assert!(matches!(
            run(&mut u, &mut sys, &signal("a", 2)),
            Err(Error::Module(_))
        ));

        // External non-member rejected.
        let nonmember = valid_key(99);
        let mut bad = TestCtx::new(Origin::External(nonmember), 0, members.clone());
        assert!(matches!(
            run(&mut u, &mut bad, &signal("a", 2)),
            Err(Error::Module(_))
        ));

        // External member accepted.
        let mut good = TestCtx::new(Origin::External(member.clone()), 0, members);
        run(&mut u, &mut good, &signal("a", 2)).unwrap();
        commit(&mut u);
        assert!(u.committed.readiness.contains_key(&member));
    }

    // ---- schedule validation ------------------------------------------------

    #[test]
    fn schedule_monotonic_reject() {
        let members = vec![valid_key(1)];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members);
        // current_version == 0, to_version 0 is not > 0.
        assert!(matches!(
            run(&mut u, &mut sys, &schedule("a", 10, 0)),
            Err(Error::Module(_))
        ));
    }

    #[test]
    fn schedule_min_lead_reject() {
        let members = vec![valid_key(1)];
        let mut u = up();
        // height 0, MIN_UPGRADE_LEAD 3 -> activation must be > 3.
        let mut sys = TestCtx::new(Origin::System, 0, members);
        assert!(matches!(
            run(&mut u, &mut sys, &schedule("a", 3, 2)),
            Err(Error::Module(_))
        ));
        // exactly floor+1 accepts.
        run(&mut u, &mut sys, &schedule("a", 4, 2)).unwrap();
    }

    #[test]
    fn schedule_at_most_one_pending_reject() {
        let members = vec![valid_key(1)];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members);
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        // a second schedule while one is pending is rejected.
        assert!(matches!(
            run(&mut u, &mut sys, &schedule("b", 20, 3)),
            Err(Error::Module(_))
        ));
    }

    #[test]
    fn schedule_clears_residual_readiness() {
        // schedule + a signal + cancel leaves readiness cleared; a fresh schedule
        // also starts from empty readiness.
        let member = valid_key(1);
        let members = vec![member.clone()];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        let mut good = TestCtx::new(Origin::External(member), 0, members.clone());
        run(&mut u, &mut good, &signal("a", 2)).unwrap();
        commit(&mut u);
        assert_eq!(u.committed.readiness.len(), 1);

        run(&mut u, &mut sys, &cancel("a")).unwrap();
        commit(&mut u);
        run(&mut u, &mut sys, &schedule("c", 10, 2)).unwrap();
        commit(&mut u);
        assert!(
            u.committed.readiness.is_empty(),
            "fresh schedule starts empty"
        );
    }

    // ---- signal identity scope + idempotence --------------------------------

    #[test]
    fn signal_wrong_identity_rejected_and_idempotent() {
        let m1 = valid_key(1);
        let m2 = valid_key(2);
        let members = vec![m1.clone(), m2.clone()];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);

        // wrong name.
        let mut c1 = TestCtx::new(Origin::External(m1.clone()), 0, members.clone());
        assert!(matches!(
            run(&mut u, &mut c1, &signal("b", 2)),
            Err(Error::Module(_))
        ));
        // wrong to_version.
        assert!(matches!(
            run(&mut u, &mut c1, &signal("a", 3)),
            Err(Error::Module(_))
        ));

        // idempotent per pubkey: two signals from m1 collapse to one entry.
        run(&mut u, &mut c1, &signal("a", 2)).unwrap();
        commit(&mut u);
        run(&mut u, &mut c1, &signal("a", 2)).unwrap();
        commit(&mut u);
        assert_eq!(u.committed.readiness.len(), 1);

        // a second member -> two entries.
        let mut c2 = TestCtx::new(Origin::External(m2.clone()), 0, members);
        run(&mut u, &mut c2, &signal("a", 2)).unwrap();
        commit(&mut u);
        assert_eq!(u.committed.readiness.len(), 2);
    }

    #[test]
    fn commitment_bound_upgrade_ignores_old_or_wrong_artifacts() {
        let member = valid_key(1);
        let members = vec![member.clone()];
        let name = "commit:clients-route-a";

        let mut legacy = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut legacy, &mut sys, &schedule(name, 10, 1)).unwrap();
        commit(&mut legacy);
        legacy
            .committed
            .readiness
            .insert(member.clone(), ReadySignal { commitment: None });
        let legacy_status = status(&legacy, &sys);
        assert_eq!(legacy_status.ready_count, 0);
        assert!(!legacy_status.armed, "old commitment: None must not arm");
        let legacy_none_root = legacy.root();
        let mut boundary = TestCtx::new(Origin::System, 10, members.clone());
        run(&mut legacy, &mut boundary, &advance()).unwrap();
        commit(&mut legacy);
        assert_eq!(legacy.committed.current_version, 0, "wrong artifact aborts");

        let mut current = up();
        run(&mut current, &mut sys, &schedule(name, 10, 1)).unwrap();
        commit(&mut current);
        let mut validator = TestCtx::new(Origin::External(member), 0, members);
        run(&mut current, &mut validator, &signal(name, 1))
            .expect("old-compatible None acceptance");
        commit(&mut current);
        assert_eq!(
            current.root(),
            legacy_none_root,
            "old and bridge binaries must commit the same pre-H root"
        );
        run(
            &mut current,
            &mut validator,
            &signal_with(name, 1, Some(b"wrong-route".to_vec())),
        )
        .expect("old-compatible signal acceptance");
        commit(&mut current);
        assert_eq!(current.committed.readiness.len(), 1);
        let wrong_status = status(&current, &validator);
        assert_eq!(wrong_status.ready_count, 0);
        assert!(!wrong_status.armed);
        run(
            &mut current,
            &mut validator,
            &signal_with(name, 1, Some(b"clients-route-a".to_vec())),
        )
        .expect("exact route commitment");
        commit(&mut current);
        let current_status = status(&current, &validator);
        assert_eq!(current_status.ready_count, 1);
        assert!(current_status.armed);
    }

    // ---- cancel -------------------------------------------------------------

    #[test]
    fn cancel_scope_and_clears_both() {
        let member = valid_key(1);
        let members = vec![member.clone()];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        let mut good = TestCtx::new(Origin::External(member), 0, members.clone());
        run(&mut u, &mut good, &signal("a", 2)).unwrap();
        commit(&mut u);

        // wrong name rejected.
        assert!(matches!(
            run(&mut u, &mut sys, &cancel("b")),
            Err(Error::Module(_))
        ));

        // at/after activation height rejected.
        let mut late = TestCtx::new(Origin::System, 10, members);
        assert!(matches!(
            run(&mut u, &mut late, &cancel("a")),
            Err(Error::Module(_))
        ));

        // valid cancel clears BOTH pending and readiness.
        run(&mut u, &mut sys, &cancel("a")).unwrap();
        commit(&mut u);
        assert!(u.committed.pending.is_none());
        assert!(u.committed.readiness.is_empty());
    }

    // ---- advance arm / abort ------------------------------------------------

    fn schedule_and_ready_all(name: &str, ah: u64, tv: u32, members: &[Vec<u8>]) -> Upgrade {
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.to_vec());
        run(&mut u, &mut sys, &schedule(name, ah, tv)).unwrap();
        commit(&mut u);
        for m in members {
            let mut c = TestCtx::new(Origin::External(m.clone()), 0, members.to_vec());
            run(&mut u, &mut c, &signal(name, tv)).unwrap();
            commit(&mut u);
        }
        u
    }

    #[test]
    fn advance_arm_flips_clears_and_frees_slot() {
        let members = vec![valid_key(1), valid_key(2)];
        let mut u = schedule_and_ready_all("a", 10, 2, &members);
        // ARM at the boundary.
        let mut sys = TestCtx::new(Origin::System, 10, members.clone());
        run(&mut u, &mut sys, &advance()).unwrap();
        commit(&mut u);
        assert_eq!(u.committed.current_version, 2, "armed flips version");
        assert!(u.committed.pending.is_none(), "pending cleared");
        assert!(u.committed.readiness.is_empty(), "readiness cleared");

        // the slot is free: a fresh, higher schedule now succeeds.
        run(&mut u, &mut sys, &schedule("b", 20, 3)).unwrap();
        commit(&mut u);
        assert!(u.committed.pending.is_some(), "second schedule accepted");
    }

    #[test]
    fn advance_abort_clears_without_flip() {
        let members = vec![valid_key(1), valid_key(2)];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        // only ONE of two members signals -> R < n.
        let mut c1 = TestCtx::new(Origin::External(members[0].clone()), 0, members.clone());
        run(&mut u, &mut c1, &signal("a", 2)).unwrap();
        commit(&mut u);

        let mut boundary = TestCtx::new(Origin::System, 10, members.clone());
        run(&mut u, &mut boundary, &advance()).unwrap();
        commit(&mut u);
        assert_eq!(
            u.committed.current_version, 0,
            "abort leaves version unchanged"
        );
        assert!(u.committed.pending.is_none(), "abort clears pending");
        assert!(u.committed.readiness.is_empty(), "abort clears readiness");

        // a second Advance is an idempotent no-op.
        let root_before = u.root();
        run(&mut u, &mut boundary, &advance()).unwrap();
        commit(&mut u);
        assert_eq!(u.root(), root_before);

        // and the slot is free.
        run(&mut u, &mut sys, &schedule("b", 20, 3)).unwrap();
        commit(&mut u);
        assert!(u.committed.pending.is_some());
    }

    #[test]
    fn advance_below_activation_is_noop() {
        let members = vec![valid_key(1), valid_key(2)];
        let mut u = schedule_and_ready_all("a", 10, 2, &members);
        let root_before = u.root();
        // height below activation: no-op even though R == n.
        let mut sys = TestCtx::new(Origin::System, 9, members);
        run(&mut u, &mut sys, &advance()).unwrap();
        commit(&mut u);
        assert_eq!(u.root(), root_before);
        assert_eq!(u.committed.current_version, 0);
        assert!(u.committed.pending.is_some());
    }

    // The boundary race: the nth SignalReady is finalized AS block H's own op, so
    // it stages readiness=n in the SAME drain the injected Advance runs in. The
    // Advance MUST decide over FROZEN committed (H-1) readiness = n-1 and ABORT —
    // matching the orchestrator arm_verdict and the host effective_version(H) stamp,
    // which both froze at n-1 and sealed forge under the OLD selector. Deciding over
    // the staged (n) readiness here would flip committed current_version to
    // to_version while forge sealed under old — a split that halts every recovering
    // node. (Regression for the hardening-audit CRITICAL finding.)
    #[test]
    fn a_same_block_late_signal_aborts_not_arms() {
        let m0 = valid_key(1);
        let m1 = valid_key(2);
        let members = vec![m0.clone(), m1.clone()];
        let mut u = up();

        // schedule; only m0 signals and COMMITS before H (readiness incomplete).
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        let mut c0 = TestCtx::new(Origin::External(m0.clone()), 1, members.clone());
        run(&mut u, &mut c0, &signal("a", 2)).unwrap();
        commit(&mut u);
        assert_eq!(
            u.committed.readiness.len(),
            1,
            "only m0 committed by end of H-1"
        );

        // block H (height 10): m1's SignalReady is THIS block's op — it stages
        // readiness=n — and the injected Advance runs in the SAME drain, before any
        // commit. (Two runs with no intervening commit = one block's staging.)
        let mut c1 = TestCtx::new(Origin::External(m1.clone()), 10, members.clone());
        run(&mut u, &mut c1, &signal("a", 2)).unwrap();
        let mut sysh = TestCtx::new(Origin::System, 10, members);
        run(&mut u, &mut sysh, &advance()).unwrap();
        commit(&mut u);

        // the Advance froze on committed (H-1) readiness {m0}=n-1 -> ABORT, not arm.
        assert_eq!(
            u.committed.current_version, 0,
            "a same-block late nth signal must NOT flip: the boundary quorum was unmet at H-1"
        );
        assert!(u.committed.pending.is_none(), "the upgrade aborts cleanly");
        assert!(
            u.committed.readiness.is_empty(),
            "readiness cleared on abort"
        );
    }

    // ---- effective_version --------------------------------------------------

    #[test]
    fn effective_version_only_when_armed() {
        let members = vec![valid_key(1), valid_key(2)];
        // committed pending + full readiness, NOT yet advanced. derive through
        // the ONE shared crate-root predicate over committed state.
        let u = schedule_and_ready_all("a", 10, 2, &members);
        let ev = |height: u64, boundary: &[Vec<u8>]| {
            crate::effective_version(
                height,
                u.committed.current_version,
                u.committed.pending.as_ref(),
                boundary,
                |member| u.committed.readiness.contains_key(member),
            )
        };
        assert_eq!(ev(10, &members), 2, "armed at/after H -> to_version");
        assert_eq!(ev(9, &members), 0, "below H -> current");
        // a boundary member not ready -> current.
        let mut plus = members.clone();
        plus.push(valid_key(3));
        assert_eq!(ev(10, &plus), 0, "missing member -> current");
        // empty boundary -> current.
        assert_eq!(ev(10, &[]), 0);
    }

    // ---- root discipline + snapshot/install ---------------------------------

    #[test]
    fn root_zero_fresh_nonzero_after_schedule() {
        let members = vec![valid_key(1)];
        let mut u = up();
        assert_eq!(u.root(), StateRoot::ZERO, "fresh module is ZERO");
        let mut sys = TestCtx::new(Origin::System, 0, members);
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        // staged only: committed root still ZERO.
        assert_eq!(u.root(), StateRoot::ZERO);
        commit(&mut u);
        assert_ne!(
            u.root(),
            StateRoot::ZERO,
            "committed schedule moves root off ZERO"
        );
    }

    #[test]
    fn snapshot_install_round_trip() {
        let members = vec![valid_key(1), valid_key(2)];
        let src = schedule_and_ready_all("forge-multi-repo", 10, 2, &members);
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);
        // snapshot IS the root preimage.
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src_root, "sha256(snapshot()) == root()");

        // install into a fresh target reconstructs root + state.
        let mut dst = up();
        dst.install(&bytes, src_root).unwrap();
        assert_eq!(dst.root(), src_root);
        assert_eq!(dst.committed, src.committed);
    }

    #[test]
    fn tampered_truncated_trailing_rejected_target_untouched() {
        let members = vec![valid_key(1), valid_key(2)];
        let src = schedule_and_ready_all("a", 10, 2, &members);
        let src_root = src.root();
        let bytes = src.snapshot();

        // target with its own committed state AND a staged change.
        let target_members = vec![valid_key(5)];
        let mut dst = up();
        let mut sys = TestCtx::new(Origin::System, 0, target_members.clone());
        run(&mut dst, &mut sys, &schedule("z", 30, 7)).unwrap();
        commit(&mut dst);
        // stage another (uncommitted) change: cancel.
        run(&mut dst, &mut sys, &cancel("z")).unwrap();
        let pre_root = dst.root();
        let pre_committed = dst.committed.clone();
        let pre_staged = dst.staged.clone();

        // tamper a FIXED-WIDTH field (current_version, bytes 0..4) so the bytes
        // still DECODE cleanly and it is the verify-then-adopt root-mismatch guard
        // — not a decode error — that rejects them.
        let mut flipped = bytes.clone();
        flipped[0] ^= 0x01;
        let err = dst.install(&flipped, src_root).unwrap_err();
        assert!(
            matches!(&err, Error::Module(m) if m.contains("root mismatch")),
            "tamper must trip the root check, not a decode error, got {err:?}"
        );

        // truncation.
        assert!(dst.install(&bytes[..bytes.len() - 1], src_root).is_err());

        // trailing byte.
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(dst.install(&trailing, src_root).is_err());

        assert_eq!(dst.root(), pre_root, "failed install left root untouched");
        assert_eq!(dst.committed, pre_committed, "committed untouched");
        assert_eq!(dst.staged, pre_staged, "stage untouched");
    }

    // ---- status query -------------------------------------------------------

    #[test]
    fn status_reports_armed_when_all_signaled() {
        let members = vec![valid_key(1), valid_key(2)];
        let mut u = up();
        let mut sys = TestCtx::new(Origin::System, 0, members.clone());
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        commit(&mut u);
        let ctx = TestCtx::new(Origin::System, 0, members.clone());
        let s = status(&u, &ctx);
        assert_eq!(s.member_count, 2);
        assert_eq!(s.ready_count, 0);
        assert!(!s.armed);
        assert!(s.pending.is_some());

        // all signal.
        for m in &members {
            let mut c = TestCtx::new(Origin::External(m.clone()), 0, members.clone());
            run(&mut u, &mut c, &signal("a", 2)).unwrap();
            commit(&mut u);
        }
        let s = status(&u, &ctx);
        assert_eq!(s.ready_count, 2);
        assert!(s.armed, "armed once every member signaled");
    }

    // ---- staging atomicity + post-activation downgrade ----------------------

    #[test]
    fn abort_block_discards_the_staged_schedule() {
        let members = vec![valid_key(1)];
        let mut u = up();
        let before = u.root();
        let mut sys = TestCtx::new(Origin::System, 0, members);
        run(&mut u, &mut sys, &schedule("a", 10, 2)).unwrap();
        // a later dispatch in the same block errors -> the host aborts the block.
        futures::executor::block_on(u.abort_block()).unwrap();
        assert_eq!(u.root(), before, "aborted schedule leaves root untouched");
        assert!(
            u.committed.pending.is_none(),
            "aborted schedule committed nothing"
        );
    }

    #[test]
    fn schedule_rejects_a_downgrade_after_activation() {
        let members = vec![valid_key(1), valid_key(2)];
        let mut u = schedule_and_ready_all("a", 10, 2, &members);
        let mut sys = TestCtx::new(Origin::System, 10, members);
        run(&mut u, &mut sys, &advance()).unwrap();
        commit(&mut u);
        assert_eq!(u.committed.current_version, 2);
        // current_version is now 2: a genuine downgrade to 1 is rejected...
        assert!(matches!(
            run(&mut u, &mut sys, &schedule("b", 20, 1)),
            Err(Error::Module(_))
        ));
        // ...and to_version == current_version is rejected too.
        assert!(matches!(
            run(&mut u, &mut sys, &schedule("b", 20, 2)),
            Err(Error::Module(_))
        ));
    }

    // ---- strict untrusted-decode guards -------------------------------------

    #[test]
    fn decode_state_guards_reject_malformed_bytes() {
        // a valid non-empty snapshot: current_version 0, pending None, two
        // readiness keys (0x01, 0x02) each with commitment None.
        let mut valid = Vec::new();
        valid.extend_from_slice(&0u32.to_le_bytes()); // current_version [0..4]
        valid.push(0); // pending tag None [4]
        valid.extend_from_slice(&2u64.to_le_bytes()); // readiness count [5..13]
        for key in [[1u8], [2u8]] {
            valid.extend_from_slice(&(key.len() as u64).to_le_bytes());
            valid.extend_from_slice(&key);
            valid.push(0); // commitment tag None
        }
        assert!(decode_state(&valid).is_ok(), "the baseline decodes");

        // bad pending tag (byte 4).
        let mut bad_pending = valid.clone();
        bad_pending[4] = 2;
        assert!(
            decode_state(&bad_pending).is_err(),
            "bad pending tag rejected"
        );

        // forged readiness count (bytes 5..13) beyond what the buffer can hold.
        let mut forged = valid.clone();
        forged[5..13].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_state(&forged).is_err(), "forged count rejected");

        // bad commitment tag: first entry's tag at offset 13+8+1 = 22.
        let mut bad_commit = valid.clone();
        bad_commit[22] = 2;
        assert!(
            decode_state(&bad_commit).is_err(),
            "bad commitment tag rejected"
        );

        // non-increasing readiness keys: same shape but keys 0x02 then 0x01.
        let mut unordered = Vec::new();
        unordered.extend_from_slice(&0u32.to_le_bytes());
        unordered.push(0);
        unordered.extend_from_slice(&2u64.to_le_bytes());
        for key in [[2u8], [1u8]] {
            unordered.extend_from_slice(&(key.len() as u64).to_le_bytes());
            unordered.extend_from_slice(&key);
            unordered.push(0);
        }
        assert!(
            decode_state(&unordered).is_err(),
            "non-increasing keys rejected (canonical encoding is unique)"
        );
    }
}
