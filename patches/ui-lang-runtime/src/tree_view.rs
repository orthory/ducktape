//! Fixed-row, keyed tree virtualization built on the shared collection engine.

use crate::virtual_list::{VirtualCollectionItemSemantics, virtual_collection};
use crate::{
    Role, VirtualListConfig, VirtualListEvent, VirtualListId, VirtualListInspection,
    VirtualListNavigation, VirtualListState,
};
use iced::advanced::text;
use iced::keyboard;
use iced::widget::{container, scrollable};
use iced::{Element, Rectangle};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::fmt;
use std::fmt::Write as _;
use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

const TREE_SELECTOR_PREFIX: &str = "__ice/tree-view";

/// Explicit native, semantic, and inspection identity for one tree instance.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct TreeViewId(VirtualListId);

impl TreeViewId {
    pub fn new(logical: impl Into<String>) -> Self {
        Self(VirtualListId::new(logical))
    }

    pub fn logical(&self) -> &str {
        self.0.logical()
    }

    pub fn selector(&self) -> String {
        self.0.selector_with_prefix(TREE_SELECTOR_PREFIX, "tree")
    }
}

/// Validated fixed-row geometry for a tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeViewConfig {
    list: VirtualListConfig,
    indentation: f32,
}

impl TreeViewConfig {
    pub fn new(row_height: f32) -> Result<Self, crate::VirtualListConfigError> {
        VirtualListConfig::new(row_height).map(|list| Self {
            list,
            indentation: 16.0,
        })
    }

    #[must_use]
    pub const fn overscan(mut self, overscan: usize) -> Self {
        self.list = self.list.overscan(overscan);
        self
    }

    #[must_use]
    pub fn indentation(mut self, indentation: f32) -> Self {
        self.indentation = if indentation.is_finite() {
            indentation.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub const fn row_height(self) -> f32 {
        self.list.row_height()
    }

    pub const fn overscan_rows(self) -> usize {
        self.list.overscan_rows()
    }

    pub const fn indentation_width(self) -> f32 {
        self.indentation
    }
}

/// Structural facts for one item in caller-owned preorder data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewNode<Key> {
    pub key: Key,
    pub parent: Option<Key>,
    pub has_children: bool,
    pub children_loaded: bool,
}

impl<Key> TreeViewNode<Key> {
    pub fn leaf(key: Key, parent: Option<Key>) -> Self {
        Self {
            key,
            parent,
            has_children: false,
            children_loaded: true,
        }
    }

    pub fn branch(key: Key, parent: Option<Key>, children_loaded: bool) -> Self {
        Self {
            key,
            parent,
            has_children: true,
            children_loaded,
        }
    }
}

/// One visible row produced by the latest successful reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewRow<Key> {
    key: Key,
    source_index: usize,
    parent: Option<Key>,
    level: usize,
    position_in_set: usize,
    size_of_set: usize,
    has_children: bool,
    children_loaded: bool,
    expanded: bool,
    editing: bool,
}

impl<Key> TreeViewRow<Key> {
    pub const fn key(&self) -> &Key {
        &self.key
    }

    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    pub const fn parent(&self) -> Option<&Key> {
        self.parent.as_ref()
    }

    pub const fn level(&self) -> usize {
        self.level
    }

    pub const fn position_in_set(&self) -> usize {
        self.position_in_set
    }

    pub const fn size_of_set(&self) -> usize {
        self.size_of_set
    }

    pub const fn has_children(&self) -> bool {
        self.has_children
    }

    pub const fn children_loaded(&self) -> bool {
        self.children_loaded
    }

    pub const fn expanded(&self) -> bool {
        self.expanded
    }

    pub const fn editing(&self) -> bool {
        self.editing
    }
}

#[derive(Debug, Clone)]
struct TreeNodeRecord<Key> {
    key: Key,
    source_index: usize,
    parent_index: Option<usize>,
    level: usize,
    position_in_set: usize,
    size_of_set: usize,
    has_children: bool,
    children_loaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeEdit<Key> {
    key: Key,
    value: String,
}

/// Invalid caller-owned tree data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeViewReconcileError<Key> {
    DuplicateKey(Key),
    MissingParent { key: Key, parent: Key },
    ParentIsLeaf { key: Key, parent: Key },
    ParentOutOfOrder { key: Key, parent: Key },
}

impl<Key> fmt::Display for TreeViewReconcileError<Key>
where
    Key: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey(key) => write!(formatter, "duplicate tree key {key:?}"),
            Self::MissingParent { key, parent } => {
                write!(
                    formatter,
                    "tree key {key:?} references missing or later parent {parent:?}"
                )
            }
            Self::ParentIsLeaf { key, parent } => write!(
                formatter,
                "tree key {key:?} references parent {parent:?}, which is marked as a leaf"
            ),
            Self::ParentOutOfOrder { key, parent } => write!(
                formatter,
                "tree key {key:?} appears after parent {parent:?}'s preorder subtree closed"
            ),
        }
    }
}

