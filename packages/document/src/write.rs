use crate::{Document, journal::ContainerResult, operation::Operation};

pub enum Errors {}

impl Document {
    pub fn apply(&mut self, op: Operation) -> Result<(), Errors> {
        match self.journal.insert_op(op) {
            ContainerResult::Inserted => Ok(()),
            ContainerResult::Flush => {
                let _flushable = self.journal.drain_flushable();

                // at this point, journal is already flushed.
                // really apply the changes
                Ok(())
            }
        }
    }
}
