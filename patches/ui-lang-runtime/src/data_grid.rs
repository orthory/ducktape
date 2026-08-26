//! Fixed-row, keyed data grid for large product data surfaces.
//!
//! Rows are virtualized vertically while fixed-width columns remain mounted for
//! every mounted row. The caller owns row data, sorting, edit values, and edit
//! validation. This module retains identity, the active cell, scroll geometry,
//! edit targeting, input, inspection, and native accessibility semantics.

use crate::virtual_list::{
    VirtualListConfig, VirtualListEvent, VirtualListId, VirtualListState,
    allocate_virtual_namespace, semantic_id,
};
use crate::{Role, StableId, accessible};
use iced::advanced::text;
use iced::advanced::widget::operation::{self, Focusable};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::keyboard;
use iced::widget::{container, keyed_column, mouse_area, row, scrollable, space};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write as _;
use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

const SELECTOR_PREFIX: &str = "__ice/data-grid";
const SCROLLBAR_WIDTH: f32 = 10.0;
type SortDirectionFn<'a, ColumnKey> = dyn Fn(&ColumnKey) -> Option<accesskit::SortDirection> + 'a;

/// Explicit identity for one retained data-grid instance.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct DataGridId(VirtualListId);

impl DataGridId {
    pub fn new(logical: impl Into<String>) -> Self {
        Self(VirtualListId::new(logical))
    }

    pub fn logical(&self) -> &str {
        self.0.logical()
    }
}

/// Validated fixed row and header geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataGridConfig {
    rows: VirtualListConfig,
    header_height: f32,
}

impl DataGridConfig {
    pub fn new(row_height: f32, header_height: f32) -> Result<Self, DataGridConfigError> {
        let rows =
            VirtualListConfig::new(row_height).map_err(|_| DataGridConfigError::RowHeight)?;
        if !header_height.is_finite() || header_height <= 0.0 {
            return Err(DataGridConfigError::HeaderHeight);
        }
        Ok(Self {
            rows,
            header_height,
        })
    }

    #[must_use]
    pub const fn overscan(mut self, rows: usize) -> Self {
        self.rows = self.rows.overscan(rows);
        self
    }

    pub const fn row_height(self) -> f32 {
        self.rows.row_height()
    }

    pub const fn header_height(self) -> f32 {
        self.header_height
    }

    pub const fn overscan_rows(self) -> usize {
        self.rows.overscan_rows()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataGridConfigError {
    RowHeight,
    HeaderHeight,
}

impl fmt::Display for DataGridConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RowHeight => "data-grid row height must be finite and positive",
            Self::HeaderHeight => "data-grid header height must be finite and positive",
        })
    }
}

impl std::error::Error for DataGridConfigError {}

/// Typed, fixed-width column metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct DataGridColumn<Key> {
    key: Key,
    label: String,
    width: f32,
    sortable: bool,
    editable: bool,
}

impl<Key> DataGridColumn<Key> {
    pub fn new(key: Key, label: impl Into<String>, width: f32) -> Self {
        Self {
            key,
            label: label.into(),
            width,
            sortable: false,
            editable: false,
        }
    }

    #[must_use]
    pub const fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    #[must_use]
    pub const fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    pub const fn key(&self) -> &Key {
        &self.key
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn width(&self) -> f32 {
        self.width
    }

    pub const fn is_sortable(&self) -> bool {
        self.sortable
    }

    pub const fn is_editable(&self) -> bool {
        self.editable
    }
}

/// Stable typed identity of one data cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataGridCellId<RowKey, ColumnKey> {
    pub row: RowKey,
    pub column: ColumnKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataGridNavigation {
    Left,
    Right,
    Up,
    Down,
    RowStart,
    RowEnd,
    GridStart,
    GridEnd,
    PageUp,
    PageDown,
}

/// Strongly typed widget interaction. Sort and edit values remain caller-owned.
#[derive(Debug, Clone, PartialEq)]
pub enum DataGridEvent<RowKey, ColumnKey> {
    ViewportChanged {
        width: f32,
        height: f32,
    },
    Scrolled {
        offset_x: f32,
        offset_y: f32,
    },
    FocusCell {
        row_index: usize,
        row: RowKey,
        column_index: usize,
        column: ColumnKey,
    },
    Navigate(DataGridNavigation),
    SortRequested(ColumnKey),
    BeginEdit(DataGridCellId<RowKey, ColumnKey>),
    CommitEdit(DataGridCellId<RowKey, ColumnKey>),
    CancelEdit(DataGridCellId<RowKey, ColumnKey>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataGridOutcome<RowKey, ColumnKey> {
    pub active_cell: Option<DataGridCellId<RowKey, ColumnKey>>,
    pub selection_changed: bool,
    pub visible_range_changed: bool,
    pub scroll_changed: bool,
    pub sort_requested: Option<ColumnKey>,
    pub edit_started: Option<DataGridCellId<RowKey, ColumnKey>>,
    pub edit_committed: Option<DataGridCellId<RowKey, ColumnKey>>,
    pub edit_cancelled: Option<DataGridCellId<RowKey, ColumnKey>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataGridInspection<RowKey, ColumnKey> {
    pub logical_rows: usize,
    pub logical_columns: usize,
    pub visible_rows: Range<usize>,
    pub mounted_rows: Range<usize>,
    pub visible_columns: Range<usize>,
    pub mounted_row_count: usize,
    pub mounted_cell_count: usize,
    pub active_cell: Option<DataGridCellId<RowKey, ColumnKey>>,
    pub selected_row: Option<RowKey>,
    pub editing_cell: Option<DataGridCellId<RowKey, ColumnKey>>,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataGridReconcileError<RowKey, ColumnKey> {
    DuplicateRowKey(RowKey),
    EmptyColumns,
    DuplicateColumnKey(ColumnKey),
    InvalidColumnWidth(ColumnKey),
}

impl<RowKey, ColumnKey> fmt::Display for DataGridReconcileError<RowKey, ColumnKey>
where
    RowKey: fmt::Debug,
    ColumnKey: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRowKey(key) => write!(formatter, "duplicate data-grid row key {key:?}"),
            Self::EmptyColumns => formatter.write_str("data grid requires at least one column"),
            Self::DuplicateColumnKey(key) => {
                write!(formatter, "duplicate data-grid column key {key:?}")
            }
            Self::InvalidColumnWidth(key) => {
                write!(formatter, "data-grid column {key:?} has an invalid width")
            }
        }
    }
}

impl<RowKey, ColumnKey> std::error::Error for DataGridReconcileError<RowKey, ColumnKey>
where
    RowKey: fmt::Debug,
    ColumnKey: fmt::Debug,
{
}

#[derive(Debug, Clone, Copy)]
struct HorizontalScroll {
    offset: f32,
    viewport_width: f32,
    content_width: f32,
}

impl Default for HorizontalScroll {
    fn default() -> Self {
        Self {
            offset: 0.0,
            viewport_width: 0.0,
            content_width: 0.0,
        }
    }
}

impl HorizontalScroll {
    fn max_offset(self) -> f32 {
        (self.content_width - self.viewport_width).max(0.0)
    }

    fn set_content_width(&mut self, width: f32) -> bool {
        self.content_width = width;
        self.set_offset(self.offset)
    }

    fn set_viewport_width(&mut self, width: f32) -> bool {
        self.viewport_width = if width.is_finite() {
            width.max(0.0)
        } else {
            0.0
        };
        self.set_offset(self.offset)
    }