impl<Key> std::error::Error for TreeViewReconcileError<Key> where Key: fmt::Debug {}

/// Keyboard movement supported by the tree contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeViewNavigation {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
}

/// Strongly typed interaction emitted by [`tree_view`].
#[derive(Debug, Clone, PartialEq)]
pub enum TreeViewEvent<Key> {
    ViewportChanged { height: f32 },
    Scrolled { offset_y: f32 },
    Select { index: usize, key: Key },
    Navigate(TreeViewNavigation),
    Toggle(Key),
    BeginRename { key: Key, value: String },
    RenameChanged(String),
    CommitRename,
    CancelRename,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewRename<Key> {
    pub key: Key,
    pub value: String,
}

/// Result of applying one tree interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewOutcome<Key> {
    pub selected: Option<Key>,
    pub selection_changed: bool,
    pub visible_range_changed: bool,
    pub scroll_changed: bool,
    pub expanded_changed: bool,
    pub load_requested: Option<Key>,
    pub rename_committed: Option<TreeViewRename<Key>>,
}

/// Position of a pointer-derived drop target relative to a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeViewDropPosition {
    Before,
    Inside,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewDropTarget<Key> {
    pub key: Key,
    pub position: TreeViewDropPosition,
}

/// Deterministic headless accounting for a tree render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewInspection<Key> {
    pub logical_nodes: usize,
    pub visible_nodes: usize,
    pub visible_range: Range<usize>,
    pub mounted_range: Range<usize>,
    pub mounted_rows: usize,
    pub expanded_nodes: usize,
    pub selected: Option<Key>,
    pub editing: Option<Key>,
}

/// Retained expansion, selection, editing, identity, and viewport state.
pub struct TreeViewState<Key> {
    list: VirtualListState<Key>,
    expanded: Arc<HashSet<Key>>,
    nodes: Arc<[TreeNodeRecord<Key>]>,
    node_indexes: Arc<HashMap<Key, usize>>,
    rows: Arc<[TreeViewRow<Key>]>,
    row_indexes: Arc<HashMap<Key, usize>>,
    editing: Option<TreeEdit<Key>>,
}

impl<Key> fmt::Debug for TreeViewState<Key>
where
    Key: fmt::Debug + Eq + Hash,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TreeViewState")
            .field("list", &self.list)
            .field("expanded", &self.expanded)
            .field("nodes", &self.nodes)
            .field("node_indexes", &self.node_indexes)
            .field("rows", &self.rows)
            .field("row_indexes", &self.row_indexes)
            .field("editing", &self.editing)
            .finish()
    }
}

