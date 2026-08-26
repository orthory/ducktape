//! Fixed-height, keyed list virtualization.
//!
//! The caller owns the item data and [`VirtualListState`]. Only the current
//! visible range plus overscan is converted into Iced elements. This keeps
//! layout, diff, and draw work proportional to mounted rows instead of the
//! logical collection size.
//!
//! The widget owns its vertical scrolling. Mount it under a bounded-height,
//! non-vertically-scrolling parent. An arbitrary standard Iced vertical
//! `Scrollable` ancestor is outside the v1 interaction contract: Iced 0.14
//! keeps touch event positions in window coordinates while translating only
//! the cursor and replacement viewport, without exposing the lost ancestor
//! transform to descendants. Ordinary non-scrolling layout parents are
//! supported. Scrolling ancestors that translate or clip the list on either
//! hit-test axis require a future explicit scroll-context contract.

use crate::virtualization::{KeyedRows, MeasuredHeights, RowScroll, Rows};
use crate::{StableId, accessible};
use iced::advanced::text;
use iced::advanced::widget::operation::{self, Focusable};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::keyboard;
use iced::widget::{column, container, scrollable, space};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};
use rustc_hash::FxHashMap as HashMap;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::fmt::Write as _;
use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_VIRTUAL_LIST_NAMESPACE: AtomicU32 = AtomicU32::new(1);
const VERTICAL_SCROLLBAR_WIDTH: f32 = 10.0;
const SELECTOR_PREFIX: &str = "__ice/virtual-list";

/// Explicit identity for one retained virtual-list instance.
///
/// The logical name is exported to inspection tools. A process-unique
/// namespace prevents two lists with the same logical name from aliasing
/// native widget or accessibility identity.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct VirtualListId {
    logical: Arc<str>,
    namespace: u32,
}

impl VirtualListId {
    /// Creates a list identity with a caller-owned logical name.
    ///
    /// The logical name must be unique among concurrently mounted lists so
    /// headless and driver selectors resolve exactly one list. The runtime
    /// namespace still keeps native widget and accessibility identity safe if
    /// separate calls accidentally use the same logical name.
    pub fn new(logical: impl Into<String>) -> Self {
        let namespace = allocate_virtual_namespace();
        Self {
            logical: Arc::from(logical.into()),
            namespace,
        }
    }

    pub fn logical(&self) -> &str {
        &self.logical
    }

    pub(crate) const fn namespace(&self) -> u32 {
        self.namespace
    }

    /// Returns the canonical exact selector for this list.
    ///
    /// The logical name is escaped as one component below a runtime-reserved,
    /// type-tagged namespace. Callers should use this helper instead of
    /// reconstructing selector strings.
    pub fn selector(&self) -> String {
        self.selector_with_prefix(SELECTOR_PREFIX, "list")
    }

    pub(crate) fn widget_id(&self, suffix: &str) -> iced::advanced::widget::Id {
        format!("__ice_virtual_list/{}/{suffix}", self.namespace).into()
    }

    pub(crate) fn semantic_id(&self, local: u32) -> StableId {
        semantic_id(self.namespace, local)
    }

    fn item_selector(&self, local: u32) -> String {
        let mut selector = self.selector_with_prefix(SELECTOR_PREFIX, "item");
        write!(&mut selector, "/{local}").expect("writing to a String cannot fail");
        selector
    }

    pub(crate) fn selector_with_prefix(&self, prefix: &str, kind: &str) -> String {
        let mut selector = String::with_capacity(
            prefix.len() + kind.len() + self.logical.len().saturating_mul(3) + 12,
        );
        selector.push_str(prefix);
        selector.push('/');
        selector.push_str(kind);
        selector.push('/');
        push_escaped_selector_component(&mut selector, &self.logical);
        selector
    }
}

pub(crate) fn allocate_virtual_namespace() -> u32 {
    NEXT_VIRTUAL_LIST_NAMESPACE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("virtual collection identity namespace exhausted")
}

pub(crate) fn semantic_id(namespace: u32, local: u32) -> StableId {
    StableId::from_node_id(accesskit::NodeId(
        (u64::from(namespace) << 32) | u64::from(local),
    ))
}

fn push_escaped_selector_component(escaped: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            escaped.push(char::from(byte));
        } else {
            escaped.push('%');
            escaped.push(char::from(HEX[usize::from(byte >> 4)]));
            escaped.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
}

/// Validated row geometry for a virtual list: a fixed per-row height, or a
/// measured mode where the height is an estimate for not-yet-measured rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualListConfig {
    row_height: f32,
    overscan: usize,
    measured: bool,
}

impl VirtualListConfig {
    /// Creates fixed-row geometry.
    ///
    /// Row height must be finite and strictly positive. The viewport is always
    /// measured from native layout and is not caller-declared geometry.
    pub fn new(row_height: f32) -> Result<Self, VirtualListConfigError> {
        if !row_height.is_finite() || row_height <= 0.0 {
            return Err(VirtualListConfigError::RowHeight);
        }
        Ok(Self {
            row_height,
            overscan: 2,
            measured: false,
        })
    }

    /// Creates measured-row geometry for variable-height rows.
    ///
    /// Rows mount at their natural height; the list measures them as they
    /// render and reports [`VirtualListEvent::RowsMeasured`], which the
    /// reducer folds into per-row corrections. `estimate` sizes rows that
    /// have never been measured.
    pub fn measured(estimate: f32) -> Result<Self, VirtualListConfigError> {
        let mut config = Self::new(estimate)?;
        config.measured = true;
        Ok(config)
    }

    /// Whether rows mount at natural height and report measurements.
    pub const fn is_measured(self) -> bool {
        self.measured
    }

    /// Sets the number of extra rows mounted on each side of the viewport.
    #[must_use]
    pub const fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    pub const fn row_height(self) -> f32 {
        self.row_height
    }

    pub const fn overscan_rows(self) -> usize {
        self.overscan
    }

    fn rows_per_page(self, viewport_height: f32) -> usize {
        (viewport_height / self.row_height).floor().max(1.0) as usize
    }
}

/// Invalid [`VirtualListConfig`] geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualListConfigError {
    RowHeight,
}

impl fmt::Display for VirtualListConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RowHeight => "virtual-list row height must be finite and positive",
        })
    }
}

impl std::error::Error for VirtualListConfigError {}

/// Keyboard movement supported by the v1 list contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualListNavigation {
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

/// A strongly typed interaction emitted by [`virtual_list`].
#[derive(Debug, Clone, PartialEq)]
pub enum VirtualListEvent<Key> {
    ViewportChanged {
        height: f32,
    },
    Scrolled {
        offset_y: f32,
    },
    Select {
        index: usize,
        key: Key,
    },
    Navigate(VirtualListNavigation),
    /// Natural layout heights observed for mounted rows in measured mode.
    RowsMeasured {
        heights: Vec<(Key, f32)>,
    },
}

/// Result of applying a virtual-list interaction to caller-owned state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualListOutcome<Key> {
    pub selected: Option<Key>,
    pub selection_changed: bool,
    pub visible_range_changed: bool,
    pub scroll_changed: bool,
}

/// Deterministic headless accounting for one virtual-list render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualListInspection {
    pub logical_items: usize,
    pub visible_range: Range<usize>,
    pub mounted_range: Range<usize>,
    pub mounted_rows: usize,
    /// Exact scroll-content child slots: mounted rows plus top and bottom spacers.
    pub child_slots: usize,
}

/// Retained selection and viewport state for one virtual list.
pub struct VirtualListState<Key> {
    id: VirtualListId,
    selected: Option<Key>,
    selected_index: Option<usize>,
    scroll: RowScroll,
    measured: MeasuredHeights,
    measured_heights: HashMap<Key, f32>,
    keyed_rows: KeyedRows<Key>,
}

impl<Key> fmt::Debug for VirtualListState<Key>
where
    Key: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VirtualListState")
            .field("id", &self.id)
            .field("selected", &self.selected)
            .field("selected_index", &self.selected_index)
            .field("scroll_offset", &self.scroll.offset())
            .field("viewport_height", &self.scroll.viewport_height())
            .field("scroll_revision", &self.scroll.revision())
            .field("reconciled_items", &self.keyed_rows.len())
            .field("next_semantic_id", &self.keyed_rows.next_local_id())
            .finish()
    }
}

impl<Key> VirtualListState<Key>
where
    Key: Clone,
{
    /// Copies data for replacing the same retained mount during an app update.
    ///
    /// The old value must not remain mounted alongside this snapshot. Use
    /// [`Self::fork`] when both values can be mounted at once.
    pub fn update_snapshot(&self) -> Self {
        Self {
            id: VirtualListId {
                logical: Arc::clone(&self.id.logical),
                namespace: self.id.namespace,
            },
            selected: self.selected.clone(),
            selected_index: self.selected_index,
            scroll: self.scroll,
            measured: self.measured.clone(),
            measured_heights: self.measured_heights.clone(),
            keyed_rows: self.keyed_rows.snapshot(),
        }
    }

    /// Forks retained data into an independently mountable list identity.
    ///
    /// `new_logical_name` must be unique among concurrently mounted lists.
    /// Requiring it here prevents the fork from aliasing the original list in
    /// headless and driver selectors.
    pub fn fork(&self, new_logical_name: impl Into<String>) -> Self {
        let mut fork = self.update_snapshot();
        fork.id = VirtualListId::new(new_logical_name);
        fork
    }
}

/// A data-identity error found during explicit reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualListReconcileError<Key> {
    DuplicateKey(Key),
}