    fn set_offset(&mut self, offset: f32) -> bool {
        let previous = self.offset;
        self.offset = if offset.is_finite() {
            offset.clamp(0.0, self.max_offset())
        } else {
            0.0
        };
        self.offset != previous
    }

    fn reveal(&mut self, start: f32, width: f32) -> bool {
        let end = start + width;
        if start < self.offset {
            self.set_offset(start)
        } else if end > self.offset + self.viewport_width {
            self.set_offset(end - self.viewport_width)
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
struct ColumnRecord<Key> {
    column: DataGridColumn<Key>,
    start: f32,
    namespace: u32,
}

/// Retained identity, active-cell, editing, and two-axis scroll state.
pub struct DataGridState<RowKey, ColumnKey> {
    rows: VirtualListState<RowKey>,
    row_keys: Arc<[RowKey]>,
    columns: Arc<[ColumnRecord<ColumnKey>]>,
    column_indexes: Arc<HashMap<ColumnKey, usize>>,
    column_namespaces: Arc<HashMap<ColumnKey, u32>>,
    header_namespace: u32,
    active_column: Option<ColumnKey>,
    editing: Option<DataGridCellId<RowKey, ColumnKey>>,
    horizontal: HorizontalScroll,
    scroll_revision: u64,
}

impl<RowKey, ColumnKey> fmt::Debug for DataGridState<RowKey, ColumnKey>
where
    RowKey: fmt::Debug + Eq + Hash,
    ColumnKey: fmt::Debug + Eq + Hash,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DataGridState")
            .field("rows", &self.rows)
            .field("row_keys", &self.row_keys)
            .field("columns", &self.columns)
            .field("active_column", &self.active_column)
            .field("editing", &self.editing)
            .field("horizontal", &self.horizontal)
            .finish()
    }
}

impl<RowKey, ColumnKey> DataGridState<RowKey, ColumnKey>
where
    RowKey: Clone + Eq + Hash,
    ColumnKey: Clone + Eq + Hash,
{
    pub fn new(id: DataGridId) -> Self {
        Self {
            rows: VirtualListState::new(id.0),
            row_keys: Arc::from([]),
            columns: Arc::from([]),
            column_indexes: Arc::new(HashMap::new()),
            column_namespaces: Arc::new(HashMap::new()),
            header_namespace: allocate_virtual_namespace(),
            active_column: None,
            editing: None,
            horizontal: HorizontalScroll::default(),
            scroll_revision: 0,
        }
    }

    pub fn update_snapshot(&self) -> Self {
        Self {
            rows: self.rows.update_snapshot(),
            row_keys: Arc::clone(&self.row_keys),
            columns: Arc::clone(&self.columns),
            column_indexes: Arc::clone(&self.column_indexes),
            column_namespaces: Arc::clone(&self.column_namespaces),
            header_namespace: self.header_namespace,
            active_column: self.active_column.clone(),
            editing: self.editing.clone(),
            horizontal: self.horizontal,
            scroll_revision: self.scroll_revision,
        }
    }

    pub fn fork(&self, new_logical_name: impl Into<String>) -> Self {
        let mut fork = self.update_snapshot();
        fork.rows = self.rows.fork(new_logical_name);
        fork.header_namespace = allocate_virtual_namespace();
        fork.column_namespaces = Arc::new(
            self.columns
                .iter()
                .map(|record| (record.column.key.clone(), allocate_virtual_namespace()))
                .collect(),
        );
        let namespaces = Arc::clone(&fork.column_namespaces);
        fork.columns = self
            .columns
            .iter()
            .map(|record| ColumnRecord {
                column: record.column.clone(),
                start: record.start,
                namespace: namespaces[&record.column.key],
            })
            .collect::<Vec<_>>()
            .into();
        fork
    }

    pub fn logical(&self) -> &str {
        self.rows.id().logical()
    }

    pub fn selector(&self) -> String {
        self.rows.id().selector_with_prefix(SELECTOR_PREFIX, "grid")
    }

    pub fn selected_row(&self) -> Option<&RowKey> {
        self.rows.selected()
    }

    pub fn active_cell(&self) -> Option<DataGridCellId<RowKey, ColumnKey>> {
        Some(DataGridCellId {
            row: self.rows.selected()?.clone(),
            column: self.active_column.as_ref()?.clone(),
        })
    }

    pub fn editing_cell(&self) -> Option<&DataGridCellId<RowKey, ColumnKey>> {
        self.editing.as_ref()
    }

    pub fn focus_task<Message>(&self) -> iced::Task<Message> {
        self.rows.focus_task()
    }

    pub fn row_selector(&self, key: &RowKey) -> Option<String> {
        self.rows.semantic_local_id(key).map(|local| {
            let mut selector = self.rows.id().selector_with_prefix(SELECTOR_PREFIX, "row");
            write!(&mut selector, "/{local}").expect("writing to a String cannot fail");
            selector
        })
    }

    pub fn header_selector(&self, key: &ColumnKey) -> Option<String> {
        self.column_namespaces.get(key).map(|namespace| {
            let mut selector = self
                .rows
                .id()
                .selector_with_prefix(SELECTOR_PREFIX, "header");
            write!(&mut selector, "/{namespace}").expect("writing to a String cannot fail");
            selector
        })
    }

    pub fn cell_selector(&self, row: &RowKey, column: &ColumnKey) -> Option<String> {
        let row = self.rows.semantic_local_id(row)?;
        let column = self.column_namespaces.get(column)?;
        let mut selector = self.rows.id().selector_with_prefix(SELECTOR_PREFIX, "cell");
        write!(&mut selector, "/{row}/{column}").expect("writing to a String cannot fail");
        Some(selector)
    }

    pub fn visible_rows(&self, config: DataGridConfig) -> Range<usize> {
        self.rows.visible_range(self.row_keys.len(), config.rows)
    }

    pub fn mounted_rows(&self, config: DataGridConfig) -> Range<usize> {
        self.rows.mounted_range(self.row_keys.len(), config.rows)
    }

    pub fn visible_columns(&self) -> Range<usize> {
        if self.columns.is_empty() || self.horizontal.viewport_width == 0.0 {
            return 0..0;
        }
        let left = self.horizontal.offset;
        let right = left + self.horizontal.viewport_width;
        let start = self
            .columns
            .partition_point(|column| column.start + column.column.width <= left);
        let end = self.columns.partition_point(|column| column.start < right);
        start..end.max(start)
    }

    pub fn inspect(&self, config: DataGridConfig) -> DataGridInspection<RowKey, ColumnKey> {
        let mounted_rows = self.mounted_rows(config);
        DataGridInspection {
            logical_rows: self.row_keys.len(),
            logical_columns: self.columns.len(),
            visible_rows: self.visible_rows(config),
            mounted_row_count: mounted_rows.len(),
            mounted_cell_count: mounted_rows.len().saturating_mul(self.columns.len()),
            mounted_rows,
            visible_columns: self.visible_columns(),
            active_cell: self.active_cell(),
            selected_row: self.rows.selected().cloned(),
            editing_cell: self.editing.clone(),
            viewport_width: self.horizontal.viewport_width,
            viewport_height: self.rows.viewport_height(),
            offset_x: self.horizontal.offset,
            offset_y: self.rows.scroll_offset(),
        }
    }

    pub fn cell_rect(
        &self,
        row: &RowKey,
        column: &ColumnKey,
        config: DataGridConfig,
    ) -> Option<Rectangle> {
        let row = self.rows.index_of(row)?;
        let column = &self.columns[*self.column_indexes.get(column)?];
        Some(Rectangle::new(
            [column.start, row as f32 * config.row_height()].into(),
            [column.column.width, config.row_height()].into(),
        ))
    }

    pub fn reconcile<Row>(
        &mut self,
        rows: &[Row],
        row_key: impl Fn(&Row) -> RowKey,
        columns: &[DataGridColumn<ColumnKey>],
        config: DataGridConfig,
    ) -> Result<(), DataGridReconcileError<RowKey, ColumnKey>> {
        if columns.is_empty() {
            return Err(DataGridReconcileError::EmptyColumns);
        }
        let columns_unchanged = columns.len() == self.columns.len()
            && columns
                .iter()
                .zip(self.columns.iter())
                .all(|(column, record)| column == &record.column);
        if !columns_unchanged {
            let mut seen_columns = HashSet::with_capacity(columns.len());
            for column in columns {
                if !column.width.is_finite() || column.width <= 0.0 {
                    return Err(DataGridReconcileError::InvalidColumnWidth(
                        column.key.clone(),
                    ));
                }
                if !seen_columns.insert(column.key.clone()) {
                    return Err(DataGridReconcileError::DuplicateColumnKey(
                        column.key.clone(),
                    ));
                }
            }
        }

        let row_keys: Vec<_> = rows.iter().map(row_key).collect();
        let mut staged_rows = self.rows.update_snapshot();
        staged_rows
            .reconcile(&row_keys, Clone::clone, config.rows)
            .map_err(|error| match error {
                crate::VirtualListReconcileError::DuplicateKey(key) => {
                    DataGridReconcileError::DuplicateRowKey(key)
                }
            })?;
        let row_keys = if row_keys.as_slice() == self.row_keys.as_ref() {
            Arc::clone(&self.row_keys)
        } else {
            row_keys.into()
        };

        let (records, column_indexes, column_namespaces, width) = if columns_unchanged {
            (
                Arc::clone(&self.columns),
                Arc::clone(&self.column_indexes),
                Arc::clone(&self.column_namespaces),
                self.horizontal.content_width,
            )
        } else {
            let mut width = 0.0_f32;
            let mut column_indexes = HashMap::with_capacity(columns.len());
            let mut column_namespaces = HashMap::with_capacity(columns.len());
            let mut records = Vec::with_capacity(columns.len());
            for (index, column) in columns.iter().enumerate() {
                let namespace = self
                    .column_namespaces
                    .get(&column.key)
                    .copied()
                    .unwrap_or_else(allocate_virtual_namespace);
                column_indexes.insert(column.key.clone(), index);
                column_namespaces.insert(column.key.clone(), namespace);
                records.push(ColumnRecord {
                    column: column.clone(),
                    start: width,
                    namespace,
                });
                width =
                    (f64::from(width) + f64::from(column.width)).min(f64::from(f32::MAX)) as f32;
            }
            (
                records.into(),
                Arc::new(column_indexes),
                Arc::new(column_namespaces),
                width,
            )
        };

        let active_column = self
            .active_column
            .as_ref()
            .filter(|key| column_indexes.contains_key(*key))
            .cloned();
        if self.active_column.is_some() && active_column.is_none() {
            staged_rows.clear_selection();
        }
        let editing = self.editing.as_ref().filter(|cell| {
            staged_rows.index_of(&cell.row).is_some() && column_indexes.contains_key(&cell.column)
        });

        self.rows = staged_rows;
        self.row_keys = row_keys;
        self.columns = records;
        self.column_indexes = column_indexes;
        self.column_namespaces = column_namespaces;
        self.active_column = active_column;
        self.editing = editing.cloned();
        if self.horizontal.set_content_width(width) {
            self.bump_scroll_revision();
        }
        Ok(())
    }

    pub fn scroll_to_cell(
        &mut self,
        cell: &DataGridCellId<RowKey, ColumnKey>,
        config: DataGridConfig,
    ) -> bool {
        let Some(row) = self.rows.index_of(&cell.row) else {
            return false;
        };
        let Some(column) = self
            .column_indexes
            .get(&cell.column)
            .map(|index| &self.columns[*index])
        else {
            return false;
        };
        let vertical = self
            .rows
            .scroll_to_item(row, self.row_keys.len(), config.rows);
        let horizontal = self.horizontal.reveal(column.start, column.column.width);
        if vertical || horizontal {
            self.bump_scroll_revision();
        }
        vertical || horizontal
    }

    fn reveal_column(&mut self, column_index: usize) {
        let column = &self.columns[column_index];
        if self.horizontal.reveal(column.start, column.column.width) {
            self.bump_scroll_revision();
        }
    }

    pub fn apply(
        &mut self,
        event: DataGridEvent<RowKey, ColumnKey>,
        config: DataGridConfig,
    ) -> DataGridOutcome<RowKey, ColumnKey> {
        let previous_active = self.active_cell();
        let previous_visible = self.visible_rows(config);
        let previous_offsets = (self.horizontal.offset, self.rows.scroll_offset());
        let mut sort_requested = None;
        let mut edit_started = None;
        let mut edit_committed = None;
        let mut edit_cancelled = None;

        match event {
            DataGridEvent::ViewportChanged { width, height } => {
                let horizontal = self.horizontal.set_viewport_width(width);
                self.apply_rows(VirtualListEvent::ViewportChanged { height }, config);
                if horizontal {
                    self.bump_scroll_revision();
                }
            }
            DataGridEvent::Scrolled { offset_x, offset_y } => {
                let horizontal = self.horizontal.set_offset(offset_x);
                self.apply_rows(VirtualListEvent::Scrolled { offset_y }, config);
                if horizontal {
                    self.bump_scroll_revision();
                }
            }
            DataGridEvent::FocusCell {
                row_index,
                row,
                column_index,
                column,
            } => {
                if self.rows.index_of(&row) == Some(row_index)
                    && self.column_indexes.get(&column) == Some(&column_index)
                {
                    self.focus_cell(row_index, row, column_index, column, config);
                }
            }
            DataGridEvent::Navigate(navigation) => self.navigate(navigation, config),
            DataGridEvent::SortRequested(column) => {
                if self
                    .column_indexes
                    .get(&column)
                    .is_some_and(|index| self.columns[*index].column.sortable)
                {
                    sort_requested = Some(column);
                }
            }
            DataGridEvent::BeginEdit(cell) => {
                if let (Some(row), Some(column)) = (
                    self.rows.index_of(&cell.row),
                    self.column_indexes.get(&cell.column).copied(),
                ) && self.columns[column].column.editable
                {
                    self.focus_cell(row, cell.row.clone(), column, cell.column.clone(), config);
                    self.editing = Some(cell.clone());
                    edit_started = Some(cell);
                }
            }
            DataGridEvent::CommitEdit(cell) if self.editing.as_ref() == Some(&cell) => {
                self.editing = None;
                edit_committed = Some(cell);
            }
            DataGridEvent::CancelEdit(cell) if self.editing.as_ref() == Some(&cell) => {
                self.editing = None;
                edit_cancelled = Some(cell);
            }
            DataGridEvent::CommitEdit(_) | DataGridEvent::CancelEdit(_) => {}
        }

        let active_cell = self.active_cell();
        DataGridOutcome {
            selection_changed: active_cell != previous_active,
            active_cell,
            visible_range_changed: self.visible_rows(config) != previous_visible,
            scroll_changed: (self.horizontal.offset, self.rows.scroll_offset()) != previous_offsets,
            sort_requested,
            edit_started,
            edit_committed,
            edit_cancelled,
        }
    }

    fn apply_rows(&mut self, event: VirtualListEvent<RowKey>, config: DataGridConfig) {
        let previous = self.rows.scroll_offset();
        self.rows
            .apply(event, &self.row_keys, Clone::clone, config.rows);
        if self.rows.scroll_offset() != previous {
            self.bump_scroll_revision();
        }
    }

    fn focus_cell(
        &mut self,
        row_index: usize,
        row: RowKey,
        column_index: usize,
        column: ColumnKey,
        config: DataGridConfig,
    ) {
        self.apply_rows(
            VirtualListEvent::Select {
                index: row_index,
                key: row.clone(),
            },
            config,
        );
        self.active_column = Some(column.clone());
        self.reveal_column(column_index);
        debug_assert_eq!(
            self.column_indexes
                .get(self.active_column.as_ref().unwrap()),
            Some(&column_index)
        );
    }

    fn navigate(&mut self, navigation: DataGridNavigation, config: DataGridConfig) {
        if self.row_keys.is_empty() || self.columns.is_empty() {
            return;
        }
        let current_row = self
            .rows
            .selected()
            .and_then(|row| self.rows.index_of(row))
            .unwrap_or(0);
        let current_column = self
            .active_column
            .as_ref()
            .and_then(|column| self.column_indexes.get(column).copied())
            .unwrap_or(0);
        let last_row = self.row_keys.len() - 1;
        let last_column = self.columns.len() - 1;
        let page = (self.rows.viewport_height() / config.row_height())
            .floor()
            .max(1.0) as usize;
        let (row, column) = match navigation {
            DataGridNavigation::Left => (current_row, current_column.saturating_sub(1)),
            DataGridNavigation::Right => (
                current_row,
                current_column.saturating_add(1).min(last_column),
            ),
            DataGridNavigation::Up => (current_row.saturating_sub(1), current_column),
            DataGridNavigation::Down => {
                (current_row.saturating_add(1).min(last_row), current_column)
            }
            DataGridNavigation::RowStart => (current_row, 0),
            DataGridNavigation::RowEnd => (current_row, last_column),
            DataGridNavigation::GridStart => (0, 0),
            DataGridNavigation::GridEnd => (last_row, last_column),
            DataGridNavigation::PageUp => (current_row.saturating_sub(page), current_column),
            DataGridNavigation::PageDown => (
                current_row.saturating_add(page).min(last_row),
                current_column,
            ),
        };
        self.focus_cell(
            row,
            self.row_keys[row].clone(),
            column,
            self.columns[column].column.key.clone(),
            config,
        );
    }

    fn bump_scroll_revision(&mut self) {
        self.scroll_revision = self.scroll_revision.wrapping_add(1);
    }

    fn column_semantic_id(&self, column: &ColumnKey, row_local: u32) -> Option<StableId> {
        self.column_namespaces
            .get(column)
            .map(|namespace| semantic_id(*namespace, row_local))
    }
}

/// Immutable context for rendering a column header.
pub struct DataGridHeaderContext<'a, ColumnKey> {
    pub column_index: usize,
    pub column: &'a DataGridColumn<ColumnKey>,
    pub sort_direction: Option<accesskit::SortDirection>,
}

/// Immutable typed context for rendering one mounted cell.
pub struct DataGridCellContext<'a, Row, ColumnKey> {
    pub row_index: usize,
    pub row: &'a Row,
    pub column_index: usize,
    pub column: &'a DataGridColumn<ColumnKey>,
    pub active: bool,
    pub selected: bool,
    pub editing: bool,
}

/// Builds a fixed-row keyed grid with a vertically fixed, horizontally synced header.
#[allow(clippy::too_many_arguments)]
pub fn data_grid<'a, Row, RowKey, ColumnKey, Message, Theme, Renderer>(
    state: &'a DataGridState<RowKey, ColumnKey>,
    rows: &'a [Row],
    config: DataGridConfig,
    grid_label: impl Into<String>,
    row_key: impl Fn(&Row) -> RowKey,
    row_label: impl Fn(&Row) -> String,
    cell_label: impl Fn(&Row, &DataGridColumn<ColumnKey>) -> String,
    sort_direction: impl Fn(&ColumnKey) -> Option<accesskit::SortDirection> + 'a,
    header: impl Fn(DataGridHeaderContext<'a, ColumnKey>) -> Element<'a, Message, Theme, Renderer>,
    cell: impl Fn(DataGridCellContext<'a, Row, ColumnKey>) -> Element<'a, Message, Theme, Renderer>,
    on_event: impl Fn(DataGridEvent<RowKey, ColumnKey>) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    RowKey: Clone + Eq + Hash + 'static,
    ColumnKey: Clone + Eq + Hash + 'static,
    Message: Clone + 'static,
    Theme: container::Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + iced::advanced::Renderer + 'a,
{
    assert_eq!(
        rows.len(),
        state.row_keys.len(),
        "data-grid rows must be reconciled before rendering"
    );
    let on_event: Rc<dyn Fn(DataGridEvent<RowKey, ColumnKey>) -> Message + 'a> = Rc::new(on_event);
    let sort_direction: Rc<SortDirectionFn<'a, ColumnKey>> = Rc::new(sort_direction);
    let pointer_focus_claim = Rc::new(Cell::new(PointerFocusClaim::None));
    let total_width = state.horizontal.content_width;

    let mut header_children = Vec::with_capacity(state.columns.len());
    for (column_index, record) in state.columns.iter().enumerate() {
        let direction = sort_direction(&record.column.key);
        let content = container(header(DataGridHeaderContext {
            column_index,
            column: &record.column,
            sort_direction: direction,
        }))
        .width(Length::Fixed(record.column.width))
        .height(Length::Fixed(config.header_height));
        let message = DataGridEvent::SortRequested(record.column.key.clone());
        let content: Element<'a, Message, Theme, Renderer> = if record.column.sortable {
            mouse_area(content)
                .on_press(on_event(message.clone()))
                .interaction(mouse::Interaction::Pointer)
                .into()
        } else {
            content.into()
        };
        let mut header = accessible(
            content,
            semantic_id(record.namespace, 1),
            Role::ColumnHeader,
        )
        .logical_id(
            state
                .header_selector(&record.column.key)
                .expect("reconciled column has a selector"),
        )
        .label(record.column.label.clone())
        .row_index(1)
        .column_index(column_index.saturating_add(1));
        if let Some(direction) = direction {
            header = header.sort_direction(direction);
        }
        if record.column.sortable {
            header = header.on_activate(on_event(message));
        }
        header_children.push(header.into());
    }
    let header_row: Element<'a, Message, Theme, Renderer> = accessible(
        row(header_children)
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(config.header_height)),
        semantic_id(state.header_namespace, 1),
        Role::Row,
    )
    .logical_id(
        state
            .rows
            .id()
            .selector_with_prefix(SELECTOR_PREFIX, "header-row"),
    )
    .row_index(1)
    .into();