impl<Key> TreeViewState<Key>
where
    Key: Clone + Eq + Hash,
{
    pub fn new(id: TreeViewId) -> Self {
        Self {
            list: VirtualListState::new(id.0),
            expanded: Arc::new(HashSet::default()),
            nodes: Arc::from([]),
            node_indexes: Arc::new(HashMap::default()),
            rows: Arc::from([]),
            row_indexes: Arc::new(HashMap::default()),
            editing: None,
        }
    }

    pub fn update_snapshot(&self) -> Self {
        Self {
            list: self.list.update_snapshot(),
            expanded: self.expanded.clone(),
            nodes: self.nodes.clone(),
            node_indexes: self.node_indexes.clone(),
            rows: self.rows.clone(),
            row_indexes: self.row_indexes.clone(),
            editing: self.editing.clone(),
        }
    }

    pub fn fork(&self, new_logical_name: impl Into<String>) -> Self {
        let mut fork = self.update_snapshot();
        fork.list = self.list.fork(new_logical_name);
        fork
    }

    pub fn logical(&self) -> &str {
        self.list.id().logical()
    }

    pub fn selector(&self) -> String {
        self.list
            .id()
            .selector_with_prefix(TREE_SELECTOR_PREFIX, "tree")
    }

    pub fn item_selector(&self, key: &Key) -> Option<String> {
        let local = self.list.semantic_local_id(key)?;
        let mut selector = self
            .list
            .id()
            .selector_with_prefix(TREE_SELECTOR_PREFIX, "item");
        write!(&mut selector, "/{local}").expect("writing to a String cannot fail");
        Some(selector)
    }

    pub fn selected(&self) -> Option<&Key> {
        self.list.selected()
    }

    pub fn expanded(&self, key: &Key) -> bool {
        self.expanded.contains(key)
    }

    pub fn rows(&self) -> &[TreeViewRow<Key>] {
        &self.rows
    }

    pub fn editing(&self) -> Option<(&Key, &str)> {
        self.editing
            .as_ref()
            .map(|editing| (&editing.key, editing.value.as_str()))
    }

    /// Returns an operation that restores keyboard focus to the tree.
    ///
    /// Callers should run this after removing a caller-rendered rename editor.
    pub fn focus_task<Message>(&self) -> iced::Task<Message> {
        self.list.focus_task()
    }

    pub fn visible_range(&self, config: TreeViewConfig) -> Range<usize> {
        self.list.visible_range(self.rows.len(), config.list)
    }

    pub fn mounted_range(&self, config: TreeViewConfig) -> Range<usize> {
        self.list.mounted_range(self.rows.len(), config.list)
    }

    pub fn reconcile<T>(
        &mut self,
        items: &[T],
        node: impl Fn(&T) -> TreeViewNode<Key>,
        config: TreeViewConfig,
    ) -> Result<(), TreeViewReconcileError<Key>> {
        let mut records: Vec<TreeNodeRecord<Key>> = Vec::with_capacity(items.len());
        let mut indexes: HashMap<Key, usize> =
            HashMap::with_capacity_and_hasher(items.len(), rustc_hash::FxBuildHasher);
        let mut root_count = 0;
        let mut open_path = Vec::<usize>::new();
        for (source_index, item) in items.iter().enumerate() {
            let node = node(item);
            if indexes.contains_key(&node.key) {
                return Err(TreeViewReconcileError::DuplicateKey(node.key));
            }
            let parent_index = node
                .parent
                .as_ref()
                .map(|parent| {
                    indexes.get(parent).copied().ok_or_else(|| {
                        TreeViewReconcileError::MissingParent {
                            key: node.key.clone(),
                            parent: parent.clone(),
                        }
                    })
                })
                .transpose()?;
            if let Some(parent_index) = parent_index {
                if !records[parent_index].has_children {
                    return Err(TreeViewReconcileError::ParentIsLeaf {
                        key: node.key,
                        parent: node.parent.expect("parent index requires a parent key"),
                    });
                }
                let parent_path_index = records[parent_index].level - 1;
                if open_path.get(parent_path_index) != Some(&parent_index) {
                    return Err(TreeViewReconcileError::ParentOutOfOrder {
                        key: node.key,
                        parent: node.parent.expect("parent index requires a parent key"),
                    });
                }
                open_path.truncate(parent_path_index + 1);
            } else {
                open_path.clear();
            }
            let level = parent_index.map_or(1, |parent| records[parent].level + 1);
            let position_in_set = if let Some(parent) = parent_index {
                // Until the final pass, a parent's empty `size_of_set` slot is
                // its direct-child count.
                records[parent].size_of_set += 1;
                records[parent].size_of_set
            } else {
                root_count += 1;
                root_count
            };
            indexes.insert(node.key.clone(), records.len());
            records.push(TreeNodeRecord {
                key: node.key,
                source_index,
                parent_index,
                level,
                position_in_set,
                size_of_set: 0,
                has_children: node.has_children,
                children_loaded: node.children_loaded,
            });
            open_path.push(records.len() - 1);
        }
        // Parents precede children, so reverse order reads every parent's child
        // count before replacing that slot with the parent's own sibling count.
        for index in (0..records.len()).rev() {
            records[index].size_of_set = records[index]
                .parent_index
                .map_or(root_count, |parent| records[parent].size_of_set);
        }
        Arc::make_mut(&mut self.expanded).retain(|key| {
            indexes
                .get(key)
                .is_some_and(|index| records[*index].has_children)
        });
        if self
            .editing
            .as_ref()
            .is_some_and(|editing| !indexes.contains_key(&editing.key))
        {
            self.editing = None;
        }
        if self
            .list
            .reconcile(&records, |record| record.key.clone(), config.list)
            .is_err()
        {
            unreachable!("validated logical tree keys remain unique");
        }
        self.node_indexes = Arc::new(indexes);
        self.nodes = records.into();
        self.rebuild_rows(config);
        Ok(())
    }

    pub fn apply(
        &mut self,
        event: TreeViewEvent<Key>,
        config: TreeViewConfig,
    ) -> TreeViewOutcome<Key> {
        let previous_selected = self.list.selected().cloned();
        let previous_range = self.visible_range(config);
        let previous_offset = self.list.scroll_offset();
        let mut expanded_changed = false;
        let mut load_requested = None;
        let mut rename_committed = None;

        match event {
            TreeViewEvent::ViewportChanged { height } => {
                self.apply_list(VirtualListEvent::ViewportChanged { height }, config);
            }
            TreeViewEvent::Scrolled { offset_y } => {
                self.apply_list(VirtualListEvent::Scrolled { offset_y }, config);
            }
            TreeViewEvent::Select { index, key } => {
                self.apply_list(VirtualListEvent::Select { index, key }, config);
            }
            TreeViewEvent::Navigate(navigation) => match navigation {
                TreeViewNavigation::Left => {
                    if let Some(selected) = self.list.selected().cloned()
                        && let Some(index) = self.node_index(&selected)
                    {
                        if self.nodes[index].has_children
                            && Arc::make_mut(&mut self.expanded).remove(&selected)
                        {
                            expanded_changed = true;
                            self.rebuild_rows(config);
                            self.select_visible_key(&selected, config);
                        } else if let Some(parent) = self.nodes[index]
                            .parent_index
                            .map(|parent| self.nodes[parent].key.clone())
                        {
                            self.select_visible_key(&parent, config);
                        }
                    }
                }
                TreeViewNavigation::Right => {
                    if let Some(selected) = self.list.selected().cloned()
                        && let Some(index) = self.node_index(&selected)
                        && self.nodes[index].has_children
                    {
                        if Arc::make_mut(&mut self.expanded).insert(selected.clone()) {
                            expanded_changed = true;
                            if !self.nodes[index].children_loaded {
                                load_requested = Some(selected.clone());
                            }
                            self.rebuild_rows(config);
                            self.select_visible_key(&selected, config);
                        } else if let Some(child) = self
                            .nodes
                            .get(index + 1)
                            .filter(|node| node.parent_index == Some(index))
                            .map(|node| node.key.clone())
                        {
                            self.select_visible_key(&child, config);
                        }
                    }
                }
                navigation => {
                    self.apply_list(
                        VirtualListEvent::Navigate(match navigation {
                            TreeViewNavigation::Up => VirtualListNavigation::Up,
                            TreeViewNavigation::Down => VirtualListNavigation::Down,
                            TreeViewNavigation::Home => VirtualListNavigation::Home,
                            TreeViewNavigation::End => VirtualListNavigation::End,
                            TreeViewNavigation::PageUp => VirtualListNavigation::PageUp,
                            TreeViewNavigation::PageDown => VirtualListNavigation::PageDown,
                            TreeViewNavigation::Left | TreeViewNavigation::Right => unreachable!(),
                        }),
                        config,
                    );
                }
            },
            TreeViewEvent::Toggle(key) => {
                if let Some(index) = self.node_index(&key)
                    && self.nodes[index].has_children
                {
                    if Arc::make_mut(&mut self.expanded).remove(&key) {
                        expanded_changed = true;
                    } else {
                        Arc::make_mut(&mut self.expanded).insert(key.clone());
                        expanded_changed = true;
                        if !self.nodes[index].children_loaded {
                            load_requested = Some(key.clone());
                        }
                    }
                    if expanded_changed {
                        self.rebuild_rows(config);
                        if self.list.selected().is_none() {
                            self.select_visible_key(&key, config);
                        }
                    }
                }
            }
            TreeViewEvent::BeginRename { key, value } => {
                if self.row_indexes.contains_key(&key) {
                    self.editing = Some(TreeEdit { key, value });
                    self.refresh_editing_flags();
                }
            }
            TreeViewEvent::RenameChanged(value) => {
                if let Some(editing) = &mut self.editing {
                    editing.value = value;
                }
            }
            TreeViewEvent::CommitRename => {
                rename_committed = self.editing.take().map(|editing| TreeViewRename {
                    key: editing.key,
                    value: editing.value,
                });
                self.refresh_editing_flags();
            }
            TreeViewEvent::CancelRename => {
                self.editing = None;
                self.refresh_editing_flags();
            }
        }

        TreeViewOutcome {
            selected: self.list.selected().cloned(),
            selection_changed: self.list.selected() != previous_selected.as_ref(),
            visible_range_changed: self.visible_range(config) != previous_range,
            scroll_changed: self.list.scroll_offset() != previous_offset,
            expanded_changed,
            load_requested,
            rename_committed,
        }
    }

    pub fn inspect(&self, config: TreeViewConfig) -> TreeViewInspection<Key> {
        let VirtualListInspection {
            visible_range,
            mounted_range,
            mounted_rows,
            ..
        } = self.list.inspect(self.rows.len(), config.list);
        TreeViewInspection {
            logical_nodes: self.nodes.len(),
            visible_nodes: self.rows.len(),
            visible_range,
            mounted_range,
            mounted_rows,
            expanded_nodes: self.expanded.len(),
            selected: self.list.selected().cloned(),
            editing: self.editing.as_ref().map(|editing| editing.key.clone()),
        }
    }

    pub fn drag_target(
        &self,
        pointer_y: f32,
        viewport: Rectangle,
        config: TreeViewConfig,
    ) -> Option<TreeViewDropTarget<Key>> {
        if !pointer_y.is_finite() || !viewport.height.is_finite() || viewport.height <= 0.0 {
            return None;
        }
        let local = pointer_y - viewport.y;
        if local < 0.0 || local > viewport.height {
            return None;
        }
        let content = local + self.list.scroll_offset();
        let index = (content / config.row_height()).floor() as usize;
        let row = self.rows.get(index)?;
        let fraction = (content % config.row_height()) / config.row_height();
        let position = if fraction < 0.25 {
            TreeViewDropPosition::Before
        } else if fraction > 0.75 || !row.has_children {
            TreeViewDropPosition::After
        } else {
            TreeViewDropPosition::Inside
        };
        Some(TreeViewDropTarget {
            key: row.key.clone(),
            position,
        })
    }

    fn node_index(&self, key: &Key) -> Option<usize> {
        self.node_indexes.get(key).copied()
    }

    fn rebuild_rows(&mut self, config: TreeViewConfig) {
        let mut visible = vec![false; self.nodes.len()];
        let mut rows = Vec::new();
        let mut row_indexes = HashMap::default();
        let editing = self.editing.as_ref().map(|editing| &editing.key);
        for (index, node) in self.nodes.iter().enumerate() {
            let shown = node.parent_index.is_none_or(|parent| {
                visible[parent] && self.expanded.contains(&self.nodes[parent].key)
            });
            visible[index] = shown;
            if shown {
                row_indexes.insert(node.key.clone(), rows.len());
                rows.push(TreeViewRow {
                    key: node.key.clone(),
                    source_index: node.source_index,
                    parent: node
                        .parent_index
                        .map(|parent| self.nodes[parent].key.clone()),
                    level: node.level,
                    position_in_set: node.position_in_set,
                    size_of_set: node.size_of_set,
                    has_children: node.has_children,
                    children_loaded: node.children_loaded,
                    expanded: node.has_children && self.expanded.contains(&node.key),
                    editing: editing == Some(&node.key),
                });
            }
        }
        self.rows = rows.into();
        self.row_indexes = Arc::new(row_indexes);
        if self
            .editing
            .as_ref()
            .is_some_and(|editing| !self.rows.iter().any(|row| row.key == editing.key))
        {
            self.editing = None;
        }
        if self
            .list
            .reconcile_retained_window(&self.rows, |row| row.key.clone(), config.list)
            .is_err()
        {
            unreachable!("visible tree rows retain logical keyed identity");
        }
    }

    fn refresh_editing_flags(&mut self) {
        let editing = self.editing.as_ref().map(|editing| &editing.key);
        for row in Arc::make_mut(&mut self.rows) {
            row.editing = editing == Some(&row.key);
        }
    }

    fn apply_list(&mut self, event: VirtualListEvent<Key>, config: TreeViewConfig) {
        self.list
            .apply(event, &self.rows, |row| row.key.clone(), config.list);
    }

    fn select_visible_key(&mut self, key: &Key, config: TreeViewConfig) {
        if let Some(index) = self.row_indexes.get(key).copied() {
            self.apply_list(
                VirtualListEvent::Select {
                    index,
                    key: key.clone(),
                },
                config,
            );
        }
    }
}

