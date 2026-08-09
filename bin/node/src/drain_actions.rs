//! Orderer-independent epoch-cutover actions shared by validators and replicas.
//!
//! The concrete loops still own drain timing and side-effect order. This seam
//! holds what both roles must decide identically — the observe -> ceiling ->
//! cutover actions, the checkpoint cadence, and the one log format that reports
//! what a checkpoint cost. The block-projection half (RootOp assembly +
//! explorer rows) now lives in [`noded::projection`], consumed by both loops.

use commonware_cryptography::ed25519;
use consensus::{ObservationOutcome, RespawnPlan, ScheduledCutover, ValsetOrchestrator};

// ============================================================================
// checkpoint cadence — what BOTH loops decide about their own cost, and how
// they report it.
// ============================================================================

/// the largest share of a loop's wall time a checkpoint may occupy: one part in
/// this. The checkpoint runs on the same select as the HTTP command arm and the
/// signal arm in BOTH roles, so its occupancy is not an internal detail — it is
/// the fraction of the time `/v1/query`, `git clone`'s ref advertisement and
/// SIGTERM are unanswerable (#1018).
pub(crate) const CHECKPOINT_DUTY_LIMIT: u32 = 8;

/// enough blocks have sealed AND the last attempt has paid for itself.
///
/// The block count alone was the whole trigger, and it cannot express cost —
/// 32 blocks is ~30s of chain while one capture measured 59-70s, so the trigger
/// kept re-firing before the previous one had finished and the node lived
/// inside the branch.
pub(crate) fn checkpoint_due(
    blocks_since: u64,
    checkpoint_blocks: u64,
    now: std::time::SystemTime,
    not_before: std::time::SystemTime,
) -> bool {
    let cadence_reached = blocks_since >= checkpoint_blocks;
    let cooled_down = now >= not_before;
    cadence_reached && cooled_down
}

/// when the next checkpoint may START, given when this one finished and what
/// the whole attempt cost. Holding it off for `LIMIT - 1` times its own cost
/// puts the branch's share of wall time at `1/LIMIT` WITHOUT anyone having to
/// know what a checkpoint costs on this box — the last one is the estimate.
///
/// This bounds how OFTEN the loop is blocked, never how LONG: one checkpoint
/// still occupies it for the full duration, so a query landing inside a slow
/// one still times out. Cutting the duration is the module's own problem
/// (#1023 was one instance).
///
/// Overflow FAILS TOWARD CHECKPOINTING, deliberately: a node that stops
/// checkpointing cannot recover quickly or admit a joiner, which is worse than
/// any occupancy — so an absurd cost yields no cooldown, never an infinite one.
pub(crate) fn cooldown_until(
    finished_at: std::time::SystemTime,
    cost: std::time::Duration,
) -> std::time::SystemTime {
    finished_at
        .checked_add(cost.saturating_mul(CHECKPOINT_DUTY_LIMIT - 1))
        .unwrap_or(finished_at)
}

/// the checkpoint's per-module cost as one compact log field,
/// `"forge=60245,chat=12"` in milliseconds, COSTLIEST FIRST — naming the module
/// that spent the loop's time is the entire point, and #1018 was one module out
/// of twenty. Every registered module appears, zeros included: "this one is 0"
/// is the answer that clears a suspect.
pub(crate) fn capture_breakdown(cost: &[(sdk::ModuleId, std::time::Duration)]) -> String {
    let mut ranked: Vec<&(sdk::ModuleId, std::time::Duration)> = cost.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
        .iter()
        .map(|(id, spent)| format!("{id}={}", spent.as_millis()))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CutoverTrigger {
    Membership(ScheduledCutover),
}

pub(crate) struct EpochActions<'a> {
    orchestrator: &'a mut ValsetOrchestrator<ed25519::PublicKey>,
    finalized_view: u64,
    members: Vec<ed25519::PublicKey>,
    residents: Vec<ed25519::PublicKey>,
}