    let header_event = Rc::clone(&on_event);
    let header_offset_y = state.rows.scroll_offset();
    let header_scroll = scrollable(header_row)
        .id(state.rows.id().widget_id("data-grid-header-scroll"))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::hidden(),
        ))
        .width(Length::Fill)
        .height(Length::Fixed(config.header_height))
        .on_scroll(move |viewport| {
            header_event(DataGridEvent::Scrolled {
                offset_x: viewport.absolute_offset().x,
                offset_y: header_offset_y,
            })
        });

    let mounted = state.mounted_rows(config);
    let top =
        (mounted.start as f64 * f64::from(config.row_height())).min(f64::from(f32::MAX)) as f32;
    let total_height = (state.row_keys.len() as f64 * f64::from(config.row_height()))
        .min(f64::from(f32::MAX)) as f32;
    let bottom =
        (total_height - (mounted.end as f64 * f64::from(config.row_height())) as f32).max(0.0);
    let mut mounted_rows = Vec::with_capacity(mounted.len());
    for row_index in mounted.clone() {
        let item = &rows[row_index];
        let item_key = row_key(item);
        assert_eq!(
            state.rows.index_of(&item_key),
            Some(row_index),
            "data-grid row order changed without reconciliation"
        );
        let row_local = state
            .rows
            .semantic_local_id(&item_key)
            .expect("reconciled row has semantic identity");
        let selected = state.rows.selected() == Some(&item_key);
        let mut cells = Vec::with_capacity(state.columns.len());
        for (column_index, record) in state.columns.iter().enumerate() {
            // Compare against the parts rather than assembling a cell id per
            // cell: `active_cell()` clones both keys to build one, and the
            // literal below cloned two more, so a mounted grid paid four
            // allocations per cell per frame purely to answer two booleans.
            // The id is built only where one is actually handed onwards.
            let active = selected && state.active_column.as_ref() == Some(&record.column.key);
            let editing = state.editing.as_ref().is_some_and(|editing| {
                editing.row == item_key && editing.column == record.column.key
            });
            let view = cell(DataGridCellContext {
                row_index,
                row: item,
                column_index,
                column: &record.column,
                active,
                selected,
                editing,
            });
            let focus_event = DataGridEvent::FocusCell {
                row_index,
                row: item_key.clone(),
                column_index,
                column: record.column.key.clone(),
            };
            let area: Element<'a, Message, Theme, Renderer> = Element::new(DataGridCellArea {
                content: container(view)
                    .width(Length::Fixed(record.column.width))
                    .height(Length::Fixed(config.row_height()))
                    .into(),
                on_press: on_event(focus_event.clone()),
                on_double_click: record.column.editable.then(|| {
                    on_event(DataGridEvent::BeginEdit(DataGridCellId {
                        row: item_key.clone(),
                        column: record.column.key.clone(),
                    }))
                }),
                pointer_focus_claim: Rc::clone(&pointer_focus_claim),
            });
            let cell = accessible(
                area,
                state
                    .column_semantic_id(&record.column.key, row_local)
                    .expect("reconciled column has semantic identity"),
                Role::Cell,
            )
            .logical_id(
                state
                    .cell_selector(&item_key, &record.column.key)
                    .expect("reconciled cell has a selector"),
            )
            .label(cell_label(item, &record.column))
            .row_index(row_index.saturating_add(2))
            .column_index(column_index.saturating_add(1))
            .on_activate(on_event(focus_event));
            cells.push(cell.into());
        }
        let row: Element<'a, Message, Theme, Renderer> = accessible(
            row(cells)
                .width(Length::Fixed(total_width))
                .height(Length::Fixed(config.row_height())),
            state.rows.semantic_id(&item_key),
            Role::Row,
        )
        .logical_id(
            state
                .row_selector(&item_key)
                .expect("reconciled row has a selector"),
        )
        .label(row_label(item))
        .row_index(row_index.saturating_add(2))
        .selected(selected)
        .into();
        mounted_rows.push((row_local, row));
    }

    let rows = keyed_column(mounted_rows).width(Length::Fixed(total_width));
    let body_content = iced::widget::column![
        space()
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(top)),
        rows,
        space()
            .width(Length::Fixed(total_width))
            .height(Length::Fixed(bottom)),
    ]
    .width(Length::Fixed(total_width));
    let body_event = Rc::clone(&on_event);
    let body_scroll = scrollable(body_content)
        .id(state.rows.id().widget_id("data-grid-body-scroll"))
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::new()
                .width(SCROLLBAR_WIDTH)
                .scroller_width(SCROLLBAR_WIDTH),
            horizontal: scrollable::Scrollbar::new()
                .width(SCROLLBAR_WIDTH)
                .scroller_width(SCROLLBAR_WIDTH),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .on_scroll(move |viewport| {
            let offset = viewport.absolute_offset();
            body_event(DataGridEvent::Scrolled {
                offset_x: offset.x,
                offset_y: offset.y,
            })
        });
    let content: Element<'a, Message, Theme, Renderer> =
        iced::widget::column![header_scroll, body_scroll]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    let active_cell = state.active_cell();
    let active_editable = active_cell.as_ref().is_some_and(|cell| {
        state
            .column_indexes
            .get(&cell.column)
            .is_some_and(|index| state.columns[*index].column.editable)
    });
    let active_sortable = active_cell.as_ref().is_some_and(|cell| {
        state
            .column_indexes
            .get(&cell.column)
            .is_some_and(|index| state.columns[*index].column.sortable)
    });
    let has_mounted_cells = !mounted.is_empty() && !state.columns.is_empty();
    let active_descendant = active_cell.as_ref().and_then(|cell| {
        state
            .rows
            .index_of(&cell.row)
            .filter(|index| mounted.contains(index))?;
        let local = state.rows.semantic_local_id(&cell.row)?;
        state.column_semantic_id(&cell.column, local)
    });
    let widget = DataGridWidget {
        content,
        id: state.rows.id().widget_id("focus"),
        header_scroll_id: state.rows.id().widget_id("data-grid-header-scroll"),
        body_scroll_id: state.rows.id().widget_id("data-grid-body-scroll"),
        namespace: state.rows.id().namespace(),
        header_height: config.header_height,
        viewport_width: state.horizontal.viewport_width,
        viewport_height: state.rows.viewport_height(),
        offset_x: state.horizontal.offset,
        offset_y: state.rows.scroll_offset(),
        scroll_revision: state.scroll_revision,
        active_cell,
        editing_cell: state.editing.clone(),
        active_editable,
        active_sortable,
        has_mounted_cells,
        pointer_focus_claim,
        on_event,
    };
    accessible(
        Element::new(widget),
        state.rows.id().semantic_id(1),
        Role::Grid,
    )
    .logical_id(state.selector())
    .label(grid_label)
    .focus_descendant()
    .active_descendant_maybe(active_descendant)
    .row_count(state.row_keys.len().saturating_add(1))
    .column_count(state.columns.len())
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerFocusClaim {
    None,
    Cell,
    Descendant,
}

