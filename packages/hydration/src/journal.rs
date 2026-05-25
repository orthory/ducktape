use operation::Operation;

use crate::config;

#[derive(Debug)]
/// Journal is 
pub struct Journal {
    current: Batch,
    high_water: usize,
}

pub enum InsertResult {
    Inserted,
    Flush(Batch),
}

impl Journal {
    pub fn new(config: &config::journal::Config) -> Self {
        Self {
            current: Batch::new(config.hwm),
            high_water: config.hwm,
        }
    }
    
    pub(crate) fn insert_op(&mut self, op: Operation) -> InsertResult {
        let current = &mut self.current;
        match current.insert_op(op) {
            OperationResult::JournalInserted => InsertResult::Inserted,

            // this journal is full; create anew
            OperationResult::JournalFull(op) => {
                let old = self.drain();
                
                // lossy result but should be fine as the journal semantic only allows
                // either Inserted or Flush anyways
                let _ = self.insert_op(op);
                InsertResult::Flush(old)
            }
        }
    }

    /// drain all flushable (tip-1) journal entries
    /// 
    /// this could totally be just returning an Iterator but collect() into Vec is going to be done anyways
    /// somewhere up the stack, so better do it here (and it's clearner as well)
    pub(crate) fn drain(&mut self) -> Batch {
        std::mem::replace(&mut self.current, Batch::new(self.high_water))
    }
}

// individual journals
#[derive(Debug)]
pub struct Batch {
    tip: usize,
    /// Logical capacity — fixed at construction. Backing `ops` grows lazily
    /// via `push`; rollover triggers when `tip >= hwm`.
    hwm: usize,
    ops: Vec<Operation>,
}

pub enum OperationResult {
    JournalInserted,
    JournalFull(Operation),
}

impl Batch {
    pub(crate) fn new(hwm: usize) -> Self {
        Self { tip: 0, hwm, ops: Vec::with_capacity(hwm) }
    }

