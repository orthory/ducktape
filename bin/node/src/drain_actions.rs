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
use sdk::StateRoot;

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
    pub(crate) op_count: usize,
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
            if let Some(op) = &frame.op
                && op.target != NOP_TARGET
            {
                let disposition = match frame.disposition {
                    node::Disposition::Applied => noded::BlockDisposition::Applied,
                    node::Disposition::Rejected => noded::BlockDisposition::Rejected,
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
            }
        }
        if let Some(system) = system_dispatches.remove(&height) {
            dispatches.extend(system);
        }
        let op_count = ops.len();
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
            op_count,
        });
    }
    actions
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CutoverTrigger {
    Membership(ScheduledCutover),
    Upgrade {
        cutover: ScheduledCutover,
        name: String,
        activation_height: u64,
    },
}

impl CutoverTrigger {
    pub(crate) fn cutover(&self) -> ScheduledCutover {
        match self {
            Self::Membership(cutover) | Self::Upgrade { cutover, .. } => *cutover,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EpochActions {
    pub(crate) trigger: Option<CutoverTrigger>,
    pub(crate) respawn: Option<RespawnPlan<ed25519::PublicKey>>,
}

/// Advance the shared observe -> ceiling -> cutover state machine from one
/// finalized boundary. The caller applies the returned ceiling and concrete
/// orderer swap in its existing role-specific order.
pub(crate) fn epoch_actions(
    orchestrator: &mut ValsetOrchestrator<ed25519::PublicKey>,
    finalized_view: u64,
    members: Vec<ed25519::PublicKey>,
    residents: Vec<ed25519::PublicKey>,
    boundary_upgrade: BoundaryUpgrade<ed25519::PublicKey>,
) -> EpochActions {
    let mut trigger = match orchestrator.observe_members(
        finalized_view,
        members.iter().cloned(),
        residents.iter().cloned(),
    ) {
        ObservationOutcome::Scheduled(cutover) => Some(CutoverTrigger::Membership(cutover)),
        _ => None,
    };
    if let Some(pending) = &boundary_upgrade.pending
        && let ObservationOutcome::Scheduled(cutover) =
            orchestrator.observe_upgrade(finalized_view, pending.activation_height)
    {
        debug_assert!(trigger.is_none(), "only one cutover slot can be scheduled");
        trigger = Some(CutoverTrigger::Upgrade {
            cutover,
            name: pending.name.clone(),
            activation_height: pending.activation_height,
        });
    }
    let respawn = orchestrator.respawn_if_due(finalized_view, members, residents, boundary_upgrade);
    EpochActions { trigger, respawn }
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
        assert_eq!(block.op_count, 2);
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
        assert_eq!(discarded.op_count, 0);
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

        let validator_arm = epoch_actions(
            &mut validator,
            7,
            boundary.clone(),
            Vec::new(),
            upgrade.clone(),
        );
        let replica_arm = epoch_actions(
            &mut replica,
            7,
            boundary.clone(),
            Vec::new(),
            upgrade.clone(),
        );
        assert_eq!(validator_arm, replica_arm);
        let trigger = validator_arm
            .trigger
            .expect("membership schedules the shared slot");
        assert!(matches!(trigger, CutoverTrigger::Membership(_)));
        assert_eq!(trigger.cutover().cutover_view(), 9);
        assert!(validator_arm.respawn.is_none());

        let validator_cutover = epoch_actions(
            &mut validator,
            9,
            boundary.clone(),
            Vec::new(),
            upgrade.clone(),
        );
        let replica_cutover = epoch_actions(&mut replica, 9, boundary, Vec::new(), upgrade);
        assert_eq!(validator_cutover, replica_cutover);
        assert!(validator_cutover.trigger.is_none());
        let plan = validator_cutover.respawn.expect("boundary cuts over");
        assert_eq!(plan.epoch(), 1);
        assert_eq!(plan.cutover_app_height(), 9);
        assert_eq!(plan.boundary_version(), 1);
        assert!(matches!(
            plan.upgrade_verdict(),
            UpgradeVerdict::Armed { name, to_version: 1 } if name == "v1"
        ));
    }
}