/// Child-first cell boundary that distinguishes a native control capture from
/// the cell's own selection press before the grid updates focus ownership.
struct DataGridCellArea<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_press: Message,
    on_double_click: Option<Message>,
    pointer_focus_claim: Rc<Cell<PointerFocusClaim>>,
}

#[derive(Default)]
struct DataGridCellAreaState {
    previous_click: Option<mouse::Click>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DataGridCellArea<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DataGridCellAreaState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DataGridCellAreaState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
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
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        let captured_before = shell.is_event_captured();
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

        let Some(position) = primary_press_position(event, cursor) else {
            return;
        };
        if !layout.bounds().contains(position) || !viewport.contains(position) {
            return;
        }
        if shell.is_event_captured() {
            if !captured_before {
                self.pointer_focus_claim.set(PointerFocusClaim::Descendant);
            }
            return;
        }

        shell.publish(self.on_press.clone());
        self.pointer_focus_claim.set(PointerFocusClaim::Cell);
        if let Some(message) = &self.on_double_click {
            let state = tree.state.downcast_mut::<DataGridCellAreaState>();
            let click = mouse::Click::new(position, mouse::Button::Left, state.previous_click);
            if click.kind() == mouse::click::Kind::Double {
                shell.publish(message.clone());
            }
            state.previous_click = Some(click);
        }
        shell.capture_event();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let interaction = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        );
        if interaction == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            interaction
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

