use crate::Document;

impl Document {
    pub fn nodes_iter(&self) -> impl Iterator<Item = nodes::Nodes> + '_ {
        NodesIter {
            cursor: Some(&self.uid),
            nodes: self,
        }
    }
}

struct NodesIter<'nodes> {
    cursor: Option<&'nodes uid::Uid>,
    nodes: &'nodes Document,
}

impl<'nodes> Iterator for NodesIter<'nodes> {
    type Item = nodes::Nodes;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.cursor?;
        let (node, next) = self.nodes.nodes_map.get(next)?;
        self.cursor = next.as_ref();
        Some(node.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::JournalContainer;
    use nodes::{FrontmatterV1, Nodes};
    use std::collections::HashMap;

    const HWM: usize = 8;

    /// Build a Document containing a chain of `n` Frontmatter nodes, each titled
    /// `"node-{i}"` so iteration order can be verified by reading titles back out.
    /// Constructs `Document` directly via its `pub(crate)` fields — no dependency
    /// on `from_reader`.
    fn make_chain(n: usize) -> Document {
        let uids: Vec<uid::Uid> = (0..n).map(|_| uid::new()).collect();
        let mut nodes_map = HashMap::with_capacity(n);
        for (i, &u) in uids.iter().enumerate() {
            let node = Nodes::Frontmatter(FrontmatterV1 {
                uid: u,
                title: format!("node-{}", i),
                ..Default::default()
            });
            let next = uids.get(i + 1).copied();
            nodes_map.insert(u, (node, next));
        }

        Document {
            uid: uids[0],
            journal: JournalContainer::new(HWM),
            nodes_map,
        }
    }

    fn marker(n: &Nodes) -> String {
        match n {
            Nodes::Frontmatter(f) => f.title.clone(),
            _ => panic!("unexpected variant in test fixture"),
        }
    }

    #[test]
    fn iterates_single_node() {
        let doc = make_chain(1);
        let collected: Vec<_> = doc.nodes_iter().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(marker(&collected[0]), "node-0");
    }

    #[test]
    fn iterates_chain_in_order() {
        let doc = make_chain(5);
        let collected: Vec<_> = doc.nodes_iter().collect();
        assert_eq!(collected.len(), 5);
        for (i, node) in collected.iter().enumerate() {
            assert_eq!(marker(node), format!("node-{}", i));
        }
    }

    #[test]
    fn iterator_terminates_after_last_node() {
        let doc = make_chain(3);
        let mut iter = doc.nodes_iter();
        for _ in 0..3 {
            assert!(iter.next().is_some());
        }
        assert!(iter.next().is_none());
        // exhausted iterator stays exhausted on subsequent calls
        assert!(iter.next().is_none());
    }

    #[test]
    fn fresh_iter_each_call() {
        let doc = make_chain(3);
        let first: Vec<_> = doc.nodes_iter().collect();
        let second: Vec<_> = doc.nodes_iter().collect();
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(marker(a), marker(b));
        }
    }

    #[test]
    fn handles_dangling_frontmatter_id() {
        // If uid points to a key not in the map (shouldn't happen under a
        // correct constructor, but iterator should be defensive), iteration
        // yields nothing rather than panicking.
        let doc = Document {
            uid: uid::new(),
            journal: JournalContainer::new(HWM),
            nodes_map: HashMap::new(),
        };
        assert_eq!(doc.nodes_iter().count(), 0);
    }

    #[test]
    fn iterates_two_node_chain() {
        let doc = make_chain(2);
        let mut iter = doc.nodes_iter();
        let a = iter.next().expect("first node");
        let b = iter.next().expect("second node");
        assert_eq!(marker(&a), "node-0");
        assert_eq!(marker(&b), "node-1");
        assert!(iter.next().is_none());
    }
}