impl<Key> VirtualListState<Key>
where
    Key: Clone + Eq + Hash,
{
    pub fn new(id: VirtualListId) -> Self {
        Self {
            id,
            selected: None,
            selected_index: None,
            scroll: RowScroll::default(),
            measured: MeasuredHeights::default(),
            measured_heights: HashMap::default(),
            keyed_rows: KeyedRows::new(2),
        }
    }

    pub fn id(&self) -> &VirtualListId {
        &self.id
    }

    pub const fn selected(&self) -> Option<&Key> {
        self.selected.as_ref()
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub const fn scroll_offset(&self) -> f32 {
        self.scroll.offset()
    }

    pub const fn viewport_height(&self) -> f32 {
        self.scroll.viewport_height()
    }

    /// Row geometry for the caller's item count and configuration, combined
    /// with this list's measured corrections. Queries stay pure functions of
    /// the passed arguments, exactly as when geometry was closed-form
    /// fixed-height math.
    fn rows(&self, item_count: usize, config: VirtualListConfig) -> Rows {
        Rows::new(
            config.row_height(),
            config.overscan_rows(),
            item_count,
            &self.measured,
        )
    }

    /// Rebuilds the sparse correction table from recorded measurements in
    /// item order. A list that never measured anything skips all of it.
    fn rebuild_measured<T>(
        &mut self,
        items: &[T],
        key: &impl Fn(&T) -> Key,
        config: VirtualListConfig,
    ) {
        if self.measured_heights.is_empty() {
            return;
        }
        self.measured = MeasuredHeights::rebuild(
            config.row_height(),
            items.iter().enumerate().filter_map(|(index, item)| {
                self.measured_heights
                    .get(&key(item))
                    .map(|height| (index, *height))
            }),
        );
    }

    /// Returns the logical rows intersecting the viewport for the current offset.
    pub fn visible_range(&self, item_count: usize, config: VirtualListConfig) -> Range<usize> {
        self.scroll.visible_range(&self.rows(item_count, config))
    }

    /// Returns the exact range mounted by [`virtual_list`], including overscan.
    pub fn mounted_range(&self, item_count: usize, config: VirtualListConfig) -> Range<usize> {
        self.scroll.mounted_range(&self.rows(item_count, config))
    }

    pub fn inspect(&self, item_count: usize, config: VirtualListConfig) -> VirtualListInspection {
        let window = self.scroll.window(&self.rows(item_count, config));
        let visible_range = window.visible;
        let mounted_range = window.mounted;
        let mounted_rows = mounted_range.len();
        VirtualListInspection {
            logical_items: item_count,
            visible_range,
            mounted_range,
            mounted_rows,
            child_slots: mounted_rows.saturating_add(2),
        }
    }

    /// Reconciles retained identity after items are inserted, reordered, or removed.
    ///
    /// A retained key follows its item to the new index. Removing the selected
    /// key clears selection; it never transfers selection to an unrelated row.
    pub fn reconcile<T>(
        &mut self,
        items: &[T],
        key: impl Fn(&T) -> Key,
        config: VirtualListConfig,
    ) -> Result<(), VirtualListReconcileError<Key>> {
        self.selected_index = self
            .keyed_rows
            .reconcile(
                items,
                &key,
                self.selected.as_ref(),
                "virtual-list semantic identity exhausted",
            )
            .map_err(VirtualListReconcileError::DuplicateKey)?;
        if self.selected_index.is_none() {
            self.selected = None;
        }
        if !self.measured_heights.is_empty() {
            let keyed_rows = &self.keyed_rows;
            self.measured_heights
                .retain(|key, _| keyed_rows.local_id(key).is_some());
        }
        self.rebuild_measured(items, &key, config);
        let rows = self.rows(items.len(), config);
        self.scroll.reconcile(&rows);
        Ok(())
    }

    /// Reconciles a filtered window without replacing the complete keyed
    /// identity map. Product widgets use this when logical items are retained
    /// but temporarily hidden by hierarchy or filtering.
    pub(crate) fn reconcile_retained_window<T>(
        &mut self,
        items: &[T],
        key: impl Fn(&T) -> Key,
        config: VirtualListConfig,
    ) -> Result<(), Key> {
        let mut retained_index = None;
        for (index, item) in items.iter().enumerate() {
            let item_key = key(item);
            if self.keyed_rows.local_id(&item_key).is_none() {
                return Err(item_key);
            }
            if self.selected.as_ref() == Some(&item_key) {
                retained_index = Some(index);
            }
        }
        if self.selected.is_some() && retained_index.is_none() {
            self.selected = None;
        }
        self.selected_index = retained_index;
        self.rebuild_measured(items, &key, config);
        let rows = self.rows(items.len(), config);
        self.scroll.reconcile(&rows);
        if let Some(index) = retained_index {
            self.scroll.reveal(index, &rows);
        }
        Ok(())
    }

    /// Applies a measured viewport, pointer, scroll, or keyboard interaction.
    pub fn apply<T>(
        &mut self,
        event: VirtualListEvent<Key>,
        items: &[T],
        key: impl Fn(&T) -> Key,
        config: VirtualListConfig,
    ) -> VirtualListOutcome<Key> {
        let previous_selected = self.selected.clone();
        let previous_range = self.visible_range(items.len(), config);
        let previous_offset = self.scroll.offset();

        match event {
            VirtualListEvent::ViewportChanged { height } => {
                let rows = self.rows(items.len(), config);
                self.scroll.set_viewport_height(height, &rows);
            }
            VirtualListEvent::Scrolled { offset_y } => {
                let rows = self.rows(items.len(), config);
                self.scroll.set_native_offset(offset_y, &rows);
            }
            VirtualListEvent::Select {
                index,
                key: selected,
            } => {
                let resolved = items
                    .get(index)
                    .filter(|item| key(item) == selected)
                    .map(|_| index)
                    .or_else(|| items.iter().position(|item| key(item) == selected));
                if let Some(index) = resolved {
                    self.selected = Some(selected);
                    self.selected_index = Some(index);
                    let rows = self.rows(items.len(), config);
                    self.scroll.reveal(index, &rows);
                }
            }
            VirtualListEvent::RowsMeasured { heights } => {
                if config.is_measured() && !heights.is_empty() {
                    // Keep the first visible row visually anchored while
                    // corrections shift everything around it; a list stuck to
                    // its live edge stays stuck.
                    let rows = self.rows(items.len(), config);
                    let viewport = self.scroll.viewport_height();
                    let bottom_stuck = self.scroll.offset() >= rows.max_offset(viewport) - 0.5;
                    let anchor = self.scroll.visible_range(&rows).start;
                    let anchor_gap = self.scroll.offset() - rows.row_top(anchor);
                    self.measured_heights.extend(heights);
                    self.rebuild_measured(items, &key, config);
                    let rows = self.rows(items.len(), config);
                    if bottom_stuck {
                        self.scroll.scroll_to_end(&rows);
                    } else {
                        self.scroll
                            .restore_offset(rows.row_top(anchor) + anchor_gap, &rows);
                    }
                }
            }
            VirtualListEvent::Navigate(navigation) => {
                if let Some(index) = navigation_index(
                    self.selected_index,
                    items.len(),
                    navigation,
                    config.rows_per_page(self.scroll.viewport_height()),
                ) {
                    self.selected = items.get(index).map(&key);
                    self.selected_index = self.selected.as_ref().map(|_| index);
                    let rows = self.rows(items.len(), config);
                    self.scroll.reveal(index, &rows);
                }
            }
        }

        VirtualListOutcome {
            selected: self.selected.clone(),
            selection_changed: self.selected != previous_selected,
            visible_range_changed: self.visible_range(items.len(), config) != previous_range,
            scroll_changed: self.scroll.offset() != previous_offset,
        }
    }

    /// Scrolls an item into view without changing selection.
    pub fn scroll_to_item(
        &mut self,
        index: usize,
        item_count: usize,
        config: VirtualListConfig,
    ) -> bool {
        let rows = self.rows(item_count, config);
        self.scroll.scroll_to_item(index, &rows)
    }

    /// Scrolls to the exact live edge of the collection.
    ///
    /// Unlike revealing the last item, this reaches the maximum native offset
    /// even when the viewport is shorter than one fixed row.
    pub fn scroll_to_end(&mut self, item_count: usize, config: VirtualListConfig) -> bool {
        let rows = self.rows(item_count, config);
        self.scroll.scroll_to_end(&rows)
    }

    /// Scrolls a stable key into view and returns whether the offset changed.
    pub fn scroll_to_key(
        &mut self,
        selected: &Key,
        item_count: usize,
        config: VirtualListConfig,
    ) -> bool {
        let Some(index) = self.keyed_rows.index(selected) else {
            return false;
        };
        self.scroll_to_item(index, item_count, config)
    }

    fn widget_id(&self) -> iced::advanced::widget::Id {
        self.id.widget_id("focus")
    }

    pub(crate) fn focus_task<Message>(&self) -> iced::Task<Message> {
        iced::widget::operation::focus(self.widget_id())
    }

    fn scroll_id(&self) -> iced::advanced::widget::Id {
        self.id.widget_id("scroll")
    }

    pub(crate) fn semantic_id(&self, key: &Key) -> StableId {
        self.id.semantic_id(
            self.keyed_rows
                .local_id(key)
                .expect("virtual-list items must be reconciled before rendering"),
        )
    }

    pub(crate) fn semantic_local_id(&self, key: &Key) -> Option<u32> {
        self.keyed_rows.local_id(key)
    }

    pub(crate) fn index_of(&self, key: &Key) -> Option<usize> {
        self.keyed_rows.index(key)
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected = None;
        self.selected_index = None;
    }

    /// Returns the canonical exact selector for a reconciled item key.
    ///
    /// `None` means the key was not present in the latest successful
    /// reconciliation.
    pub fn item_selector(&self, key: &Key) -> Option<String> {
        self.keyed_rows
            .local_id(key)
            .map(|local| self.id.item_selector(local))
    }
}

fn navigation_index(
    selected: Option<usize>,
    item_count: usize,
    navigation: VirtualListNavigation,
    page: usize,
) -> Option<usize> {
    if item_count == 0 {
        return None;
    }
    let last = item_count - 1;
    Some(match navigation {
        VirtualListNavigation::Home => 0,
        VirtualListNavigation::End => last,
        VirtualListNavigation::Down => {
            selected.map_or(0, |index| index.saturating_add(1).min(last))
        }
        VirtualListNavigation::Up => selected.map_or(last, |index| index.saturating_sub(1)),
        VirtualListNavigation::PageDown => {
            selected.map_or(0, |index| index.saturating_add(page).min(last))
        }
        VirtualListNavigation::PageUp => selected.map_or(last, |index| index.saturating_sub(page)),
    })
}

/// Builds a fixed-height keyed virtual list.
///
/// `view` is called exactly once for each row in [`VirtualListState::mounted_range`].
/// It is never called for offscreen items outside overscan.
/// `collection_label` supplies the accessible name for the list.
/// `label` supplies the AccessKit name for each mounted item.
///
/// The parent must provide a bounded height and must not scroll the list
/// vertically. This widget's pointer and touch guarantees cover its owned
/// native scrollable and viewport, not an arbitrary scrolling ancestor.
#[allow(clippy::too_many_arguments)]
pub fn virtual_list<'a, T, Key, Message, Theme, Renderer>(
    state: &VirtualListState<Key>,
    items: &'a [T],
    config: VirtualListConfig,
    collection_label: impl Into<String>,
    key: impl Fn(&T) -> Key,
    label: impl Fn(&T) -> String,
    view: impl Fn(usize, &'a T, bool) -> Element<'a, Message, Theme, Renderer>,
    on_event: impl Fn(VirtualListEvent<Key>) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Key: Clone + Eq + Hash + 'static,
    Message: Clone + 'static,
    Theme: container::Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + iced::advanced::Renderer + 'a,
{
    virtual_collection(
        state,
        items,
        config,
        collection_label,
        crate::Role::List,
        state.id.selector(),
        key,
        label,
        view,
        |index, _, local| VirtualCollectionItemSemantics {
            selector: state.id.item_selector(local),
            role: crate::Role::ListItem,
            position_in_set: index.saturating_add(1),
            size_of_set: items.len(),
            level: None,
            expanded: None,
        },
        on_event,
        |_| None,
        "virtual-list",
    )
}

pub(crate) struct VirtualCollectionItemSemantics {
    pub(crate) selector: String,
    pub(crate) role: crate::Role,
    pub(crate) position_in_set: usize,
    pub(crate) size_of_set: usize,
    pub(crate) level: Option<usize>,
    pub(crate) expanded: Option<bool>,
}

type ExtraKeyHandler<'a, Message> = Rc<dyn Fn(&keyboard::Key) -> Option<Message> + 'a>;

#[allow(clippy::too_many_arguments)]
pub(crate) fn virtual_collection<'a, T, Key, Message, Theme, Renderer>(
    state: &VirtualListState<Key>,
    items: &'a [T],
    config: VirtualListConfig,
    collection_label: impl Into<String>,
    collection_role: crate::Role,
    collection_selector: impl Into<String>,
    key: impl Fn(&T) -> Key,
    label: impl Fn(&T) -> String,
    view: impl Fn(usize, &'a T, bool) -> Element<'a, Message, Theme, Renderer>,
    item_semantics: impl Fn(usize, &T, u32) -> VirtualCollectionItemSemantics,
    on_event: impl Fn(VirtualListEvent<Key>) -> Message + 'a,
    on_key: impl Fn(&keyboard::Key) -> Option<Message> + 'a,
    draw_probe: &'static str,
) -> Element<'a, Message, Theme, Renderer>
where
    Key: Clone + Eq + Hash + 'static,
    Message: Clone + 'static,
    Theme: container::Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + iced::advanced::Renderer + 'a,
{
    let rows = state.rows(items.len(), config);
    let window = state.scroll.window(&rows);
    let scroll_offset = window.offset;
    let range = window.mounted.clone();
    let top = window.top_spacer;
    let bottom = window.bottom_spacer;
    let mut mounted_keys = Vec::with_capacity(range.len());
    let mut mounted_children = Vec::with_capacity(range.len());
    let mut mounted = Vec::with_capacity(range.len());
    for index in range.clone() {
        let item = &items[index];
        let item_key = key(item);
        let selected = state.selected.as_ref() == Some(&item_key);
        let row = container(view(index, item, selected))
            .width(Length::Fill)
            .height(if config.is_measured() {
                Length::Shrink
            } else {
                Length::Fixed(config.row_height())
            });
        let semantic_key = state
            .keyed_rows
            .local_id(&item_key)
            .expect("virtual-list items must be reconciled before rendering");
        let semantics = item_semantics(index, item, semantic_key);
        let mut accessible_row = accessible(row, state.semantic_id(&item_key), semantics.role)
            .logical_id(semantics.selector)
            .label(label(item))
            .position_in_set(semantics.position_in_set)
            .size_of_set(semantics.size_of_set)
            .selected(selected);
        if let Some(level) = semantics.level {
            accessible_row = accessible_row.level(level);
        }
        if let Some(expanded) = semantics.expanded {
            accessible_row = accessible_row.expanded(expanded);
        }
        let row: Element<'a, Message, Theme, Renderer> = accessible_row.into();
        mounted_keys.push(semantic_key);
        mounted_children.push(row);
        mounted.push((index, item_key));
    }

    let on_event: Rc<dyn Fn(VirtualListEvent<Key>) -> Message + 'a> = Rc::new(on_event);
    let on_key: ExtraKeyHandler<'a, Message> = Rc::new(on_key);
    let on_scroll = Rc::clone(&on_event);
    let touch_claim = Rc::new(Cell::new(TouchClaim::None));
    let native_scroll_offset = Rc::new(Cell::new(scroll_offset));
    let scrolled_offset = Rc::clone(&native_scroll_offset);
    let realized_heights = Rc::new(RefCell::new(Vec::new()));
    let mounted_rows: Element<'a, Message, Theme, Renderer> = Element::new(MountedRows {
        keys: mounted_keys,
        children: mounted_children,
        touch_claim: Rc::clone(&touch_claim),
        scroll_offset: Rc::clone(&native_scroll_offset),
        realized_heights: Rc::clone(&realized_heights),
        draw_probe,
    });
    let content = scrollable(
        column![
            space()
                .height(if top == 0.0 {
                    Length::Shrink
                } else {
                    Length::Fixed(top)
                })
                .width(Length::Fill),
            mounted_rows,
            space()
                .height(if bottom == 0.0 {
                    Length::Shrink
                } else {
                    Length::Fixed(bottom)
                })
                .width(Length::Fill),
        ]
        .width(Length::Fill),
    )
    .id(state.scroll_id())
    .direction(scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(VERTICAL_SCROLLBAR_WIDTH)
            .scroller_width(VERTICAL_SCROLLBAR_WIDTH),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(move |viewport| {
        scrolled_offset.set(viewport.absolute_offset().y);
        on_scroll(VirtualListEvent::Scrolled {
            offset_y: viewport.absolute_offset().y,
        })
    });
    let list = VirtualList {
        content: content.into(),
        id: state.widget_id(),
        scroll_id: state.scroll_id(),
        namespace: state.id.namespace,
        mounted,
        config,
        rows,
        realized_heights,
        total_height: window.total_height,
        scroll_offset,
        viewport_height: state.scroll.viewport_height(),
        scroll_revision: state.scroll.revision(),
        native_scroll_offset,
        touch_claim,
        on_event,
        on_key,
    };
    let active_descendant = state.selected.as_ref().and_then(|selected| {
        state
            .selected_index
            .filter(|index| range.contains(index))
            .map(|_| state.semantic_id(selected))
    });
    accessible(Element::new(list), state.id.semantic_id(1), collection_role)
        .logical_id(collection_selector)
        .label(collection_label)
        .focus_descendant()
        .active_descendant_maybe(active_descendant)
        .size_of_set(items.len())
        .into()
}

struct VirtualList<'a, Key, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    id: iced::advanced::widget::Id,
    scroll_id: iced::advanced::widget::Id,
    namespace: u32,
    mounted: Vec<(usize, Key)>,
    config: VirtualListConfig,
    rows: Rows,
    realized_heights: Rc<RefCell<Vec<f32>>>,
    total_height: f32,
    scroll_offset: f32,
    viewport_height: f32,
    scroll_revision: u64,
    native_scroll_offset: Rc<Cell<f32>>,
    touch_claim: Rc<Cell<TouchClaim>>,
    on_event: Rc<dyn Fn(VirtualListEvent<Key>) -> Message + 'a>,
    on_key: ExtraKeyHandler<'a, Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TouchClaim {
    None,
    Row,
    Child,
    Scrollbar,
}