fn primary_press_position(event: &Event, cursor: mouse::Cursor) -> Option<Point> {
    match event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => cursor.position(),
        Event::Touch(iced::touch::Event::FingerPressed { position, .. }) => Some(*position),
        _ => None,
    }
}

struct DataGridWidget<'a, RowKey, ColumnKey, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    id: iced::advanced::widget::Id,
    header_scroll_id: iced::advanced::widget::Id,
    body_scroll_id: iced::advanced::widget::Id,
    namespace: u32,
    header_height: f32,
    viewport_width: f32,
    viewport_height: f32,
    offset_x: f32,
    offset_y: f32,
    scroll_revision: u64,
    active_cell: Option<DataGridCellId<RowKey, ColumnKey>>,
    editing_cell: Option<DataGridCellId<RowKey, ColumnKey>>,
    active_editable: bool,
    active_sortable: bool,
    has_mounted_cells: bool,
    pointer_focus_claim: Rc<Cell<PointerFocusClaim>>,
    on_event: Rc<dyn Fn(DataGridEvent<RowKey, ColumnKey>) -> Message + 'a>,
}

struct DataGridWidgetState {
    namespace: u32,
    focused: bool,
    focus_visible: bool,
    measured_viewport: Size,
    reported_viewport: Option<Size>,
    applied_scroll_revision: Option<u64>,
}

