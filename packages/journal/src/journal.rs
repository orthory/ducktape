use std::{collections::VecDeque};

use crate::operation::Operation;

#[derive(Debug)]
pub struct JournalContainer<const HWM: usize> {
    backlog: VecDeque<Journal<HWM>>
}

pub enum ContainerResult {
    Inserted,
    Flush
}

impl <const HWM: usize>JournalContainer<HWM> {
    pub fn new() -> Self {
        Self {
            backlog: VecDeque::from(vec![Journal::<HWM>::new()]),
        }
    }

    pub fn insert_op(&mut self, op: Operation) -> ContainerResult {
        let current = self.backlog.back_mut()
            .expect("backlog should never be empty");
        
        match current.insert_op(op) {
            OperationResult::JournalInserted => {
                ContainerResult::Inserted
            },

            // this journal is full; create anew
            OperationResult::JournalFull(op) => {
                self.backlog.push_back(Journal::<HWM>::new());

                // lossy result but should be fine as the journal semantic only allows
                // either Inserted or Flush anyways
                let _ = self.insert_op(op);
                ContainerResult::Flush
            },
        }
    }

    pub fn drain_flushable(&mut self) -> Vec<Journal<HWM>> {
        self.backlog
            .drain(..self.backlog.len().saturating_sub(1))
            .collect()
    }
}

// individual journals
#[derive(Debug)]
struct Journal<const HWM: usize> {
    tip: usize,
    ops: [Option<Operation>; HWM]
}

pub enum OperationResult {
    JournalInserted,
    JournalFull(Operation)
}

impl <const HWM: usize>Journal<HWM> {
    pub(crate) fn new() -> Self {
        Self {
            tip: 0,
            ops: std::array::from_fn(|_| None),
        }
    }
    
    pub fn insert_op(&mut self, op: Operation) -> OperationResult {
        if self.tip >= HWM {
            return OperationResult::JournalFull(op)
        }
        self.ops[self.tip] = Some(op); 
        self.tip += 1;

        OperationResult::JournalInserted
    }
}

impl <const HWM: usize>Default for Journal<HWM> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::OpId;

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

    // -------- JournalContainer --------

    #[test]
    fn new_container_holds_one_empty_journal() {
        let c = JournalContainer::<3>::new();
        assert_eq!(c.backlog.len(), 1);
        assert_eq!(c.backlog.back().unwrap().tip, 0);
    }

    #[test]
    fn insert_below_capacity_returns_inserted() {
        let mut c = JournalContainer::<3>::new();
        for k in 1..=3 {
            let result = c.insert_op(op(k));
            assert!(matches!(result, ContainerResult::Inserted));
        }
        // single journal, fully populated, no rollover yet
        assert_eq!(c.backlog.len(), 1);
        assert_eq!(c.backlog.back().unwrap().tip, 3);
    }

    #[test]
    fn insert_past_capacity_rolls_over_and_returns_flush() {
        let mut c = JournalContainer::<3>::new();
        c.insert_op(op(1));
        c.insert_op(op(2));
        c.insert_op(op(3));
        // 4th triggers rollover
        let result = c.insert_op(op(4));
        assert!(matches!(result, ContainerResult::Flush));
        assert_eq!(c.backlog.len(), 2);
        // first journal is full
        assert_eq!(c.backlog.front().unwrap().tip, 3);
        // new current carries the 4th op
        assert_eq!(c.backlog.back().unwrap().tip, 1);
        assert_eq!(
            op_marker(c.backlog.back().unwrap().ops[0].as_ref().unwrap()),
            4
        );
    }

    #[test]
    fn drain_returns_empty_when_only_current() {
        let mut c = JournalContainer::<3>::new();
        c.insert_op(op(1));
        c.insert_op(op(2));
        let drained = c.drain_flushable();
        assert!(drained.is_empty());
        // current journal preserved
        assert_eq!(c.backlog.len(), 1);
        assert_eq!(c.backlog.back().unwrap().tip, 2);
    }

    #[test]
    fn drain_takes_full_journals_keeps_current() {
        let mut c = JournalContainer::<3>::new();
        // 5 ops: first journal full (1,2,3), second has 4,5
        for k in 1..=5 {
            c.insert_op(op(k));
        }
        assert_eq!(c.backlog.len(), 2);

        let drained = c.drain_flushable();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].tip, 3);
        assert_eq!(op_marker(drained[0].ops[0].as_ref().unwrap()), 1);
        assert_eq!(op_marker(drained[0].ops[2].as_ref().unwrap()), 3);

        // current journal intact with ops 4 and 5
        assert_eq!(c.backlog.len(), 1);
        assert_eq!(c.backlog.back().unwrap().tip, 2);
        assert_eq!(
            op_marker(c.backlog.back().unwrap().ops[0].as_ref().unwrap()),
            4
        );
        assert_eq!(
            op_marker(c.backlog.back().unwrap().ops[1].as_ref().unwrap()),
            5
        );
    }

    #[test]
    fn multiple_rollovers_drain_in_order() {
        let mut c = JournalContainer::<3>::new();
        // 7 ops: 3 + 3 + 1
        for k in 1..=7 {
            c.insert_op(op(k));
        }
        assert_eq!(c.backlog.len(), 3);

        let drained = c.drain_flushable();
        assert_eq!(drained.len(), 2);
        // FIFO: drained[0] is the oldest
        assert_eq!(op_marker(drained[0].ops[0].as_ref().unwrap()), 1);
        assert_eq!(op_marker(drained[1].ops[0].as_ref().unwrap()), 4);
        // current still holds op 7
        assert_eq!(c.backlog.back().unwrap().tip, 1);
        assert_eq!(
            op_marker(c.backlog.back().unwrap().ops[0].as_ref().unwrap()),
            7
        );
    }

    #[test]
    fn insert_continues_after_drain() {
        let mut c = JournalContainer::<2>::new();
        c.insert_op(op(1));
        c.insert_op(op(2)); // fills first journal
        c.insert_op(op(3)); // rollover
        let drained = c.drain_flushable();
        assert_eq!(drained.len(), 1);

        // current keeps accepting
        let result = c.insert_op(op(4));
        assert!(matches!(result, ContainerResult::Inserted));
        assert_eq!(c.backlog.back().unwrap().tip, 2);
    }

    // -------- Journal --------

    #[test]
    fn new_journal_is_empty() {
        let j = Journal::<3>::new();
        assert_eq!(j.tip, 0);
        assert!(j.ops.iter().all(|slot| slot.is_none()));
    }

    #[test]
    fn journal_inserts_in_order_and_advances_tip() {
        let mut j = Journal::<3>::new();
        for k in 1..=3 {
            assert!(matches!(j.insert_op(op(k)), OperationResult::JournalInserted));
        }
        assert_eq!(j.tip, 3);
        assert_eq!(op_marker(j.ops[0].as_ref().unwrap()), 1);
        assert_eq!(op_marker(j.ops[1].as_ref().unwrap()), 2);
        assert_eq!(op_marker(j.ops[2].as_ref().unwrap()), 3);
    }

    #[test]
    fn journal_returns_full_at_capacity() {
        let mut j = Journal::<2>::new();
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
        // tip didn't advance past HWM
        assert_eq!(j.tip, 2);
    }

    #[test]
    fn full_journal_stays_full_on_repeated_attempts() {
        let mut j = Journal::<1>::new();
        j.insert_op(op(1));
        // every further attempt rejects without panic
        for k in 2..=5 {
            assert!(matches!(j.insert_op(op(k)), OperationResult::JournalFull(_)));
        }
        assert_eq!(j.tip, 1);
    }
}
