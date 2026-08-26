//! Private virtualization machinery shared by product widgets.
//!
//! This module deliberately owns no widget roles, input bindings, messages, or
//! public API. Lists, trees, grids, and log timelines share row windows,
//! stable keyed identity, and scroll synchronization while defining different
//! semantics.
//!
//! Row geometry is ONE model: a closed-form estimate plus sparse measured
//! corrections. A uniform list has no corrections, so its math is exactly the
//! fixed-height arithmetic it always was — no storage, no rebuild on append.
//! A measured list (a chat timeline) carries an index-sorted correction table
//! rebuilt by its owner as real layout heights land; every query stays
//! `O(log)` in rows and corrections. The window math never branches on which
//! kind of list it serves.
//!
//! Identity maps here are rebuilt over every row on each reconcile, so they
//! hash with `rustc-hash` rather than the standard SipHash. The keys are the
//! application's own row keys, never attacker-supplied input, so the
//! DoS resistance buys nothing and the reconcile pays for it on every row.

use rustc_hash::FxHashMap as HashMap;
use std::hash::Hash;
use std::ops::Range;
use std::sync::Arc;

/// One measured row: how far its height deviates from the estimate, plus the
/// running deviation total through this row for `O(log k)` prefix queries.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MeasuredEntry {
    index: usize,
    delta: f64,
    cumulative: f64,
}

/// Index-sorted measured-height corrections, empty for uniform lists.
///
/// Owners key real measurements by item identity and rebuild this table on
/// reconcile; the [`Arc`] keeps snapshot-per-update reducer clones free.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MeasuredHeights {
    entries: Arc<Vec<MeasuredEntry>>,
}

impl MeasuredHeights {
    /// Rebuilds the correction table from `(index, measured height)` pairs,
    /// which must be sorted by index with each height finite and
    /// non-negative. Rows not mentioned stay at the estimate.
    pub(crate) fn rebuild(estimate: f32, measured: impl IntoIterator<Item = (usize, f32)>) -> Self {
        let mut entries = Vec::new();
        let mut cumulative = 0.0;
        let mut previous = None;
        for (index, height) in measured {
            assert!(
                previous.is_none() || previous < Some(index),
                "measured heights must be sorted by unique row index"
            );
            previous = Some(index);
            let delta = f64::from(height.max(0.0)) - f64::from(estimate);
            if delta == 0.0 {
                continue;
            }
            cumulative += delta;
            entries.push(MeasuredEntry {
                index,
                delta,
                cumulative,
            });
        }
        Self {
            entries: Arc::new(entries),
        }
    }

    /// The summed deviation of measured rows before row `index`.
    fn cumulative_before(&self, index: usize) -> f64 {
        let position = self.entries.partition_point(|entry| entry.index < index);
        position
            .checked_sub(1)
            .map_or(0.0, |previous| self.entries[previous].cumulative)
    }

    /// Row `index`'s deviation from the estimate.
    fn delta(&self, index: usize) -> f64 {
        self.entries
            .binary_search_by_key(&index, |entry| entry.index)
            .map_or(0.0, |position| self.entries[position].delta)
    }
}

/// Row geometry for one query: the caller's estimate and item count combined
/// with the owner's measured corrections. Cheap to construct per call.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rows {
    estimate: f32,
    overscan: usize,
    item_count: usize,
    measured: MeasuredHeights,
}

impl Rows {
    pub(crate) fn new(
        estimate: f32,
        overscan: usize,
        item_count: usize,
        measured: &MeasuredHeights,
    ) -> Self {
        Self {
            estimate,
            overscan,
            item_count,
            measured: measured.clone(),
        }
    }

    pub(crate) const fn item_count(&self) -> usize {
        self.item_count
    }

    /// Row `index`'s top edge; `row_top(item_count)` is the total height.
    pub(crate) fn row_top(&self, index: usize) -> f32 {
        self.row_top_f64(index).min(f64::from(f32::MAX)) as f32
    }

