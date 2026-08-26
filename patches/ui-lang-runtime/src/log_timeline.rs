//! Fixed-height virtualized log-stream state.
//!
//! [`LogTimelineState`] composes [`VirtualListState`] instead of implementing a
//! second list widget. It adds only the product semantics specific to an
//! append-only log: following the live edge, pausing after historical
//! navigation, counting unread appended rows, and explicitly resuming tail
//! follow. Rows remain owned by the caller.

use crate::{
    VirtualListConfig, VirtualListEvent, VirtualListId, VirtualListInspection, VirtualListOutcome,
    VirtualListReconcileError, VirtualListState, virtual_list,
};
use iced::Element;
use iced::advanced::text;
use iced::widget::{container, scrollable};
use std::fmt;
use std::hash::Hash;
use std::ops::Range;
use std::sync::Arc;

/// A strongly typed interaction handled by [`LogTimelineState::apply`].
#[derive(Debug, Clone, PartialEq)]
pub enum LogTimelineEvent<Key> {
    /// Selection, keyboard, viewport, or native-scroll input from the composed
    /// fixed-height list.
    List(VirtualListEvent<Key>),
    /// Explicitly returns to the live edge and clears the unread count.
    ResumeTail,
}

impl<Key> From<VirtualListEvent<Key>> for LogTimelineEvent<Key> {
    fn from(event: VirtualListEvent<Key>) -> Self {
        Self::List(event)
    }
}

/// A rejected log-stream update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogTimelineReconcileError<Key> {
    /// Two rows in the proposed stream have the same stable key.
    DuplicateKey(Key),
    /// Existing history was removed, reordered, or replaced. Use
    /// [`LogTimelineState::replace`] for an intentional stream reset.
    HistoryChanged {
        first_changed_index: usize,
        previous_count: usize,
        current_count: usize,
    },
}

impl<Key> fmt::Display for LogTimelineReconcileError<Key> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey(_) => formatter.write_str("log-timeline keys must be unique"),
            Self::HistoryChanged {
                first_changed_index,
                previous_count,
                current_count,
            } => write!(
                formatter,
                "log-timeline append changed history at index {first_changed_index} \
                 ({previous_count} previous rows, {current_count} current rows); use replace for an intentional reset"
            ),
        }
    }
}

impl<Key> std::error::Error for LogTimelineReconcileError<Key> where Key: fmt::Debug {}

impl<Key> From<VirtualListReconcileError<Key>> for LogTimelineReconcileError<Key> {
    fn from(error: VirtualListReconcileError<Key>) -> Self {
        match error {
            VirtualListReconcileError::DuplicateKey(key) => Self::DuplicateKey(key),
        }
    }
}

/// Result of applying one timeline interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTimelineOutcome<Key> {
    pub list: VirtualListOutcome<Key>,
    pub following_tail: bool,
    pub tail_follow_changed: bool,
    pub unread_count: usize,
}

/// Deterministic headless state for one timeline render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTimelineInspection {
    pub list: VirtualListInspection,
    pub following_tail: bool,
    pub at_live_edge: bool,
    pub unread_count: usize,
}

/// Retained state for an append-only, fixed-height log stream.
///
/// The state retains stable keys but never owns row payloads or row elements.
/// Selection, navigation, viewport geometry, virtualization, and AccessKit
/// semantics are delegated to the composed [`VirtualListState`].
pub struct LogTimelineState<Key> {
    list: VirtualListState<Key>,
    keys: Arc<[Key]>,
    following_tail: bool,
    unread_count: usize,
}

impl<Key> fmt::Debug for LogTimelineState<Key>
where
    Key: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LogTimelineState")
            .field("list", &self.list)
            .field("keys", &self.keys)
            .field("following_tail", &self.following_tail)
            .field("unread_count", &self.unread_count)
            .finish()
    }
}

impl<Key> LogTimelineState<Key>
where
    Key: Clone,
{
    /// Copies retained data for replacing the same mounted timeline.
    ///
    /// The old value must not remain mounted alongside this snapshot. Use
    /// [`Self::fork`] for two concurrently mounted timelines.
    pub fn update_snapshot(&self) -> Self {
        Self {
            list: self.list.update_snapshot(),
            keys: Arc::clone(&self.keys),
            following_tail: self.following_tail,
            unread_count: self.unread_count,
        }
    }

    /// Forks retained data under an independent native and semantic identity.
    pub fn fork(&self, new_logical_name: impl Into<String>) -> Self {
        Self {
            list: self.list.fork(new_logical_name),
            keys: Arc::clone(&self.keys),
            following_tail: self.following_tail,
            unread_count: self.unread_count,
        }
    }
}