/// Builds a fixed-height virtual tree with true Tree/TreeItem semantics.
#[allow(clippy::too_many_arguments)]
pub fn tree_view<'a, T, Key, Message, Theme, Renderer>(
    state: &'a TreeViewState<Key>,
    items: &'a [T],
    config: TreeViewConfig,
    collection_label: impl Into<String>,
    label: impl Fn(&T) -> String + 'a,
    view: impl Fn(&'a TreeViewRow<Key>, &'a T, bool) -> Element<'a, Message, Theme, Renderer>,
    on_event: impl Fn(TreeViewEvent<Key>) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Key: Clone + Eq + Hash + 'static,
    Message: Clone + 'static,
    Theme: container::Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + iced::advanced::Renderer + 'a,
{
    let row_callbacks = Rc::new((label, on_event));
    let list_callbacks = Rc::clone(&row_callbacks);
    let key_callbacks = Rc::clone(&row_callbacks);
    virtual_collection(
        &state.list,
        &state.rows,
        config.list,
        collection_label,
        Role::Tree,
        state.selector(),
        |row| row.key.clone(),
        move |row| (row_callbacks.0)(&items[row.source_index]),
        move |_, row, selected| view(row, &items[row.source_index], selected),
        |_, row, local| {
            let mut selector = state
                .list
                .id()
                .selector_with_prefix(TREE_SELECTOR_PREFIX, "item");
            write!(&mut selector, "/{local}").expect("writing to a String cannot fail");
            VirtualCollectionItemSemantics {
                selector,
                role: Role::TreeItem,
                position_in_set: row.position_in_set,
                size_of_set: row.size_of_set,
                level: Some(row.level),
                expanded: row.has_children.then_some(row.expanded),
            }
        },
        move |event| {
            (list_callbacks.1)(match event {
                VirtualListEvent::ViewportChanged { height } => {
                    TreeViewEvent::ViewportChanged { height }
                }
                VirtualListEvent::Scrolled { offset_y } => TreeViewEvent::Scrolled { offset_y },
                VirtualListEvent::Select { index, key } => TreeViewEvent::Select { index, key },
                VirtualListEvent::Navigate(navigation) => {
                    TreeViewEvent::Navigate(match navigation {
                        VirtualListNavigation::Up => TreeViewNavigation::Up,
                        VirtualListNavigation::Down => TreeViewNavigation::Down,
                        VirtualListNavigation::Home => TreeViewNavigation::Home,
                        VirtualListNavigation::End => TreeViewNavigation::End,
                        VirtualListNavigation::PageUp => TreeViewNavigation::PageUp,
                        VirtualListNavigation::PageDown => TreeViewNavigation::PageDown,
                    })
                }
                VirtualListEvent::RowsMeasured { .. } => {
                    unreachable!("fixed-row collections never measure rows")
                }
            })
        },
        move |key| {
            let event = match key {
                keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                    Some(TreeViewEvent::Navigate(TreeViewNavigation::Left))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                    Some(TreeViewEvent::Navigate(TreeViewNavigation::Right))
                }
                _ => None,
            };
            event.map(|event| (key_callbacks.1)(event))
        },
        "tree-view",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ROOT_ID, SnapshotOperation};
    use iced::advanced::renderer;
    use iced::advanced::widget::operation::{self, Operation as _, Outcome};
    use iced::{Font, Pixels, Size, Theme};
    use iced_test::runtime::UserInterface;
    use iced_test::runtime::user_interface;

    #[derive(Debug)]
    struct Item {
        key: u64,
        parent: Option<u64>,
        branch: bool,
        loaded: bool,
    }

    #[derive(Debug, Clone)]
    struct Message;

    fn item(key: u64, parent: Option<u64>) -> Item {
        Item {
            key,
            parent,
            branch: false,
            loaded: true,
        }
    }

    fn branch(key: u64, parent: Option<u64>, loaded: bool) -> Item {
        Item {
            key,
            parent,
            branch: true,
            loaded,
        }
    }

    fn node(item: &Item) -> TreeViewNode<u64> {
        TreeViewNode {
            key: item.key,
            parent: item.parent,
            has_children: item.branch,
            children_loaded: item.loaded,
        }
    }

    fn config() -> TreeViewConfig {
        TreeViewConfig::new(20.0).unwrap().overscan(2)
    }

    fn fixture() -> Vec<Item> {
        vec![
            branch(1, None, true),
            item(2, Some(1)),
            branch(3, Some(1), true),
            item(4, Some(3)),
            item(5, None),
        ]
    }

    #[test]
    fn reconciliation_validates_preorder_and_is_atomic() {
        let mut state = TreeViewState::new(TreeViewId::new("tree"));
        let items = fixture();
        state.reconcile(&items, node, config()).unwrap();
        assert_eq!(
            state.rows.iter().map(|row| row.key).collect::<Vec<_>>(),
            vec![1, 5]
        );
        assert_eq!(
            state
                .nodes
                .iter()
                .map(|node| (node.key, node.position_in_set, node.size_of_set))
                .collect::<Vec<_>>(),
            vec![(1, 1, 2), (2, 1, 2), (3, 2, 2), (4, 1, 1), (5, 2, 2)]
        );

        let duplicate = vec![item(7, None), item(7, None)];
        assert_eq!(
            state.reconcile(&duplicate, node, config()),
            Err(TreeViewReconcileError::DuplicateKey(7))
        );
        assert_eq!(state.rows.len(), 2);

        let missing = vec![item(8, Some(9))];
        assert_eq!(
            state.reconcile(&missing, node, config()),
            Err(TreeViewReconcileError::MissingParent { key: 8, parent: 9 })
        );
        assert_eq!(state.rows.len(), 2);

        let leaf_parent = vec![item(20, None), item(21, Some(20))];
        assert_eq!(
            state.reconcile(&leaf_parent, node, config()),
            Err(TreeViewReconcileError::ParentIsLeaf {
                key: 21,
                parent: 20,
            })
        );
        assert_eq!(state.rows.len(), 2);

        let out_of_order = vec![
            branch(10, None, true),
            item(11, Some(10)),
            item(12, None),
            item(13, Some(10)),
        ];
        assert_eq!(
            state.reconcile(&out_of_order, node, config()),
            Err(TreeViewReconcileError::ParentOutOfOrder {
                key: 13,
                parent: 10,
            })
        );
        assert_eq!(state.rows.len(), 2);

        let deep = (0..1_000_u64)
            .map(|key| {
                if key == 999 {
                    item(key, key.checked_sub(1))
                } else {
                    branch(key, key.checked_sub(1), true)
                }
            })
            .collect::<Vec<_>>();
        state.reconcile(&deep, node, config()).unwrap();
        assert_eq!(state.nodes.last().map(|node| node.level), Some(1_000));
        assert_eq!(state.rows.len(), 1);
    }

    #[test]
    fn expansion_navigation_lazy_loading_and_selection_are_hierarchical() {
        let items = fixture();
        let mut state = TreeViewState::new(TreeViewId::new("tree"));
        state.reconcile(&items, node, config()).unwrap();
        state.apply(TreeViewEvent::Select { index: 0, key: 1 }, config());
        let outcome = state.apply(TreeViewEvent::Navigate(TreeViewNavigation::Right), config());
        assert!(outcome.expanded_changed);
        assert_eq!(outcome.load_requested, None);
        assert_eq!(
            state.rows.iter().map(|row| row.key).collect::<Vec<_>>(),
            vec![1, 2, 3, 5]
        );
        state.apply(TreeViewEvent::Navigate(TreeViewNavigation::Right), config());
        assert_eq!(state.selected(), Some(&2));
        state.apply(TreeViewEvent::Navigate(TreeViewNavigation::Left), config());
        assert_eq!(state.selected(), Some(&1));

        let lazy = vec![branch(10, None, false)];
        state.reconcile(&lazy, node, config()).unwrap();
        state.apply(TreeViewEvent::Select { index: 0, key: 10 }, config());
        let outcome = state.apply(TreeViewEvent::Navigate(TreeViewNavigation::Right), config());
        assert_eq!(outcome.load_requested, Some(10));
        assert!(state.expanded(&10));
    }

    #[test]
    fn collapsing_an_ancestor_rehomes_hidden_selection() {
        let items = fixture();
        let mut state = TreeViewState::new(TreeViewId::new("tree"));
        state.reconcile(&items, node, config()).unwrap();
        state.apply(TreeViewEvent::Toggle(1), config());
        state.apply(TreeViewEvent::Toggle(3), config());
        state.apply(TreeViewEvent::Select { index: 3, key: 4 }, config());
        state.apply(
            TreeViewEvent::BeginRename {
                key: 4,
                value: "Nested".into(),
            },
            config(),
        );
        assert_eq!(state.selected(), Some(&4));
        state.apply(TreeViewEvent::Toggle(1), config());
        assert_eq!(state.selected(), Some(&1));
        assert_eq!(state.editing(), None);
        assert_eq!(
            state.rows.iter().map(|row| row.key).collect::<Vec<_>>(),
            vec![1, 5]
        );
    }

    #[test]
    fn collapsing_and_reexpanding_retains_descendant_semantic_identity() {
        let items = fixture();
        let mut state = TreeViewState::new(TreeViewId::new("identity-tree"));
        state.reconcile(&items, node, config()).unwrap();
        state.apply(TreeViewEvent::Toggle(1), config());
        state.apply(TreeViewEvent::Toggle(3), config());
        let selector = state.item_selector(&4).expect("descendant selector");
        let local = state
            .list
            .semantic_local_id(&4)
            .expect("descendant identity");

        state.apply(TreeViewEvent::Toggle(1), config());
        assert_eq!(state.item_selector(&4).as_deref(), Some(selector.as_str()));
        assert_eq!(state.list.semantic_local_id(&4), Some(local));
        state.apply(TreeViewEvent::Toggle(1), config());
        assert_eq!(state.item_selector(&4).as_deref(), Some(selector.as_str()));
        assert_eq!(state.list.semantic_local_id(&4), Some(local));
    }

    #[test]
    fn rename_and_drag_target_contracts_are_deterministic() {
        let items = fixture();
        let mut state = TreeViewState::new(TreeViewId::new("tree"));
        state.reconcile(&items, node, config()).unwrap();
        state.apply(
            TreeViewEvent::BeginRename {
                key: 2,
                value: "Hidden child".into(),
            },
            config(),
        );
        assert_eq!(state.editing(), None);
        state.apply(
            TreeViewEvent::BeginRename {
                key: 1,
                value: "Root".into(),
            },
            config(),
        );
        state.apply(TreeViewEvent::RenameChanged("Renamed".into()), config());
        let outcome = state.apply(TreeViewEvent::CommitRename, config());
        assert_eq!(
            outcome.rename_committed,
            Some(TreeViewRename {
                key: 1,
                value: "Renamed".into()
            })
        );

        let viewport = Rectangle::new([0.0, 100.0].into(), [200.0, 40.0].into());
        assert_eq!(
            state.drag_target(102.0, viewport, config()),
            Some(TreeViewDropTarget {
                key: 1,
                position: TreeViewDropPosition::Before
            })
        );
        assert_eq!(
            state.drag_target(110.0, viewport, config()),
            Some(TreeViewDropTarget {
                key: 1,
                position: TreeViewDropPosition::Inside
            })
        );
        assert_eq!(
            state.drag_target(130.0, viewport, config()),
            Some(TreeViewDropTarget {
                key: 5,
                position: TreeViewDropPosition::After
            })
        );
    }

    #[test]
    fn inspection_and_selectors_expose_only_the_visible_virtual_window() {
        let mut items = Vec::with_capacity(100_000);
        for key in 0..100_000 {
            items.push(item(key, None));
        }
        let mut state = TreeViewState::new(TreeViewId::new("repo/tree"));
        state.reconcile(&items, node, config()).unwrap();
        state.apply(TreeViewEvent::ViewportChanged { height: 100.0 }, config());
        let inspection = state.inspect(config());
        assert_eq!(inspection.logical_nodes, 100_000);
        assert_eq!(inspection.visible_nodes, 100_000);
        assert_eq!(inspection.visible_range, 0..5);
        assert_eq!(inspection.mounted_range, 0..7);
        assert_eq!(inspection.mounted_rows, 7);
        assert_eq!(state.selector(), "__ice/tree-view/tree/repo%2Ftree");
        assert!(
            state
                .item_selector(&99_999)
                .unwrap()
                .starts_with("__ice/tree-view/item/repo%2Ftree/")
        );

        let snapshot = state.update_snapshot();
        assert!(Arc::ptr_eq(&state.nodes, &snapshot.nodes));
        assert!(Arc::ptr_eq(&state.rows, &snapshot.rows));
        assert!(Arc::ptr_eq(&state.expanded, &snapshot.expanded));
    }

    #[test]
    fn accesskit_exports_tree_hierarchy_and_expansion() {
        let items = fixture();
        let mut state = TreeViewState::new(TreeViewId::new("semantic-tree"));
        state.reconcile(&items, node, config()).unwrap();
        state.apply(TreeViewEvent::Toggle(1), config());
        state.apply(TreeViewEvent::ViewportChanged { height: 100.0 }, config());
        state.apply(TreeViewEvent::Select { index: 2, key: 3 }, config());

        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = tree_view(
            &state,
            &items,
            config(),
            "Repository tree",
            |item| format!("Item {}", item.key),
            |_, item, _| iced::widget::text(item.key).into(),
            |_| Message,
        );
        let mut renderer = iced_test::futures::futures::executor::block_on(
            <iced_test::renderer::Renderer as renderer::Headless>::new(
                Font::DEFAULT,
                Pixels(16.0),
                None,
            ),
        )
        .expect("headless renderer");
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut operation = SnapshotOperation::<Message>::named("Tree view test");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(snapshot) = operation.finish() else {
            panic!("snapshot operation did not finish");
        };
        let tree = snapshot
            .update
            .nodes
            .iter()
            .find(|(id, node)| *id != ROOT_ID && node.role() == Role::Tree)
            .map(|(_, node)| node)
            .expect("tree semantic node");
        assert_eq!(tree.label(), Some("Repository tree"));
        assert_eq!(tree.size_of_set(), Some(4));
        let rows = snapshot
            .update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::TreeItem)
            .map(|(_, node)| node)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 4);
        let root = rows
            .iter()
            .find(|node| node.label() == Some("Item 1"))
            .expect("root item");
        assert_eq!(root.level(), Some(1));
        assert_eq!(root.position_in_set(), Some(1));
        assert_eq!(root.size_of_set(), Some(2));
        assert_eq!(root.is_expanded(), Some(true));
        let selected_branch = rows
            .iter()
            .find(|node| node.label() == Some("Item 3"))
            .expect("selected branch");
        assert_eq!(selected_branch.level(), Some(2));
        assert_eq!(selected_branch.position_in_set(), Some(2));
        assert_eq!(selected_branch.size_of_set(), Some(2));
        assert_eq!(selected_branch.is_expanded(), Some(false));
        assert_eq!(selected_branch.is_selected(), Some(true));
    }
}