/// Transparent keyed-row boundary that observes actual descendant capture.
///
/// Iced's scrollable captures every plain touch press to prepare native touch
/// scrolling. This boundary distinguishes row and descendant ownership in
/// translated content coordinates; the outer list separately reserves the
/// native scrollbar rail from the touch event position.
struct MountedRows<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    keys: Vec<u32>,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    touch_claim: Rc<Cell<TouchClaim>>,
    scroll_offset: Rc<Cell<f32>>,
    realized_heights: Rc<RefCell<Vec<f32>>>,
    draw_probe: &'static str,
}

struct MountedRowsState {
    keys: Vec<u32>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MountedRows<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MountedRowsState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(MountedRowsState {
            keys: self.keys.clone(),
        })
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.state.downcast_ref::<MountedRowsState>().keys == self.keys {
            tree.diff_children(&self.children);
            return;
        }

        let state = tree.state.downcast_mut::<MountedRowsState>();
        let previous_children = std::mem::take(&mut tree.children);
        let mut retained = std::mem::take(&mut state.keys)
            .into_iter()
            .zip(previous_children)
            .collect::<HashMap<_, _>>();
        tree.children = self
            .keys
            .iter()
            .zip(&self.children)
            .map(|(key, child)| {
                retained
                    .remove(key)
                    .unwrap_or_else(|| Tree::new(child.as_widget()))
            })
            .collect();
        state.keys.clone_from(&self.keys);
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn size_hint(&self) -> Size<Length> {
        self.size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(Length::Fill).height(Length::Shrink);
        let node = layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            &limits,
            Length::Fill,
            Length::Shrink,
            iced::Padding::ZERO,
            0.0,
            iced::Alignment::Start,
            &mut self.children,
            &mut tree.children,
        );
        let mut realized = self.realized_heights.borrow_mut();
        realized.clear();
        realized.extend(node.children().iter().map(|child| child.size().height));
        node
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            for ((child, tree), layout) in self
                .children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                child
                    .as_widget_mut()
                    .operate(tree, layout, renderer, operation);
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let observes_touch = match event {
            Event::Touch(
                iced::touch::Event::FingerPressed { position, .. }
                | iced::touch::Event::FingerLifted { position, .. },
            ) => {
                let content_position =
                    Point::new(position.x, position.y + self.scroll_offset.get());
                layout.bounds().contains(content_position) && viewport.contains(content_position)
            }
            _ => false,
        };
        let captured_before = shell.is_event_captured();
        for ((child, tree), layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
        if observes_touch {
            let captured_after = shell.is_event_captured();
            self.touch_claim.set(if !captured_before && captured_after {
                TouchClaim::Child
            } else {
                TouchClaim::Row
            });
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((child, tree), layout) in self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
        {
            child
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        if layout.bounds().height > 0.0
            && let Some(bounds) = layout.bounds().intersection(viewport)
        {
            // Keep readiness tied to an actual renderer primitive in the
            // mounted-row subtree, never merely the outer list wrapper.
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    ..renderer::Quad::default()
                },
                iced::Color::TRANSPARENT,
            );
            crate::dev::record_draw_probe(self.draw_probe);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

struct State {
    namespace: u32,
    focused: bool,
    focus_visible: bool,
    measured_viewport_height: f32,
    reported_viewport_height: Option<f32>,
    reported_row_heights: Vec<(usize, f32)>,
    applied_scroll_revision: Option<u64>,
    touch: Option<TouchGesture>,
}

impl State {
    fn new(namespace: u32) -> Self {
        Self {
            namespace,
            focused: false,
            focus_visible: false,
            measured_viewport_height: 0.0,
            reported_viewport_height: None,
            reported_row_heights: Vec::new(),
            applied_scroll_revision: None,
            touch: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TouchGesture {
    finger: iced::touch::Finger,
    origin: Point,
}

impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }

    fn focus(&mut self) {
        self.focused = true;
        self.focus_visible = true;
    }

    fn unfocus(&mut self) {
        self.focused = false;
        self.focus_visible = false;
    }
}

impl<Key, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VirtualList<'_, Key, Message, Theme, Renderer>
where
    Key: Clone + 'static,
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.namespace))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.state.downcast_ref::<State>().namespace != self.namespace {
            *tree = Tree::new(self as &dyn Widget<Message, Theme, Renderer>);
            return;
        }
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        tree.state.downcast_mut::<State>().measured_viewport_height = node.size().height;

        if tree.state.downcast_ref::<State>().applied_scroll_revision != Some(self.scroll_revision)
        {
            let mut scroll = operation::scrollable::scroll_to::<()>(
                self.scroll_id.clone(),
                operation::scrollable::AbsoluteOffset {
                    x: None,
                    y: Some(self.scroll_offset),
                },
            );
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
                &mut scroll,
            );
            self.native_scroll_offset.set(self.scroll_offset);
            tree.state.downcast_mut::<State>().applied_scroll_revision = Some(self.scroll_revision);
        }
        node
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.focusable(
            Some(&self.id),
            layout.bounds(),
            tree.state.downcast_mut::<State>(),
        );
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let mut touch_tap = None;
        if matches!(
            event,
            Event::Touch(
                iced::touch::Event::FingerPressed { .. } | iced::touch::Event::FingerLifted { .. }
            )
        ) {
            self.touch_claim.set(TouchClaim::None);
        }
        match event {
            Event::Touch(iced::touch::Event::FingerMoved { id, position }) => {
                if state.touch.is_some_and(|gesture| {
                    gesture.finger == *id && gesture.origin.distance(*position) > 8.0
                }) {
                    state.touch = None;
                }
            }
            Event::Touch(iced::touch::Event::FingerLifted { id, position }) => {
                if state.touch.take().is_some_and(|gesture| {
                    gesture.finger == *id
                        && gesture.origin.distance(*position) <= 8.0
                        && layout.bounds().contains(*position)
                }) {
                    touch_tap = Some(*position);
                }
            }
            Event::Touch(iced::touch::Event::FingerLost { id, .. })
                if state.touch.is_some_and(|gesture| gesture.finger == *id) =>
            {
                state.touch = None;
            }
            _ => {}
        }
        if state.reported_viewport_height != Some(state.measured_viewport_height) {
            state.reported_viewport_height = Some(state.measured_viewport_height);
            if state.measured_viewport_height != self.viewport_height {
                shell.publish((self.on_event)(VirtualListEvent::ViewportChanged {
                    height: state.measured_viewport_height,
                }));
            }
        }

        if self.config.is_measured() {
            let realized = self.realized_heights.borrow();
            let changed = !state.reported_row_heights.iter().copied().eq(self
                .mounted
                .iter()
                .zip(realized.iter())
                .map(|((index, _), height)| (*index, *height)));
            if changed {
                let heights: Vec<(Key, f32)> = self
                    .mounted
                    .iter()
                    .zip(realized.iter())
                    .filter(|((index, _), height)| {
                        (self.rows.row_height(*index) - **height).abs() > 0.01
                    })
                    .map(|((_, key), height)| (key.clone(), *height))
                    .collect();
                state.reported_row_heights.clear();
                state.reported_row_heights.extend(
                    self.mounted
                        .iter()
                        .zip(realized.iter())
                        .map(|((index, _), height)| (*index, *height)),
                );
                if !heights.is_empty() {
                    shell.publish((self.on_event)(VirtualListEvent::RowsMeasured { heights }));
                }
            }
        }

        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if let Event::Touch(
            iced::touch::Event::FingerPressed { position, .. }
            | iced::touch::Event::FingerLifted { position, .. },
        ) = event
            && self.touch_over_vertical_scrollbar(
                *position,
                layout.bounds(),
                state.measured_viewport_height,
            )
        {
            self.touch_claim.set(TouchClaim::Scrollbar);
        }

