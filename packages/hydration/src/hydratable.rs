pub trait Hydratable {
    type Op;
    fn hydrate(&mut self, op: impl Iterator<Item = Self::Op>);
}
