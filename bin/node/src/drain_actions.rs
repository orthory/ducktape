//! Orderer-independent epoch-cutover actions shared by validators and replicas.
//!
//! The concrete loops still own drain timing and side-effect order. This seam
//! only stages the observe -> ceiling -> cutover decisions both roles must
//! interpret identically. The block-projection half (RootOp assembly + explorer
//! rows) now lives in [`noded::projection`], consumed by both loops.

use commonware_cryptography::ed25519;
use consensus::{
    BoundaryUpgrade, ObservationOutcome, RespawnPlan, ScheduledCutover, ValsetOrchestrator,
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CutoverTrigger {
    Membership(ScheduledCutover),
    Upgrade {
        cutover: ScheduledCutover,
        name: String,
        activation_height: u64,
    },
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

    pub(crate) fn observe_upgrade(
        &mut self,
        boundary_upgrade: &BoundaryUpgrade<ed25519::PublicKey>,
    ) -> Option<CutoverTrigger> {
        let pending = boundary_upgrade.pending.as_ref()?;
        match self
            .orchestrator
            .observe_upgrade(self.finalized_view, pending.activation_height)
        {
            ObservationOutcome::Scheduled(cutover) => Some(CutoverTrigger::Upgrade {
                cutover,
                name: pending.name.clone(),
                activation_height: pending.activation_height,
            }),
            _ => None,
        }
    }

    pub(crate) fn respawn(
        self,
        boundary_upgrade: BoundaryUpgrade<ed25519::PublicKey>,
    ) -> Option<RespawnPlan<ed25519::PublicKey>> {
        self.orchestrator.respawn_if_due(
            self.finalized_view,
            self.members,
            self.residents,
            boundary_upgrade,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use commonware_cryptography::{Signer as _, ed25519};
    use consensus::{BoundaryUpgrade, PendingUpgrade, UpgradeVerdict, ValsetOrchestrator};

    use super::*;

    #[test]
    fn epoch_actions_pin_validator_replica_parity_through_upgrade_cutover() {
        let a = ed25519::PrivateKey::from_seed(1).public_key();
        let b = ed25519::PrivateKey::from_seed(2).public_key();
        let c = ed25519::PrivateKey::from_seed(3).public_key();
        let initial = vec![a.clone(), b.clone()];
        let boundary = vec![a.clone(), b.clone(), c.clone()];
        let upgrade = BoundaryUpgrade {
            current_version: 0,
            pending: Some(PendingUpgrade {
                name: "v1".into(),
                activation_height: 9,
                to_version: 1,
                ready: boundary.iter().cloned().collect::<BTreeSet<_>>(),
            }),
        };
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
        assert_eq!(
            validator_arm.observe_upgrade(&upgrade),
            replica_arm.observe_upgrade(&upgrade)
        );
        let validator_plan = validator_arm.respawn(upgrade.clone());
        let replica_plan = replica_arm.respawn(upgrade.clone());
        assert_eq!(validator_plan, replica_plan);
        assert!(validator_plan.is_none());

        let mut validator_cutover =
            EpochActions::new(&mut validator, 9, boundary.clone(), Vec::new());
        let mut replica_cutover = EpochActions::new(&mut replica, 9, boundary, Vec::new());
        assert_eq!(
            validator_cutover.observe_members(),
            replica_cutover.observe_members()
        );
        assert_eq!(
            validator_cutover.observe_upgrade(&upgrade),
            replica_cutover.observe_upgrade(&upgrade)
        );
        let validator_plan = validator_cutover.respawn(upgrade.clone());
        let replica_plan = replica_cutover.respawn(upgrade);
        assert_eq!(validator_plan, replica_plan);
        let plan = validator_plan.expect("boundary cuts over");
        assert_eq!(plan.epoch(), 1);
        assert_eq!(plan.cutover_app_height(), 9);
        assert_eq!(plan.boundary_version(), 1);
        assert!(matches!(
            plan.upgrade_verdict(),
            UpgradeVerdict::Armed { name, to_version: 1 } if name == "v1"
        ));
    }
}