impl Focusable for DataGridWidgetState {
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

impl<RowKey, ColumnKey, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DataGridWidget<'_, RowKey, ColumnKey, Message, Theme, Renderer>
where
    RowKey: Clone + Eq + 'static,
    ColumnKey: Clone + Eq + 'static,
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DataGridWidgetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DataGridWidgetState {
            namespace: self.namespace,
            focused: false,
            focus_visible: false,
            measured_viewport: Size::ZERO,
            reported_viewport: None,
            applied_scroll_revision: None,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.state.downcast_ref::<DataGridWidgetState>().namespace != self.namespace {
            *tree = Tree::new(self as &dyn Widget<Message, Theme, Renderer>);
        } else {
            tree.diff_children(std::slice::from_ref(&self.content));
        }
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
        tree.state
            .downcast_mut::<DataGridWidgetState>()
            .measured_viewport = Size::new(
            node.size().width,
            (node.size().height - self.header_height).max(0.0),
        );
        if tree
            .state
            .downcast_ref::<DataGridWidgetState>()
            .applied_scroll_revision
            != Some(self.scroll_revision)
        {
            let mut header = operation::scrollable::scroll_to::<()>(
                self.header_scroll_id.clone(),
                operation::scrollable::AbsoluteOffset {
                    x: Some(self.offset_x),
                    y: None,
                },
            );
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
                &mut header,
            );
            let mut body = operation::scrollable::scroll_to::<()>(
                self.body_scroll_id.clone(),
                operation::scrollable::AbsoluteOffset {
                    x: Some(self.offset_x),
                    y: Some(self.offset_y),
                },
            );
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
                &mut body,
            );
            tree.state
                .downcast_mut::<DataGridWidgetState>()
                .applied_scroll_revision = Some(self.scroll_revision);
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
            tree.state.downcast_mut::<DataGridWidgetState>(),
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
        let pointer_press = primary_press_position(event, cursor);
        if pointer_press.is_some() {
            self.pointer_focus_claim.set(PointerFocusClaim::None);
        }
        let state = tree.state.downcast_mut::<DataGridWidgetState>();
        if state.reported_viewport != Some(state.measured_viewport) {
            state.reported_viewport = Some(state.measured_viewport);
            if state.measured_viewport.width != self.viewport_width
                || state.measured_viewport.height != self.viewport_height
            {
                shell.publish((self.on_event)(DataGridEvent::ViewportChanged {
                    width: state.measured_viewport.width,
                    height: state.measured_viewport.height,
                }));
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
        if let Some(position) = pointer_press {
            state.focused = match self.pointer_focus_claim.get() {
                PointerFocusClaim::Cell => true,
                PointerFocusClaim::Descendant => false,
                PointerFocusClaim::None => {
                    !shell.is_event_captured() && layout.bounds().contains(position)
                }
            };
            state.focus_visible = false;
        }
        if shell.is_event_captured() || !state.focused {
            return;
        }
        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
            return;
        };
        let primary = modifiers.control() || modifiers.logo();
        let navigation = match key {
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) if modifiers.is_empty() => {
                Some(DataGridNavigation::Left)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) if modifiers.is_empty() => {
                Some(DataGridNavigation::Right)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) if modifiers.is_empty() => {
                Some(DataGridNavigation::Up)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) if modifiers.is_empty() => {
                Some(DataGridNavigation::Down)
            }
            keyboard::Key::Named(keyboard::key::Named::Home) if primary => {
                Some(DataGridNavigation::GridStart)
            }
            keyboard::Key::Named(keyboard::key::Named::End) if primary => {
                Some(DataGridNavigation::GridEnd)
            }
            keyboard::Key::Named(keyboard::key::Named::Home) if modifiers.is_empty() => {
                Some(DataGridNavigation::RowStart)
            }
            keyboard::Key::Named(keyboard::key::Named::End) if modifiers.is_empty() => {
                Some(DataGridNavigation::RowEnd)
            }
            keyboard::Key::Named(keyboard::key::Named::PageUp) if modifiers.is_empty() => {
                Some(DataGridNavigation::PageUp)
            }
            keyboard::Key::Named(keyboard::key::Named::PageDown) if modifiers.is_empty() => {
                Some(DataGridNavigation::PageDown)
            }
            _ => None,
        };
        let message = navigation.map(DataGridEvent::Navigate).or_else(|| {
            let active = self.active_cell.clone()?;
            match key {
                keyboard::Key::Named(keyboard::key::Named::F2)
                    if modifiers.is_empty() && self.active_editable =>
                {
                    Some(DataGridEvent::BeginEdit(active))
                }
                keyboard::Key::Named(keyboard::key::Named::Enter)
                    if modifiers.is_empty() && self.editing_cell.as_ref() == Some(&active) =>
                {
                    Some(DataGridEvent::CommitEdit(active))
                }
                keyboard::Key::Named(keyboard::key::Named::Enter)
                    if modifiers.is_empty() && self.active_editable =>
                {
                    Some(DataGridEvent::BeginEdit(active))
                }
                keyboard::Key::Named(keyboard::key::Named::Enter)
                    if modifiers.is_empty() && self.active_sortable =>
                {
                    Some(DataGridEvent::SortRequested(active.column))
                }
                keyboard::Key::Named(keyboard::key::Named::Escape)
                    if modifiers.is_empty() && self.editing_cell.as_ref() == Some(&active) =>
                {
                    Some(DataGridEvent::CancelEdit(active))
                }
                _ => None,
            }
        });
        if let Some(message) = message {
            shell.publish((self.on_event)(message));
            shell.capture_event();
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
        if self.has_mounted_cells {
            let bounds = layout.bounds();
            let body = Rectangle {
                y: bounds.y + self.header_height,
                height: (bounds.height - self.header_height).max(0.0),
                ..bounds
            };
            if let Some(bounds) = body.intersection(viewport) {
                // Tie readiness to a renderer primitive after the mounted-cell
                // subtree has completed its draw path.
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    iced::Color::TRANSPARENT,
                );
                crate::dev::record_draw_probe("data-grid");
            }
        }
        if tree
            .state
            .downcast_ref::<DataGridWidgetState>()
            .focus_visible
        {
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ROOT_ID, SnapshotOperation};
    use iced::advanced::renderer;
    use iced::advanced::widget::operation::{self, Outcome};
    use iced::{Font, Pixels, Theme};
    use iced_test::runtime::UserInterface;
    use iced_test::runtime::user_interface;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Column {
        Name,
        Status,
        Owner,
    }