impl<Key> LogTimelineState<Key>
where
    Key: Clone + Eq + Hash,
{
    /// Creates an empty timeline that follows the tail by default.
    pub fn new(id: VirtualListId) -> Self {
        Self {
            list: VirtualListState::new(id),
            keys: Arc::from([]),
            following_tail: true,
            unread_count: 0,
        }
    }

    pub fn id(&self) -> &VirtualListId {
        self.list.id()
    }

    pub const fn selected(&self) -> Option<&Key> {
        self.list.selected()
    }

    pub const fn selected_index(&self) -> Option<usize> {
        self.list.selected_index()
    }

    pub const fn scroll_offset(&self) -> f32 {
        self.list.scroll_offset()
    }

    pub const fn viewport_height(&self) -> f32 {
        self.list.viewport_height()
    }

    pub const fn is_following_tail(&self) -> bool {
        self.following_tail
    }

    pub const fn unread_count(&self) -> usize {
        self.unread_count
    }

    pub fn visible_range(&self, config: VirtualListConfig) -> Range<usize> {
        self.list.visible_range(self.keys.len(), config)
    }

    pub fn mounted_range(&self, config: VirtualListConfig) -> Range<usize> {
        self.list.mounted_range(self.keys.len(), config)
    }

    pub fn item_selector(&self, key: &Key) -> Option<String> {
        self.list.item_selector(key)
    }

    /// Reconciles an append-only stream and preserves stable row identity.
    ///
    /// Existing keys must remain an exact prefix. Appended rows move a
    /// following timeline to the live edge; while paused, they increment the
    /// saturating unread count. The update is atomic on validation failure.
    pub fn reconcile<T>(
        &mut self,
        rows: &[T],
        key: impl Fn(&T) -> Key,
        config: VirtualListConfig,
    ) -> Result<(), LogTimelineReconcileError<Key>> {
        let keys: Vec<Key> = rows.iter().map(key).collect();
        if let Some(first_changed_index) = first_history_change(&self.keys, &keys) {
            return Err(LogTimelineReconcileError::HistoryChanged {
                first_changed_index,
                previous_count: self.keys.len(),
                current_count: keys.len(),
            });
        }

        let appended = keys.len().saturating_sub(self.keys.len());
        self.list
            .reconcile(&keys, Clone::clone, config)
            .map_err(LogTimelineReconcileError::from)?;
        self.keys = keys.into();
        if self.following_tail {
            self.list.scroll_to_end(self.keys.len(), config);
        } else {
            self.unread_count = self.unread_count.saturating_add(appended);
        }
        Ok(())
    }

    /// Intentionally replaces the complete stream and resumes tail follow.
    ///
    /// Use this explicit boundary for log rotation, query changes, or clearing
    /// history; ordinary updates should use [`Self::reconcile`].
    pub fn replace<T>(
        &mut self,
        rows: &[T],
        key: impl Fn(&T) -> Key,
        config: VirtualListConfig,
    ) -> Result<(), LogTimelineReconcileError<Key>> {
        let keys: Vec<Key> = rows.iter().map(key).collect();
        self.list
            .reconcile(&keys, Clone::clone, config)
            .map_err(LogTimelineReconcileError::from)?;
        self.keys = keys.into();
        self.following_tail = true;
        self.unread_count = 0;
        self.list.scroll_to_end(self.keys.len(), config);
        Ok(())
    }

    /// Applies list input or an explicit tail-resume request.
    ///
    /// Any list interaction that leaves the viewport above the live edge
    /// pauses following. Scrolling back to the bottom does not silently resume;
    /// callers use [`LogTimelineEvent::ResumeTail`] for that transition.
    pub fn apply(
        &mut self,
        event: LogTimelineEvent<Key>,
        config: VirtualListConfig,
    ) -> LogTimelineOutcome<Key> {
        let previous_range = self.list.visible_range(self.keys.len(), config);
        let previous_offset = self.list.scroll_offset();
        let previous_follow = self.following_tail;

        let mut list = match event {
            LogTimelineEvent::List(event) => {
                let viewport_changed = matches!(event, VirtualListEvent::ViewportChanged { .. });
                let list = self.list.apply(event, &self.keys, Clone::clone, config);
                if self.following_tail {
                    if viewport_changed {
                        self.list.scroll_to_end(self.keys.len(), config);
                    } else if !self.at_live_edge(config) {
                        self.following_tail = false;
                    }
                }
                list
            }
            LogTimelineEvent::ResumeTail => {
                self.following_tail = true;
                self.unread_count = 0;
                self.list.scroll_to_end(self.keys.len(), config);
                VirtualListOutcome {
                    selected: self.list.selected().cloned(),
                    selection_changed: false,
                    visible_range_changed: false,
                    scroll_changed: false,
                }
            }
        };
        list.visible_range_changed =
            self.list.visible_range(self.keys.len(), config) != previous_range;
        list.scroll_changed = self.list.scroll_offset() != previous_offset;

        LogTimelineOutcome {
            list,
            following_tail: self.following_tail,
            tail_follow_changed: self.following_tail != previous_follow,
            unread_count: self.unread_count,
        }
    }

    /// Scrolls a stable key into view without changing selection.
    ///
    /// Moving away from the live edge pauses following; a paused timeline stays
    /// paused even when the requested key is at the tail.
    pub fn scroll_to_key(&mut self, key: &Key, config: VirtualListConfig) -> bool {
        let changed = self.list.scroll_to_key(key, self.keys.len(), config);
        if self.following_tail && !self.at_live_edge(config) {
            self.following_tail = false;
        }
        changed
    }

    pub fn inspect(&self, config: VirtualListConfig) -> LogTimelineInspection {
        LogTimelineInspection {
            list: self.list.inspect(self.keys.len(), config),
            following_tail: self.following_tail,
            at_live_edge: self.at_live_edge(config),
            unread_count: self.unread_count,
        }
    }

    fn at_live_edge(&self, config: VirtualListConfig) -> bool {
        let total_height = ((self.keys.len() as f64) * f64::from(config.row_height()))
            .min(f64::from(f32::MAX)) as f32;
        let maximum = (total_height - self.list.viewport_height()).max(0.0);
        (maximum - self.list.scroll_offset()).abs() <= f32::EPSILON * maximum.max(1.0)
    }
}

