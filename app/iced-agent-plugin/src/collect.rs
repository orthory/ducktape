//! The tree collector: an `iced::advanced::widget::Operation` that turns the
//! bracketed [`SemProbe`](crate::sem::SemProbe) stream emitted by `sem` nodes
//! into per-window [`WindowSnapshot`]s, plus [`to_accesskit`] converting a
//! snapshot into the `TreeUpdate` pushed to the OS adapter.
//!
//! Ref invariant: a node's `@N` handle maps to AccessKit `NodeId(N)`, one
//! monotonic counter across all windows in a single walk.

use std::sync::{Arc, Mutex};

use iced::advanced::widget::operation::Focusable;
use iced::advanced::widget::{Id, Operation};
use iced::advanced::widget::operation::Outcome;
use iced::Rectangle;

use crate::protocol::{Rect, Role, SemNode};
use crate::sem::SemProbe;

/// Shared store the app's snapshot task fills each tick and the bridge reads.
pub type SnapshotSlot = Arc<Mutex<Vec<WindowSnapshot>>>;

/// A flattened, ref-addressable view of one node, for `find` / target resolve.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlatNode {
    #[serde(rename = "ref")]
    pub r#ref: String,
    pub role: Role,
    pub name: String,
    pub bounds: Rect,
}

/// One window's collected tree: its root (a `Role::Window` sem node), the flat
/// list for lookup, and the window name (the root sem node's name).
#[derive(Debug, Clone)]
pub struct WindowSnapshot {
    pub window_name: String,
    pub nodes: SemNode,
    pub flat: Vec<FlatNode>,
}

impl WindowSnapshot {
    /// Builds a snapshot from a collected root node.
    pub fn from_root(root: SemNode) -> Self {
        let mut flat = Vec::new();
        flatten(&root, &mut flat);
        Self {
            window_name: root.name.clone(),
            nodes: root,
            flat,
        }
    }
}

fn flatten(node: &SemNode, out: &mut Vec<FlatNode>) {
    out.push(FlatNode {
        r#ref: node.r#ref.clone(),
        role: node.role,
        name: node.name.clone(),
        bounds: node.bounds,
    });
    for child in &node.children {
        flatten(child, out);
    }
}

/// Collects `sem` bracket probes into a tree. Constructed per snapshot tick;
/// after the runtime walks every window it calls [`Operation::finish`], which
/// publishes the finished snapshots into the shared slot.
#[derive(Default)]
pub struct Collector {
    slot: SnapshotSlot,
    stack: Vec<SemNode>,
    roots: Vec<SemNode>,
    counter: u64,
}

impl Collector {
    /// A collector that publishes into `slot` when the walk finishes.
    pub fn new(slot: SnapshotSlot) -> Self {
        Self {
            slot,
            ..Self::default()
        }
    }
}

impl Operation<()> for Collector {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
        // Non-scoping: keep descending into every child.
        operate(self);
    }

    fn custom(&mut self, _id: Option<&Id>, bounds: Rectangle, state: &mut dyn std::any::Any) {
        let Some(probe) = state.downcast_mut::<SemProbe>() else {
            return;
        };
        match probe {
            SemProbe::Enter {
                role,
                name,
                value,
                disabled,
            } => {
                self.counter += 1;
                self.stack.push(SemNode {
                    r#ref: format!("@{}", self.counter),
                    role: *role,
                    name: std::mem::take(name),
                    value: value.take(),
                    bounds: bounds.into(),
                    disabled: *disabled,
                    focused: false,
                    children: Vec::new(),
                });
            }
            SemProbe::Exit => {
                let Some(node) = self.stack.pop() else {
                    return;
                };
                match self.stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => self.roots.push(node),
                }
            }
        }
    }

    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        if state.is_focused()
            && let Some(top) = self.stack.last_mut()
        {
            top.focused = true;
        }
    }

    fn finish(&self) -> Outcome<()> {
        let snapshots: Vec<WindowSnapshot> =
            self.roots.iter().cloned().map(WindowSnapshot::from_root).collect();
        if let Ok(mut guard) = self.slot.lock() {
            *guard = snapshots;
        }
        Outcome::None
    }
}

/// Parses an `@N` handle into its numeric id. Malformed refs (never produced
/// by the collector) fall back to `0`.
fn ref_id(r#ref: &str) -> u64 {
    r#ref.trim_start_matches('@').parse().unwrap_or(0)
}

/// Depth-first id of the first focused node, if any.
fn first_focused(node: &SemNode) -> Option<u64> {
    if node.focused {
        return Some(ref_id(&node.r#ref));
    }
    node.children.iter().find_map(first_focused)
}

/// Converts a snapshot into an AccessKit `TreeUpdate`. `@N` maps to
/// `NodeId(N)`; the tree root is the window node's id, and focus points at the
/// first focused node (or the root).
pub fn to_accesskit(snapshot: &WindowSnapshot) -> iced_winit::accesskit::TreeUpdate {
    use iced_winit::accesskit::{NodeId, Tree, TreeId, TreeUpdate};

    let mut nodes = Vec::new();
    let root_id = walk(&snapshot.nodes, &mut nodes);
    let focus = first_focused(&snapshot.nodes).unwrap_or(root_id);
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(NodeId(root_id))),
        tree_id: TreeId::ROOT,
        focus: NodeId(focus),
    }
}