    pub fn insert_op(&mut self, op: Operation) -> OperationResult {
        if self.tip >= self.hwm {
            return OperationResult::JournalFull(op);
        }
        self.ops.push(op);
        self.tip += 1;

        OperationResult::JournalInserted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use operation::OpId;

    // distinguish test ops by their `start` field — keeps assertions simple
    fn op(k: u32) -> Operation {
        Operation::OnUserDelete {
            op_id: OpId::new(1, k),
            node_id: uid::new(),
            start: k,
            len: 1,
        }
    }

    fn op_marker(o: &Operation) -> u32 {
        match o {
            Operation::OnUserDelete { start, .. } => *start,
            _ => panic!("unexpected op variant in test"),
        }
    }

    // -------- Journal --------

    #[test]
    fn new_journal_starts_with_empty_current() {
        let c = Journal::new(&config::journal::Config { hwm: 3 });
        assert_eq!(c.current.tip, 0);
        assert!(c.current.ops.is_empty());
    }

    #[test]
    fn insert_below_capacity_returns_inserted() {
        let mut c = Journal::new(&config::journal::Config { hwm: 3 });
        for k in 1..=3 {
            let result = c.insert_op(op(k));
            assert!(matches!(result, InsertResult::Inserted));
        }
        // current fully populated, no rollover yet
        assert_eq!(c.current.tip, 3);
    }

    #[test]
    fn insert_past_capacity_rolls_over_and_returns_flush() {
        let mut c = Journal::new(&config::journal::Config { hwm: 3 });
        c.insert_op(op(1));
        c.insert_op(op(2));
        c.insert_op(op(3));
        // 4th triggers rollover — Flush carries the rotated entry
        let flushed = match c.insert_op(op(4)) {
            InsertResult::Flush(entry) => entry,
            _ => panic!("expected Flush"),
        };
        assert_eq!(flushed.tip, 3);
        assert_eq!(op_marker(&flushed.ops[0]), 1);
        assert_eq!(op_marker(&flushed.ops[2]), 3);
        // new current carries the 4th op
        assert_eq!(c.current.tip, 1);
        assert_eq!(op_marker(&c.current.ops[0]), 4);
    }

    #[test]
    fn drain_returns_current_even_if_partial() {
        let mut c = Journal::new(&config::journal::Config { hwm: 3 });
        c.insert_op(op(1));
        c.insert_op(op(2));
        // drain returns whatever's in current — partial is fine
        let drained = c.drain();
        assert_eq!(drained.tip, 2);
        assert_eq!(op_marker(&drained.ops[0]), 1);
        assert_eq!(op_marker(&drained.ops[1]), 2);
        // current is now fresh
        assert_eq!(c.current.tip, 0);
        assert!(c.current.ops.is_empty());
    }

    #[test]
    fn rollover_returns_full_entry_and_keeps_overflow_op_in_current() {
        let mut c = Journal::new(&config::journal::Config { hwm: 3 });
        // 5 ops: rollover happens at op 4
        let mut flushed = Vec::new();
        for k in 1..=5 {
            if let InsertResult::Flush(e) = c.insert_op(op(k)) {
                flushed.push(e);
            }
        }
        // exactly one rollover
        assert_eq!(flushed.len(), 1);
        assert_eq!(flushed[0].tip, 3);
        assert_eq!(op_marker(&flushed[0].ops[0]), 1);
        assert_eq!(op_marker(&flushed[0].ops[2]), 3);
        // current now has 4 and 5
        assert_eq!(c.current.tip, 2);
        assert_eq!(op_marker(&c.current.ops[0]), 4);
        assert_eq!(op_marker(&c.current.ops[1]), 5);
    }

    #[test]
    fn multiple_rollovers_emit_separate_flushes_in_order() {
        let mut c = Journal::new(&config::journal::Config { hwm: 3 });
        // 7 ops: rollovers happen at op 4 and op 7
        let mut flushed = Vec::new();
        for k in 1..=7 {
            if let InsertResult::Flush(e) = c.insert_op(op(k)) {
                flushed.push(e);
            }
        }
        assert_eq!(flushed.len(), 2);
        // FIFO: first flush is the older entry
        assert_eq!(op_marker(&flushed[0].ops[0]), 1);
        assert_eq!(op_marker(&flushed[1].ops[0]), 4);
        // current still holds op 7
        assert_eq!(c.current.tip, 1);
        assert_eq!(op_marker(&c.current.ops[0]), 7);
    }

    #[test]
    fn insert_continues_after_drain() {
        let mut c = Journal::new(&config::journal::Config { hwm: 2 });
        c.insert_op(op(1));
        c.insert_op(op(2)); // fills current
        let _ = c.insert_op(op(3)); // rollover -> Flush; current = [3]

        // drain current (which now holds op 3)
        let drained = c.drain();
        assert_eq!(drained.tip, 1);
        assert_eq!(op_marker(&drained.ops[0]), 3);

        // current keeps accepting
        let result = c.insert_op(op(4));
        assert!(matches!(result, InsertResult::Inserted));
        assert_eq!(c.current.tip, 1);
        assert_eq!(op_marker(&c.current.ops[0]), 4);
    }

    // -------- Entry --------

    #[test]
    fn new_journal_is_empty() {
        let j = Batch::new(3);
        assert_eq!(j.tip, 0);
        assert!(j.ops.is_empty());
    }

    #[test]
    fn journal_inserts_in_order_and_advances_tip() {
        let mut j = Batch::new(3);
        for k in 1..=3 {
            assert!(matches!(j.insert_op(op(k)), OperationResult::JournalInserted));
        }
        assert_eq!(j.tip, 3);
        assert_eq!(op_marker(&j.ops[0]), 1);
        assert_eq!(op_marker(&j.ops[1]), 2);
        assert_eq!(op_marker(&j.ops[2]), 3);
    }

    #[test]
    fn journal_returns_full_at_capacity() {
        let mut j = Batch::new(2);
        j.insert_op(op(1));
        j.insert_op(op(2));
        let rejected = j.insert_op(op(3));
        match rejected {
            OperationResult::JournalFull(returned) => {
                // op handed back intact for retry
                assert_eq!(op_marker(&returned), 3);
            }
            _ => panic!("expected JournalFull"),
        }
        // tip didn't advance past capacity
        assert_eq!(j.tip, 2);
    }

    #[test]
    fn full_journal_stays_full_on_repeated_attempts() {
        let mut j = Batch::new(1);
        j.insert_op(op(1));
        // every further attempt rejects without panic
        for k in 2..=5 {
            assert!(matches!(j.insert_op(op(k)), OperationResult::JournalFull(_)));
        }
        assert_eq!(j.tip, 1);
    }
}
