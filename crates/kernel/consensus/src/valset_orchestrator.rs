use std::collections::BTreeSet;

/// epoch-scoped membership for both consensus and the validator-owned transport mesh.
///
/// This derives transport membership directly from the validator set.
/// A WireGuard mesh can project bootnodes, relayers, and control participants
/// from this same epoch value without assuming a static relay outside the set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochMembership<Member> {
    consensus_members: BTreeSet<Member>,
    transport_members: BTreeSet<Member>,
}

impl<Member> EpochMembership<Member>
where
    Member: Ord + Clone,
{
    pub fn from_validator_set(members: impl IntoIterator<Item = Member>) -> Self {
        Self::from_sets(members, std::iter::empty())
    }

    /// the two-tier epoch membership: `validators` seat the consensus engine;
    /// `residents` (the staged-admission tier) hold transport standing only.
    /// transport is the UNION — the seam where the two fields diverge.
    pub fn from_sets(
        validators: impl IntoIterator<Item = Member>,
        residents: impl IntoIterator<Item = Member>,
    ) -> Self {
        let consensus_members: BTreeSet<Member> = validators.into_iter().collect();
        let mut transport_members = consensus_members.clone();
        transport_members.extend(residents);
        Self {
            consensus_members,
            transport_members,
        }
    }

    pub fn consensus_members(&self) -> &BTreeSet<Member> {
        &self.consensus_members
    }

    pub fn transport_members(&self) -> &BTreeSet<Member> {
        &self.transport_members
    }
}

/// a deterministic epoch cutover, armed by observing a finalized membership
/// change. carries only VIEW coordinates — the next participant set is
/// deliberately NOT pinned here: it is read from app state at the boundary
/// (see [`ValsetOrchestrator::respawn_if_due`]), where the discard ceiling
/// has frozen state identically on every honest node. pinning the set at
/// observation time would break restart recovery (a node resuming
/// mid-window cannot reconstruct membership as of the observation block)
/// and would respawn WITHOUT a second change that lands inside the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledCutover {
    observed_view: u64,
    cutover_view: u64,
    next_epoch: u64,
}

impl ScheduledCutover {
    pub fn observed_view(&self) -> u64 {
        self.observed_view
    }

    pub fn cutover_view(&self) -> u64 {
        self.cutover_view
    }

    pub fn next_epoch(&self) -> u64 {
        self.next_epoch
    }
}

/// the boundary snapshot of the upgrade module's committed state, read at the
/// SAME finalized view (and thus discard-ceiling-frozen state) the orchestrator
/// reads the boundary valset from. carries the agreed `current_version` and the
/// single pending upgrade (if any) with its FROZEN readiness set.
///
/// this is a NON-hashed DISPATCH input — like `cutover_view`/`epoch_base` it is a
/// coordinate the respawn reads, never a value folded into any module root. the
/// authoritative version state lives in the upgrade module's app-hashed root; this
/// is the boundary READ of it (design §"One boundary carries both concerns").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryUpgrade<Member> {
    pub current_version: u32,
    pub pending: Option<PendingUpgrade<Member>>,
}

impl<Member> BoundaryUpgrade<Member> {
    /// a boundary with no pending upgrade at `current_version` — what every
    /// membership-only respawn (and any node without the upgrade module) passes.
    pub fn baseline(current_version: u32) -> Self {
        Self {
            current_version,
            pending: None,
        }
    }
}

/// the frozen coordinates of the single pending upgrade at the boundary, with the
/// readiness set projected into the orchestrator's `Member` key type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpgrade<Member> {
    pub name: String,
    pub activation_height: u64,
    pub to_version: u32,
    /// the members who have signaled readiness for THIS upgrade, at the boundary.
    pub ready: BTreeSet<Member>,
}

/// the deterministic arm/abort verdict evaluated EXACTLY ONCE at the finalized
/// boundary. a NON-hashed dispatch/observability signal — the app-hashed effect
/// (the `current_version` flip + pending clear) is carried by the in-block
/// System-origin `Advance` the host injects at the same height, not by this enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeVerdict {
    /// no pending upgrade reached its activation height at this boundary.
    None,
    /// the pending upgrade ARMED: `H` reached and EVERY boundary member signaled.
    Armed { name: String, to_version: u32 },
    /// the pending upgrade reached `H` but readiness was unmet — clean ABORT.
    Abort { name: String },
}

