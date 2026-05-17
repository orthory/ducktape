use nodes::Nodes;
use uid::Uid;

use crate::{Document, Errors};

pub enum DocumentIngressError {}

impl Document {
    pub fn write(&mut self, _pos: usize, _node: Nodes) -> Result<Uid, Errors> {
        todo!()
    }

    pub fn write_nodes() -> Result<String, Errors> {
        todo!()
    }
}