    #[derive(Debug)]
    struct Item {
        id: u64,
        name: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Message {
        Grid(DataGridEvent<u64, Column>),
        Input(String),
    }

    fn config() -> DataGridConfig {
        DataGridConfig::new(20.0, 24.0).unwrap().overscan(2)
    }

    fn columns() -> Vec<DataGridColumn<Column>> {
        vec![
            DataGridColumn::new(Column::Name, "Name", 120.0)
                .sortable(true)
                .editable(true),
            DataGridColumn::new(Column::Status, "Status", 80.0).sortable(true),
            DataGridColumn::new(Column::Owner, "Owner", 100.0).editable(true),
        ]
    }

    fn items(count: usize) -> Vec<Item> {
        (0..count)
            .map(|id| Item {
                id: id as u64,
                name: format!("Item {id}"),
            })
            .collect()
    }

    fn prepared(count: usize) -> (DataGridState<u64, Column>, Vec<Item>) {
        let items = items(count);
        let mut state = DataGridState::new(DataGridId::new("test-grid"));
        state
            .reconcile(&items, |item| item.id, &columns(), config())
            .unwrap();
        state.apply(
            DataGridEvent::ViewportChanged {
                width: 160.0,
                height: 100.0,
            },
            config(),
        );
        (state, items)
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

    fn plain_grid<'a>(
        state: &'a DataGridState<u64, Column>,
        items: &'a [Item],
    ) -> Element<'a, Message, Theme, iced_test::renderer::Renderer> {
        data_grid(
            state,
            items,
            config(),
            "Interactive grid",
            |item| item.id,
            |item| item.name.clone(),
            |item, column| format!("{} {}", item.name, column.label()),
            |_| None,
            |header| iced::widget::text(header.column.label()).into(),
            |cell| iced::widget::text(format!("{} {}", cell.row.id, cell.column.label())).into(),
            Message::Grid,
        )
    }