/// Staged shared observe -> ceiling -> cutover actions. Callers invoke these
/// methods in order so each concrete loop keeps ceiling writes and async reads
/// at the same visible points as before the refactor.
impl<'a> EpochActions<'a> {
    pub(crate) fn new(
        orchestrator: &'a mut ValsetOrchestrator<ed25519::PublicKey>,
        finalized_view: u64,
        members: Vec<ed25519::PublicKey>,
        residents: Vec<ed25519::PublicKey>,
    ) -> Self {
        Self {
            orchestrator,
            finalized_view,
            members,
            residents,
        }
    }

    pub(crate) fn observe_members(&mut self) -> Option<CutoverTrigger> {
        match self.orchestrator.observe_members(
            self.finalized_view,
            self.members.iter().cloned(),
            self.residents.iter().cloned(),
        ) {
            ObservationOutcome::Scheduled(cutover) => Some(CutoverTrigger::Membership(cutover)),
            _ => None,
        }
    }

    pub(crate) fn respawn(self) -> Option<RespawnPlan<ed25519::PublicKey>> {
        self.orchestrator
            .respawn_if_due(self.finalized_view, self.members, self.residents)
    }
}

#[cfg(test)]
mod tests {
    use commonware_cryptography::{Signer as _, ed25519};
    use consensus::ValsetOrchestrator;

    use super::*;

    #[test]
    fn the_capture_breakdown_names_the_costliest_module_first() {
        let cost = vec![
            ("chat".to_string(), std::time::Duration::from_millis(12)),
            ("forge".to_string(), std::time::Duration::from_millis(60245)),
            ("valset".to_string(), std::time::Duration::ZERO),
        ];
        assert_eq!(
            capture_breakdown(&cost),
            "forge=60245,chat=12,valset=0",
            "reading the field IS the attribution; the module that spent the \
             loop's time must be the first thing in it",
        );
    }

    #[test]
    fn epoch_actions_pin_validator_replica_parity_through_cutover() {
        let a = ed25519::PrivateKey::from_seed(1).public_key();
        let b = ed25519::PrivateKey::from_seed(2).public_key();
        let c = ed25519::PrivateKey::from_seed(3).public_key();
        let initial = vec![a.clone(), b.clone()];
        let boundary = vec![a.clone(), b.clone(), c.clone()];
        let mut validator = ValsetOrchestrator::new(2, initial.clone());
        let mut replica = ValsetOrchestrator::new(2, initial);

        let mut validator_arm = EpochActions::new(&mut validator, 7, boundary.clone(), Vec::new());
        let mut replica_arm = EpochActions::new(&mut replica, 7, boundary.clone(), Vec::new());
        let validator_trigger = validator_arm.observe_members();
        let replica_trigger = replica_arm.observe_members();
        assert_eq!(validator_trigger, replica_trigger);
        assert!(matches!(
            validator_trigger,
            Some(CutoverTrigger::Membership(cutover)) if cutover.cutover_view() == 9
        ));
        let validator_plan = validator_arm.respawn();
        let replica_plan = replica_arm.respawn();
        assert_eq!(validator_plan, replica_plan);
        assert!(validator_plan.is_none());

        let mut validator_cutover =
            EpochActions::new(&mut validator, 9, boundary.clone(), Vec::new());
        let mut replica_cutover = EpochActions::new(&mut replica, 9, boundary, Vec::new());
        assert_eq!(
            validator_cutover.observe_members(),
            replica_cutover.observe_members()
        );
        let validator_plan = validator_cutover.respawn();
        let replica_plan = replica_cutover.respawn();
        assert_eq!(validator_plan, replica_plan);
        let plan = validator_plan.expect("boundary cuts over");
        assert_eq!(plan.epoch(), 1);
        assert_eq!(plan.cutover_app_height(), 9);
    }
}
