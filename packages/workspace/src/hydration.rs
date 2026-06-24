use crate::workspace::Workspace;

use hydration::Hydratable;

impl Hydratable for Workspace {
    type Op = crate::op::Op;

    fn hydrate(&mut self, op: impl Iterator<Item = Self::Op>) {
        todo!()
    }
}