/// the respawn parameters a live adapter hands to the next consensus engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespawnPlan<Member> {
    epoch: u64,
    epoch_base: u64,
    cutover_view: u64,
    cutover_app_height: u64,
    valset: EpochMembership<Member>,
    /// the effective protocol version at this boundary, read from frozen upgrade
    /// state at the SAME finalized view as `valset` (design §"One boundary carries
    /// both concerns"). NON-hashed — the driver stamps it into each dual-path
    /// module's `active_version` (branch selector), never into a root preimage.
    boundary_version: u32,
    /// the once-at-`H` arm/abort verdict against the frozen readiness + boundary
    /// valset. NON-hashed. the app-hashed reconciliation rides the host's in-block
    /// System `Advance` at the same height.
    upgrade_verdict: UpgradeVerdict,
}

impl<Member> RespawnPlan<Member> {
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn epoch_base(&self) -> u64 {
        self.epoch_base
    }

    pub fn cutover_view(&self) -> u64 {
        self.cutover_view
    }

    pub fn cutover_app_height(&self) -> u64 {
        self.cutover_app_height
    }

    pub fn valset(&self) -> &EpochMembership<Member> {
        &self.valset
    }

    /// the effective protocol version to stamp into each dual-path module's
    /// `active_version` at this boundary (design §4). NON-hashed.
    pub fn boundary_version(&self) -> u32 {
        self.boundary_version
    }

