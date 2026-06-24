use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Op {
    Document(document::op::Op),
    Workspace(workspace::op::Op),
}
