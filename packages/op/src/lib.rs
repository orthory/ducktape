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
            Op::Workspace(workspace::op::Op::EntryMut { .. }) => Lane::Broadcast,
            Op::Workspace(_) => Lane::Consensus,
            Op::Vcs(_) => Lane::Consensus,
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
    fn vcs_op_routes_consensus() {
        let op = Op::Vcs(vcs::op::Op::Init);
        assert_eq!(op.lane(), Lane::Consensus);
    }
}
