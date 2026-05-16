use std::ops::Range;

use nodes::{Node, Nodes};
use uid::Uid;

use crate::{Document, DocumentInstanceError};

pub enum DocumentIngressError {

}

impl Document {
    pub fn write(&mut self, pos: usize, node: Nodes) -> Result<Uid, DocumentInstanceError> {
        let splice_range =

        self.nodes.

        todo!()
    }

    pub fn write_nodes() -> Result<String, DocumentInstanceError> {
        todo!()
    }


}