fn walk(node: &SemNode, out: &mut Vec<(iced_winit::accesskit::NodeId, iced_winit::accesskit::Node)>) -> u64 {
    use iced_winit::accesskit::{Node, NodeId, Rect};

    let id = ref_id(&node.r#ref);
    let mut ak = Node::new(node.role.to_accesskit());
    if !node.name.is_empty() {
        ak.set_label(node.name.clone());
    }
    if let Some(value) = &node.value {
        ak.set_value(value.clone());
    }
    let b = &node.bounds;
    ak.set_bounds(Rect::new(
        b.x as f64,
        b.y as f64,
        (b.x + b.width) as f64,
        (b.y + b.height) as f64,
    ));
    if node.disabled {
        ak.set_disabled();
    }
    let child_ids: Vec<NodeId> = node.children.iter().map(|c| NodeId(walk(c, out))).collect();
    if !child_ids.is_empty() {
        ak.set_children(child_ids);
    }
    out.push((NodeId(id), ak));
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::widget::Operation;

    fn rect() -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 20.0,
        }
    }

    fn enter(role: Role, name: &str) -> SemProbe {
        SemProbe::Enter {
            role,
            name: name.into(),
            value: None,
            disabled: false,
        }
    }

    #[test]
    fn bracket_stream_builds_hierarchy() {
        let mut c = Collector::default();
        let b = rect();
        let mut e_root = enter(Role::Window, "main");
        let mut e_btn = enter(Role::Button, "Forge");
        let mut exit = SemProbe::Exit;
        let mut exit2 = SemProbe::Exit;
        c.custom(None, b, &mut e_root);
        c.custom(None, b, &mut e_btn);
        c.custom(None, b, &mut exit);
        c.custom(None, b, &mut exit2);

        assert_eq!(c.roots.len(), 1);
        assert_eq!(c.roots[0].r#ref, "@1");
        assert_eq!(c.roots[0].children.len(), 1);
        assert_eq!(c.roots[0].children[0].name, "Forge");
        assert_eq!(c.roots[0].children[0].r#ref, "@2");
    }

    #[test]
    fn finish_publishes_snapshots() {
        let slot: SnapshotSlot = SnapshotSlot::default();
        let mut c = Collector::new(slot.clone());
        let b = rect();
        let mut e_root = enter(Role::Window, "main");
        let mut e_btn = enter(Role::Button, "Forge");
        let mut exit = SemProbe::Exit;
        let mut exit2 = SemProbe::Exit;
        c.custom(None, b, &mut e_root);
        c.custom(None, b, &mut e_btn);
        c.custom(None, b, &mut exit);
        c.custom(None, b, &mut exit2);
        c.finish();

        let snaps = slot.lock().unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].window_name, "main");
        // flat = window + button.
        assert_eq!(snaps[0].flat.len(), 2);
        assert_eq!(snaps[0].flat[1].r#ref, "@2");
    }

    #[test]
    fn accesskit_ids_match_refs() {
        use iced_winit::accesskit::NodeId;

        let mut c = Collector::default();
        let b = rect();
        let mut e_root = enter(Role::Window, "main");
        let mut e_btn = enter(Role::Button, "Forge");
        let mut exit = SemProbe::Exit;
        let mut exit2 = SemProbe::Exit;
        c.custom(None, b, &mut e_root);
        c.custom(None, b, &mut e_btn);
        c.custom(None, b, &mut exit);
        c.custom(None, b, &mut exit2);

        let snapshot = WindowSnapshot::from_root(c.roots.pop().unwrap());
        let update = to_accesskit(&snapshot);

        // Tree root is the window node @1 == NodeId(1).
        assert_eq!(update.tree.as_ref().unwrap().root, NodeId(1));
        // NodeId(2) is the "Forge" button.
        let forge = update
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(2))
            .expect("NodeId(2) present");
        assert_eq!(forge.1.label(), Some("Forge"));
        // Its parent window carries NodeId(2) as a child.
        let window = update
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(1))
            .expect("NodeId(1) present");
        assert_eq!(window.1.children(), &[NodeId(2)]);
    }

    #[test]
    fn focus_points_at_focused_node() {
        use iced_winit::accesskit::NodeId;

        // Build a root with a focused child by hand (Collector focusable path
        // needs a live Focusable; here we assert to_accesskit's focus pick).
        let root = SemNode {
            r#ref: "@1".into(),
            role: Role::Window,
            name: "main".into(),
            value: None,
            bounds: Rect { x: 0.0, y: 0.0, width: 100.0, height: 20.0 },
            disabled: false,
            focused: false,
            children: vec![SemNode {
                r#ref: "@2".into(),
                role: Role::TextInput,
                name: "search".into(),
                value: Some("forge".into()),
                bounds: Rect { x: 0.0, y: 0.0, width: 50.0, height: 10.0 },
                disabled: false,
                focused: true,
                children: Vec::new(),
            }],
        };
        let update = to_accesskit(&WindowSnapshot::from_root(root));
        assert_eq!(update.focus, NodeId(2));
    }
}
