use std::collections::BTreeSet;

/// committed validator-set root observed after consensus finalizes a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValsetRoot(pub [u8; 32]);

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
        let consensus_members: BTreeSet<Member> = members.into_iter().collect();
        let transport_members = consensus_members.clone();
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

/// the finalized valset state an orchestrator observes from app state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedValset<Member> {
    root: ValsetRoot,
    membership: EpochMembership<Member>,
}

impl<Member> ObservedValset<Member>
where
    Member: Ord + Clone,
{
    pub fn from_validator_set(root: ValsetRoot, members: impl IntoIterator<Item = Member>) -> Self {
        Self {
            root,
            membership: EpochMembership::from_validator_set(members),
        }
    }

    pub fn root(&self) -> ValsetRoot {
        self.root
    }

    pub fn membership(&self) -> &EpochMembership<Member> {
        &self.membership
    }
}

/// a deterministic epoch cutover scheduled by observing a finalized membership change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledCutover<Member> {
    observed_view: u64,
    cutover_view: u64,
    next_epoch: u64,
    next_valset: ObservedValset<Member>,
}

impl<Member> ScheduledCutover<Member> {
    pub fn observed_view(&self) -> u64 {
        self.observed_view
    }

    pub fn cutover_view(&self) -> u64 {
        self.cutover_view
    }

    pub fn next_epoch(&self) -> u64 {
        self.next_epoch
    }

    pub fn next_valset(&self) -> &ObservedValset<Member> {
        &self.next_valset
    }
}

/// the respawn parameters a live adapter would hand to the next consensus engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespawnPlan<Member> {
    epoch: u64,
    epoch_base: u64,
    cutover_view: u64,
    cutover_app_height: u64,
    valset: ObservedValset<Member>,
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

    pub fn valset(&self) -> &ObservedValset<Member> {
        &self.valset
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationOutcome<Member> {
    Unchanged,
    Scheduled(ScheduledCutover<Member>),
    Pending(ScheduledCutover<Member>),
}

/// deterministic state machine for the epoch cutover contract.
///
/// It deliberately stops before real networking: callers feed it finalized valset
/// observations and finalized views, then use the returned [`RespawnPlan`] to wire
/// concrete consensus and transport engines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValsetOrchestrator<Member> {
    cutover_delay: u64,
    epoch: u64,
    epoch_base: u64,
    current: ObservedValset<Member>,
    pending: Option<ScheduledCutover<Member>>,
}

impl<Member> ValsetOrchestrator<Member>
where
    Member: Ord + Clone,
{
    pub fn new(cutover_delay: u64, initial: ObservedValset<Member>) -> Self {
        Self {
            cutover_delay,
            epoch: 0,
            epoch_base: 0,
            current: initial,
            pending: None,
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn epoch_base(&self) -> u64 {
        self.epoch_base
    }

    pub fn current_valset(&self) -> &ObservedValset<Member> {
        &self.current
    }

    pub fn pending_cutover(&self) -> Option<&ScheduledCutover<Member>> {
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

    pub fn observe_finalized_valset(
        &mut self,
        finalized_view: u64,
        observed: ObservedValset<Member>,
    ) -> ObservationOutcome<Member> {
        if let Some(cutover) = &self.pending {
            return ObservationOutcome::Pending(cutover.clone());
        }

        if observed == self.current {
            return ObservationOutcome::Unchanged;
        }

        let cutover_view = finalized_view
            .checked_add(self.cutover_delay)
            .expect("cutover view overflow");
        let next_epoch = self.epoch.checked_add(1).expect("epoch overflow");
        let cutover = ScheduledCutover {
            observed_view: finalized_view,
            cutover_view,
            next_epoch,
            next_valset: observed,
        };
        self.pending = Some(cutover.clone());
        ObservationOutcome::Scheduled(cutover)
    }

    pub fn respawn_if_due(&mut self, finalized_view: u64) -> Option<RespawnPlan<Member>> {
        let cutover = self.pending.as_ref()?;
        if finalized_view < cutover.cutover_view {
            return None;
        }

        let cutover = self.pending.take().expect("checked pending");
        let cutover_app_height = self.app_height(cutover.cutover_view);
        self.epoch = cutover.next_epoch;
        self.epoch_base = cutover_app_height;
        self.current = cutover.next_valset.clone();

        Some(RespawnPlan {
            epoch: self.epoch,
            epoch_base: self.epoch_base,
            cutover_view: cutover.cutover_view,
            cutover_app_height,
            valset: cutover.next_valset,
        })
    }
}