    fn editing_root<'a>(
        state: &'a DataGridState<u64, Column>,
        items: &'a [Item],
        editor_id: iced::advanced::widget::Id,
        after_id: iced::advanced::widget::Id,
    ) -> Element<'a, Message, Theme, iced_test::renderer::Renderer> {
        let editor = editor_id.clone();
        let grid = data_grid(
            state,
            items,
            config(),
            "Editing grid",
            |item| item.id,
            |item| item.name.clone(),
            |item, column| format!("{} {}", item.name, column.label()),
            |_| None,
            |header| iced::widget::text(header.column.label()).into(),
            move |cell| {
                if cell.editing {
                    iced::widget::text_input("Cell value", "draft")
                        .id(editor.clone())
                        .on_input(Message::Input)
                        .into()
                } else {
                    iced::widget::text(format!("{} {}", cell.row.id, cell.column.label())).into()
                }
            },
            Message::Grid,
        );
        iced::widget::column![
            iced::widget::container(grid).height(124.0),
            iced::widget::container(
                iced::widget::text_input("After grid", "")
                    .id(after_id)
                    .on_input(Message::Input)
            )
            .height(30.0),
        ]
        .into()
    }

    #[derive(Default)]
    struct FocusedIds(Vec<iced::advanced::widget::Id>);

    impl Operation for FocusedIds {
        fn focusable(
            &mut self,
            id: Option<&iced::advanced::widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn Focusable,
        ) {
            if state.is_focused()
                && let Some(id) = id
            {
                self.0.push(id.clone());
            }
        }

        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
            operate(self);
        }
    }

    #[test]
    fn reconciliation_is_atomic_and_keeps_keyed_active_identity() {
        let (mut state, mut items) = prepared(100);
        state.apply(
            DataGridEvent::FocusCell {
                row_index: 42,
                row: 42,
                column_index: 2,
                column: Column::Owner,
            },
            config(),
        );
        let selector = state.cell_selector(&42, &Column::Owner).unwrap();
        items.reverse();
        state
            .reconcile(&items, |item| item.id, &columns(), config())
            .unwrap();
        assert_eq!(state.active_cell().unwrap().row, 42);
        assert_eq!(state.rows.index_of(&42), Some(57));
        assert_eq!(state.cell_selector(&42, &Column::Owner), Some(selector));

        let snapshot = state.update_snapshot();
        assert_eq!(
            snapshot.column_semantic_id(
                &Column::Owner,
                snapshot.rows.semantic_local_id(&42).unwrap()
            ),
            state.column_semantic_id(&Column::Owner, state.rows.semantic_local_id(&42).unwrap())
        );
        let fork = state.fork("forked-test-grid");
        assert_ne!(fork.selector(), state.selector());
        assert_ne!(
            fork.column_semantic_id(&Column::Owner, fork.rows.semantic_local_id(&42).unwrap()),
            state.column_semantic_id(&Column::Owner, state.rows.semantic_local_id(&42).unwrap())
        );

        let duplicate = vec![
            Item {
                id: 1,
                name: "one".into(),
            },
            Item {
                id: 1,
                name: "duplicate".into(),
            },
        ];
        assert_eq!(
            state.reconcile(&duplicate, |item| item.id, &columns(), config()),
            Err(DataGridReconcileError::DuplicateRowKey(1))
        );
        assert_eq!(state.rows.index_of(&42), Some(57));
    }

    #[test]
    fn navigation_scroll_sort_and_caller_owned_edit_lifecycle_are_typed() {
        let (mut state, _) = prepared(100);
        state.apply(
            DataGridEvent::Navigate(DataGridNavigation::GridEnd),
            config(),
        );
        assert_eq!(
            state.active_cell(),
            Some(DataGridCellId {
                row: 99,
                column: Column::Owner
            })
        );
        assert!(state.rows.scroll_offset() > 0.0);
        assert!(state.horizontal.offset > 0.0);

        let sort = state.apply(DataGridEvent::SortRequested(Column::Status), config());
        assert_eq!(sort.sort_requested, Some(Column::Status));
        let ignored = state.apply(DataGridEvent::SortRequested(Column::Owner), config());
        assert_eq!(ignored.sort_requested, None);

        let cell = DataGridCellId {
            row: 99,
            column: Column::Owner,
        };
        let started = state.apply(DataGridEvent::BeginEdit(cell.clone()), config());
        assert_eq!(started.edit_started, Some(cell.clone()));
        assert_eq!(state.editing_cell(), Some(&cell));
        let committed = state.apply(DataGridEvent::CommitEdit(cell.clone()), config());
        assert_eq!(committed.edit_committed, Some(cell));
        assert_eq!(state.editing_cell(), None);
    }

    #[test]
    fn focusing_visible_rows_and_columns_preserves_the_vertical_offset() {
        let (mut state, _) = prepared(100);
        state.apply(
            DataGridEvent::Scrolled {
                offset_x: 0.0,
                offset_y: 400.0,
            },
            config(),
        );
        assert_eq!(state.rows.scroll_offset(), 400.0);

        state.apply(
            DataGridEvent::FocusCell {
                row_index: 22,
                row: 22,
                column_index: 2,
                column: Column::Owner,
            },
            config(),
        );
        assert_eq!(state.rows.scroll_offset(), 400.0);
        assert!(state.horizontal.offset > 0.0);

        state.apply(DataGridEvent::Navigate(DataGridNavigation::Left), config());
        assert_eq!(state.rows.scroll_offset(), 400.0);
        assert_eq!(state.active_cell().unwrap().row, 22);
    }

    #[test]
    fn plain_cell_click_claims_grid_focus_and_keeps_keyboard_editing() {
        let (mut state, items) = prepared(10);
        let grid_focus_id = state.rows.id().widget_id("focus");
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            plain_grid(&state, &items),
            Size::new(160.0, 124.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let point = Point::new(20.0, 34.0);
        let _ = ui.update(
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let focus = messages
            .iter()
            .find_map(|message| match message {
                Message::Grid(event @ DataGridEvent::FocusCell { .. }) => Some(event.clone()),
                _ => None,
            })
            .expect("plain cell click must focus the cell");
        let mut focused = FocusedIds::default();
        ui.operate(&renderer, &mut focused);
        assert_eq!(focused.0, [grid_focus_id]);

        let cache = ui.into_cache();
        state.apply(focus, config());
        let mut ui = UserInterface::build(
            plain_grid(&state, &items),
            Size::new(160.0, 124.0),
            cache,
            &mut renderer,
        );
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::ArrowRight)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            [Message::Grid(DataGridEvent::Navigate(
                DataGridNavigation::Right
            ))]
        );

        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::F2)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(
            messages,
            [Message::Grid(DataGridEvent::BeginEdit(DataGridCellId {
                row: 0,
                column: Column::Name,
            }))]
        );
    }

    #[test]
    fn native_editor_capture_exclusively_owns_focus_through_escape_and_tab() {
        let (mut state, items) = prepared(10);
        let cell = DataGridCellId {
            row: 0,
            column: Column::Name,
        };
        state.apply(
            DataGridEvent::FocusCell {
                row_index: 0,
                row: 0,
                column_index: 0,
                column: Column::Name,
            },
            config(),
        );
        state.apply(DataGridEvent::BeginEdit(cell), config());
        let editor_id: iced::advanced::widget::Id = "data-grid-editor".into();
        let after_id: iced::advanced::widget::Id = "after-data-grid".into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            editing_root(&state, &items, editor_id.clone(), after_id.clone()),
            Size::new(160.0, 160.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let point = Point::new(20.0, 34.0);
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
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::Grid(DataGridEvent::FocusCell { .. })))
        );
        let mut focused = FocusedIds::default();
        ui.operate(&renderer, &mut focused);
        assert_eq!(focused.0.as_slice(), std::slice::from_ref(&editor_id));

        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::Escape)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let mut focused = FocusedIds::default();
        ui.operate(&renderer, &mut focused);
        assert!(focused.0.is_empty());
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::ArrowDown)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::Grid(DataGridEvent::Navigate(_))))
        );

        messages.clear();
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
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::Tab)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        let mut focus_next: Box<dyn Operation> = Box::new(operation::focusable::focus_next::<()>());
        loop {
            ui.operate(&renderer, focus_next.as_mut());
            match focus_next.finish() {
                Outcome::Chain(next) => focus_next = next,
                Outcome::None | Outcome::Some(()) => break,
            }
        }
        let mut focused = FocusedIds::default();
        ui.operate(&renderer, &mut focused);
        assert_eq!(focused.0, [after_id]);
        messages.clear();
        let _ = ui.update(
            &[key_pressed(keyboard::key::Named::ArrowDown)],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::Grid(DataGridEvent::Navigate(_))))
        );
    }

    #[test]
    fn inspection_and_scroll_to_cell_use_logical_geometry() {
        let (mut state, _) = prepared(100_000);
        let target = DataGridCellId {
            row: 80_000,
            column: Column::Owner,
        };
        assert!(state.scroll_to_cell(&target, config()));
        let inspection = state.inspect(config());
        assert_eq!(inspection.logical_rows, 100_000);
        assert_eq!(inspection.logical_columns, 3);
        assert!(inspection.visible_rows.contains(&80_000));
        assert_eq!(
            inspection.mounted_cell_count,
            inspection.mounted_row_count * 3
        );
        assert_eq!(
            state.cell_rect(&80_000, &Column::Owner, config()),
            Some(Rectangle::new(
                [200.0, 1_600_000.0].into(),
                [100.0, 20.0].into()
            ))
        );
    }

    #[test]
    fn render_builds_only_mounted_rows_times_all_fixed_columns() {
        let items = items(100_000);
        let columns = (0_u8..16)
            .map(|column| DataGridColumn::new(column, format!("Column {column}"), 80.0))
            .collect::<Vec<_>>();
        let mut state = DataGridState::new(DataGridId::new("100k-by-16"));
        state
            .reconcile(&items, |item| item.id, &columns, config())
            .unwrap();
        state.apply(
            DataGridEvent::ViewportChanged {
                width: 640.0,
                height: 336.0,
            },
            config(),
        );
        let builds = Cell::new(0_usize);
        let element: Element<'_, (), Theme, iced_test::renderer::Renderer> = data_grid(
            &state,
            &items,
            config(),
            "Performance grid",
            |item| item.id,
            |item| item.name.clone(),
            |item, column| format!("{} {}", item.name, column.label()),
            |_| None,
            |_| iced::widget::text("header").into(),
            |_| {
                builds.set(builds.get() + 1);
                iced::widget::text("cell").into()
            },
            |_| (),
        );
        assert_eq!(
            builds.get(),
            state.mounted_rows(config()).len().saturating_mul(16)
        );
        drop(element);
    }

    #[test]
    fn accesskit_exports_mounted_grid_rows_headers_and_cells() {
        let (mut state, items) = prepared(100);
        state.apply(
            DataGridEvent::FocusCell {
                row_index: 2,
                row: 2,
                column_index: 1,
                column: Column::Status,
            },
            config(),
        );
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = data_grid(
            &state,
            &items,
            config(),
            "Orders grid",
            |item| item.id,
            |item| item.name.clone(),
            |item, column| format!("{} {}", item.name, column.label()),
            |column| (column == &Column::Status).then_some(accesskit::SortDirection::Ascending),
            |header| iced::widget::text(header.column.label()).into(),
            |cell| iced::widget::text(format!("{} {}", cell.row.id, cell.column.label())).into(),
            Message::Grid,
        );
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            element,
            Size::new(160.0, 124.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut operation = SnapshotOperation::<Message>::named("Data grid test");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(snapshot) = operation.finish() else {
            panic!("snapshot operation did not finish");
        };
        let (grid_id, grid) = snapshot
            .update
            .nodes
            .iter()
            .find(|(id, node)| *id != ROOT_ID && node.role() == Role::Grid)
            .expect("grid semantic node");
        assert_eq!(grid.label(), Some("Orders grid"));
        assert_eq!(grid.row_count(), Some(101));
        assert_eq!(grid.column_count(), Some(3));
        let headers = snapshot
            .update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::ColumnHeader)
            .collect::<Vec<_>>();
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[1].1.column_index(), Some(2));
        assert_eq!(
            headers[1].1.sort_direction(),
            Some(accesskit::SortDirection::Ascending)
        );
        let rows = snapshot
            .update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Row && node.row_index() != Some(1))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), state.mounted_rows(config()).len());
        assert_eq!(rows[2].1.is_selected(), Some(true));
        let cells = snapshot
            .update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Cell)
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), rows.len() * 3);
        let active = cells
            .iter()
            .find(|(_, node)| node.row_index() == Some(4) && node.column_index() == Some(2))
            .map(|(id, _)| *id)
            .expect("active cell");
        assert_eq!(grid.active_descendant(), Some(active));
        assert!(grid.supports_action(crate::Action::Focus));
        assert_ne!(*grid_id, ROOT_ID);
    }
}