    /// the deterministic arm/abort verdict evaluated once at this boundary.
    pub fn upgrade_verdict(&self) -> &UpgradeVerdict {
        &self.upgrade_verdict
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationOutcome {
    Unchanged,
    Scheduled(ScheduledCutover),
    Pending(ScheduledCutover),
}

/// deterministic state machine for the epoch cutover contract.
///
/// every input is either replicated state or a recorded epoch coordinate, so
/// every honest node runs it identically and a restarted node can resume it:
///
/// - `current` is the CURRENT EPOCH'S engine participant set (the set the
///   live engine was spawned over — recorded in the recovery manifest), NOT
///   the instantaneous valset projection, which may already include a
///   mid-window change.
/// - a membership change is observed at exactly the changing block's view
///   (the ordered lane's observation barrier guarantees per-block
///   granularity), arming a cutover at `observed + cutover_delay` — the same
///   view on every node.
/// - the next participant set is read at the BOUNDARY: the caller passes the
///   valset projection after the discard ceiling froze state, so a second
///   change landing inside the window rides the same cutover on every node.
///
/// callers feed finalized observations and use the returned [`RespawnPlan`]
/// to wire concrete consensus and transport engines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValsetOrchestrator<Member> {
    cutover_delay: u64,
    epoch: u64,
    epoch_base: u64,
    current: BTreeSet<Member>,
    /// the current epoch's RESIDENT set (transport standing, no quorum seat).
    /// a resident change arms the same single cutover slot as a validator
    /// change — mesh admission rides the epoch boundary.
    current_residents: BTreeSet<Member>,
    pending: Option<ScheduledCutover>,
}

impl<Member> ValsetOrchestrator<Member>
where
    Member: Ord + Clone,
{
    pub fn new(cutover_delay: u64, initial: impl IntoIterator<Item = Member>) -> Self {
        Self::resume(cutover_delay, initial, std::iter::empty(), 0, 0, None)
    }

    /// resume at recovered `(epoch, epoch_base)` coordinates instead of
    /// genesis. `initial` is the recovered epoch's ENGINE PARTICIPANT SET and
    /// `initial_residents` its resident set (both from the recovery manifest —
    /// not the instantaneous valset, which may already include a change
    /// awaiting cutover); `pending_cutover_view` re-arms a cutover the
    /// pre-restart process had scheduled, so a node that crashed mid-window
    /// rejoins the same deterministic boundary.
    pub fn resume(
        cutover_delay: u64,
        initial: impl IntoIterator<Item = Member>,
        initial_residents: impl IntoIterator<Item = Member>,
        epoch: u64,
        epoch_base: u64,
        pending_cutover_view: Option<u64>,
    ) -> Self {
        let pending = pending_cutover_view.map(|cutover_view| ScheduledCutover {
            // the arming block's view; saturation is display-only (a real
            // cutover view is always >= the delay that armed it).
            observed_view: cutover_view.saturating_sub(cutover_delay),
            cutover_view,
            next_epoch: epoch
                .checked_add(1)
                .expect("epoch overflow: corrupt resume coordinates — fail-stop"),
        });
        Self {
            cutover_delay,
            epoch,
            epoch_base,
            current: initial.into_iter().collect(),
            current_residents: initial_residents.into_iter().collect(),
            pending,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn epoch_base(&self) -> u64 {
        self.epoch_base
    }

    /// the current epoch's engine participant set.
    pub fn current_members(&self) -> &BTreeSet<Member> {
        &self.current
    }

    /// the current epoch's resident set (transport standing only).
    pub fn current_residents(&self) -> &BTreeSet<Member> {
        &self.current_residents
    }

    pub fn pending_cutover(&self) -> Option<&ScheduledCutover> {
        self.pending.as_ref()
    }

    /// the app-level height for `view` in the CURRENT epoch: `epoch_base + view`.
    ///
    /// overflow policy (applies to every `checked_add().expect()` in this type):
    /// `epoch_base`, views, and epochs only ever grow by finalized consensus
    /// progress — reaching u64::MAX takes ~1.8e19 finalized views, unreachable
    /// by construction. the checked-add panic is deliberate FAIL-STOP hardening
    /// against a corrupted input (a bad genesis base, a bit-flipped view), not a
    /// reachable consensus path; saturating or wrapping instead would silently
    /// desynchronize heights across validators, which is strictly worse than
    /// halting the one node holding corrupt state.
    pub fn app_height(&self, view: u64) -> u64 {
        self.epoch_base
            .checked_add(view)
            .expect("app height overflow: corrupt epoch_base/view — fail-stop")
    }

    /// observe the finalized valset projection at `finalized_view` (the last
    /// drained view — the observation barrier guarantees this is exactly the
    /// changing block's view when membership moved). a change in EITHER tier
    /// against the current epoch — the participant set or the resident set —
    /// arms a cutover `cutover_delay` views out; the caller mirrors it into
    /// the ordered lane's discard ceiling. resident changes ride the same
    /// boundary because transport membership is epoch-scoped (per-epoch mesh
    /// tracking + channel bank), and a respawn with an unchanged participant
    /// set is safe: the boundary carry re-proposes accepted ops.
    pub fn observe_members(
        &mut self,
        finalized_view: u64,
        members: impl IntoIterator<Item = Member>,
        residents: impl IntoIterator<Item = Member>,
    ) -> ObservationOutcome {
        if let Some(cutover) = self.pending {
            // an armed boundary never moves: a further change inside the
            // window is picked up by the boundary read at respawn.
            return ObservationOutcome::Pending(cutover);
        }

        let observed: BTreeSet<Member> = members.into_iter().collect();
        let observed_residents: BTreeSet<Member> = residents.into_iter().collect();
        if observed == self.current && observed_residents == self.current_residents {
            return ObservationOutcome::Unchanged;
        }

        let cutover = ScheduledCutover {
            observed_view: finalized_view,
            cutover_view: finalized_view
                .checked_add(self.cutover_delay)
                .expect("cutover view overflow"),
            next_epoch: self.epoch.checked_add(1).expect("epoch overflow"),
        };
        self.pending = Some(cutover);
        ObservationOutcome::Scheduled(cutover)
    }

    /// observe a pending upgrade whose activation lands at `activation_app_height`.
    ///
    /// arms the SINGLE shared cutover slot at `cutover_view = activation_app_height
    /// - epoch_base` (`checked_sub`, fail-stop on corrupt coords) ONLY when the slot
    /// is empty and the boundary is strictly in the future; when a membership
    /// cutover already holds the slot this returns `Pending` — the version flip then
    /// rides that same boundary via [`respawn_if_due`](Self::respawn_if_due)'s
    /// boundary read, never a competing arm (design §"One boundary carries both
    /// concerns"; orchestrator holds only one pending cutover). idempotent once
    /// armed: a second call returns `Pending`.
    pub fn observe_upgrade(
        &mut self,
        finalized_view: u64,
        activation_app_height: u64,
    ) -> ObservationOutcome {
        if let Some(cutover) = self.pending {
            // an armed boundary never moves; the version flip is applied by the
            // boundary read when this cutover crosses.
            return ObservationOutcome::Pending(cutover);
        }

        let cutover_view = activation_app_height
            .checked_sub(self.epoch_base)
            .expect("activation height precedes epoch base — corrupt upgrade coords, fail-stop");
        // the boundary must be strictly future. the module's schedule-time
        // MIN_UPGRADE_LEAD (>= cutover_delay) guarantees this; guard deterministically
        // rather than arm a past/current boundary that could never gate views.
        if cutover_view <= finalized_view {
            return ObservationOutcome::Unchanged;
        }

        let cutover = ScheduledCutover {
            observed_view: finalized_view,
            cutover_view,
            next_epoch: self
                .epoch
                .checked_add(1)
                .expect("epoch overflow: corrupt upgrade coords — fail-stop"),
        };
        self.pending = Some(cutover);
        ObservationOutcome::Scheduled(cutover)
    }

    /// the deterministic arm/abort predicate, evaluated EXACTLY ONCE at the
    /// finalized boundary against the frozen boundary valset + the pending
    /// upgrade's frozen readiness set. returns `(boundary_version, verdict)`.
    ///
    /// PURE — no clock/IO/RNG — so every honest node computes the identical value.
    /// this STRUCTURALLY MIRRORS `upgrade::effective_version` (risk R4):
    /// armed iff the pending upgrade reached its activation height, the boundary
    /// valset is non-empty, and every boundary member is present in `ready`. the
    /// upgrade module's own `Advance` handler and `effective_version` helper route
    /// through the shared `upgrade::effective_version` predicate; this generic mirror can
    /// never disagree with them on the same frozen inputs (the orchestrator is
    /// generic over `Member`, so it cannot call the `Vec<u8>`-typed shared fn
    /// directly — the mirror is the single deterministic contract, guarded by the
    /// coincident-boundary tests below).
    fn arm_verdict(
        boundary_app_height: u64,
        up: &BoundaryUpgrade<Member>,
        boundary_valset: &BTreeSet<Member>,
    ) -> (u32, UpgradeVerdict) {
        match &up.pending {
            Some(p) if boundary_app_height >= p.activation_height => {
                let armed = !boundary_valset.is_empty()
                    && boundary_valset.iter().all(|m| p.ready.contains(m));
                if armed {
                    (
                        p.to_version,
                        UpgradeVerdict::Armed {
                            name: p.name.clone(),
                            to_version: p.to_version,
                        },
                    )
                } else {
                    (
                        up.current_version,
                        UpgradeVerdict::Abort {
                            name: p.name.clone(),
                        },
                    )
                }
            }
            // no pending, or the boundary is below its activation height: the
            // version does not flip early (`effective_version` is pure below `H`).
            _ => (up.current_version, UpgradeVerdict::None),
        }
    }

    /// cross the armed boundary once `finalized_view` reaches it.
    /// `boundary_members` and `boundary_residents` are the valset projections
    /// read from app state NOW — the discard ceiling froze state at the
    /// boundary, so every honest node reads the identical sets.
    /// `boundary_upgrade` is the upgrade module's state read from that SAME
    /// frozen boundary, so ONE respawn applies membership changes (both
    /// tiers) AND a pending-upgrade-at-`H` at a single finalized view.
    /// commits the cutover: the new epoch's participant + resident sets,
    /// base, coordinates, the boundary version, and the once-evaluated
    /// arm/abort verdict.
    pub fn respawn_if_due(
        &mut self,
        finalized_view: u64,
        boundary_members: impl IntoIterator<Item = Member>,
        boundary_residents: impl IntoIterator<Item = Member>,
        boundary_upgrade: BoundaryUpgrade<Member>,
    ) -> Option<RespawnPlan<Member>> {
        let cutover = self.pending.as_ref()?;
        if finalized_view < cutover.cutover_view {
            return None;
        }

        let cutover = self.pending.take().expect("checked pending");
        let cutover_app_height = self.app_height(cutover.cutover_view);
        let valset = EpochMembership::from_sets(boundary_members, boundary_residents);
        // read the boundary version at the SAME frozen boundary set + app height:
        // evaluate the arm/abort verdict EXACTLY ONCE here (no timers, no
        // node-local readiness view). ONE respawn carries both concerns.
        let (boundary_version, upgrade_verdict) =
            Self::arm_verdict(cutover_app_height, &boundary_upgrade, valset.consensus_members());
        self.epoch = cutover.next_epoch;
        self.epoch_base = cutover_app_height;
        self.current = valset.consensus_members().clone();
        self.current_residents = valset
            .transport_members()
            .difference(valset.consensus_members())
            .cloned()
            .collect();

        Some(RespawnPlan {
            epoch: self.epoch,
            epoch_base: self.epoch_base,
            cutover_view: cutover.cutover_view,
            cutover_app_height,
            valset,
            boundary_version,
            upgrade_verdict,
        })
    }
}
