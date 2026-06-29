use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Op {
    Document(document::op::Op),
    Workspace(workspace::op::Op),
    Vcs(vcs::op::Op),
    Control(control::op::Op),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Broadcast,
    Consensus,
}

impl Op {
    pub fn lane(&self) -> Lane {
        match self {
            Op::Document(_) => Lane::Broadcast,

            // workspace: EntryMut (a doc fragment) gossips; structural ops
            // consense. NOTE: under the canonical-head inversion these structural
            // ops also move to Broadcast (branches gossip; only main-advances
            // consense), but that's tied to branch-gossip (B5) and the
            // dual-homing seam (group D). left unchanged here on purpose — A0
            // reshapes only the vcs slice of routing.
            Op::Workspace(workspace::op::Op::EntryMut { .. }) => Lane::Broadcast,
            Op::Workspace(_) => Lane::Consensus,

            // vcs (the canonical-head routing): consensus total-orders only an
            // advance of the ONE canonical ref. every other ref gossips, and an
            // Announce is a pure availability hint.
            Op::Vcs(vcs::op::Op::RefUpdate { name, .. }) if name.as_str() == vcs::op::MAIN_REF => {
                Lane::Consensus
            }
            Op::Vcs(vcs::op::Op::RefUpdate { .. }) => Lane::Broadcast,
            Op::Vcs(vcs::op::Op::Announce { .. }) => Lane::Broadcast,

            Op::Control(_) => Lane::Consensus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_op_routes_broadcast() {
        let op = Op::Document(document::op::Op::OnUserBlur);
        assert_eq!(op.lane(), Lane::Broadcast);
    }

    #[test]
    fn vcs_main_ref_update_routes_consensus() {
        let op = Op::Vcs(vcs::op::Op::RefUpdate {
            name: vcs::op::MAIN_REF.to_string(),
            target: vcs::ObjectId::from_bytes([0u8; 32]),
            prev: None,
        });
        assert_eq!(op.lane(), Lane::Consensus);
    }

    #[test]
    fn vcs_non_main_ref_update_routes_broadcast() {
        let op = Op::Vcs(vcs::op::Op::RefUpdate {
            name: "refs/heads/feature".to_string(),
            target: vcs::ObjectId::from_bytes([1u8; 32]),
            prev: None,
        });
        assert_eq!(op.lane(), Lane::Broadcast);
    }

    #[test]
    fn vcs_announce_routes_broadcast() {
        let op = Op::Vcs(vcs::op::Op::Announce { objects: Vec::new() });
        assert_eq!(op.lane(), Lane::Broadcast);
    }

    #[test]
    fn workspace_structural_op_routes_consensus() {
        let op = Op::Workspace(workspace::op::Op::RemoveEntry { entry_id: uid::new() });
        assert_eq!(op.lane(), Lane::Consensus);
    }
}