        if let Event::Touch(iced::touch::Event::FingerPressed { id, position }) = event {
            state.touch = (self.touch_claim.get() == TouchClaim::Row
                && layout.bounds().contains(*position))
            .then_some(TouchGesture {
                finger: *id,
                origin: *position,
            });
            if self.touch_claim.get() == TouchClaim::Child {
                state.focused = false;
                state.focus_visible = false;
            }
        }

        if let Some(position) = touch_tap {
            if self.touch_claim.get() == TouchClaim::Row && !shell.is_event_captured() {
                state.focused = true;
                state.focus_visible = false;
                self.publish_selection(position, layout.bounds(), shell);
                shell.capture_event();
            }
            return;
        }

        if shell.is_event_captured() {
            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                    | Event::Touch(iced::touch::Event::FingerPressed { .. })
            ) && state.touch.is_none()
            {
                state.focused = false;
                state.focus_visible = false;
            }
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor
                    .position()
                    .filter(|position| layout.bounds().contains(*position))
                {
                    state.focused = true;
                    state.focus_visible = false;
                    self.publish_selection(position, layout.bounds(), shell);
                    shell.capture_event();
                    return;
                }
                state.focused = false;
                state.focus_visible = false;
            }
            Event::Touch(iced::touch::Event::FingerPressed { id: _, position }) => {
                if !layout.bounds().contains(*position) {
                    state.focused = false;
                    state.focus_visible = false;
                    state.touch = None;
                }
            }
            Event::Touch(iced::touch::Event::FingerLifted { .. }) => {
                // Touch taps are resolved after descendant and scrollbar
                // ownership is observed above.
            }
            _ => {}
        }

        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event
            && state.focused
        {
            let navigation = match key {
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    Some(VirtualListNavigation::Up)
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    Some(VirtualListNavigation::Down)
                }
                keyboard::Key::Named(keyboard::key::Named::Home) => {
                    Some(VirtualListNavigation::Home)
                }
                keyboard::Key::Named(keyboard::key::Named::End) => Some(VirtualListNavigation::End),
                keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                    Some(VirtualListNavigation::PageUp)
                }
                keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                    Some(VirtualListNavigation::PageDown)
                }
                _ => None,
            };
            if let Some(navigation) = navigation {
                shell.publish((self.on_event)(VirtualListEvent::Navigate(navigation)));
                shell.capture_event();
            } else if let Some(message) = (self.on_key)(key) {
                shell.publish(message);
                shell.capture_event();
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        if tree.state.downcast_ref::<State>().focus_visible {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: iced::Border {
                        color: style.text_color,
                        width: 2.0,
                        radius: 3.0.into(),
                    },
                    ..renderer::Quad::default()
                },
                iced::Color::TRANSPARENT,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let child = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        );
        if child != mouse::Interaction::None {
            child
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<Key, Message, Theme, Renderer> VirtualList<'_, Key, Message, Theme, Renderer>
where
    Key: Clone + 'static,
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn touch_over_vertical_scrollbar(
        &self,
        position: Point,
        bounds: Rectangle,
        viewport_height: f32,
    ) -> bool {
        if self.total_height <= viewport_height || !bounds.contains(position) {
            return false;
        }

        let width = VERTICAL_SCROLLBAR_WIDTH.min(bounds.width.max(0.0));
        position.x >= bounds.x + bounds.width - width
    }

    fn publish_selection(
        &self,
        position: Point,
        bounds: Rectangle,
        shell: &mut Shell<'_, Message>,
    ) {
        let local_y = position.y - bounds.y;
        let Some(index) = self
            .rows
            .index_at(self.native_scroll_offset.get() + local_y)
        else {
            return;
        };
        if let Some((index, key)) = self.mounted.iter().find(|(mounted, _)| *mounted == index) {
            shell.publish((self.on_event)(VirtualListEvent::Select {
                index: *index,
                key: key.clone(),
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Config as DriverConfig, Driver, Location};
    use crate::{ROOT_ID, SnapshotOperation};
    use accesskit::{NodeId, TreeId};
    use iced::advanced::widget::Tree as WidgetTree;
    use iced::advanced::widget::operation::{self, Outcome};
    use iced::advanced::{Layout, renderer};
    use iced::{Font, Pixels, Point, Theme};
    use iced_test::futures::futures::StreamExt as _;
    use iced_test::runtime::UserInterface;
    use iced_test::runtime::user_interface;
    use std::cell::Cell;
    use std::fmt;
    use std::rc::Rc;

    #[derive(Debug, Clone, PartialEq)]
    enum Message {
        List(VirtualListEvent<u64>),
        First(VirtualListEvent<u64>),
        Second(VirtualListEvent<u64>),
        Child,
        Input(String),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct DisplayCollision(u64);

    impl Hash for DisplayCollision {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            0_u8.hash(state);
        }
    }

    impl fmt::Display for DisplayCollision {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("same-display")
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct CollisionMessage;

    fn config() -> VirtualListConfig {
        VirtualListConfig::new(20.0).unwrap().overscan(2)
    }

    fn prepared_state<Key>(logical: &str) -> VirtualListState<Key>
    where
        Key: Clone + Eq + Hash,
    {
        let mut state = VirtualListState::new(VirtualListId::new(logical));
        state.apply::<Key>(
            VirtualListEvent::ViewportChanged { height: 100.0 },
            &[],
            Clone::clone,
            config(),
        );
        state
    }

    fn renderer() -> iced_test::renderer::Renderer {
        iced_test::futures::futures::executor::block_on(
            <iced_test::renderer::Renderer as renderer::Headless>::new(
                Font::DEFAULT,
                Pixels(16.0),
                None,
            ),
        )
        .expect("headless renderer")
    }

    fn key_pressed(named: keyboard::key::Named) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(named),
            modified_key: keyboard::Key::Named(named),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
            text: None,
            repeat: false,
        })
    }

    fn redraw() -> Event {
        Event::Window(iced::window::Event::RedrawRequested(
            iced::time::Instant::now(),
        ))
    }

    struct ScrollProbe {
        target: iced::advanced::widget::Id,
        offset_y: Option<f32>,
    }

    struct RowIdentity(u64);

    struct RetainedRowIdentity {
        key: u64,
        creations: Rc<Cell<usize>>,
    }

    struct CaptureWithoutCursor;

    struct CursorOnly;

    #[derive(Debug)]
    struct ForkMountState {
        original: VirtualListState<u64>,
        fork: VirtualListState<u64>,
        items: Vec<u64>,
    }

    #[derive(Debug, Clone)]
    enum ForkMountMessage {
        Original(VirtualListEvent<u64>),
        Fork(VirtualListEvent<u64>),
    }

    fn fork_mount_boot() -> ForkMountState {
        let items = vec![10, 20];
        let mut original = VirtualListState::new(VirtualListId::new("driver/original"));
        original.reconcile(&items, |item| *item, config()).unwrap();
        original.apply(
            VirtualListEvent::ViewportChanged { height: 80.0 },
            &items,
            |item| *item,
            config(),
        );
        let fork = original.fork("driver/original/item/2");
        ForkMountState {
            original,
            fork,
            items,
        }
    }

    fn fork_mount_update(
        state: &mut ForkMountState,
        message: ForkMountMessage,
    ) -> iced::Task<ForkMountMessage> {
        let (list, event) = match message {
            ForkMountMessage::Original(event) => (&mut state.original, event),
            ForkMountMessage::Fork(event) => (&mut state.fork, event),
        };
        list.apply(event, &state.items, |item| *item, config());
        iced::Task::none()
    }

    fn fork_mount_view(state: &ForkMountState) -> Element<'_, ForkMountMessage> {
        let original = virtual_list(
            &state.original,
            &state.items,
            config(),
            "Original",
            |item| *item,
            |item| format!("Original {item}"),
            |_, item, _| iced::widget::text(*item).into(),
            ForkMountMessage::Original,
        );
        let fork = virtual_list(
            &state.fork,
            &state.items,
            config(),
            "Fork",
            |item| *item,
            |item| format!("Fork {item}"),
            |_, item, _| iced::widget::text(*item).into(),
            ForkMountMessage::Fork,
        );
        iced::widget::column![
            container(original).height(Length::Fixed(80.0)),
            container(fork).height(Length::Fixed(80.0)),
        ]
        .into()
    }

    impl Widget<Message, Theme, iced_test::renderer::Renderer> for RowIdentity {
        fn tag(&self) -> tree::Tag {
            tree::Tag::of::<u64>()
        }

        fn state(&self) -> tree::State {
            tree::State::new(self.0)
        }

        fn diff(&self, tree: &mut Tree) {
            assert_eq!(
                *tree.state.downcast_ref::<u64>(),
                self.0,
                "row widget state transferred to a different stable key"
            );
        }

        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fill)
        }

        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &iced_test::renderer::Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(limits.max())
        }

        fn draw(
            &self,
            _tree: &Tree,
            _renderer: &mut iced_test::renderer::Renderer,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }
    }

    impl Widget<Message, Theme, iced_test::renderer::Renderer> for RetainedRowIdentity {
        fn tag(&self) -> tree::Tag {
            tree::Tag::of::<u64>()
        }

        fn state(&self) -> tree::State {
            self.creations.set(self.creations.get() + 1);
            tree::State::new(self.key)
        }

        fn diff(&self, tree: &mut Tree) {
            assert_eq!(
                *tree.state.downcast_ref::<u64>(),
                self.key,
                "retained row tree moved to a different stable key"
            );
        }

        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fill)
        }

        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &iced_test::renderer::Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(limits.max())
        }

        fn draw(
            &self,
            _tree: &Tree,
            _renderer: &mut iced_test::renderer::Renderer,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }
    }

    impl CaptureWithoutCursor {
        fn captures(event: &Event) -> bool {
            matches!(
                event,
                Event::Touch(iced::touch::Event::FingerPressed { .. })
            )
        }
    }

    impl Widget<Message, Theme, iced_test::renderer::Renderer> for CaptureWithoutCursor {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fill)
        }

        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &iced_test::renderer::Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(limits.max())
        }

        fn update(
            &mut self,
            _tree: &mut Tree,
            event: &Event,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _renderer: &iced_test::renderer::Renderer,
            _clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            _viewport: &Rectangle,
        ) {
            if Self::captures(event) {
                shell.publish(Message::Child);
                shell.capture_event();
            }
        }

        fn mouse_interaction(
            &self,
            _tree: &Tree,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
            _renderer: &iced_test::renderer::Renderer,
        ) -> mouse::Interaction {
            mouse::Interaction::None
        }

        fn draw(
            &self,
            _tree: &Tree,
            _renderer: &mut iced_test::renderer::Renderer,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }
    }

    impl Widget<Message, Theme, iced_test::renderer::Renderer> for CursorOnly {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fill)
        }

        fn layout(
            &mut self,
            _tree: &mut Tree,
            _renderer: &iced_test::renderer::Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(limits.max())
        }

        fn mouse_interaction(
            &self,
            _tree: &Tree,
            _layout: Layout<'_>,
            cursor: mouse::Cursor,
            _viewport: &Rectangle,
            _renderer: &iced_test::renderer::Renderer,
        ) -> mouse::Interaction {
            if matches!(cursor, mouse::Cursor::Available(_)) {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::None
            }
        }

        fn draw(
            &self,
            _tree: &Tree,
            _renderer: &mut iced_test::renderer::Renderer,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }
    }

    impl Operation for ScrollProbe {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
            operate(self);
        }

        fn scrollable(
            &mut self,
            id: Option<&iced::advanced::widget::Id>,
            _bounds: Rectangle,
            _content_bounds: Rectangle,
            translation: Vector,
            _state: &mut dyn operation::Scrollable,
        ) {
            if id == Some(&self.target) {
                self.offset_y = Some(translation.y);
            }
        }
    }

    #[test]
    fn validates_fixed_geometry_and_bounds_empty_ranges() {
        assert_eq!(
            VirtualListConfig::new(0.0),
            Err(VirtualListConfigError::RowHeight)
        );
        let mut state = prepared_state::<u64>("empty");
        state.reconcile::<u64>(&[], |key| *key, config()).unwrap();
        assert_eq!(state.visible_range(0, config()), 0..0);
        assert_eq!(state.mounted_range(0, config()), 0..0);
        let outcome = state.apply(
            VirtualListEvent::Navigate(VirtualListNavigation::End),
            &[],
            |key| *key,
            config(),
        );
        assert_eq!(outcome.selected, None);
        assert_eq!(
            state.reconcile(&[7_u64, 7], |key| *key, config()),
            Err(VirtualListReconcileError::DuplicateKey(7))
        );
    }

    #[test]
    fn duplicate_reconciliation_is_atomic() {
        let mut state = prepared_state("atomic-duplicate");
        state
            .reconcile(&["kept".to_owned()], Clone::clone, config())
            .unwrap();
        let keyed_rows = state.keyed_rows.snapshot();
        let next_semantic_id = state.keyed_rows.next_local_id();
        assert_eq!(
            state.reconcile(
                &["new".to_owned(), "new".to_owned()],
                Clone::clone,
                config(),
            ),
            Err(VirtualListReconcileError::DuplicateKey("new".to_owned()))
        );
        assert!(state.keyed_rows.shares_ids_with(&keyed_rows));
        assert_eq!(state.keyed_rows.next_local_id(), next_semantic_id);
    }

    #[test]
    fn fresh_layout_and_resize_emit_measured_viewport_changes() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = VirtualListState::new(VirtualListId::new("measured-viewport"));
        state.reconcile(&items, |key| *key, config()).unwrap();
        let mut renderer = renderer();

        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Measured results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 60.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let _ = ui.update(
            &[redraw()],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            messages.contains(&Message::List(VirtualListEvent::ViewportChanged {
                height: 60.0
            }))
        );
        let cache = ui.into_cache();
        state.apply(
            match messages
                .iter()
                .position(|message| {
                    matches!(
                        message,
                        Message::List(VirtualListEvent::ViewportChanged { height: 60.0 })
                    )
                })
                .map(|index| messages.remove(index))
                .expect("measured viewport event")
            {
                Message::List(event) => event,
                message => panic!("unexpected viewport message: {message:?}"),
            },
            &items,
            |key| *key,
            config(),
        );
        assert_eq!(state.visible_range(items.len(), config()), 0..3);
        messages.clear();

        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Measured results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut ui = UserInterface::build(element, Size::new(240.0, 140.0), cache, &mut renderer);
        let _ = ui.update(
            &[redraw()],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            messages.contains(&Message::List(VirtualListEvent::ViewportChanged {
                height: 140.0
            }))
        );
    }

    #[test]
    fn fresh_mount_and_remount_synchronize_native_scroll_without_a_task() {
        let items: Vec<u64> = (0..100).collect();
        let mut first = prepared_state("native-scroll-sync-first");
        first.reconcile(&items, |key| *key, config()).unwrap();
        assert!(first.scroll_to_item(42, items.len(), config()));
        let mut remounted = prepared_state("native-scroll-sync-remounted");
        remounted.reconcile(&items, |key| *key, config()).unwrap();
        assert!(remounted.scroll_to_item(84, items.len(), config()));

        let mut cache = user_interface::Cache::default();
        for (state, expected_offset) in [(&first, 840.0), (&remounted, 1_680.0)] {
            let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
                state,
                &items,
                config(),
                "Native scroll results",
                |key| *key,
                |key| format!("Item {key}"),
                |index, _, _| iced::widget::text(index).into(),
                Message::List,
            );
            let mut renderer = renderer();
            let mut ui =
                UserInterface::build(element, Size::new(240.0, 100.0), cache, &mut renderer);
            let mut probe = ScrollProbe {
                target: state.scroll_id(),
                offset_y: None,
            };
            ui.operate(&renderer, &mut probe);
            assert_eq!(probe.offset_y, Some(expected_offset));
            cache = ui.into_cache();
        }
    }

    #[test]
    fn programmatic_scroll_back_to_zero_updates_the_native_scrollable() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("native-scroll-zero");
        state.reconcile(&items, |key| *key, config()).unwrap();
        assert!(state.scroll_to_item(42, items.len(), config()));
        let mut renderer = renderer();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Native zero results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut probe = ScrollProbe {
            target: state.scroll_id(),
            offset_y: None,
        };
        ui.operate(&renderer, &mut probe);
        assert_eq!(probe.offset_y, Some(840.0));
        let cache = ui.into_cache();

        assert!(state.scroll_to_item(0, items.len(), config()));
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Native zero results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut ui = UserInterface::build(element, Size::new(240.0, 100.0), cache, &mut renderer);
        let mut probe = ScrollProbe {
            target: state.scroll_id(),
            offset_y: None,
        };
        ui.operate(&renderer, &mut probe);
        assert_eq!(probe.offset_y, Some(0.0));
    }

    #[test]
    fn direct_render_and_queries_share_ranges_across_count_and_config_changes() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("direct-render");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let first = config();
        let builds = Cell::new(0_usize);
        let element: Element<'_, Message> = virtual_list(
            &state,
            &items,
            first,
            "Direct render results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| {
                builds.set(builds.get() + 1);
                iced::widget::text(index).into()
            },
            Message::List,
        );
        assert_eq!(state.visible_range(items.len(), first), 0..5);
        assert_eq!(state.mounted_range(items.len(), first), 0..7);
        assert_eq!(builds.get(), 7);
        drop(element);

        let changed = VirtualListConfig::new(10.0).unwrap().overscan(1);
        state.apply(
            VirtualListEvent::ViewportChanged { height: 35.0 },
            &items,
            |key| *key,
            changed,
        );
        assert_eq!(state.visible_range(items.len(), changed), 0..4);
        assert_eq!(state.mounted_range(items.len(), changed), 0..5);
        assert_eq!(state.mounted_range(1, changed), 0..1);

        let mut scrolled = prepared_state("shrinking-render");
        scrolled.reconcile(&items, |key| *key, first).unwrap();
        scrolled.apply(
            VirtualListEvent::Scrolled { offset_y: 1_900.0 },
            &items,
            |key| *key,
            first,
        );
        assert_eq!(scrolled.visible_range(1, first), 0..1);
        assert_eq!(scrolled.mounted_range(1, first), 0..1);
        let one = &items[..1];
        scrolled.reconcile(one, |key| *key, first).unwrap();
        let one_build = Cell::new(0_usize);
        let element: Element<'_, Message> = virtual_list(
            &scrolled,
            one,
            first,
            "Shrunk results",
            |key| *key,
            |key| format!("Item {key}"),
            |_, _, _| {
                one_build.set(one_build.get() + 1);
                iced::widget::text("one").into()
            },
            Message::List,
        );
        assert_eq!(one_build.get(), 1);
        drop(element);
    }

    #[test]
    fn selection_follows_stable_key_and_clears_when_deleted() {
        let mut state = prepared_state("reconcile");
        let items = [10_u64, 20, 30];
        state.apply(
            VirtualListEvent::Select { index: 1, key: 20 },
            &items,
            |key| *key,
            config(),
        );
        let reordered = [30, 10, 20];
        state.reconcile(&reordered, |key| *key, config()).unwrap();
        assert_eq!(state.selected(), Some(&20));
        assert_eq!(state.selected_index(), Some(2));
        state.reconcile(&[30, 10], |key| *key, config()).unwrap();
        assert_eq!(state.selected(), None);
        assert_eq!(state.selected_index(), None);
    }

    #[test]
    fn semantic_identity_is_retained_by_key_and_namespaced_by_list() {
        let mut first = prepared_state("list");
        first
            .reconcile(&[20_u64, 30], |key| *key, config())
            .unwrap();
        let before_reorder = first.semantic_id(&20);
        first
            .reconcile(&[30_u64, 20], |key| *key, config())
            .unwrap();
        assert_eq!(before_reorder, first.semantic_id(&20));
        assert_ne!(before_reorder, first.semantic_id(&30));
        let mut second = prepared_state("list");
        second.reconcile(&[20_u64], |key| *key, config()).unwrap();
        assert_ne!(first.widget_id(), second.widget_id());
        assert_ne!(first.scroll_id(), second.scroll_id());
        assert_ne!(before_reorder, second.semantic_id(&20));
    }

    #[test]
    fn explicit_state_fork_has_independent_native_and_semantic_identity() {
        let mut original = prepared_state("forked-list");
        original
            .reconcile(&["first".to_owned()], Clone::clone, config())
            .unwrap();
        let fork = original.fork("forked-list-copy");
        assert_eq!(fork.id().logical(), "forked-list-copy");
        assert_ne!(original.id().logical(), fork.id().logical());
        assert_ne!(original.id().namespace, fork.id().namespace);
        assert_ne!(original.widget_id(), fork.widget_id());
        assert_ne!(original.scroll_id(), fork.scroll_id());
        assert_ne!(
            original.semantic_id(&"first".to_owned()),
            fork.semantic_id(&"first".to_owned())
        );

        let update = original.update_snapshot();
        assert_eq!(original.id(), update.id());
        assert!(original.keyed_rows.shares_ids_with(&update.keyed_rows));
        assert_eq!(
            original.semantic_id(&"first".to_owned()),
            update.semantic_id(&"first".to_owned())
        );

        assert!(original.keyed_rows.shares_ids_with(&fork.keyed_rows));
        let mut reconciled_fork = fork;
        reconciled_fork
            .reconcile(&["first".to_owned()], Clone::clone, config())
            .unwrap();
        assert!(
            !original
                .keyed_rows
                .shares_ids_with(&reconciled_fork.keyed_rows)
        );
    }

    #[test]
    fn concurrently_mounted_fork_has_exact_driver_list_and_row_selectors() {
        const SOURCE: Location = Location::new(
            "virtual-list-driver.ice",
            1,
            1,
            "find each forked list and row",
        );
        let mut driver = Driver::new(
            iced::application::<ForkMountState, ForkMountMessage, Theme, iced::Renderer>(
                fork_mount_boot,
                fork_mount_update,
                fork_mount_view,
            ),
            DriverConfig::new("forked_virtual_list_selectors").viewport(240.0, 160.0),
        );

        let selectors = {
            let state = driver.state();
            [
                state.original.id().selector(),
                state.original.item_selector(&10).unwrap(),
                state.fork.id().selector(),
                state.fork.item_selector(&10).unwrap(),
            ]
        };
        let unique = selectors.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), selectors.len());

        for selector in selectors {
            assert!(driver.target(&selector, SOURCE).visible());
        }
    }

    #[test]
    fn selectors_are_canonical_escaped_and_type_tagged() {
        let items = [10_u64];
        let mut state = VirtualListState::new(VirtualListId::new("a/%/\u{D55C}"));
        state.reconcile(&items, |item| *item, config()).unwrap();

        assert_eq!(
            state.id().selector(),
            "__ice/virtual-list/list/a%2F%25%2F%ED%95%9C"
        );
        assert_eq!(
            state.item_selector(&10).as_deref(),
            Some("__ice/virtual-list/item/a%2F%25%2F%ED%95%9C/2")
        );
        assert_ne!(state.id().selector(), state.item_selector(&10).unwrap());
        assert_eq!(state.item_selector(&99), None);
    }

    #[test]
    fn owned_string_keys_are_supported_without_interning() {
        let items = ["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];
        let mut state = prepared_state("string-keys");
        state.reconcile(&items, Clone::clone, config()).unwrap();
        state.apply(
            VirtualListEvent::Select {
                index: 1,
                key: "beta".to_owned(),
            },
            &items,
            Clone::clone,
            config(),
        );
        assert_eq!(state.selected().map(String::as_str), Some("beta"));
        let element: Element<'_, (), Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "String key results",
            Clone::clone,
            Clone::clone,
            |_, item, _| iced::widget::text(item).into(),
            |_| (),
        );
        drop(element);
    }

    #[test]
    fn unchanged_mounted_keys_still_reset_a_changed_widget_tag() {
        let mounted = |child| MountedRows {
            keys: vec![2],
            children: vec![child],
            touch_claim: Rc::new(Cell::new(TouchClaim::None)),
            scroll_offset: Rc::new(Cell::new(0.0)),
            realized_heights: Rc::new(RefCell::new(Vec::new())),
            draw_probe: "mounted-rows-test",
        };
        let first_child: Element<'_, Message, Theme, iced_test::renderer::Renderer> =
            iced::widget::text("row").into();
        let first = mounted(first_child);
        let mut tree =
            WidgetTree::new(&first as &dyn Widget<Message, Theme, iced_test::renderer::Renderer>);
        let previous_tag = tree.children[0].tag;

        let changed_child: Element<'_, Message, Theme, iced_test::renderer::Renderer> =
            iced::widget::button("row").into();
        let changed_tag = changed_child.as_widget().tag();
        let changed = mounted(changed_child);
        tree.diff(&changed as &dyn Widget<Message, Theme, iced_test::renderer::Renderer>);

        assert_ne!(previous_tag, changed_tag);
        assert_eq!(tree.children[0].tag, changed_tag);
    }

    #[test]
    fn mounted_widget_state_follows_stable_key_across_reorder() {
        let mut items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("retained-row-state");
        state.reconcile(&items, |item| *item, config()).unwrap();
        let mut renderer = renderer();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Retained rows",
            |item| *item,
            |item| format!("Item {item}"),
            |_, item, _| Element::new(RowIdentity(*item)),
            Message::List,
        );
        let ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let cache = ui.into_cache();

        items.swap(0, 1);
        state.reconcile(&items, |item| *item, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Retained rows",
            |item| *item,
            |item| format!("Item {item}"),
            |_, item, _| Element::new(RowIdentity(*item)),
            Message::List,
        );
        let _ui = UserInterface::build(element, Size::new(240.0, 100.0), cache, &mut renderer);
    }

    #[test]
    fn overlapping_mounted_windows_retain_intersection_row_trees() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("sliding-retained-row-state");
        state.reconcile(&items, |item| *item, config()).unwrap();
        let creations = Rc::new(Cell::new(0));
        let build = |state: &VirtualListState<u64>| {
            virtual_list(
                state,
                &items,
                config(),
                "Sliding retained rows",
                |item| *item,
                |item| format!("Item {item}"),
                |_, item, _| {
                    Element::new(RetainedRowIdentity {
                        key: *item,
                        creations: Rc::clone(&creations),
                    })
                },
                Message::List,
            )
        };
        let mut renderer = renderer();
        let ui = UserInterface::build(
            build(&state),
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        assert_eq!(creations.get(), 7);
        let cache = ui.into_cache();

        state.apply(
            VirtualListEvent::Scrolled { offset_y: 20.0 },
            &items,
            |item| *item,
            config(),
        );
        let ui = UserInterface::build(build(&state), Size::new(240.0, 100.0), cache, &mut renderer);
        assert_eq!(creations.get(), 8, "only the entering edge row is new");
        let cache = ui.into_cache();

        state.apply(
            VirtualListEvent::Scrolled { offset_y: 800.0 },
            &items,
            |item| *item,
            config(),
        );
        let ui = UserInterface::build(build(&state), Size::new(240.0, 100.0), cache, &mut renderer);
        let before_slide = creations.get();
        let cache = ui.into_cache();
        state.apply(
            VirtualListEvent::Scrolled { offset_y: 820.0 },
            &items,
            |item| *item,
            config(),
        );
        let _ui =
            UserInterface::build(build(&state), Size::new(240.0, 100.0), cache, &mut renderer);
        assert_eq!(
            creations.get(),
            before_slide + 1,
            "a one-row sliding window must preserve every overlapping row tree"
        );
    }

    #[test]
    fn colliding_key_hashes_do_not_alias_semantic_identity_across_reorder() {
        fn identities(
            state: &VirtualListState<DisplayCollision>,
            items: &[DisplayCollision],
        ) -> std::collections::HashMap<String, NodeId> {
            let element: Element<'_, CollisionMessage, Theme, iced_test::renderer::Renderer> =
                virtual_list(
                    state,
                    items,
                    config(),
                    "Collision results",
                    |item| *item,
                    |item| format!("Item {}", item.0),
                    |_, item, _| iced::widget::text(item.0).into(),
                    |_event: VirtualListEvent<DisplayCollision>| CollisionMessage,
                );
            let mut renderer = renderer();
            let mut ui = UserInterface::build(
                element,
                Size::new(240.0, 100.0),
                user_interface::Cache::default(),
                &mut renderer,
            );
            let mut operation = SnapshotOperation::<CollisionMessage>::named("Collision test");
            ui.operate(&renderer, &mut operation::black_box(&mut operation));
            let Outcome::Some(snapshot) = operation.finish() else {
                panic!("snapshot operation did not finish");
            };
            snapshot
                .update
                .nodes
                .into_iter()
                .filter(|(_, node)| node.role() == crate::Role::ListItem)
                .map(|(id, node)| (node.label().expect("item label").to_owned(), id))
                .collect()
        }

        let before_items = [
            DisplayCollision(1),
            DisplayCollision(2),
            DisplayCollision(3),
        ];
        let after_items = [
            DisplayCollision(3),
            DisplayCollision(1),
            DisplayCollision(2),
        ];
        let mut state = prepared_state("collision-list");
        state
            .reconcile(&before_items, |item| *item, config())
            .unwrap();
        let before = identities(&state, &before_items);
        state
            .reconcile(&after_items, |item| *item, config())
            .unwrap();
        let after = identities(&state, &after_items);
        assert_ne!(before["Item 1"], before["Item 2"]);
        assert_eq!(before["Item 1"], after["Item 1"]);
        assert_eq!(before["Item 2"], after["Item 2"]);
        assert_eq!(before["Item 3"], after["Item 3"]);
    }

    #[test]
    fn keyboard_navigation_and_programmatic_scroll_are_bounded() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("keyboard");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let navigate = |state: &mut VirtualListState<u64>, navigation| {
            state.apply(
                VirtualListEvent::Navigate(navigation),
                &items,
                |key| *key,
                config(),
            );
        };
        navigate(&mut state, VirtualListNavigation::End);
        assert_eq!(state.selected(), Some(&99));
        assert_eq!(state.scroll_offset(), 1_900.0);
        navigate(&mut state, VirtualListNavigation::PageUp);
        assert_eq!(state.selected(), Some(&94));
        navigate(&mut state, VirtualListNavigation::Up);
        assert_eq!(state.selected(), Some(&93));
        navigate(&mut state, VirtualListNavigation::Home);
        assert_eq!(state.selected(), Some(&0));
        navigate(&mut state, VirtualListNavigation::Up);
        assert_eq!(state.selected(), Some(&0));
        navigate(&mut state, VirtualListNavigation::PageDown);
        assert_eq!(state.selected(), Some(&5));
        navigate(&mut state, VirtualListNavigation::Down);
        assert_eq!(state.selected(), Some(&6));
        assert!(state.scroll_to_item(42, items.len(), config()));
        assert_eq!(state.scroll_offset(), 840.0);
        assert_eq!(state.visible_range(items.len(), config()), 42..47);
        assert_eq!(state.mounted_range(items.len(), config()), 40..49);
        assert!(state.scroll_to_key(&84, items.len(), config()));
        assert_eq!(state.scroll_offset(), 1_680.0);
        assert!(!state.scroll_to_key(&1_000, items.len(), config()));
        assert!(!state.scroll_to_item(usize::MAX, items.len(), config()));
        assert_eq!(state.scroll_offset(), 1_680.0);
    }

    #[test]
    fn builds_only_visible_and_overscan_rows_for_one_hundred_thousand_items() {
        let items: Vec<u64> = (0..100_000).collect();
        let mut state = prepared_state("performance");
        state.reconcile(&items, |key| *key, config()).unwrap();
        state.apply(
            VirtualListEvent::Scrolled {
                offset_y: 1_000_000.0,
            },
            &items,
            |key| *key,
            config(),
        );
        let builds = Cell::new(0_usize);
        let element: Element<'_, Message> = virtual_list(
            &state,
            &items,
            config(),
            "Performance results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| {
                builds.set(builds.get() + 1);
                iced::widget::text(index).into()
            },
            Message::List,
        );
        assert_eq!(
            builds.get(),
            state.mounted_range(items.len(), config()).len()
        );
        assert!(builds.get() <= config().rows_per_page(100.0) + config().overscan_rows() * 2 + 1);
        let inspection = state.inspect(items.len(), config());
        assert_eq!(inspection.logical_items, 100_000);
        assert_eq!(inspection.mounted_rows, builds.get());
        assert_eq!(inspection.child_slots, builds.get() + 2);
        drop(element);
    }

    #[test]
    fn accesskit_exports_only_mounted_rows_with_collection_metadata() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("semantic-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        state.apply(
            VirtualListEvent::Select { index: 2, key: 2 },
            &items,
            |key| *key,
            config(),
        );
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Semantic results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
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
        let mut operation = SnapshotOperation::<Message>::named("Virtual list test");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(snapshot) = operation.finish() else {
            panic!("snapshot operation did not finish");
        };
        let (list_id, list) = snapshot
            .update
            .nodes
            .iter()
            .find(|(id, node)| *id != ROOT_ID && node.role() == crate::Role::List)
            .map(|(id, node)| (*id, node))
            .expect("list semantic node");
        assert_eq!(list.size_of_set(), Some(100));
        assert_eq!(list.label(), Some("Semantic results"));
        assert!(list.supports_action(crate::Action::Focus));
        let rows = snapshot
            .update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == crate::Role::ListItem)
            .map(|(_, node)| node)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), state.mounted_range(items.len(), config()).len());
        assert_eq!(rows[0].position_in_set(), Some(1));
        assert_eq!(rows[0].size_of_set(), Some(100));
        assert_eq!(rows[0].is_selected(), Some(false));
        assert_eq!(rows[2].label(), Some("Item 2"));
        assert_eq!(rows[2].is_selected(), Some(true));
        let selected_row_id = snapshot
            .update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Item 2"))
            .map(|(id, _)| *id)
            .expect("selected row semantic node");
        assert_eq!(list.active_descendant(), Some(selected_row_id));

        let focus = snapshot.dispatch(crate::ActionRequest {
            action: crate::Action::Focus,
            target_tree: TreeId::ROOT,
            target_node: list_id,
            data: None,
        });
        let mut stream = iced_test::runtime::task::into_stream(focus).expect("focus task");
        let action = iced_test::futures::futures::executor::block_on(stream.next())
            .expect("focus operation");
        let iced_test::runtime::Action::Widget(mut focus) = action else {
            panic!("focus dispatch must produce a widget operation");
        };
        ui.operate(&renderer, focus.as_mut());
        let mut operation = SnapshotOperation::<Message>::named("Focused virtual list test");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(focused) = operation.finish() else {
            panic!("focused snapshot operation did not finish");
        };
        assert_eq!(focused.update.focus, list_id);
        let mut messages = Vec::new();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::End)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![Message::List(VirtualListEvent::Navigate(
                VirtualListNavigation::End
            ))]
        );

        let cache = ui.into_cache();
        let Message::List(event) = messages.pop().expect("navigation event") else {
            panic!("unexpected navigation message");
        };
        state.apply(event, &items, |key| *key, config());
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Semantic results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut ui = UserInterface::build(element, Size::new(240.0, 100.0), cache, &mut renderer);
        let mut operation = SnapshotOperation::<Message>::named("Rebuilt virtual list test");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(rebuilt) = operation.finish() else {
            panic!("rebuilt snapshot operation did not finish");
        };
        let (rebuilt_list_id, rebuilt_list) = rebuilt
            .update
            .nodes
            .iter()
            .find(|(id, node)| *id != ROOT_ID && node.role() == crate::Role::List)
            .expect("rebuilt list semantic node");
        let last_row_id = rebuilt
            .update
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some("Item 99"))
            .map(|(id, _)| *id)
            .expect("revealed last row semantic node");
        assert_eq!(rebuilt.update.focus, *rebuilt_list_id);
        assert_eq!(rebuilt_list.active_descendant(), Some(last_row_id));
        assert_eq!(
            rebuilt
                .update
                .nodes
                .iter()
                .find(|(id, _)| *id == last_row_id)
                .and_then(|(_, node)| node.is_selected()),
            Some(true)
        );
    }

    #[test]
    fn headless_mouse_and_keyboard_emit_typed_interactions() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("headless-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Headless results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, selected| {
                iced::widget::text(format!("row {index} selected={selected}")).into()
            },
            Message::List,
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
        ui.draw(
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );
        let point = Point::new(10.0, 45.0);
        let mut messages = Vec::new();
        let _ = ui.update(
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![Message::List(VirtualListEvent::Select { index: 2, key: 2 })]
        );
        let mut operation = SnapshotOperation::<Message>::named("Pointer-focused list");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(pointer_focused) = operation.finish() else {
            panic!("pointer-focused snapshot operation did not finish");
        };
        let list_id = pointer_focused
            .update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == crate::Role::List)
            .map(|(id, _)| *id)
            .expect("list semantic node");
        assert_eq!(pointer_focused.update.focus, list_id);
        messages.clear();
        let _ = ui.update(
            &[Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::PageDown),
                modified_key: keyboard::Key::Named(keyboard::key::Named::PageDown),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::PageDown),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            })],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![Message::List(VirtualListEvent::Navigate(
                VirtualListNavigation::PageDown
            ))]
        );
    }

    #[test]
    fn batched_native_scroll_and_click_use_the_live_native_offset() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("batched-native-offset");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Batched native results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let point = Point::new(10.0, 10.0);
        let mut messages = Vec::new();
        let _ = ui.update(
            &[
                Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
                }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            ],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::List(VirtualListEvent::Scrolled { offset_y })
                if (*offset_y - 60.0).abs() < f32::EPSILON
        )));
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::List(VirtualListEvent::Select { index: 3, key: 3 })
        )));
    }

    #[test]
    fn interactive_row_content_captures_click_before_row_selection() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("interactive-row-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Interactive results",
            |key| *key,
            |key| format!("Item {key}"),
            |_, _, _| {
                iced::widget::button("Child action")
                    .on_press(Message::Child)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            },
            Message::List,
        );
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let point = Point::new(20.0, 10.0);
        let _ = ui.update(
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, vec![Message::Child]);
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::ArrowDown)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(messages.is_empty());
    }

    #[test]
    fn interactive_row_content_captures_touch_and_cursor_semantics() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("interactive-touch-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Interactive touch results",
            |key| *key,
            |key| format!("Item {key}"),
            |_, _, _| {
                iced::widget::button("Child action")
                    .on_press(Message::Child)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            },
            Message::List,
        );
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let point = Point::new(20.0, 10.0);
        let finger = iced::touch::Finger(11);
        let mut messages = Vec::new();
        let _ = ui.update(
            &[
                Event::Touch(iced::touch::Event::FingerPressed {
                    id: finger,
                    position: point,
                }),
                Event::Touch(iced::touch::Event::FingerLifted {
                    id: finger,
                    position: point,
                }),
            ],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, vec![Message::Child]);

        let items = [0_u64];
        let mut state = prepared_state("text-cursor-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Text cursor results",
            |key| *key,
            |key| format!("Item {key}"),
            |_, _, _| {
                iced::widget::text_input("Filter", "")
                    .on_input(Message::Input)
                    .width(Length::Fill)
                    .into()
            },
            Message::List,
        );
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let (ui_state, _) = ui.update(
            &[],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let user_interface::State::Updated {
            mouse_interaction, ..
        } = ui_state
        else {
            panic!("cursor-only update unexpectedly invalidated the interface");
        };
        assert_eq!(mouse_interaction, mouse::Interaction::Text);
    }

    #[test]
    fn touch_ownership_uses_capture_instead_of_cursor_shape() {
        let items = [0_u64];
        let point = Point::new(20.0, 10.0);
        let finger = iced::touch::Finger(12);
        let mut renderer = renderer();

        let mut captured = prepared_state("capture-without-cursor");
        captured.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &captured,
            &items,
            config(),
            "Captured child",
            |key| *key,
            |key| format!("Item {key}"),
            |_, _, _| Element::new(CaptureWithoutCursor),
            Message::List,
        );
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let _ = ui.update(
            &[
                Event::Touch(iced::touch::Event::FingerPressed {
                    id: finger,
                    position: point,
                }),
                Event::Touch(iced::touch::Event::FingerLifted {
                    id: finger,
                    position: point,
                }),
            ],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, vec![Message::Child]);

        let mut cursor_only = prepared_state("cursor-without-capture");
        cursor_only.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &cursor_only,
            &items,
            config(),
            "Cursor-only child",
            |key| *key,
            |key| format!("Item {key}"),
            |_, _, _| Element::new(CursorOnly),
            Message::List,
        );
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        messages.clear();
        for (finger, cursor) in [
            (finger, mouse::Cursor::Unavailable),
            (
                iced::touch::Finger(14),
                mouse::Cursor::Available(Point::new(500.0, 500.0)),
            ),
        ] {
            let _ = ui.update(
                &[
                    Event::Touch(iced::touch::Event::FingerPressed {
                        id: finger,
                        position: point,
                    }),
                    Event::Touch(iced::touch::Event::FingerLifted {
                        id: finger,
                        position: point,
                    }),
                ],
                cursor,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut messages,
            );
        }
        assert_eq!(
            messages,
            vec![
                Message::List(VirtualListEvent::Select { index: 0, key: 0 }),
                Message::List(VirtualListEvent::Select { index: 0, key: 0 }),
            ]
        );
    }

    #[test]
    fn scrolled_touch_uses_event_position_when_cursor_is_unavailable_or_elsewhere() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("scrolled-touch-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        assert!(state.scroll_to_item(40, items.len(), config()));
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Scrolled touch results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let point = Point::new(10.0, 10.0);

        for (finger, cursor) in [
            (iced::touch::Finger(21), mouse::Cursor::Unavailable),
            (
                iced::touch::Finger(22),
                mouse::Cursor::Available(Point::new(500.0, 500.0)),
            ),
        ] {
            let _ = ui.update(
                &[
                    Event::Touch(iced::touch::Event::FingerPressed {
                        id: finger,
                        position: point,
                    }),
                    Event::Touch(iced::touch::Event::FingerLifted {
                        id: finger,
                        position: point,
                    }),
                ],
                cursor,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut messages,
            );
        }

        assert_eq!(
            messages,
            vec![
                Message::List(VirtualListEvent::Select { index: 40, key: 40 }),
                Message::List(VirtualListEvent::Select { index: 40, key: 40 }),
            ]
        );
    }

    #[test]
    fn native_scrollbar_press_and_drag_never_select_a_row() {
        let items: Vec<u64> = (0..100).collect();
        let mut state = prepared_state("scrollbar-list");
        state.reconcile(&items, |key| *key, config()).unwrap();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            config(),
            "Scrollbar results",
            |key| *key,
            |key| format!("Item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::List,
        );
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        ui.draw(
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            mouse::Cursor::Unavailable,
        );
        let mut messages = Vec::new();
        let press = Point::new(239.0, 40.0);

        for (finger, cursor) in [
            (iced::touch::Finger(13), mouse::Cursor::Unavailable),
            (
                iced::touch::Finger(14),
                mouse::Cursor::Available(Point::new(500.0, 500.0)),
            ),
        ] {
            let _ = ui.update(
                &[
                    Event::Touch(iced::touch::Event::FingerPressed {
                        id: finger,
                        position: press,
                    }),
                    Event::Touch(iced::touch::Event::FingerLifted {
                        id: finger,
                        position: press,
                    }),
                ],
                cursor,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut messages,
            );
        }
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::List(VirtualListEvent::Select { .. }))),
            "touching a fresh native scrollbar must not select the row beneath it"
        );

        messages.clear();
        let _ = ui.update(
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            mouse::Cursor::Available(press),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let drag = Point::new(239.0, 85.0);
        let _ = ui.update(
            &[Event::Mouse(mouse::Event::CursorMoved { position: drag })],
            mouse::Cursor::Available(drag),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::List(VirtualListEvent::Scrolled { offset_y }) if *offset_y > 0.0
        )));
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::List(VirtualListEvent::Select { .. })))
        );

        messages.clear();
        let tap = Point::new(239.0, 40.0);
        for (finger, cursor) in [
            (iced::touch::Finger(15), mouse::Cursor::Unavailable),
            (
                iced::touch::Finger(16),
                mouse::Cursor::Available(Point::new(500.0, 500.0)),
            ),
        ] {
            let _ = ui.update(
                &[
                    Event::Touch(iced::touch::Event::FingerPressed {
                        id: finger,
                        position: tap,
                    }),
                    Event::Touch(iced::touch::Event::FingerLifted {
                        id: finger,
                        position: tap,
                    }),
                ],
                cursor,
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut messages,
            );
        }
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::List(VirtualListEvent::Select { .. }))),
            "touching the native scrollbar must not select the row beneath it"
        );
    }

    #[test]
    fn pointer_focus_moves_between_lists_and_clears_for_text_input() {
        let items: Vec<u64> = (0..100).collect();
        let mut first = prepared_state("first-list");
        first.reconcile(&items, |key| *key, config()).unwrap();
        let mut second = prepared_state("second-list");
        second.reconcile(&items, |key| *key, config()).unwrap();
        let first_list: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &first,
            &items,
            config(),
            "First results",
            |key| *key,
            |key| format!("First item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::First,
        );
        let second_list: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &second,
            &items,
            config(),
            "Second results",
            |key| *key,
            |key| format!("Second item {key}"),
            |index, _, _| iced::widget::text(index).into(),
            Message::Second,
        );
        let input: Element<'_, Message, Theme, iced_test::renderer::Renderer> =
            iced::widget::container(
                iced::widget::text_input("Filter", "").on_input(Message::Input),
            )
            .height(36.0)
            .into();
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> =
            iced::widget::column![first_list, second_list, input].into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 236.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        for point in [Point::new(10.0, 40.0), Point::new(10.0, 140.0)] {
            let _ = ui.update(
                &[Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Left,
                ))],
                mouse::Cursor::Available(point),
                &mut renderer,
                &mut iced::advanced::clipboard::Null,
                &mut messages,
            );
            messages.clear();
        }
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::ArrowDown)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![Message::Second(VirtualListEvent::Navigate(
                VirtualListNavigation::Down
            ))]
        );

        messages.clear();
        let input_point = Point::new(20.0, 218.0);
        let _ = ui.update(
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            mouse::Cursor::Available(input_point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::ArrowDown)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            messages
                .iter()
                .all(|message| matches!(message, Message::Input(_)))
        );

        messages.clear();
        let first_point = Point::new(10.0, 40.0);
        let finger = iced::touch::Finger(7);
        let _ = ui.update(
            &[
                Event::Touch(iced::touch::Event::FingerPressed {
                    id: finger,
                    position: first_point,
                }),
                Event::Touch(iced::touch::Event::FingerLifted {
                    id: finger,
                    position: first_point,
                }),
            ],
            mouse::Cursor::Available(first_point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![Message::First(VirtualListEvent::Select {
                index: 2,
                key: 2,
            })]
        );
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::Home)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![Message::First(VirtualListEvent::Navigate(
                VirtualListNavigation::Home
            ))]
        );

        messages.clear();
        let _ = ui.update(
            &[Event::Touch(iced::touch::Event::FingerPressed {
                id: iced::touch::Finger(8),
                position: input_point,
            })],
            mouse::Cursor::Available(input_point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::End)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            messages
                .iter()
                .all(|message| matches!(message, Message::Input(_)))
        );
    }

    #[derive(Default)]
    struct RecordingRenderer {
        quads: Vec<renderer::Quad>,
    }

    impl renderer::Renderer for RecordingRenderer {
        fn start_layer(&mut self, _bounds: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _transformation: iced::Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, quad: renderer::Quad, _background: impl Into<iced::Background>) {
            self.quads.push(quad);
        }
        fn reset(&mut self, _new_bounds: Rectangle) {}
        fn allocate_image(
            &mut self,
            _handle: &iced::advanced::image::Handle,
            _callback: impl FnOnce(
                Result<iced::advanced::image::Allocation, iced::advanced::image::Error>,
            ) + Send
            + 'static,
        ) {
            panic!("focus leaf never allocates images");
        }
    }

    struct FocusLeaf;

    impl Widget<Message, (), RecordingRenderer> for FocusLeaf {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fixed(100.0), Length::Fixed(40.0))
        }

        fn layout(
            &mut self,
            _tree: &mut WidgetTree,
            _renderer: &RecordingRenderer,
            _limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(Size::new(100.0, 40.0))
        }

        fn draw(
            &self,
            _tree: &WidgetTree,
            _renderer: &mut RecordingRenderer,
            _theme: &(),
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }
    }

    #[test]
    fn readiness_primitive_is_emitted_only_for_a_drawn_mounted_row_subtree() {
        let viewport = Rectangle::with_size(Size::new(100.0, 40.0));
        let limits = layout::Limits::new(Size::ZERO, viewport.size());
        let claim = Rc::new(Cell::new(TouchClaim::None));
        let mut empty = MountedRows::<Message, (), RecordingRenderer> {
            keys: Vec::new(),
            children: Vec::new(),
            touch_claim: Rc::clone(&claim),
            scroll_offset: Rc::new(Cell::new(0.0)),
            realized_heights: Rc::new(RefCell::new(Vec::new())),
            draw_probe: "mounted-rows-test",
        };
        let mut empty_tree = WidgetTree::new(&empty as &dyn Widget<Message, (), RecordingRenderer>);
        let mut renderer = RecordingRenderer::default();
        let empty_node = empty.layout(&mut empty_tree, &renderer, &limits);
        empty.draw(
            &empty_tree,
            &mut renderer,
            &(),
            &renderer::Style::default(),
            Layout::new(&empty_node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        assert!(renderer.quads.is_empty());

        let mut mounted = MountedRows::<Message, (), RecordingRenderer> {
            keys: vec![2],
            children: vec![Element::new(FocusLeaf)],
            touch_claim: claim,
            scroll_offset: Rc::new(Cell::new(0.0)),
            realized_heights: Rc::new(RefCell::new(Vec::new())),
            draw_probe: "mounted-rows-test",
        };
        let mut mounted_tree =
            WidgetTree::new(&mounted as &dyn Widget<Message, (), RecordingRenderer>);
        let mounted_node = mounted.layout(&mut mounted_tree, &renderer, &limits);
        mounted.draw(
            &mounted_tree,
            &mut renderer,
            &(),
            &renderer::Style::default(),
            Layout::new(&mounted_node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        assert_eq!(renderer.quads.len(), 1);
        assert_eq!(renderer.quads[0].bounds.height, 40.0);
    }

    #[test]
    fn keyboard_or_accessibility_focus_draws_a_visible_list_outline() {
        let id: iced::advanced::widget::Id = "visible-list-focus".into();
        let list = VirtualList {
            content: Element::new(FocusLeaf),
            id: id.clone(),
            scroll_id: "visible-list-scroll".into(),
            namespace: 1,
            mounted: Vec::<(usize, u64)>::new(),
            config: config(),
            rows: VirtualListState::<u64>::new(VirtualListId::new("visible-list-rows"))
                .rows(0, config()),
            realized_heights: Rc::new(RefCell::new(Vec::new())),
            total_height: 40.0,
            scroll_offset: 0.0,
            viewport_height: 40.0,
            scroll_revision: 0,
            native_scroll_offset: Rc::new(Cell::new(0.0)),
            touch_claim: Rc::new(Cell::new(TouchClaim::None)),
            on_event: Rc::new(Message::List),
            on_key: Rc::new(|_| None),
        };
        let mut element: Element<'_, Message, (), RecordingRenderer> = Element::new(list);
        let mut tree = WidgetTree::new(&element);
        let mut renderer = RecordingRenderer::default();
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 40.0)),
        );
        let mut focus = operation::focusable::focus::<()>(id);
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut focus);
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &(),
            &renderer::Style {
                text_color: iced::Color::WHITE,
            },
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &Rectangle::with_size(Size::new(100.0, 40.0)),
        );
        assert_eq!(renderer.quads.len(), 1);
        assert_eq!(renderer.quads[0].border.width, 2.0);
        assert_eq!(renderer.quads[0].border.color, iced::Color::WHITE);
    }

    /// Loading older messages prepends rows, shifting every existing item's
    /// index. Measurements are keyed by item identity precisely so they
    /// survive that; keyed by index they would all land on the wrong rows.
    #[test]
    fn measured_heights_survive_a_prepend() {
        let measured = VirtualListConfig::measured(20.0).unwrap();
        let items: Vec<u64> = (100..110).collect();
        let mut state = VirtualListState::new(VirtualListId::new("measured-prepend"));
        state.reconcile(&items, |key| *key, measured).unwrap();
        state.apply(
            VirtualListEvent::ViewportChanged { height: 100.0 },
            &items,
            |key| *key,
            measured,
        );
        // Two rows measure taller than the estimate.
        state.apply(
            VirtualListEvent::RowsMeasured {
                heights: vec![(101, 60.0), (104, 50.0)],
            },
            &items,
            |key| *key,
            measured,
        );
        let before = state.rows(items.len(), measured);
        assert_eq!(before.row_height(1), 60.0);
        assert_eq!(before.row_height(4), 50.0);
        assert_eq!(before.total_height(), 10.0 * 20.0 + 40.0 + 30.0);

        // Five older messages arrive at the top: every prior item shifts by 5.
        let older: Vec<u64> = (95..110).collect();
        state.reconcile(&older, |key| *key, measured).unwrap();
        let after = state.rows(older.len(), measured);
        assert_eq!(
            after.row_height(6),
            60.0,
            "item 101 kept its measured height at its new index"
        );
        assert_eq!(after.row_height(9), 50.0, "and so did item 104");
        assert_eq!(
            after.row_height(1),
            20.0,
            "a newly prepended row is still an estimate"
        );
        assert_eq!(after.total_height(), 15.0 * 20.0 + 40.0 + 30.0);
    }

    #[test]
    fn measured_rows_fold_into_geometry_and_keep_the_anchor() {
        let measured = VirtualListConfig::measured(20.0).unwrap();
        let items: Vec<u64> = (0..20).collect();
        let mut state = VirtualListState::new(VirtualListId::new("measured-anchor"));
        state.reconcile(&items, |key| *key, measured).unwrap();
        state.apply(
            VirtualListEvent::ViewportChanged { height: 100.0 },
            &items,
            |key| *key,
            measured,
        );
        state.apply(
            VirtualListEvent::Scrolled { offset_y: 200.0 },
            &items,
            |key| *key,
            measured,
        );
        assert_eq!(state.visible_range(items.len(), measured), 10..15);

        // A correction above the viewport shifts the offset so the anchor row
        // does not visually move.
        state.apply(
            VirtualListEvent::RowsMeasured {
                heights: vec![(2, 60.0)],
            },
            &items,
            |key| *key,
            measured,
        );
        assert_eq!(state.scroll_offset(), 240.0);
        assert_eq!(state.visible_range(items.len(), measured), 10..15);

        // A correction below the viewport moves nothing.
        state.apply(
            VirtualListEvent::RowsMeasured {
                heights: vec![(18, 90.0)],
            },
            &items,
            |key| *key,
            measured,
        );
        assert_eq!(state.scroll_offset(), 240.0);

        // A list stuck to its live edge stays stuck through corrections.
        assert!(state.scroll_to_end(items.len(), measured));
        assert_eq!(state.scroll_offset(), 410.0);
        state.apply(
            VirtualListEvent::RowsMeasured {
                heights: vec![(0, 30.0)],
            },
            &items,
            |key| *key,
            measured,
        );
        assert_eq!(state.scroll_offset(), 420.0);

        // Reconciling away a measured row prunes its correction.
        let shorter: Vec<u64> = (1..20).collect();
        state.reconcile(&shorter, |key| *key, measured).unwrap();
        let total_without_row_zero = 19.0 * 20.0 + 40.0 + 70.0;
        let rows = state.rows(shorter.len(), measured);
        assert_eq!(rows.total_height(), total_without_row_zero);

        // Fixed-geometry lists ignore measurement reports entirely.
        let fixed = config();
        let mut uniform = VirtualListState::new(VirtualListId::new("measured-ignored"));
        uniform.reconcile(&items, |key| *key, fixed).unwrap();
        let outcome = uniform.apply(
            VirtualListEvent::RowsMeasured {
                heights: vec![(2, 60.0)],
            },
            &items,
            |key| *key,
            fixed,
        );
        assert!(!outcome.scroll_changed && !outcome.visible_range_changed);
        assert_eq!(uniform.rows(items.len(), fixed).total_height(), 20.0 * 20.0);
    }

    #[test]
    fn measured_mode_reports_natural_row_heights_and_stabilizes() {
        let measured = VirtualListConfig::measured(20.0).unwrap();
        let items: Vec<u64> = (0..40).collect();
        let mut state = VirtualListState::new(VirtualListId::new("measured-report"));
        state.reconcile(&items, |key| *key, measured).unwrap();
        state.apply(
            VirtualListEvent::ViewportChanged { height: 100.0 },
            &items,
            |key| *key,
            measured,
        );
        let mut renderer = renderer();

        let view_rows = |index: usize,
                         _: &u64,
                         _: bool|
         -> Element<'_, Message, Theme, iced_test::renderer::Renderer> {
            iced::widget::container(iced::widget::text(index))
                .width(Length::Fill)
                .height(if index.is_multiple_of(2) { 30.0 } else { 50.0 })
                .into()
        };
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            measured,
            "Measured heights",
            |key| *key,
            |key| format!("Item {key}"),
            view_rows,
            Message::List,
        );
        let mut ui = UserInterface::build(
            element,
            Size::new(240.0, 100.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let _ = ui.update(
            &[redraw()],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let cache = ui.into_cache();
        let reported = messages
            .iter()
            .find_map(|message| match message {
                Message::List(VirtualListEvent::RowsMeasured { heights }) => Some(heights.clone()),
                _ => None,
            })
            .expect("mounted rows must report their natural heights");
        assert!(reported.contains(&(0, 30.0)) && reported.contains(&(1, 50.0)));
        for message in messages.drain(..) {
            if let Message::List(event) = message {
                state.apply(event, &items, |key| *key, measured);
            }
        }
        // Alternating 30/50 rows average 40px: a 100px viewport sees three.
        assert_eq!(state.visible_range(items.len(), measured), 0..3);

        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = virtual_list(
            &state,
            &items,
            measured,
            "Measured heights",
            |key| *key,
            |key| format!("Item {key}"),
            view_rows,
            Message::List,
        );
        let mut ui = UserInterface::build(element, Size::new(240.0, 100.0), cache, &mut renderer);
        let _ = ui.update(
            &[redraw()],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            !messages.iter().any(|message| matches!(
                message,
                Message::List(VirtualListEvent::RowsMeasured { .. })
            )),
            "applied measurements must not be re-reported: {messages:?}"
        );
    }
}