fn first_history_change<Key: Eq>(previous: &[Key], current: &[Key]) -> Option<usize> {
    previous
        .iter()
        .zip(current)
        .position(|(previous, current)| previous != current)
        .or_else(|| (current.len() < previous.len()).then_some(current.len()))
}

/// Builds a fixed-height virtualized log timeline.
///
/// Row mounting, stable keyed identity, pointer selection, keyboard navigation,
/// native scrolling, headless selectors, and AccessKit list semantics are
/// supplied by [`virtual_list`]. The caller owns `rows` and all row payloads.
#[allow(clippy::too_many_arguments)]
pub fn log_timeline<'a, T, Key, Message, Theme, Renderer>(
    state: &LogTimelineState<Key>,
    rows: &'a [T],
    config: VirtualListConfig,
    collection_label: impl Into<String>,
    key: impl Fn(&T) -> Key,
    label: impl Fn(&T) -> String,
    view: impl Fn(usize, &'a T, bool) -> Element<'a, Message, Theme, Renderer>,
    on_event: impl Fn(LogTimelineEvent<Key>) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Key: Clone + Eq + Hash + 'static,
    Message: Clone + 'static,
    Theme: container::Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + iced::advanced::Renderer + 'a,
{
    virtual_list(
        &state.list,
        rows,
        config,
        collection_label,
        key,
        label,
        view,
        move |event| on_event(LogTimelineEvent::List(event)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VirtualListNavigation;
    use iced::advanced::renderer;
    use iced::advanced::widget::operation::{self, Operation, Outcome};
    use iced::{Font, Pixels, Size, Theme};
    use iced_test::runtime::{UserInterface, user_interface};

    #[derive(Debug, Clone, PartialEq)]
    enum Message {
        Timeline(LogTimelineEvent<u64>),
    }

    fn config() -> VirtualListConfig {
        VirtualListConfig::new(20.0).unwrap().overscan(2)
    }

    fn measured_state(rows: &[u64]) -> LogTimelineState<u64> {
        let mut state = LogTimelineState::new(VirtualListId::new("runtime-log"));
        state.reconcile(rows, |row| *row, config()).unwrap();
        state.apply(
            LogTimelineEvent::List(VirtualListEvent::ViewportChanged { height: 100.0 }),
            config(),
        );
        state
    }

    fn outcome_flags(
        outcome: &LogTimelineOutcome<u64>,
    ) -> (Option<u64>, bool, bool, bool, bool, bool) {
        (
            outcome.list.selected,
            outcome.list.selection_changed,
            outcome.list.visible_range_changed,
            outcome.list.scroll_changed,
            outcome.following_tail,
            outcome.tail_follow_changed,
        )
    }

    #[test]
    fn follows_tail_by_default_and_counts_only_paused_appends() {
        let mut rows: Vec<u64> = (0..20).collect();
        let mut state = measured_state(&rows);
        assert!(state.is_following_tail());
        assert_eq!(state.scroll_offset(), 300.0);

        let scrolled = state.apply(
            LogTimelineEvent::List(VirtualListEvent::Scrolled { offset_y: 200.0 }),
            config(),
        );
        assert_eq!(
            outcome_flags(&scrolled),
            (None, false, true, true, false, true),
        );
        assert!(!state.is_following_tail());

        rows.extend(20..23);
        state.reconcile(&rows, |row| *row, config()).unwrap();
        assert_eq!(state.unread_count(), 3);
        assert_eq!(state.scroll_offset(), 200.0);

        let resumed = state.apply(LogTimelineEvent::ResumeTail, config());
        assert_eq!(
            outcome_flags(&resumed),
            (None, false, true, true, true, true),
        );
        assert_eq!(resumed.unread_count, 0);
        assert_eq!(state.scroll_offset(), 360.0);

        rows.push(23);
        state.reconcile(&rows, |row| *row, config()).unwrap();
        assert_eq!(state.scroll_offset(), 380.0);
        assert_eq!(state.unread_count(), 0);
    }

    #[test]
    fn scrolling_back_to_bottom_does_not_implicitly_resume() {
        let rows: Vec<u64> = (0..20).collect();
        let mut state = measured_state(&rows);
        state.apply(
            LogTimelineEvent::List(VirtualListEvent::Scrolled { offset_y: 200.0 }),
            config(),
        );
        state.apply(
            LogTimelineEvent::List(VirtualListEvent::Scrolled { offset_y: 300.0 }),
            config(),
        );
        assert!(!state.is_following_tail());
        assert!(state.inspect(config()).at_live_edge);
    }

    #[test]
    fn viewport_resize_preserves_tail_follow_at_the_exact_live_edge() {
        let rows: Vec<u64> = (0..20).collect();
        let mut state = measured_state(&rows);
        let resized = state.apply(
            LogTimelineEvent::List(VirtualListEvent::ViewportChanged { height: 60.0 }),
            config(),
        );
        assert_eq!(
            outcome_flags(&resized),
            (None, false, true, true, true, false),
        );
        assert!(state.is_following_tail());
        assert_eq!(state.scroll_offset(), 340.0);
        assert!(state.inspect(config()).at_live_edge);

        state.apply(
            LogTimelineEvent::List(VirtualListEvent::ViewportChanged { height: 10.0 }),
            config(),
        );
        assert!(state.is_following_tail());
        assert_eq!(state.scroll_offset(), 390.0);
        assert!(state.inspect(config()).at_live_edge);

        state.apply(
            LogTimelineEvent::List(VirtualListEvent::ViewportChanged { height: 140.0 }),
            config(),
        );
        assert!(state.is_following_tail());
        assert_eq!(state.scroll_offset(), 260.0);
        assert!(state.inspect(config()).at_live_edge);
    }

    #[test]
    fn selection_and_keyboard_navigation_are_delegated_to_virtual_list() {
        let rows: Vec<u64> = (0..20).collect();
        let mut state = measured_state(&rows);
        let selected = state.apply(
            LogTimelineEvent::List(VirtualListEvent::Select { index: 19, key: 19 }),
            config(),
        );
        assert_eq!(
            outcome_flags(&selected),
            (Some(19), true, false, false, true, false),
        );
        let outcome = state.apply(
            LogTimelineEvent::List(VirtualListEvent::Navigate(VirtualListNavigation::Up)),
            config(),
        );
        assert_eq!(outcome.list.selected, Some(18));
        assert_eq!(state.selected(), Some(&18));
        assert!(state.is_following_tail());

        state.apply(
            LogTimelineEvent::List(VirtualListEvent::Navigate(VirtualListNavigation::Home)),
            config(),
        );
        assert_eq!(state.selected(), Some(&0));
        assert!(!state.is_following_tail());
    }

    #[test]
    fn scroll_to_key_pauses_follow_and_keeps_selection() {
        let rows: Vec<u64> = (0..100).collect();
        let mut state = measured_state(&rows);
        assert!(state.scroll_to_key(&40, config()));
        assert_eq!(state.selected(), None);
        assert!(!state.is_following_tail());
        assert_eq!(state.visible_range(config()), 40..45);
    }

    #[test]
    fn stable_keys_retain_selectors_across_append() {
        let mut rows = vec!["alpha".to_owned(), "beta".to_owned()];
        let mut state = LogTimelineState::new(VirtualListId::new("stable-log"));
        state.reconcile(&rows, Clone::clone, config()).unwrap();
        let before = state.item_selector(&"alpha".to_owned()).unwrap();
        rows.push("gamma".to_owned());
        state.reconcile(&rows, Clone::clone, config()).unwrap();
        assert_eq!(state.item_selector(&"alpha".to_owned()), Some(before));
    }

    #[test]
    fn changed_history_is_rejected_atomically_and_replace_is_explicit() {
        let rows = [10_u64, 20, 30];
        let mut state = measured_state(&rows);
        let selector = state.item_selector(&20).unwrap();
        let error = state.reconcile(&[10, 25, 30], |row| *row, config());
        assert_eq!(
            error,
            Err(LogTimelineReconcileError::HistoryChanged {
                first_changed_index: 1,
                previous_count: 3,
                current_count: 3,
            })
        );
        assert_eq!(state.item_selector(&20), Some(selector));

        state.replace(&[100, 200], |row| *row, config()).unwrap();
        assert!(state.is_following_tail());
        assert_eq!(state.unread_count(), 0);
        assert_eq!(state.item_selector(&20), None);
    }

    #[test]
    fn duplicate_append_is_rejected_without_publishing_keys() {
        let mut state = measured_state(&[1_u64, 2]);
        assert_eq!(
            state.reconcile(&[1, 2, 2], |row| *row, config()),
            Err(LogTimelineReconcileError::DuplicateKey(2))
        );
        assert_eq!(state.inspect(config()).list.logical_items, 2);
    }

    #[test]
    fn headless_inspection_stays_bounded_for_one_hundred_thousand_rows() {
        let rows: Vec<u64> = (0..100_000).collect();
        let state = measured_state(&rows);
        let inspection = state.inspect(config());
        assert!(inspection.following_tail);
        assert!(inspection.at_live_edge);
        assert_eq!(inspection.unread_count, 0);
        assert_eq!(inspection.list.logical_items, 100_000);
        assert_eq!(inspection.list.visible_range, 99_995..100_000);
        assert_eq!(inspection.list.mounted_range, 99_993..100_000);
        assert_eq!(inspection.list.mounted_rows, 7);
        assert_eq!(inspection.list.child_slots, 9);
    }

    #[test]
    fn accessibility_is_the_composed_virtual_list_contract() {
        let rows: Vec<u64> = (0..100).collect();
        let mut state = measured_state(&rows);
        state.apply(
            LogTimelineEvent::List(VirtualListEvent::Select { index: 98, key: 98 }),
            config(),
        );
        let element: Element<'_, Message, Theme, iced_test::renderer::Renderer> = log_timeline(
            &state,
            &rows,
            config(),
            "Build output",
            |row| *row,
            |row| format!("Build line {row}"),
            |_, row, _| iced::widget::text(row).into(),
            Message::Timeline,
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
        let mut operation = crate::SnapshotOperation::<Message>::named("Log timeline test");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        let Outcome::Some(snapshot) = operation.finish() else {
            panic!("snapshot operation did not finish");
        };
        let (_, list) = snapshot
            .update
            .nodes
            .iter()
            .find(|(id, node)| *id != crate::ROOT_ID && node.role() == crate::Role::List)
            .expect("timeline list semantic node");
        assert_eq!(list.label(), Some("Build output"));
        assert_eq!(list.size_of_set(), Some(100));
        assert!(list.supports_action(crate::Action::Focus));
        let items = snapshot
            .update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == crate::Role::ListItem)
            .collect::<Vec<_>>();
        assert_eq!(items.len(), state.mounted_range(config()).len());
        let (selected_id, selected) = items
            .into_iter()
            .find(|(_, node)| node.label() == Some("Build line 98"))
            .expect("selected timeline row");
        assert_eq!(selected.position_in_set(), Some(99));
        assert_eq!(selected.size_of_set(), Some(100));
        assert_eq!(selected.is_selected(), Some(true));
        assert_eq!(list.active_descendant(), Some(*selected_id));
    }
}
