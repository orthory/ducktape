use nodes::Nodes;

use crate::Document;

impl Document {
    /// Yields parsed nodes one at a time, cloned, in document order. Pair with an
    /// fs driver or similar when a streamed read is preferable to materializing the
    /// whole vector.
    pub fn nodes_iter(&self) -> impl Iterator<Item = Nodes> + '_ {
        self.nodes.iter().cloned()
    }

    /// Returns every parsed node in document order, cloned. This is the bulk
    /// "structured view" of the document — the primary entry point for consumers
    /// that want all nodes at once before subscribing to incremental updates.
    pub fn nodes(&self) -> Vec<Nodes> {
        self.nodes.clone()
    }
}