    fn row_top_f64(&self, index: usize) -> f64 {
        index as f64 * f64::from(self.estimate) + self.measured.cumulative_before(index)
    }

    pub(crate) fn row_height(&self, index: usize) -> f32 {
        (f64::from(self.estimate) + self.measured.delta(index)) as f32
    }

    pub(crate) fn total_height(&self) -> f32 {
        self.row_top(self.item_count)
    }

    /// The row containing content-space `y` (top edge inclusive), or `None`
    /// past the end of the content — a press below the last row must not
    /// select it.
    pub(crate) fn index_at(&self, y: f32) -> Option<usize> {
        if self.item_count == 0 || y >= self.total_height() {
            return None;
        }
        Some(
            self.partition_tops(self.item_count, |top| top <= f64::from(y.max(0.0)))
                .saturating_sub(1),
        )
    }

    pub(crate) fn max_offset(&self, viewport_height: f32) -> f32 {
        (self.total_height() - viewport_height).max(0.0)
    }

    /// The count of rows in `0..limit` whose top satisfies the predicate,
    /// which must be monotone along the (non-decreasing) row tops.
    fn partition_tops(&self, limit: usize, predicate: impl Fn(f64) -> bool) -> usize {
        let mut low = 0;
        let mut high = limit;
        while low < high {
            let middle = low + (high - low) / 2;
            if predicate(self.row_top_f64(middle)) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    fn visible_range(&self, offset: f32, viewport_height: f32) -> Range<usize> {
        let count = self.item_count;
        if count == 0 || viewport_height == 0.0 {
            return 0..0;
        }
        let offset = offset.clamp(0.0, self.max_offset(viewport_height));
        let first = self
            .partition_tops(count, |top| top <= f64::from(offset))
            .saturating_sub(1);
        let bottom = f64::from(offset) + f64::from(viewport_height);
        let end = self.partition_tops(count, |top| top < bottom);
        first..end.min(count)
    }

    fn mounted_range(&self, offset: f32, viewport_height: f32) -> Range<usize> {
        let visible = self.visible_range(offset, viewport_height);
        if visible.is_empty() {
            return visible;
        }
        visible.start.saturating_sub(self.overscan)
            ..visible
                .end
                .saturating_add(self.overscan)
                .min(self.item_count)
    }

    pub(crate) fn window(&self, offset: f32, viewport_height: f32) -> RowWindow {
        let offset = offset.clamp(0.0, self.max_offset(viewport_height));
        let visible = self.visible_range(offset, viewport_height);
        let mounted = self.mounted_range(offset, viewport_height);
        let total_height = self.total_height();
        let top_spacer = self.row_top(mounted.start);
        let bottom_spacer = (total_height - self.row_top(mounted.end)).max(0.0);
        RowWindow {
            offset,
            visible,
            mounted,
            total_height,
            top_spacer,
            bottom_spacer,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowWindow {
    pub(crate) offset: f32,
    pub(crate) visible: Range<usize>,
    pub(crate) mounted: Range<usize>,
    pub(crate) total_height: f32,
    pub(crate) top_spacer: f32,
    pub(crate) bottom_spacer: f32,
}

#[derive(Debug)]
pub(crate) struct KeyedRows<Key> {
    entries: Arc<HashMap<Key, KeyedRow>>,
    next_local_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyedRow {
    local_id: u32,
    index: usize,
}

impl<Key> KeyedRows<Key> {
    pub(crate) fn new(first_local_id: u32) -> Self {
        Self {
            entries: Arc::new(HashMap::default()),
            next_local_id: first_local_id,
        }
    }

    pub(crate) fn snapshot(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            next_local_id: self.next_local_id,
        }
    }

    pub(crate) fn local_id(&self, key: &Key) -> Option<u32>
    where
        Key: Eq + Hash,
    {
        self.entries.get(key).map(|entry| entry.local_id)
    }

    pub(crate) fn index(&self, key: &Key) -> Option<usize>
    where
        Key: Eq + Hash,
    {
        self.entries.get(key).map(|entry| entry.index)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) const fn next_local_id(&self) -> u32 {
        self.next_local_id
    }

    #[cfg(test)]
    pub(crate) fn shares_ids_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.entries, &other.entries)
    }
}

impl<Key> KeyedRows<Key>
where
    Key: Clone + Eq + Hash,
{
    /// Atomically replaces the key set and returns the retained key's index.
    pub(crate) fn reconcile<T>(
        &mut self,
        items: &[T],
        key: impl Fn(&T) -> Key,
        retained: Option<&Key>,
        exhausted_message: &'static str,
    ) -> Result<Option<usize>, Key> {
        let mut entries = HashMap::with_capacity_and_hasher(items.len(), rustc_hash::FxBuildHasher);
        let mut retained_index = None;
        let mut next_local_id = self.next_local_id;
        for (index, item) in items.iter().enumerate() {
            let item_key = key(item);
            if entries.contains_key(&item_key) {
                return Err(item_key);
            }
            if retained == Some(&item_key) {
                retained_index = Some(index);
            }
            let local_id = self
                .entries
                .get(&item_key)
                .map(|entry| entry.local_id)
                .unwrap_or_else(|| {
                    let local_id = next_local_id;
                    next_local_id = next_local_id
                        .checked_add(1)
                        .unwrap_or_else(|| panic!("{exhausted_message}"));
                    local_id
                });
            entries.insert(item_key, KeyedRow { local_id, index });
        }
        self.entries = Arc::new(entries);
        self.next_local_id = next_local_id;
        Ok(retained_index)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RowScroll {
    offset: f32,
    viewport_height: f32,
    revision: u64,
}

impl RowScroll {
    pub(crate) const fn offset(self) -> f32 {
        self.offset
    }

    pub(crate) const fn viewport_height(self) -> f32 {
        self.viewport_height
    }

    pub(crate) const fn revision(self) -> u64 {
        self.revision
    }

    pub(crate) fn window(self, rows: &Rows) -> RowWindow {
        rows.window(self.offset, self.viewport_height)
    }

    pub(crate) fn visible_range(self, rows: &Rows) -> Range<usize> {
        rows.visible_range(self.offset, self.viewport_height)
    }

    pub(crate) fn mounted_range(self, rows: &Rows) -> Range<usize> {
        rows.mounted_range(self.offset, self.viewport_height)
    }

    pub(crate) fn reconcile(&mut self, rows: &Rows) -> bool {
        self.set_offset(self.offset, rows, true)
    }

    pub(crate) fn set_viewport_height(&mut self, height: f32, rows: &Rows) -> bool {
        self.viewport_height = if height.is_finite() {
            height.max(0.0)
        } else {
            0.0
        };
        self.set_offset(self.offset, rows, true)
    }

    pub(crate) fn set_native_offset(&mut self, offset: f32, rows: &Rows) -> bool {
        self.set_offset(offset, rows, false)
    }

    pub(crate) fn reveal(&mut self, index: usize, rows: &Rows) -> bool {
        let top = f64::from(rows.row_top(index));
        let bottom = top + f64::from(rows.row_height(index));
        let offset = f64::from(self.offset);
        let viewport_bottom = offset + f64::from(self.viewport_height);
        if top < offset {
            self.set_offset(top as f32, rows, true)
        } else if bottom > viewport_bottom {
            self.set_offset(
                (bottom - f64::from(self.viewport_height)) as f32,
                rows,
                true,
            )
        } else {
            false
        }
    }

    pub(crate) fn scroll_to_item(&mut self, index: usize, rows: &Rows) -> bool {
        if index >= rows.item_count() {
            return false;
        }
        let offset = rows
            .row_top(index)
            .min(rows.max_offset(self.viewport_height));
        self.set_offset(offset, rows, true)
    }

    pub(crate) fn scroll_to_end(&mut self, rows: &Rows) -> bool {
        self.set_offset(rows.max_offset(self.viewport_height), rows, true)
    }

    /// Programmatically restores an offset after geometry shifted under it
    /// (anchor correction), synchronizing the native scrollable.
    pub(crate) fn restore_offset(&mut self, offset: f32, rows: &Rows) -> bool {
        self.set_offset(offset, rows, true)
    }

    fn set_offset(&mut self, offset: f32, rows: &Rows, synchronize_native: bool) -> bool {
        let previous = self.offset;
        self.offset = if offset.is_finite() {
            offset.clamp(0.0, rows.max_offset(self.viewport_height))
        } else {
            0.0
        };
        let changed = self.offset != previous;
        if changed && synchronize_native {
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(estimate: f32, overscan: usize, item_count: usize) -> Rows {
        Rows::new(estimate, overscan, item_count, &MeasuredHeights::default())
    }

    /// The closed-form fixed-height math the sparse model generalizes.
    /// Uniform geometry must keep reproducing it exactly.
    fn reference_visible(
        row_height: f32,
        item_count: usize,
        offset: f32,
        viewport_height: f32,
    ) -> Range<usize> {
        if item_count == 0 || viewport_height == 0.0 {
            return 0..0;
        }
        let total = ((item_count as f64) * f64::from(row_height)).min(f64::from(f32::MAX)) as f32;
        let max_offset = (total - viewport_height).max(0.0);
        let offset = offset.clamp(0.0, max_offset);
        let first = (offset / row_height).floor() as usize;
        let end = ((offset + viewport_height) / row_height).ceil() as usize;
        first..end.min(item_count)
    }

    #[test]
    fn uniform_windows_match_the_fixed_height_contract() {
        for row_height in [0.5, 1.0, 20.0, 31.25] {
            for overscan in [0, 1, 2, 17] {
                for item_count in [0, 1, 2, 10, 100_000] {
                    let rows = uniform(row_height, overscan, item_count);
                    for viewport_height in [0.0, 0.25, row_height, row_height * 3.5, 100_000.0] {
                        for offset in [0.0, 0.25, row_height, 1_000_000.0, f32::MAX] {
                            let window = rows.window(offset, viewport_height);
                            let visible =
                                reference_visible(row_height, item_count, offset, viewport_height);
                            let mounted = if visible.is_empty() {
                                visible.clone()
                            } else {
                                visible.start.saturating_sub(overscan)
                                    ..visible.end.saturating_add(overscan).min(item_count)
                            };
                            assert_eq!(
                                window.visible, visible,
                                "h={row_height} n={item_count} off={offset} vh={viewport_height}"
                            );
                            assert_eq!(window.mounted, mounted);
                            assert!(window.top_spacer.is_finite());
                            assert!(window.bottom_spacer.is_finite());
                        }
                    }
                }
            }
        }
    }

    /// Hand-computed geometry with two corrections: row 3 measures 50 and
    /// row 10 measures 5 against a 20px estimate.
    #[test]
    fn measured_corrections_shift_tops_heights_and_windows() {
        let measured = MeasuredHeights::rebuild(20.0, [(3, 50.0), (10, 5.0)]);
        let rows = Rows::new(20.0, 0, 12, &measured);

        assert_eq!(rows.row_top(0), 0.0);
        assert_eq!(rows.row_top(3), 60.0);
        assert_eq!(rows.row_top(4), 110.0);
        assert_eq!(rows.row_height(3), 50.0);
        assert_eq!(rows.row_height(4), 20.0);
        assert_eq!(rows.row_top(10), 230.0);
        assert_eq!(rows.row_height(10), 5.0);
        assert_eq!(rows.row_top(11), 235.0);
        assert_eq!(rows.total_height(), 255.0);

        // A viewport over the tall row: rows 2..5 intersect 40..120.
        assert_eq!(rows.window(40.0, 80.0).visible, 2..5);
        // The tall row alone fills a viewport strictly inside it.
        assert_eq!(rows.window(65.0, 40.0).visible, 3..4);

        // Reveal from above scrolls the tall row's bottom edge into view.
        let mut scroll = RowScroll::default();
        scroll.set_viewport_height(40.0, &rows);
        assert!(scroll.reveal(3, &rows));
        assert_eq!(scroll.offset(), 70.0);

        // Queries ignore corrections beyond the item count.
        let shrunk = Rows::new(20.0, 0, 5, &measured);
        assert_eq!(shrunk.total_height(), 130.0);
    }

    /// Row tops stay monotone and consistent with per-row heights across a
    /// mixed measured/uniform table, including zero-height rows.
    #[test]
    fn measured_tops_are_monotone_and_sum_heights() {
        let measured = MeasuredHeights::rebuild(18.0, [(0, 44.0), (1, 0.0), (7, 91.5), (8, 3.25)]);
        let rows = Rows::new(18.0, 0, 10, &measured);
        let mut expected_top = 0.0_f64;
        for index in 0..10 {
            assert!((f64::from(rows.row_top(index)) - expected_top).abs() < 1e-6);
            expected_top += f64::from(rows.row_height(index));
        }
        assert!((f64::from(rows.total_height()) - expected_top).abs() < 1e-6);

        for index in 0..10 {
            if rows.row_height(index) > 0.0 {
                let inside = rows.row_top(index) + rows.row_height(index) * 0.5;
                assert_eq!(rows.window(inside, 1.0).visible.start, index);
            }
        }
    }

    #[test]
    fn keyed_reconciliation_is_atomic_and_retains_local_identity() {
        let mut keys = KeyedRows::new(2);
        assert_eq!(
            keys.reconcile(&[10, 20, 30], |key| *key, Some(&20), "exhausted"),
            Ok(Some(1))
        );
        let id_10 = keys.local_id(&10).unwrap();
        let id_20 = keys.local_id(&20).unwrap();
        let before = keys.snapshot();

        assert_eq!(
            keys.reconcile(&[30, 20, 10], |key| *key, Some(&20), "exhausted"),
            Ok(Some(1))
        );
        assert_eq!(keys.local_id(&10), Some(id_10));
        assert_eq!(keys.local_id(&20), Some(id_20));
        assert!(!keys.shares_ids_with(&before));

        let stable = keys.snapshot();
        assert_eq!(
            keys.reconcile(&[40, 40], |key| *key, None, "exhausted"),
            Err(40)
        );
        assert!(keys.shares_ids_with(&stable));
        assert_eq!(keys.next_local_id(), stable.next_local_id());
    }

    #[test]
    fn scroll_revision_changes_only_for_programmatic_native_sync() {
        let rows = uniform(20.0, 2, 100);
        let mut scroll = RowScroll::default();
        assert!(!scroll.set_viewport_height(100.0, &rows));
        assert_eq!(scroll.revision(), 0);
        assert!(scroll.set_native_offset(200.0, &rows));
        assert_eq!(scroll.offset(), 200.0);
        assert_eq!(scroll.revision(), 0);
        assert!(scroll.reveal(0, &rows));
        assert_eq!(scroll.offset(), 0.0);
        assert_eq!(scroll.revision(), 1);
        assert!(scroll.scroll_to_item(99, &rows));
        assert_eq!(scroll.offset(), 1_900.0);
        assert_eq!(scroll.revision(), 2);
        let shrunk = uniform(20.0, 2, 10);
        assert!(scroll.set_viewport_height(200.0, &shrunk));
        assert_eq!(scroll.offset(), 0.0);
        assert_eq!(scroll.revision(), 3);
    }
}
