//! Orderer-independent drain projections shared by validators and replicas.
//!
//! The concrete loops still own drain timing and side-effect order. This seam
//! only turns an already-drained result into the block and epoch actions both
//! roles must interpret identically.

use std::collections::BTreeMap;

use commonware_cryptography::ed25519;
use consensus::{
    BoundaryUpgrade, ObservationOutcome, RespawnPlan, ScheduledCutover, ValsetOrchestrator,
};
use sdk::{Origin, StateRoot};

use crate::constants::NOP_TARGET;
use crate::explorer::explorer_root_op;
use crate::util::hex;

pub(crate) struct BlockAction {
    pub(crate) height: u64,
    pub(crate) dispatches: Vec<host::DispatchRecord>,
    pub(crate) record: Option<Vec<u8>>,
    pub(crate) sealed_hash: Option<StateRoot>,
    pub(crate) applied: bool,
    pub(crate) latency_us: u64,
    pub(crate) applied_ops: usize,
    pub(crate) rejected_ops: usize,
}

/// Group a drain's per-frame outcomes into the per-block actions consumed by
/// both role loops. Member dispatches precede System dispatches, matching live
/// indexing and replay; discarded frames retain their existing empty action.
pub(crate) fn block_actions(
    drained: &[node::DrainedFrame],
    system_dispatches: Vec<(u64, Vec<host::DispatchRecord>)>,
    blobs: &blobstore::BlobHandle,
) -> Vec<BlockAction> {
    let mut system_dispatches: BTreeMap<_, _> = system_dispatches.into_iter().collect();
    let mut actions = Vec::new();
    let mut i = 0;
    while i < drained.len() {
        let height = drained[i].height;
        let mut dispatches = Vec::new();
        let mut latency_us = 0u64;
        let mut applied = false;
        let mut ops = Vec::new();
        let mut applied_ops = 0usize;
        let mut rejected_ops = 0usize;
        let mut block_hash = None;
        let mut block_app_hash = None;
        let mut sealed_hash = None;
        while i < drained.len() && drained[i].height == height {
            let frame = &drained[i];
            i += 1;
            if frame.disposition == node::Disposition::Discarded {
                continue;
            }
            sealed_hash = Some(frame.app_hash);
            if let (node::Disposition::Applied, Some(op)) = (&frame.disposition, &frame.op) {
                applied = true;
                latency_us = latency_us.saturating_add(op.latency_us);
                dispatches.extend(op.dispatches.iter().cloned());
            }
            // the envelope's released continuation: its dispatches join the
            // block's op stream right after its parent's (the host's event
            // order), INDEPENDENT of the parent's disposition — a rejected
            // parent still releases, and an applied continuation is real work.
            if let Some(cont) = frame.op.as_ref().and_then(|op| op.continuation.as_ref())
                && cont.disposition == node::Disposition::Applied
            {
                applied = true;
                dispatches.extend(cont.dispatches.iter().cloned());
            }
            if let Some(op) = &frame.op
                && op.target != NOP_TARGET
            {
                let disposition = match frame.disposition {
                    node::Disposition::Applied => {
                        applied_ops += 1;
                        noded::BlockDisposition::Applied
                    }
                    node::Disposition::Rejected => {
                        rejected_ops += 1;
                        noded::BlockDisposition::Rejected
                    }
                    node::Disposition::Discarded => continue,
                };
                if block_hash.is_none() {
                    block_hash = Some(frame.id);
                    block_app_hash = Some(frame.app_hash);
                }
                ops.push(explorer_root_op(
                    blobs,
                    &op.origin,
                    &op.target,
                    &op.payload,
                    &op.dispatches,
                    disposition,
                ));
                // the continuation is its own row, right after its parent:
                // `Origin::Module(parent_target)` is the sending lane, and its
                // own disposition — not the parent's — is the row's.
                if let Some(cont) = &op.continuation {
                    let cont_disposition = match cont.disposition {
                        node::Disposition::Applied => {
                            applied_ops += 1;
                            noded::BlockDisposition::Applied
                        }
                        _ => {
                            rejected_ops += 1;
                            noded::BlockDisposition::Rejected
                        }
                    };
                    ops.push(explorer_root_op(
                        blobs,
                        &Origin::Module(op.target.clone()),
                        &cont.target,
                        &cont.payload,
                        &cont.dispatches,
                        cont_disposition,
                    ));
                }
            }
        }
        if let Some(system) = system_dispatches.remove(&height) {
            dispatches.extend(system);
        }
        let record = (!ops.is_empty()).then(|| {
            noded::block_row(&noded::BlockRecord {
                height,
                hash: block_hash
                    .map(|hash| noded::hex_bytes(&hash))
                    .unwrap_or_default(),
                commit_hash: block_app_hash.map(|hash| hex(&hash)).unwrap_or_default(),
                ops,
            })
        });
        actions.push(BlockAction {
            height,
            dispatches,
            record,
            sealed_hash,
            applied,
            latency_us,
            applied_ops,
            rejected_ops,
        });
    }
    actions
}

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
    use sdk::{Origin, StateRoot};

    use super::*;

    fn dispatch(module: &str, payload: &[u8]) -> host::DispatchRecord {
        host::DispatchRecord {
            module: module.into(),
            origin: Origin::Module("source".into()),
            payload: payload.to_vec(),
            emitted_msgs: 0,
            emitted_events: 0,
        }
    }

    fn drained(
        id: u8,
        height: u64,
        disposition: node::Disposition,
        root: u8,
        op: Option<node::DrainedOp>,
    ) -> node::DrainedFrame {
        node::DrainedFrame {
            id: [id; 32],
            height,
            disposition,
            app_hash: StateRoot([root; sdk::ROOT_LEN]),
            op,
            reason: None,
        }
    }

    #[test]
    fn block_actions_pin_validator_replica_parity() {
        let member_dispatch = dispatch("chat", b"member");
        let system_dispatch = dispatch("upgrade", b"system");
        let frames = vec![
            drained(
                1,
                7,
                node::Disposition::Applied,
                9,
                Some(node::DrainedOp {
                    origin: Origin::External(vec![1]),
                    target: "chat".into(),
                    payload: b"applied".to_vec(),
                    dispatches: vec![member_dispatch.clone()],
                    latency_us: 11,
                    continuation: None,
                }),
            ),
            drained(
                2,
                7,
                node::Disposition::Rejected,
                9,
                Some(node::DrainedOp {
                    origin: Origin::External(vec![2]),
                    target: "chat".into(),
                    payload: b"rejected".to_vec(),
                    dispatches: Vec::new(),
                    latency_us: 99,
                    continuation: None,
                }),
            ),
            drained(3, 8, node::Disposition::Discarded, 9, None),
        ];
        let actions = block_actions(
            &frames,
            vec![(7, vec![system_dispatch.clone()])],
            &blobstore::BlobHandle::default(),
        );

        assert_eq!(actions.len(), 2);
        let block = &actions[0];
        assert_eq!(block.height, 7);
        assert_eq!(block.dispatches, vec![member_dispatch, system_dispatch]);
        assert!(block.applied);
        assert_eq!(block.latency_us, 11);
        assert_eq!((block.applied_ops, block.rejected_ops), (1, 1));
        assert_eq!(block.sealed_hash, Some(StateRoot([9; sdk::ROOT_LEN])));
        let row: serde_json::Value =
            serde_json::from_slice(block.record.as_deref().expect("explorer row")).unwrap();
        assert_eq!(row["ops"][0]["disposition"], "applied");
        assert_eq!(row["ops"][1]["disposition"], "rejected");
        assert_eq!(row["hash"], noded::hex_bytes(&[1; 32]));

        let discarded = &actions[1];
        assert_eq!(discarded.height, 8);
        assert!(discarded.dispatches.is_empty());
        assert!(!discarded.applied);
        assert_eq!((discarded.applied_ops, discarded.rejected_ops), (0, 0));
        assert_eq!(discarded.sealed_hash, None);
        assert_eq!(discarded.record, None);
    }

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
