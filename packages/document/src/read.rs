use sections::Sections;

use crate::Document;

impl Document {
    /// Yields parsed sections one at a time, cloned, in document order. Pair with an
    /// fs driver or similar when a streamed read is preferable to materializing the
    /// whole vector.
    pub fn sections_iter(&self) -> impl Iterator<Item = Sections> + '_ {
        self.sections.iter().cloned()
    }
 
    /// Returns every parsed section in document order, cloned. This is the bulk
    /// "structured view" of the document — the primary entry point for consumers
    /// that want all sections at once before subscribing to incremental updates.
    pub fn sections(&self) -> Vec<Sections> {
        self.sections.clone()
    }
}
