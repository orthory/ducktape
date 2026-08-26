//! Runtime support for generated Ice applications.

mod boot;
pub use boot::boot_dispatch;
mod dashed_border;
#[cfg(feature = "data-grid")]
mod data_grid;
#[doc(hidden)]
pub mod dev;
mod dynamic_themer;
mod flex;
mod hover_reveal;
mod log_timeline;
mod memo_lazy;
mod press_area;
mod qr;
mod resize_handle;
pub mod rev;
#[cfg(feature = "full-runtime")]
pub mod rich_text_editor;
mod scroll_anchor;
mod secret;
mod selectable_text;
pub mod selection;
mod stack_relief;
pub mod template;
#[doc(hidden)]
#[cfg(feature = "test-runtime")]
pub mod testing;
#[cfg(not(feature = "test-runtime"))]
#[path = "testing_minimal.rs"]
pub mod testing;
pub mod tray;
mod tree_view;
mod virtual_children;
mod virtual_list;
mod virtual_scroll;
mod virtualization;
mod zstack;

pub use dashed_border::*;
#[cfg(feature = "data-grid")]
pub use data_grid::*;
pub use dynamic_themer::*;
pub use flex::*;
pub use hover_reveal::*;
pub use log_timeline::*;
pub use memo_lazy::*;
pub use press_area::*;
pub use qr::*;
pub use resize_handle::*;
#[cfg(feature = "full-runtime")]
pub use rich_text_editor::{ContentVersion, EditorChange, RichTextEditor};
pub use scroll_anchor::*;
pub use secret::{Secret, SecretStore};
pub use selectable_text::*;
pub use stack_relief::*;
pub use tree_view::*;
pub use virtual_children::*;
pub use virtual_list::*;
pub use virtual_scroll::*;
pub use zstack::*;

#[cfg(feature = "data-grid")]
pub use accesskit::SortDirection as AccessibilitySortDirection;
pub use accesskit::{Action, ActionRequest, Node, NodeId, Role, Toggled, TreeUpdate};

use accesskit::{Rect, Tree, TreeId};
use iced::advanced::widget::operation::{Focusable, Operation, Outcome, Scrollable, TextInput};
use iced::advanced::widget::{self, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::keyboard::{self, key};
use iced::{Element, Event, Length, Padding, Rectangle, Size, Subscription, Task, Vector};
use std::any::Any;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

const ROOT_ID: NodeId = NodeId(0);

/// Stores state for component scopes that only live while mounted.
///
/// Pruning happens one pass late, at the START of the next render, and that is
/// deliberate: `view` returning is not the end of building the tree. A
/// `responsive` (and any other deferred builder) constructs its subtree during
/// layout, so components under one call `mount` after `finish_render` has
/// already run. Pruning there saw an empty active set, dropped their state, and
/// the next pass built it again from scratch — which for ordinary state is
/// invisible, and for an animation means restarting the motion every frame.
/// Holding the root until the next `begin_render` lets the active set collect
/// the whole pass, deferred builders included.
#[derive(Debug)]
pub struct MountedComponentState<T> {
    values: RefCell<HashMap<String, T>>,
    active: RefCell<HashSet<String>>,
    /// The root whose finished render is still waiting to be pruned.
    pending: RefCell<Option<String>>,
    next_generation: Cell<u64>,
    /// Scopes whose `boot` already fired; pruned with their values, so an
    /// instance that leaves and comes back boots again.
    booted: RefCell<HashSet<String>>,
}

impl<T> Default for MountedComponentState<T> {
    fn default() -> Self {
        Self {
            values: RefCell::new(HashMap::new()),
            active: RefCell::new(HashSet::new()),
            pending: RefCell::new(None),
            next_generation: Cell::new(0),
            booted: RefCell::new(HashSet::new()),
        }
    }
}

impl<T> MountedComponentState<T> {
    /// Prunes the previous render's scopes, then starts tracking a new one.
    pub fn begin_render(&self) {
        if let Some(root) = self.pending.borrow_mut().take() {
            self.prune(&root);
        }
        self.active.borrow_mut().clear();
    }

    /// Marks a component scope as present in the current render.
    pub fn mount(&self, scope: String) {
        self.active.borrow_mut().insert(scope);
    }

    /// Marks a component scope as present, answering whether this is the
    /// instance's FIRST sighting — the caller queues the boot message it
    /// builds from the render site's prop values. The mark is pruned with
    /// the instance, so a scope that leaves the tree and comes back boots
    /// again.
    pub fn mount_boot(&self, scope: String) -> bool {
        let first = self.booted.borrow_mut().insert(scope.clone());
        self.active.borrow_mut().insert(scope);
        first
    }

    /// Records that `root` finished rendering. Scopes under it that never
    /// mounted are dropped at the next [`Self::begin_render`].
    pub fn finish_render(&self, root: &str) {
        self.pending.borrow_mut().replace(root.to_owned());
    }

    fn prune(&self, root: &str) {
        let active = self.active.borrow();
        let survives = |scope: &str| {
            let suffix = scope.strip_prefix(root);
            !suffix.is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('/'))
                || active.contains(scope)
        };
        self.values.borrow_mut().retain(|scope, _| survives(scope));
        self.booted.borrow_mut().retain(|scope| survives(scope));
    }

    /// Every live instance scope: the ones sighted by the current render
    /// pass plus the ones holding materialized state. A freshly mounted
    /// instance has no `values` entry until its first delivered event, so
    /// a harness that just rendered must see it HERE.
    pub fn scopes(&self) -> Vec<String> {
        let values = self.values.borrow();
        let active = self.active.borrow();
        let mut scopes: Vec<String> = values.keys().cloned().collect();
        for scope in active.iter() {
            if !values.contains_key(scope) {
                scopes.push(scope.clone());
            }
        }
        scopes
    }

    /// Borrows all mounted scope values.
    pub fn values(&self) -> Ref<'_, HashMap<String, T>> {
        self.values.borrow()
    }

    /// Mutably borrows all mounted scope values.
    pub fn values_mut(&self) -> RefMut<'_, HashMap<String, T>> {
        self.values.borrow_mut()
    }

    /// Returns a render-lifetime-stable generation for async completion filters.
    pub fn next_generation(&self) -> u64 {
        let next = self.next_generation.get().wrapping_add(1);
        self.next_generation.set(next);
        next
    }
}

/// A deterministic identity for one semantic node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StableId(NodeId);

impl StableId {
    /// Hashes a compiler-owned key with a stable FNV-1a hash.
    pub fn new(key: impl AsRef<str>) -> Self {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in key.as_ref().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self(NodeId(if hash == 0 { 1 } else { hash }))
    }

    pub(crate) const fn from_node_id(node_id: NodeId) -> Self {
        Self(node_id)
    }

    /// Returns the AccessKit node identity.
    pub const fn node_id(self) -> NodeId {
        self.0
    }

    /// Returns the corresponding Iced widget identity used for focus actions.
    pub fn widget_id(self) -> widget::Id {
        format!("__ice_accessibility/{}", self.0.0).into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusBehavior {
    None,
    Wrapper,
    Descendant,
}

#[derive(Clone)]
struct SemanticSnapshot {
    // This metadata stays independent of `Message` so test inspection can
    // retain it through `Element::map`, whose wrapper changes the message type
    // without changing the accessible widget's stored state.
    id: StableId,
    logical_id: Option<String>,
    source: Option<testing::Location>,
    role: Role,
    label: Option<String>,
    description: Option<String>,
    value: Option<String>,
    checked: Option<bool>,
    selected: Option<bool>,
    expanded: Option<bool>,
    level: Option<usize>,
    row_count: Option<usize>,
    column_count: Option<usize>,
    row_index: Option<usize>,
    column_index: Option<usize>,
    sort_direction: Option<accesskit::SortDirection>,
    position_in_set: Option<usize>,
    size_of_set: Option<usize>,
    active_descendant: Option<StableId>,
    disabled: bool,
    focus: FocusBehavior,
    focused: bool,
    supports_activate: bool,
}

#[derive(Clone)]
struct Semantics<Message> {
    snapshot: SemanticSnapshot,
    /// The id `operate` hands to its `custom` reader, when the caller named
    /// one. Left unset otherwise: the id derived from `snapshot.id` is a
    /// formatted `String` that only `operate` ever reads, so deriving it there
    /// costs one allocation per pass rather than one per node per build plus
    /// another for every `diff` that clones the node.
    focus_id: Option<widget::Id>,
    activate: Option<Message>,
}

impl<Message> std::ops::Deref for Semantics<Message> {
    type Target = SemanticSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl<Message> std::ops::DerefMut for Semantics<Message> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.snapshot
    }
}

impl<Message> Semantics<Message> {
    fn new(id: StableId, role: Role) -> Self {
        let focus = match role {
            Role::Button | Role::DefaultButton | Role::CheckBox | Role::Switch => {
                FocusBehavior::Wrapper
            }
            Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::PasswordInput
            | Role::Slider
            | Role::ComboBox => FocusBehavior::Descendant,
            _ => FocusBehavior::None,
        };

        Self {
            snapshot: SemanticSnapshot {
                id,
                logical_id: None,
                source: None,
                role,
                label: None,
                description: None,
                value: None,
                checked: None,
                selected: None,
                expanded: None,
                level: None,
                row_count: None,
                column_count: None,
                row_index: None,
                column_index: None,
                sort_direction: None,
                position_in_set: None,
                size_of_set: None,
                active_descendant: None,
                disabled: false,
                focus,
                focused: false,
                supports_activate: false,
            },
            focus_id: None,
            activate: None,
        }
    }
}

struct SemanticState<Message> {
    semantics: Semantics<Message>,
    focus_visible: bool,
}

impl<Message> Focusable for SemanticState<Message> {
    fn is_focused(&self) -> bool {
        self.semantics.focused
    }

    fn focus(&mut self) {
        self.semantics.focused = true;
        self.focus_visible = true;
    }

    fn unfocus(&mut self) {
        self.semantics.focused = false;
        self.focus_visible = false;
    }
}

struct SemanticEnd;

struct WithoutFocus<'a> {
    inner: &'a mut dyn Operation,
}

impl Operation for WithoutFocus<'_> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        self.inner.traverse(&mut |inner| {
            let mut filtered = WithoutFocus { inner };
            operate(&mut filtered);
        });
    }

    fn container(&mut self, id: Option<&widget::Id>, bounds: Rectangle) {
        self.inner.container(id, bounds);
    }

    fn scrollable(
        &mut self,
        id: Option<&widget::Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        self.inner
            .scrollable(id, bounds, content_bounds, translation, state);
    }

    fn focusable(
        &mut self,
        _id: Option<&widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn Focusable,
    ) {
        state.unfocus();
    }

    fn text_input(
        &mut self,
        id: Option<&widget::Id>,
        bounds: Rectangle,
        state: &mut dyn TextInput,
    ) {
        self.inner.text_input(id, bounds, state);
    }

    fn text(&mut self, id: Option<&widget::Id>, bounds: Rectangle, text: &str) {
        self.inner.text(id, bounds, text);
    }

    fn custom(&mut self, id: Option<&widget::Id>, bounds: Rectangle, state: &mut dyn Any) {
        self.inner.custom(id, bounds, state);
    }

    fn finish(&self) -> Outcome<()> {
        self.inner.finish()
    }
}

/// The ink channel through which a generated button hands its
/// status-resolved text color to `color=inherit` svg content.
///
/// iced's inherited-ink channel (`renderer::Style.text_color`) reaches text
/// widgets but never an svg's style closure, so the generated button block
/// binds one of these cells instead: the button's style closure writes its
/// FINAL `text_color` (disabled pass included), and iced's button draw
/// resolves that closure before drawing content, so an svg style closure
/// reading the cell during the same draw always sees this frame's status ink.
pub type ButtonInk = std::rc::Rc<std::cell::Cell<iced::Color>>;

/// Creates the ink cell a generated button shares with its `color=inherit`
/// svg content. The initial value is never drawn: the button's style closure
/// overwrites it before any reader draws.
pub fn button_ink() -> ButtonInk {
    std::rc::Rc::new(std::cell::Cell::new(iced::Color::TRANSPARENT))
}

/// Wraps an Iced widget with semantics owned by Ice.
pub struct Accessible<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    semantics: Semantics<Message>,
    focus_ring: Option<FocusRing>,
}

/// Recipe-owned looks for the wrapper's keyboard focus ring.
///
/// The ring's visibility is not configurable: it always keys on the wrapper's
/// focus-visible state, so a pointer press never wears it and keyboard
/// traversal always does. Only its paint is the caller's.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FocusRing {
    color: iced::Color,
    radius: f32,
}

/// Creates an accessible wrapper around an Iced widget.
pub fn accessible<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    id: StableId,
    role: Role,
) -> Accessible<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    Accessible {
        content: content.into(),
        semantics: Semantics::new(id, role),
        focus_ring: None,
    }
}

impl<'a, Message, Theme, Renderer> Accessible<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// Retains the logical Ice selector used to build this semantic node.
    #[doc(hidden)]
    pub fn logical_id(mut self, id: impl Into<String>) -> Self {
        self.semantics.logical_id = Some(id.into());
        self.semantics.source = testing::current_render_source();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.semantics.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.semantics.description = Some(description.into());
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.semantics.value = Some(value.into());
        self
    }

    pub fn value_maybe(mut self, value: Option<String>) -> Self {
        self.semantics.value = value;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.semantics.checked = Some(checked);
        self
    }

    /// Marks whether this semantic item is selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.semantics.selected = Some(selected);
        self
    }

    /// Marks a hierarchical item as expanded or collapsed.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.semantics.expanded = Some(expanded);
        self
    }

    /// Sets the one-based level of an item in a hierarchical collection.
    pub fn level(mut self, level: usize) -> Self {
        self.semantics.level = Some(level);
        self
    }

    /// Sets the total logical row count of a table or grid.
    pub fn row_count(mut self, count: usize) -> Self {
        self.semantics.row_count = Some(count);
        self
    }

    /// Sets the total logical column count of a table or grid.
    pub fn column_count(mut self, count: usize) -> Self {
        self.semantics.column_count = Some(count);
        self
    }

    /// Sets the one-based logical row index of a row or cell.
    pub fn row_index(mut self, index: usize) -> Self {
        self.semantics.row_index = Some(index);
        self
    }

    /// Sets the one-based logical column index of a header or cell.
    pub fn column_index(mut self, index: usize) -> Self {
        self.semantics.column_index = Some(index);
        self
    }

    /// Sets the current sort direction of a sortable column header.
    pub fn sort_direction(mut self, direction: accesskit::SortDirection) -> Self {
        self.semantics.sort_direction = Some(direction);
        self
    }

    /// Sets this item's one-based position in its logical collection.
    pub fn position_in_set(mut self, position: usize) -> Self {
        self.semantics.position_in_set = Some(position);
        self
    }

    /// Sets the total number of items in this semantic collection.
    pub fn size_of_set(mut self, size: usize) -> Self {
        self.semantics.size_of_set = Some(size);
        self
    }

    /// Identifies the currently active semantic descendant of this collection.
    pub fn active_descendant(mut self, id: StableId) -> Self {
        self.semantics.active_descendant = Some(id);
        self
    }

    /// Identifies the active descendant when the item is currently mounted.
    pub fn active_descendant_maybe(mut self, id: Option<StableId>) -> Self {
        self.semantics.active_descendant = id;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.semantics.disabled = disabled;
        self
    }

    pub fn focus_id(mut self, id: impl Into<widget::Id>) -> Self {
        self.semantics.focus_id = Some(id.into());
        self
    }

    /// Maps focus from a native focusable descendant onto this semantic node.
    pub fn focus_descendant(mut self) -> Self {
        self.semantics.focus = FocusBehavior::Descendant;
        self
    }

    pub fn on_activate(mut self, message: Message) -> Self {
        self.semantics.supports_activate = true;
        self.semantics.activate = Some(message);
        self
    }

    pub fn on_activate_maybe(mut self, message: Option<Message>) -> Self {
        self.semantics.supports_activate = message.is_some();
        self.semantics.activate = message;
        self
    }

    /// Styles the keyboard focus ring this wrapper draws when focus is
    /// visible. The default ring uses the ambient text color with a
    /// three-pixel radius; a styled ring keeps the two-pixel stroke and takes
    /// the given color and corner radius instead.
    pub fn focus_ring(mut self, color: iced::Color, radius: f32) -> Self {
        self.focus_ring = Some(FocusRing { color, radius });
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Accessible<'_, Message, Theme, Renderer>
where
    Message: Clone + 'static,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SemanticState<Message>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SemanticState {
            semantics: self.semantics.clone(),
            focus_visible: false,
        })
    }

    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let state = tree.state.downcast_mut::<SemanticState<Message>>();
        let focused = state.semantics.focused;
        state.semantics = self.semantics.clone();
        state.semantics.focused = focused;
        if state.semantics.disabled {
            state.semantics.focused = false;
            state.focus_visible = false;
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
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<SemanticState<Message>>();
        let focus_id = state
            .semantics
            .focus_id
            .clone()
            .unwrap_or_else(|| state.semantics.id.widget_id());
        if state.semantics.disabled {
            state.semantics.focused = false;
            state.focus_visible = false;
        }
        operation.custom(None, layout.bounds(), &mut state.semantics.snapshot);
        operation.custom(Some(&focus_id), layout.bounds(), state);

        if !state.semantics.disabled && state.semantics.focus == FocusBehavior::Wrapper {
            operation.focusable(Some(&focus_id), layout.bounds(), state);
        }

        if state.semantics.focus == FocusBehavior::Wrapper
            || (state.semantics.disabled && state.semantics.focus == FocusBehavior::Descendant)
        {
            operation.traverse(&mut |operation| {
                let mut operation = WithoutFocus { inner: operation };
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    layout,
                    renderer,
                    &mut operation,
                );
            });
        } else {
            operation.traverse(&mut |operation| {
                self.content.as_widget_mut().operate(
                    &mut tree.children[0],
                    layout,
                    renderer,
                    operation,
                );
            });
        }

        operation.custom(None, layout.bounds(), &mut SemanticEnd);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SemanticState<Message>>();
        let wrapper_focus = state.semantics.focus == FocusBehavior::Wrapper;

        if wrapper_focus && !state.semantics.disabled {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                    state.semantics.focused = cursor.is_over(layout.bounds());
                    state.focus_visible = false;
                }
                _ => {}
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

        if shell.is_event_captured() || state.semantics.disabled || !state.semantics.focused {
            return;
        }

        let Event::Keyboard(keyboard::Event::KeyPressed { key, repeat, .. }) = event else {
            return;
        };

        // The web's `:focus-visible` heuristic: keyboard interaction with a
        // pointer-focused control makes its focus visible again.
        if wrapper_focus && !state.focus_visible {
            state.focus_visible = true;
            shell.request_redraw();
        }

        if *repeat {
            return;
        }

        let activates = match state.semantics.role {
            Role::Button | Role::DefaultButton => matches!(
                key,
                keyboard::Key::Named(key::Named::Enter | key::Named::Space)
            ),
            Role::CheckBox | Role::Switch => {
                matches!(key, keyboard::Key::Named(key::Named::Space))
            }
            _ => false,
        };

        if activates && let Some(message) = state.semantics.activate.clone() {
            shell.publish(message);
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
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
        let state = tree.state.downcast_ref::<SemanticState<Message>>();
        if state.focus_visible && !state.semantics.disabled {
            let ring = self.focus_ring.unwrap_or(FocusRing {
                color: style.text_color,
                radius: 3.0,
            });
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: iced::Border {
                        color: ring.color,
                        width: 2.0,
                        radius: ring.radius.into(),
                    },
                    ..renderer::Quad::default()
                },
                iced::Color::TRANSPARENT,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
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

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Accessible<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'static,
    Renderer: iced::advanced::Renderer + 'a,
    Theme: 'a,
{
    fn from(accessible: Accessible<'a, Message, Theme, Renderer>) -> Self {
        Self::new(accessible)
    }
}

/// Root wrapper that turns Tab and Shift+Tab into Ice focus operations.
pub struct Navigation<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    next: Message,
    previous: Message,
}

pub fn navigation<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    next: Message,
    previous: Message,
) -> Navigation<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    Navigation {
        content: content.into(),
        next,
        previous,
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Navigation<'_, Message, Theme, Renderer>
where
    Message: Clone + 'static,
    Renderer: iced::advanced::Renderer,
{
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut widget::Tree) {
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
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
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
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let tab = if let Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(key::Named::Tab),
            modifiers,
            repeat: false,
            ..
        }) = event
        {
            (!modifiers.control() && !modifiers.alt() && !modifiers.logo())
                .then(|| modifiers.shift())
        } else {
            None
        };

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

        if let Some(previous) = tab
            && !shell.is_event_captured()
        {
            shell.publish(if previous {
                self.previous.clone()
            } else {
                self.next.clone()
            });
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
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

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
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

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Navigation<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'static,
    Renderer: iced::advanced::Renderer + 'a,
    Theme: 'a,
{
    fn from(navigation: Navigation<'a, Message, Theme, Renderer>) -> Self {
        Self::new(navigation)
    }
}

/// Content that keyboard focus cannot enter.
///
/// A modal layer captures the pointer with a backdrop, but nothing about
/// `Stack` confines the keyboard: [`iced::widget::Stack::operate`] visits every
/// layer unconditionally, so Tab — which Ice routes through the very same
/// `operate` call — walks straight into the inputs sitting invisibly behind the
/// dimmed backdrop, and the next keystroke lands somewhere the user cannot see.
///
/// Wrapping the covered layer in this keeps focus operations out of it: the
/// subtree is traversed with [`WithoutFocus`], so counting, moving and
/// restoring focus all behave as if it held no focusable widget at all, and any
/// focus it still held when the layer opened is dropped on the first operation.
/// Everything else an operation asks for — accessibility semantics, scroll
/// position, text — still answers, because covering a layer hides it from the
/// keyboard, not from the machinery that reports what is on screen.
///
/// Keyboard events stop here too. Denying focus is not enough on its own: a
/// widget that was already focused when the layer opened keeps its focus until
/// something operates on the tree, and until then every keystroke would still
/// be delivered to it. Nothing above is skipped — the root Tab handler and the
/// layer itself are both outside this wrapper — and every other kind of event
/// still passes, so animations and window changes behind the layer carry on.
pub struct FocusBarrier<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

/// Creates a [`FocusBarrier`] around content a modal layer covers.
pub fn focus_barrier<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> FocusBarrier<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    FocusBarrier {
        content: content.into(),
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FocusBarrier<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    // Transparent to the widget tree, exactly as `iced::widget::opaque` is:
    // the barrier appears and disappears as the layer above it opens and
    // shuts, and a wrapper that owned a tree node of its own would rebuild
    // everything under it on each transition — dropping the scroll offsets,
    // selections and cursors of the very content it is protecting.
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            tree,
            layout,
            renderer,
            &mut WithoutFocus { inner: operation },
        );
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if matches!(event, Event::Keyboard(_)) {
            return;
        }
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<FocusBarrier<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: iced::advanced::Renderer + 'a,
    Theme: 'a,
{
    fn from(barrier: FocusBarrier<'a, Message, Theme, Renderer>) -> Self {
        Self::new(barrier)
    }
}

#[derive(Clone)]
struct ActionTarget<Message> {
    activate: Option<Message>,
    focus: Option<SemanticFocus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SemanticFocus {
    base: StableId,
    occurrence: u64,
}

/// A complete AccessKit tree and the action map for the same UI frame.
#[derive(Clone)]
pub struct Snapshot<Message> {
    pub update: TreeUpdate,
    actions: HashMap<NodeId, ActionTarget<Message>>,
}

impl<Message> fmt::Debug for Snapshot<Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Snapshot")
            .field("update", &self.update)
            .field("action_count", &self.actions.len())
            .finish()
    }
}

impl<Message: Clone + Send + 'static> Snapshot<Message> {
    pub fn dispatch(&self, request: ActionRequest) -> Task<Message> {
        if request.target_tree != TreeId::ROOT {
            return Task::none();
        }
        let Some(target) = self.actions.get(&request.target_node) else {
            return Task::none();
        };
        match request.action {
            Action::Click => target.activate.clone().map_or_else(Task::none, Task::done),
            Action::Focus => target.focus.map_or_else(Task::none, focus_semantic),
            _ => Task::none(),
        }
    }
}

fn duplicate_node_id(base: NodeId, occurrence: u64) -> NodeId {
    let mut value = base
        .0
        .wrapping_add(occurrence.wrapping_mul(0x9e3779b97f4a7c15));
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    NodeId(if value == 0 { 1 } else { value })
}

fn disambiguate_semantic_id(
    base: StableId,
    occurrences: &mut HashMap<NodeId, u64>,
    used_ids: &mut HashSet<NodeId>,
) -> (NodeId, SemanticFocus) {
    let next_occurrence = occurrences.entry(base.node_id()).or_default();
    let mut occurrence = *next_occurrence;
    let mut id = if occurrence == 0 {
        base.node_id()
    } else {
        duplicate_node_id(base.node_id(), occurrence)
    };
    while used_ids.contains(&id) {
        occurrence += 1;
        id = duplicate_node_id(base.node_id(), occurrence);
    }
    *next_occurrence = occurrence + 1;
    used_ids.insert(id);
    (id, SemanticFocus { base, occurrence })
}

struct FocusOperation<Message> {
    target: SemanticFocus,
    occurrences: HashMap<NodeId, u64>,
    used_ids: HashSet<NodeId>,
    frames: Vec<Option<(SemanticFocus, FocusBehavior, bool)>>,
    marker: std::marker::PhantomData<Message>,
}

impl<Message: Send + 'static> Operation<()> for FocusOperation<Message> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
        operate(self);
    }

    fn custom(&mut self, _id: Option<&widget::Id>, _bounds: Rectangle, state: &mut dyn Any) {
        if state.downcast_mut::<SemanticEnd>().is_some() {
            self.frames.pop();
            return;
        }
        let Some(state) = state.downcast_mut::<SemanticState<Message>>() else {
            return;
        };
        if self.frames.iter().flatten().any(|(_, _, atomic)| *atomic) {
            self.frames.push(None);
            return;
        }
        let (_, current) = disambiguate_semantic_id(
            state.semantics.id,
            &mut self.occurrences,
            &mut self.used_ids,
        );
        self.frames.push(Some((
            current,
            state.semantics.focus,
            atomic_role(state.semantics.role),
        )));

        if state.semantics.focus == FocusBehavior::Wrapper {
            if !state.semantics.disabled && current == self.target {
                state.focus();
            } else {
                state.unfocus();
            }
        }
    }

    fn focusable(
        &mut self,
        _id: Option<&widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn Focusable,
    ) {
        if self
            .frames
            .iter()
            .rev()
            .flatten()
            .find(|(_, focus, _)| *focus != FocusBehavior::None)
            .is_some_and(|(current, _, _)| *current == self.target)
        {
            state.focus();
        } else {
            state.unfocus();
        }
    }

    fn finish(&self) -> Outcome<()> {
        Outcome::Some(())
    }
}

fn focus_semantic<Message: Send + 'static>(target: SemanticFocus) -> Task<Message> {
    iced::advanced::widget::operate(FocusOperation::<Message> {
        target,
        occurrences: HashMap::new(),
        used_ids: HashSet::from([ROOT_ID]),
        frames: Vec::new(),
        marker: std::marker::PhantomData,
    })
    .discard()
}

struct SnapshotOperation<Message> {
    nodes: Vec<(NodeId, Node)>,
    root_children: Vec<NodeId>,
    frames: Vec<SemanticFrame>,
    actions: HashMap<NodeId, ActionTarget<Message>>,
    occurrences: HashMap<NodeId, u64>,
    used_ids: HashSet<NodeId>,
    focus: NodeId,
    root_label: String,
    translation: Vector,
    pending_translation: Option<Vector>,
}

struct SemanticFrame {
    node_index: Option<usize>,
    children: Vec<NodeId>,
    focus: Option<NodeId>,
    semantic_focus: Option<SemanticFocus>,
    atomic: bool,
}

fn atomic_role(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::DefaultButton
            | Role::CheckBox
            | Role::Switch
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::PasswordInput
            | Role::Slider
            | Role::ProgressIndicator
            | Role::Image
            | Role::Label
    )
}

impl<Message> Default for SnapshotOperation<Message> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            root_children: Vec::new(),
            frames: Vec::new(),
            actions: HashMap::new(),
            occurrences: HashMap::new(),
            used_ids: HashSet::from([ROOT_ID]),
            focus: ROOT_ID,
            root_label: "Ice application".into(),
            translation: Vector::ZERO,
            pending_translation: None,
        }
    }
}

impl<Message> SnapshotOperation<Message> {
    fn named(root_label: impl Into<String>) -> Self {
        Self {
            root_label: root_label.into(),
            ..Self::default()
        }
    }
}

impl<Message: Clone + Send + 'static> Operation<Snapshot<Message>> for SnapshotOperation<Message> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Snapshot<Message>>)) {
        let translation = self.pending_translation.take().unwrap_or(Vector::ZERO);
        self.translation += translation;
        operate(self);
        self.translation -= translation;
    }

    fn scrollable(
        &mut self,
        _id: Option<&widget::Id>,
        _bounds: Rectangle,
        _content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        self.pending_translation = Some(translation);
    }

    fn custom(&mut self, _id: Option<&widget::Id>, bounds: Rectangle, state: &mut dyn Any) {
        if state.downcast_mut::<SemanticEnd>().is_some() {
            let Some(frame) = self.frames.pop() else {
                return;
            };
            if let Some(index) = frame.node_index {
                self.nodes[index].1.set_children(frame.children);
            }
            return;
        }
        if let Some(state) = state.downcast_mut::<SemanticState<Message>>() {
            let Some(frame) = self.frames.last() else {
                return;
            };
            let (Some(index), Some(focus)) = (frame.node_index, frame.semantic_focus) else {
                return;
            };
            let id = self.nodes[index].0;
            if !state.semantics.disabled {
                let node = &mut self.nodes[index].1;
                if state.semantics.focus != FocusBehavior::None {
                    node.add_action(Action::Focus);
                }
                if state.semantics.activate.is_some() {
                    node.add_action(Action::Click);
                }
                self.actions.insert(
                    id,
                    ActionTarget {
                        activate: state.semantics.activate.clone(),
                        focus: (state.semantics.focus != FocusBehavior::None).then_some(focus),
                    },
                );
            }
            return;
        }
        let Some(semantics) = state.downcast_mut::<SemanticSnapshot>() else {
            return;
        };
        if self.frames.iter().any(|frame| frame.atomic) {
            self.frames.push(SemanticFrame {
                node_index: None,
                children: Vec::new(),
                focus: None,
                semantic_focus: None,
                atomic: false,
            });
            return;
        }
        let (id, focus) =
            disambiguate_semantic_id(semantics.id, &mut self.occurrences, &mut self.used_ids);
        let finite = |value: f32| {
            if value.is_nan() {
                0.0
            } else {
                f64::from(value.clamp(f32::MIN, f32::MAX))
            }
        };
        let x = finite(bounds.x) - finite(self.translation.x);
        let y = finite(bounds.y) - finite(self.translation.y);
        let mut node = Node::new(semantics.role);
        node.set_bounds(Rect {
            x0: x,
            y0: y,
            x1: x + finite(bounds.width),
            y1: y + finite(bounds.height),
        });
        if let Some(label) = &semantics.label {
            node.set_label(label.clone());
        }
        if let Some(description) = &semantics.description {
            node.set_description(description.clone());
        }
        if let Some(value) = &semantics.value {
            node.set_value(value.clone());
        }
        if let Some(checked) = semantics.checked {
            node.set_toggled(Toggled::from(checked));
        }
        if let Some(selected) = semantics.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = semantics.expanded {
            node.set_expanded(expanded);
        }
        if let Some(level) = semantics.level {
            node.set_level(level);
        }
        if let Some(row_count) = semantics.row_count {
            node.set_row_count(row_count);
        }
        if let Some(column_count) = semantics.column_count {
            node.set_column_count(column_count);
        }
        if let Some(row_index) = semantics.row_index {
            node.set_row_index(row_index);
        }
        if let Some(column_index) = semantics.column_index {
            node.set_column_index(column_index);
        }
        if let Some(sort_direction) = semantics.sort_direction {
            node.set_sort_direction(sort_direction);
        }
        if let Some(position) = semantics.position_in_set {
            node.set_position_in_set(position);
        }
        if let Some(size) = semantics.size_of_set {
            node.set_size_of_set(size);
        }
        if let Some(active_descendant) = semantics.active_descendant {
            node.set_active_descendant(active_descendant.node_id());
        }
        if semantics.disabled {
            node.set_disabled();
        }
        if semantics.focused {
            self.focus = id;
        }
        if let Some(parent) = self
            .frames
            .iter_mut()
            .rev()
            .find(|frame| frame.node_index.is_some())
        {
            parent.children.push(id);
        } else {
            self.root_children.push(id);
        }
        let node_index = self.nodes.len();
        self.nodes.push((id, node));
        self.frames.push(SemanticFrame {
            node_index: Some(node_index),
            children: Vec::new(),
            focus: (semantics.focus != FocusBehavior::None).then_some(id),
            semantic_focus: Some(focus),
            atomic: atomic_role(semantics.role),
        });
    }

    fn focusable(
        &mut self,
        _id: Option<&widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn Focusable,
    ) {
        if state.is_focused()
            && let Some(id) = self.frames.iter().rev().find_map(|frame| frame.focus)
        {
            self.focus = id;
        }
    }

    fn finish(&self) -> Outcome<Snapshot<Message>> {
        let mut root = Node::new(Role::Window);
        root.set_label(self.root_label.clone());
        root.set_children(self.root_children.clone());
        let mut nodes = Vec::with_capacity(self.nodes.len() + 1);
        nodes.push((ROOT_ID, root));
        nodes.extend(self.nodes.iter().cloned());
        Outcome::Some(Snapshot {
            update: TreeUpdate {
                nodes,
                tree: Some(Tree {
                    root: ROOT_ID,
                    toolkit_name: Some("Ice/Iced".into()),
                    toolkit_version: Some(concat!(env!("CARGO_PKG_VERSION"), "/0.14").into()),
                }),
                tree_id: TreeId::ROOT,
                focus: self.focus,
            },
            actions: self.actions.clone(),
        })
    }
}

/// Captures the live Iced widget tree as an AccessKit update.
pub fn snapshot<Message>(root_label: impl Into<String>) -> Task<Snapshot<Message>>
where
    Message: Clone + Send + 'static,
{
    iced::advanced::widget::operate(SnapshotOperation::named(root_label))
}

#[derive(Clone)]
struct ActionSubscription {
    id: u64,
    receiver: Arc<Mutex<Option<iced::futures::channel::mpsc::Receiver<ActionRequest>>>>,
}

impl PartialEq for ActionSubscription {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ActionSubscription {}

impl Hash for ActionSubscription {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

fn action_stream(
    subscription: &ActionSubscription,
) -> iced::futures::channel::mpsc::Receiver<ActionRequest> {
    subscription
        .receiver
        .lock()
        .expect("accessibility action receiver lock")
        .take()
        .unwrap_or_else(|| {
            let (_sender, receiver) =
                iced::futures::channel::mpsc::channel(ACCESSIBILITY_ACTION_BUFFER);
            receiver
        })
}

static NEXT_BRIDGE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

// Keep this configured buffer aligned with iced_winit::Proxy::MAX_SIZE.
// `futures` reserves one additional slot for the sole sender.
const ACCESSIBILITY_ACTION_BUFFER: usize = 100;

/// Whether any platform assistive technology has activated the accessibility
/// tree in this process. Flipped by the adapters' activation/deactivation
/// callbacks below; read by the generated per-update snapshot gate.
static AT_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True from the moment assistive technology asks for the tree (and, on
/// Linux, until it deactivates). Generated applications gate the per-update
/// accessibility snapshot on this: until an AT connects, walking the whole
/// widget tree after every message builds a `TreeUpdate` nobody consumes and
/// schedules an extra frame to deliver it. Test builds bypass the gate with
/// `cfg!(test)` — the Ice test harness drives the app through this tree.
pub fn accessibility_active() -> bool {
    AT_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// The native Win32 handle captured before Iced shows its first window.
#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
pub struct NativeWindow {
    id: iced::window::Id,
    hwnd: std::num::NonZeroIsize,
}

#[cfg(target_os = "windows")]
impl NativeWindow {
    pub fn id(self) -> iced::window::Id {
        self.id
    }
}

/// Captures the Win32 window handle on Iced's window-owning thread.
#[cfg(target_os = "windows")]
pub fn native_window(id: iced::window::Id) -> Task<NativeWindow> {
    iced::window::run(id, move |window| {
        let handle = window.window_handle().expect("Iced Windows window handle");
        let hwnd = match handle.as_raw() {
            iced::window::raw_window_handle::RawWindowHandle::Win32(handle) => handle.hwnd,
            _ => unreachable!("Iced uses a Win32 window on Windows"),
        };
        NativeWindow { id, hwnd }
    })
}

/// Owns the native adapter and the action map for the latest frame.
pub struct Bridge<Message> {
    id: u64,
    snapshot: Option<Snapshot<Message>>,
    receiver: Arc<Mutex<Option<iced::futures::channel::mpsc::Receiver<ActionRequest>>>>,
    latest_tree: Arc<Mutex<Option<TreeUpdate>>>,
    #[cfg(target_os = "linux")]
    adapter: Option<accesskit_unix::Adapter>,
    #[cfg(target_os = "windows")]
    adapter: Option<accesskit_windows::SubclassingAdapter>,
    #[cfg(target_os = "windows")]
    sender: Option<iced::futures::channel::mpsc::Sender<ActionRequest>>,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    window: Option<iced::window::Id>,
}

impl<Message> fmt::Debug for Bridge<Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bridge")
            .field("id", &self.id)
            .field("has_snapshot", &self.snapshot.is_some())
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
struct Activation {
    latest_tree: Arc<Mutex<Option<TreeUpdate>>>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        AT_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
        self.latest_tree
            .lock()
            .expect("accessibility tree lock")
            .clone()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
struct Actions {
    sender: iced::futures::channel::mpsc::Sender<ActionRequest>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl accesskit::ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        // A native callback cannot await without risking an event-loop cycle.
        // Preserve the bounded backlog and drop only overload or disconnects.
        let _ = self.sender.try_send(request);
    }
}

#[cfg(target_os = "linux")]
struct Deactivation;

#[cfg(target_os = "linux")]
impl accesskit::DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {
        AT_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<Message> Bridge<Message> {
    pub fn new() -> Self {
        Self::with_native_adapter(true)
    }

    /// Creates a deterministic bridge without exporting a native platform tree.
    ///
    /// This is used for daemon/multi-window applications until Iced exposes a
    /// window-scoped widget-operation boundary.
    pub fn without_native_adapter() -> Self {
        Self::with_native_adapter(false)
    }

    fn with_native_adapter(native: bool) -> Self {
        let id = NEXT_BRIDGE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (sender, receiver) = iced::futures::channel::mpsc::channel(ACCESSIBILITY_ACTION_BUFFER);
        let receiver = Arc::new(Mutex::new(Some(receiver)));
        let latest_tree = Arc::new(Mutex::new(None));
        #[cfg(target_os = "linux")]
        let adapter = native.then(|| {
            accesskit_unix::Adapter::new(
                Activation {
                    latest_tree: Arc::clone(&latest_tree),
                },
                Actions { sender },
                Deactivation,
            )
        });
        #[cfg(target_os = "windows")]
        let (adapter, sender) = (None, native.then_some(sender));
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = native;
            drop(sender);
        }

        Self {
            id,
            snapshot: None,
            receiver,
            latest_tree,
            #[cfg(target_os = "linux")]
            adapter,
            #[cfg(target_os = "windows")]
            adapter,
            #[cfg(target_os = "windows")]
            sender,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            window: None,
        }
    }

    pub fn subscription(&self) -> Subscription<ActionRequest> {
        Subscription::run_with(
            ActionSubscription {
                id: self.id,
                receiver: Arc::clone(&self.receiver),
            },
            action_stream,
        )
    }

    pub fn update(&mut self, snapshot: Snapshot<Message>) {
        // One clone out of the snapshot; the adapter clone happens inside
        // `update_if_active`'s closure, so it is paid only while assistive
        // technology is actually listening, and the activation cache takes
        // the value by move.
        let update = snapshot.update.clone();
        #[cfg(target_os = "linux")]
        if let Some(adapter) = &mut self.adapter {
            adapter.update_if_active(|| update.clone());
        }
        #[cfg(target_os = "windows")]
        if let Some(adapter) = &mut self.adapter
            && let Some(events) = adapter.update_if_active(|| update.clone())
        {
            events.raise();
        }
        *self.latest_tree.lock().expect("accessibility tree lock") = Some(update);
        self.snapshot = Some(snapshot);
    }

    /// Returns whether UI Automation owns the initial Win32 window.
    #[cfg(target_os = "windows")]
    pub fn is_attached(&self) -> bool {
        self.adapter.is_some()
    }

    /// Attaches UI Automation before the initial Win32 window is first shown.
    #[cfg(target_os = "windows")]
    pub fn attach_window(&mut self, window: NativeWindow) -> bool {
        let Some(sender) = self.sender.take() else {
            return false;
        };
        self.window = Some(window.id);
        self.adapter = Some(accesskit_windows::SubclassingAdapter::new(
            accesskit_windows::HWND(window.hwnd.get() as *mut core::ffi::c_void),
            Activation {
                latest_tree: Arc::clone(&self.latest_tree),
            },
            Actions { sender },
        ));
        true
    }

    /// Applies focus truth for the single native window owned by this bridge.
    pub fn window_event(&mut self, id: iced::window::Id, event: iced::window::Event) {
        #[cfg(target_os = "linux")]
        {
            let Some(adapter) = &mut self.adapter else {
                return;
            };
            let window = self.window.get_or_insert(id);
            if *window != id {
                return;
            }
            match event {
                iced::window::Event::Focused => adapter.update_window_focus_state(true),
                iced::window::Event::Unfocused | iced::window::Event::Closed => {
                    adapter.update_window_focus_state(false);
                }
                _ => {}
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let _ = (id, event);
        #[cfg(target_os = "windows")]
        let _ = (id, event);
    }
}

impl<Message: Clone + Send + 'static> Bridge<Message> {
    pub fn dispatch(&self, request: ActionRequest) -> Task<Message> {
        self.snapshot
            .as_ref()
            .map_or_else(Task::none, |snapshot| snapshot.dispatch(request))
    }
}

impl<Message> Default for Bridge<Message> {
    fn default() -> Self {
        Self::new()
    }
}

/// Focuses the next enabled semantic/native focus target in view-tree order.
pub fn focus_next<Message>() -> Task<Message> {
    iced::widget::operation::focus_next()
}

/// Focuses the previous enabled semantic/native focus target in view-tree order.
pub fn focus_previous<Message>() -> Task<Message> {
    iced::widget::operation::focus_previous()
}

/// Adds gradient stops after discarding malformed stops from an extern value.
pub fn add_gradient_stops(
    linear: iced::gradient::Linear,
    stops: impl IntoIterator<Item = iced::gradient::ColorStop>,
) -> iced::gradient::Linear {
    iced::gradient::Linear::new(linear.angle)
        .add_stops(linear.stops.into_iter().flatten())
        .add_stops(stops)
}

/// Converts viewer scale bounds to a finite, positive, ordered `f32` range.
pub fn viewer_scale_bounds(min: f64, max: f64) -> (f32, f32) {
    let positive = |value: f64| {
        let value = value as f32;
        if value.is_nan() {
            f32::EPSILON
        } else {
            value.clamp(f32::EPSILON, f32::MAX)
        }
    };
    let min = positive(min);
    let max = positive(max);
    (min.min(max), min.max(max))
}

/// Converts progress inputs to a finite, ordered range and bounded value.
pub fn progress_range(min: f64, max: f64, value: f64) -> (std::ops::RangeInclusive<f32>, f32) {
    let finite = |value: f64| {
        let value = value as f32;
        if value.is_nan() {
            0.0
        } else {
            value.clamp(-f32::MAX, f32::MAX)
        }
    };
    let min = finite(min);
    let max = finite(max);
    let (min, max) = (min.min(max), min.max(max));
    let value = finite(value).clamp(min, max);
    (min..=max, value)
}

/// Returns animation time remaining without letting overshooting easing produce a negative duration.
pub fn animation_remaining_millis(
    animation: &iced::Animation<bool>,
    at: iced::time::Instant,
) -> f64 {
    animation
        .clone()
        .easing(iced::animation::Easing::Linear)
        .remaining(at)
        .as_secs_f64()
        * 1_000.0
}

/// Bounds spacing so Iced can multiply it by every gap without overflowing.
pub fn bounded_spacing(spacing: f64, entries: usize) -> f32 {
    let spacing = bounded_nonnegative_f32(spacing);
    let gaps = entries.saturating_sub(1) as f32;
    if gaps <= 1.0 {
        spacing
    } else {
        spacing.min((f32::MAX / gaps).next_down())
    }
}

/// Converts padding without letting opposing sides overflow Iced's `f32` totals.
pub fn bounded_padding(top: f64, right: f64, bottom: f64, left: f64) -> Padding {
    let top = bounded_nonnegative_f32(top);
    let left = bounded_nonnegative_f32(left);
    Padding {
        top,
        right: bounded_nonnegative_f32(right).min(f32::MAX - left),
        bottom: bounded_nonnegative_f32(bottom).min(f32::MAX - top),
        left,
    }
}

/// Splits text into the units a tracked `text` renders one widget per.
///
/// Grapheme clusters, never `char`s: tracking already gives up shaping and
/// kerning, but splitting inside a cluster would separate a combining mark or
/// an emoji sequence from its base and render mojibake rather than wide text.
pub fn graphemes(value: &str) -> impl Iterator<Item = &str> {
    unicode_segmentation::UnicodeSegmentation::graphemes(value, true)
}

/// Bounds one table padding/separator metric across an entire row or column.
pub fn bounded_table_metric(value: f64, entries: usize) -> f32 {
    let terms = entries.max(1) as f32 * 3.0;
    bounded_nonnegative_f32(value).min((f32::MAX / terms).next_down())
}

fn bounded_nonnegative_f32(value: f64) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, f64::from(f32::MAX)) as f32
    }
}

/// Bounds a fill factor so Iced can sum its peers in a `u16`.
pub fn bounded_fill_length(length: impl Into<Length>, entries: usize) -> Length {
    let length = length.into();
    let max_factor = u16::try_from(entries.max(1)).map_or(0, |entries| u16::MAX / entries);
    match length {
        Length::Fill | Length::FillPortion(_) if max_factor == 0 => Length::Shrink,
        Length::FillPortion(factor) => Length::FillPortion(factor.min(max_factor)),
        length => length,
    }
}

/// Bounds one axis of an element only when its native fill-factor sum would overflow.
pub fn bounded_fill_element<'a, Message, Theme, Renderer>(
    content: Element<'a, Message, Theme, Renderer>,
    entries: usize,
    horizontal: bool,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    if entries <= 1 {
        return content;
    }
    Element::new(BoundedFill {
        content,
        entries,
        horizontal,
    })
}

struct BoundedFill<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    entries: usize,
    horizontal: bool,
}

impl<Message, Theme, Renderer> BoundedFill<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self, mut size: Size<Length>) -> Size<Length> {
        let length = if self.horizontal {
            &mut size.width
        } else {
            &mut size.height
        };
        *length = bounded_fill_length(*length, self.entries);
        size
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BoundedFill<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.size(self.content.as_widget().size())
    }

    fn size_hint(&self) -> Size<Length> {
        self.size(self.content.as_widget().size_hint())
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut widget::Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

/// Crops a screenshot after checking the invariants assumed by Iced's native crop.
pub fn crop_screenshot(
    screenshot: &iced::window::Screenshot,
    region: Rectangle<u32>,
) -> Result<iced::window::Screenshot, iced::window::screenshot::CropError> {
    use iced::window::screenshot::CropError;

    if region.width == 0 || region.height == 0 {
        return Err(CropError::Zero);
    }
    let in_bounds = region
        .x
        .checked_add(region.width)
        .is_some_and(|right| right <= screenshot.size.width)
        && region
            .y
            .checked_add(region.height)
            .is_some_and(|bottom| bottom <= screenshot.size.height);
    let expected = u128::from(screenshot.size.width) * u128::from(screenshot.size.height) * 4;
    if !in_bounds || expected != screenshot.rgba.len() as u128 {
        return Err(CropError::OutOfBounds);
    }
    screenshot.crop(region)
}

#[cfg(test)]
#[global_allocator]
static TEST_GLOBAL: &stats_alloc::StatsAlloc<std::alloc::System> =
    &stats_alloc::INSTRUMENTED_SYSTEM;

#[cfg(test)]
#[allow(clippy::let_unit_value)]
mod tests {
    use super::*;
    use iced::advanced::widget::Tree as WidgetTree;
    use iced::advanced::widget::operation;
    use iced::advanced::{Layout, Widget, layout};
    use iced::{Font, Pixels, Point, Theme};
    use iced_test::futures::futures::StreamExt;
    use iced_test::runtime::UserInterface;
    use iced_test::runtime::user_interface;

    type TestRenderer = iced_test::renderer::Renderer;
    type TestUi<'a> = UserInterface<'a, Message, Theme, TestRenderer>;
    type TestElement<'a> = Element<'a, Message, Theme, TestRenderer>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Message {
        First,
        Last,
        Next,
        Previous,
    }

    #[test]
    fn mount_boot_answers_first_sighting_and_reboots_after_prune() {
        let state: MountedComponentState<i32> = MountedComponentState::default();

        state.begin_render();
        assert!(state.mount_boot("App/Id(1)/pane".to_owned()));
        assert!(
            !state.mount_boot("App/Id(1)/pane".to_owned()),
            "one sighting per materialized instance"
        );
        state.finish_render("App/Id(1)");

        // Present again next pass: already booted.
        state.begin_render();
        assert!(!state.mount_boot("App/Id(1)/pane".to_owned()));
        state.finish_render("App/Id(1)");

        // Absent for a pass: the prune drops the booted mark with the
        // instance, so coming back boots again.
        state.begin_render();
        state.finish_render("App/Id(1)");
        state.begin_render();
        assert!(state.mount_boot("App/Id(1)/pane".to_owned()));
        state.finish_render("App/Id(1)");
    }

    #[test]
    fn mounted_component_state_prunes_scopes_and_drops_abort_handles() {
        let state = MountedComponentState::default();
        assert_eq!(state.next_generation(), 1);
        let (_, handle) = iced::Task::<()>::none().abortable();
        let observer = handle.clone();
        state
            .values_mut()
            .insert("app/search".into(), Some(handle.abort_on_drop()));
        state.values_mut().insert("app/keep".into(), None);
        state.values_mut().insert("other/search".into(), None);

        state.begin_render();
        state.mount("app/keep".into());
        state.finish_render("app");
        // Pruning lands at the start of the next pass, so a subtree still
        // being built cannot be mistaken for one that left.
        state.begin_render();

        assert!(observer.is_aborted());
        assert_eq!(state.values().len(), 2);
        assert!(state.values().contains_key("app/keep"));
        assert!(state.values().contains_key("other/search"));
        state.finish_render("app");
        state.begin_render();
        assert_eq!(state.values().len(), 1);
        assert_eq!(state.next_generation(), 2);
    }

    /// `view` returning is not the end of building the tree: a `responsive`
    /// builds its subtree during layout, so a component under one mounts after
    /// its root has finished rendering. Pruning there would drop state the
    /// pass was still about to claim — and rebuilding it every pass restarts
    /// any animation it holds, which is a highlight that never goes out.
    #[test]
    fn a_scope_mounted_after_its_root_finished_survives_the_next_pass() {
        let state = MountedComponentState::<u32>::default();

        state.begin_render();
        state.finish_render("app");
        // The deferred builder runs now, after the root reported it was done.
        state.values_mut().insert("app/deferred".into(), 7);
        state.mount("app/deferred".into());

        state.begin_render();
        state.finish_render("app");
        assert_eq!(
            state.values().get("app/deferred"),
            Some(&7),
            "a deferred mount is not a scope that left the tree"
        );

        // A scope that really does stop rendering still goes, one pass later.
        state.begin_render();
        state.finish_render("app");
        state.begin_render();
        assert!(state.values().is_empty());
    }

    #[test]
    fn safely_adds_stops_to_malformed_gradients() {
        let mut malformed = iced::gradient::Linear::new(iced::Radians(0.0));
        malformed.stops[0] = Some(iced::gradient::ColorStop {
            offset: f32::NAN,
            color: iced::Color::BLACK,
        });
        let safe = add_gradient_stops(
            malformed,
            [iced::gradient::ColorStop {
                offset: 0.5,
                color: iced::Color::WHITE,
            }],
        );

        assert_eq!(safe.stops[0].map(|stop| stop.offset), Some(0.5));
        assert!(safe.stops[1..].iter().all(Option::is_none));
    }

    #[test]
    fn normalizes_viewer_scale_bounds() {
        assert_eq!(viewer_scale_bounds(4.0, 0.5), (0.5, 4.0));
        assert_eq!(
            viewer_scale_bounds(f64::NAN, f64::INFINITY),
            (f32::EPSILON, f32::MAX)
        );
    }

    #[test]
    fn normalizes_progress_ranges() {
        assert_eq!(progress_range(10.0, -10.0, 20.0), (-10.0..=10.0, 10.0));
        assert_eq!(progress_range(f64::NAN, 1.0, f64::NAN), (0.0..=1.0, 0.0));
    }

    #[test]
    fn reads_remaining_time_with_overshooting_easing() {
        let started = iced::time::Instant::now();
        let animation = iced::Animation::new(false)
            .duration(std::time::Duration::from_secs(1))
            .easing(iced::animation::Easing::EaseOutBack)
            .go(true, started);
        let halfway = started
            .checked_add(std::time::Duration::from_millis(500))
            .expect("halfway instant");

        assert_eq!(animation_remaining_millis(&animation, halfway), 500.0);
    }

    #[test]
    fn bounds_native_spacing() {
        assert_eq!(bounded_spacing(f64::NAN, 3), 0.0);
        assert_eq!(bounded_spacing(-1.0, 3), 0.0);
        assert_eq!(bounded_spacing(8.0, 3), 8.0);
        for entries in [0, 1, 2, 3, usize::MAX] {
            let spacing = bounded_spacing(f64::MAX, entries);
            assert!((spacing * entries.saturating_sub(1) as f32).is_finite());
        }
    }

    #[test]
    fn bounds_native_padding() {
        let padding = bounded_padding(f64::MAX, f64::MAX, f64::MAX, f64::MAX);
        assert!(padding.x().is_finite());
        assert!(padding.y().is_finite());
        assert_eq!(bounded_padding(f64::NAN, -1.0, 2.0, 3.0).top, 0.0);
    }

    #[test]
    fn bounds_native_table_metrics() {
        for entries in [0, 1, 2, usize::MAX] {
            let metric = bounded_table_metric(f64::MAX, entries);
            let spacing = metric * 2.0 + metric;
            let total = spacing * entries.saturating_sub(1) as f32 + metric * 2.0;
            assert!(total.is_finite());
        }
    }

    #[test]
    fn bounds_native_fill_factors() {
        assert_eq!(
            bounded_fill_length(Length::FillPortion(u16::MAX), 2),
            Length::FillPortion(u16::MAX / 2)
        );
        assert_eq!(
            bounded_fill_length(Length::Fill, usize::from(u16::MAX) + 1),
            Length::Shrink
        );

        let column_item: TestElement<'_> = iced::widget::space()
            .height(Length::FillPortion(u16::MAX))
            .into();
        assert_eq!(
            bounded_fill_element(column_item, 2, false)
                .as_widget()
                .size()
                .height,
            Length::FillPortion(u16::MAX / 2)
        );
        let row_item: TestElement<'_> = iced::widget::space()
            .width(Length::FillPortion(u16::MAX))
            .into();
        assert_eq!(
            bounded_fill_element(row_item, 2, true)
                .as_widget()
                .size()
                .width,
            Length::FillPortion(u16::MAX / 2)
        );
    }

    #[test]
    fn safely_rejects_invalid_screenshot_crops() {
        use iced::window::screenshot::CropError;

        let one = Rectangle {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        let short = iced::window::Screenshot::new(vec![0; 4], Size::new(2, 2), 1.0);
        assert!(matches!(
            crop_screenshot(&short, one),
            Err(CropError::OutOfBounds)
        ));

        let valid = iced::window::Screenshot::new(vec![0; 4], Size::new(1, 1), 1.0);
        assert!(matches!(
            crop_screenshot(
                &valid,
                Rectangle {
                    x: u32::MAX,
                    y: 0,
                    width: 2,
                    height: 1,
                }
            ),
            Err(CropError::OutOfBounds)
        ));
        assert!(crop_screenshot(&valid, one).is_ok());
    }

    fn renderer() -> TestRenderer {
        iced_test::futures::futures::executor::block_on(<TestRenderer as renderer::Headless>::new(
            Font::DEFAULT,
            Pixels(16.0),
            None,
        ))
        .expect("headless renderer")
    }

    fn button(
        label: &'static str,
        id: StableId,
        message: Message,
        role: Role,
        disabled: bool,
    ) -> TestElement<'static> {
        let native: TestElement<'static> = iced::widget::button(iced::widget::text(label))
            .on_press_maybe((!disabled).then_some(message.clone()))
            .into();
        accessible(native, id, role)
            .label(label)
            .description(format!("{label} description"))
            .checked(role == Role::CheckBox)
            .disabled(disabled)
            .on_activate_maybe((!disabled).then_some(message))
            .into()
    }

    fn interface() -> (TestUi<'static>, TestRenderer) {
        let repeated = StableId::new("repeated-control");
        let children = vec![
            button("First", repeated, Message::First, Role::Button, false),
            button(
                "Disabled",
                StableId::new("disabled-control"),
                Message::First,
                Role::Button,
                true,
            ),
            button("Last", repeated, Message::Last, Role::CheckBox, false),
        ];
        let content: TestElement<'static> = iced::widget::Column::with_children(children).into();
        let root: TestElement<'static> =
            navigation(content, Message::Next, Message::Previous).into();
        let mut renderer = renderer();
        let ui = UserInterface::build(
            root,
            Size::new(400.0, 240.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        (ui, renderer)
    }

    fn snapshot(ui: &mut TestUi<'_>, renderer: &TestRenderer) -> Snapshot<Message> {
        let mut operation = SnapshotOperation::<Message>::named("Test application");
        ui.operate(renderer, &mut operation::black_box(&mut operation));
        match operation.finish() {
            Outcome::Some(snapshot) => snapshot,
            _ => panic!("snapshot operation did not finish"),
        }
    }

    #[test]
    #[ignore = "accessibility snapshot allocation contract run explicitly in CI"]
    fn performance_contract_snapshot_finalization_allocations() {
        const SAMPLES: usize = 256;
        const ALLOCATIONS_PER_SNAPSHOT: usize = 16;

        let (mut ui, renderer) = interface();
        let mut operation = SnapshotOperation::<Message>::named("Test application");
        ui.operate(&renderer, &mut operation::black_box(&mut operation));
        assert_eq!(operation.nodes.len(), 3);

        let finish = || std::hint::black_box(operation.finish());
        std::mem::drop(finish());
        let region = stats_alloc::Region::new(TEST_GLOBAL);
        for _ in 0..SAMPLES {
            std::mem::drop(finish());
        }
        let stats = region.change();

        eprintln!(
            "256 accessibility snapshot finalizations: allocations={} bytes={}",
            stats.allocations, stats.bytes_allocated
        );
        assert_eq!(stats.allocations, SAMPLES * ALLOCATIONS_PER_SNAPSHOT);
    }

    fn semantic_nodes(snapshot: &Snapshot<Message>) -> Vec<(NodeId, &Node)> {
        snapshot
            .update
            .nodes
            .iter()
            .filter(|(id, _)| *id != ROOT_ID)
            .map(|(id, node)| (*id, node))
            .collect()
    }

    fn focus_next(ui: &mut TestUi<'_>, renderer: &TestRenderer) {
        let mut operation: Box<dyn Operation> = Box::new(operation::focusable::focus_next::<()>());
        loop {
            ui.operate(renderer, operation.as_mut());
            match operation.finish() {
                Outcome::Chain(next) => operation = next,
                Outcome::None | Outcome::Some(()) => break,
            }
        }
    }

    #[test]
    fn builds_real_accesskit_nodes_and_disambiguates_repeated_ids() {
        let (mut ui, renderer) = interface();
        let snapshot = snapshot(&mut ui, &renderer);
        let nodes = semantic_nodes(&snapshot);

        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].1.role(), Role::Button);
        assert_eq!(nodes[0].1.label(), Some("First"));
        assert_eq!(nodes[0].1.description(), Some("First description"));
        assert!(nodes[0].1.supports_action(Action::Click));
        assert!(nodes[0].1.supports_action(Action::Focus));
        assert!(nodes[1].1.is_disabled());
        assert!(!nodes[1].1.supports_action(Action::Click));
        assert_eq!(nodes[2].1.role(), Role::CheckBox);
        assert_eq!(nodes[2].1.toggled(), Some(Toggled::True));

        assert_ne!(nodes[0].0, nodes[2].0, "repeated source IDs stay unique");
        assert_eq!(snapshot.update.focus, ROOT_ID);
        assert_eq!(snapshot.actions[&nodes[0].0].activate, Some(Message::First));
        assert_eq!(snapshot.actions[&nodes[2].0].activate, Some(Message::Last));
        assert!(!snapshot.actions.contains_key(&nodes[1].0));

        let click = snapshot.dispatch(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: nodes[0].0,
            data: None,
        });
        let mut stream = iced_test::runtime::task::into_stream(click).expect("click task");
        let action =
            iced_test::futures::futures::executor::block_on(stream.next()).expect("click output");
        assert!(matches!(
            action,
            iced_test::runtime::Action::Output(Message::First)
        ));

        let root = snapshot
            .update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_ID)
            .map(|(_, node)| node)
            .expect("root node");
        assert_eq!(root.label(), Some("Test application"));
        assert_eq!(root.children(), &[nodes[0].0, nodes[1].0, nodes[2].0]);
    }

    #[test]
    fn mapped_elements_retain_accesskit_semantics() {
        let inner: Element<'static, Option<()>, Theme, TestRenderer> = accessible(
            iced::widget::text("Chart"),
            StableId::new("mapped-chart"),
            Role::Image,
        )
        .label("Market chart")
        .into();
        let root: TestElement<'static> = inner.map(|_| Message::First);
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(320.0, 200.0),
            user_interface::Cache::default(),
            &mut renderer,
        );

        let snapshot = snapshot(&mut ui, &renderer);
        let nodes = semantic_nodes(&snapshot);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].1.role(), Role::Image);
        assert_eq!(nodes[0].1.label(), Some("Market chart"));
    }

    #[test]
    fn logical_keys_keep_node_ids_when_source_order_changes() {
        fn ids(order: [(&'static str, &'static str); 2]) -> HashMap<String, NodeId> {
            let children: Vec<TestElement<'static>> = order
                .into_iter()
                .map(|(key, label)| {
                    button(
                        label,
                        StableId::new(key),
                        Message::First,
                        Role::Button,
                        false,
                    )
                })
                .collect();
            let root: TestElement<'static> = iced::widget::Column::with_children(children).into();
            let mut renderer = renderer();
            let mut ui = UserInterface::build(
                root,
                Size::new(400.0, 160.0),
                user_interface::Cache::default(),
                &mut renderer,
            );
            semantic_nodes(&snapshot(&mut ui, &renderer))
                .into_iter()
                .map(|(id, node)| (node.label().expect("label").to_owned(), id))
                .collect()
        }

        let before = ids([("item-a", "A"), ("item-b", "B")]);
        let after = ids([("item-b", "B"), ("item-a", "A")]);
        assert_eq!(before, after);
    }

    #[test]
    fn builds_hierarchy_and_suppresses_atomic_control_descendants() {
        let group_id = StableId::new("group");
        let readable_id = StableId::new("readable");
        let button_id = StableId::new("atomic-button");
        let nested_id = StableId::new("nested-button-label");

        let readable: TestElement<'static> =
            accessible(iced::widget::text("Readable"), readable_id, Role::Label)
                .value("Readable")
                .into();
        let nested: TestElement<'static> =
            accessible(iced::widget::text("Nested"), nested_id, Role::Label)
                .value("Nested")
                .into();
        let native_button: TestElement<'static> =
            iced::widget::button(nested).on_press(Message::First).into();
        let atomic: TestElement<'static> = accessible(native_button, button_id, Role::Button)
            .label("Atomic")
            .on_activate(Message::First)
            .into();
        let children = vec![readable, atomic];
        let column: TestElement<'static> = iced::widget::Column::with_children(children).into();
        let root: TestElement<'static> =
            accessible(column, group_id, Role::GenericContainer).into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(400.0, 240.0),
            user_interface::Cache::default(),
            &mut renderer,
        );

        let snapshot = snapshot(&mut ui, &renderer);
        let node = |id| {
            snapshot
                .update
                .nodes
                .iter()
                .find(|(candidate, _)| *candidate == id)
                .map(|(_, node)| node)
                .expect("semantic node")
        };
        let root = node(ROOT_ID);
        let group = node(group_id.node_id());
        let readable = node(readable_id.node_id());
        let button = node(button_id.node_id());

        assert_eq!(root.children(), &[group_id.node_id()]);
        assert_eq!(
            group.children(),
            &[readable_id.node_id(), button_id.node_id()]
        );
        assert_eq!(readable.role(), Role::Label);
        assert_eq!(readable.value(), Some("Readable"));
        assert!(button.children().is_empty());
        assert!(
            snapshot
                .update
                .nodes
                .iter()
                .all(|(id, _)| *id != nested_id.node_id())
        );
    }

    #[test]
    fn password_nodes_never_expose_the_plaintext_value() {
        const SECRET: &str = "correct horse battery staple";
        let id = StableId::new("password");
        let native: TestElement<'static> = iced::widget::text_input("Password", SECRET).into();
        let root: TestElement<'static> = accessible(native, id, Role::PasswordInput)
            .label("Password")
            .value_maybe(None)
            .into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(400.0, 80.0),
            user_interface::Cache::default(),
            &mut renderer,
        );

        let snapshot = snapshot(&mut ui, &renderer);
        let node = semantic_nodes(&snapshot)[0].1;
        assert_eq!(node.role(), Role::PasswordInput);
        assert_eq!(node.value(), None);
        assert!(!format!("{node:?}").contains(SECRET));
    }

    #[test]
    fn tab_order_skips_disabled_and_tree_focus_follows_operations() {
        let (mut ui, renderer) = interface();
        let initial = snapshot(&mut ui, &renderer);
        let nodes = semantic_nodes(&initial);
        let first = nodes[0].0;
        let last = nodes[2].0;

        focus_next(&mut ui, &renderer);
        assert_eq!(snapshot(&mut ui, &renderer).update.focus, first);
        focus_next(&mut ui, &renderer);
        assert_eq!(snapshot(&mut ui, &renderer).update.focus, last);

        let focus = initial.dispatch(ActionRequest {
            action: Action::Focus,
            target_tree: TreeId::ROOT,
            target_node: first,
            data: None,
        });
        let mut stream = iced_test::runtime::task::into_stream(focus).expect("focus task");
        let action = iced_test::futures::futures::executor::block_on(stream.next())
            .expect("focus operation");
        let iced_test::runtime::Action::Widget(mut operation) = action else {
            panic!("focus dispatch must produce a widget operation");
        };
        ui.operate(&renderer, operation.as_mut());
        assert_eq!(snapshot(&mut ui, &renderer).update.focus, first);

        let mut disabled_focus = FocusOperation::<Message> {
            target: SemanticFocus {
                base: StableId::new("disabled-control"),
                occurrence: 0,
            },
            occurrences: HashMap::new(),
            used_ids: HashSet::from([ROOT_ID]),
            frames: Vec::new(),
            marker: std::marker::PhantomData,
        };
        ui.operate(&renderer, &mut operation::black_box(&mut disabled_focus));
        assert_eq!(snapshot(&mut ui, &renderer).update.focus, ROOT_ID);
    }

    #[test]
    fn focus_actions_follow_disambiguated_node_ids() {
        let repeated = StableId(NodeId(42));
        let colliding = StableId(duplicate_node_id(repeated.node_id(), 1));
        let nested: TestElement<'static> =
            accessible(iced::widget::text("Nested"), repeated, Role::Label)
                .label("Nested")
                .into();
        let native_atomic: TestElement<'static> =
            iced::widget::button(nested).on_press(Message::First).into();
        let atomic: TestElement<'static> =
            accessible(native_atomic, StableId(NodeId(1_000)), Role::Button)
                .label("Atomic")
                .on_activate(Message::First)
                .into();
        let children = vec![
            button("First", repeated, Message::First, Role::Button, false),
            button("Collision", colliding, Message::First, Role::Button, false),
            atomic,
            button("Last", repeated, Message::Last, Role::Button, false),
        ];
        let root: TestElement<'static> = iced::widget::Column::with_children(children).into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(400.0, 240.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let initial = snapshot(&mut ui, &renderer);
        let target = semantic_nodes(&initial)
            .into_iter()
            .find(|(_, node)| node.label() == Some("Last"))
            .map(|(id, _)| id)
            .expect("last node");

        let focus = initial.dispatch(ActionRequest {
            action: Action::Focus,
            target_tree: TreeId::ROOT,
            target_node: target,
            data: None,
        });
        let mut stream = iced_test::runtime::task::into_stream(focus).expect("focus task");
        let action = iced_test::futures::futures::executor::block_on(stream.next())
            .expect("focus operation");
        let iced_test::runtime::Action::Widget(mut operation) = action else {
            panic!("focus dispatch must produce a widget operation");
        };
        ui.operate(&renderer, operation.as_mut());

        assert_eq!(snapshot(&mut ui, &renderer).update.focus, target);
    }

    #[test]
    fn tab_and_keyboard_activation_emit_exactly_one_message() {
        let (mut ui, mut renderer) = interface();
        let mut messages = Vec::new();
        let events = iced_test::simulator::tap_key(key::Named::Tab, None).collect::<Vec<_>>();
        let _ = ui.update(
            &events,
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, [Message::Next]);

        messages.clear();
        focus_next(&mut ui, &renderer);
        let events = iced_test::simulator::tap_key(key::Named::Enter, None).collect::<Vec<_>>();
        let _ = ui.update(
            &events,
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, [Message::First]);

        messages.clear();
        focus_next(&mut ui, &renderer);
        let events = iced_test::simulator::tap_key(key::Named::Space, None).collect::<Vec<_>>();
        let _ = ui.update(
            &events,
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, [Message::Last]);
    }

    #[test]
    fn pointer_focus_has_one_owner() {
        let (mut ui, mut renderer) = interface();
        let initial = snapshot(&mut ui, &renderer);
        let nodes = semantic_nodes(&initial);
        let first = nodes[0].0;
        let last = nodes[2].0;
        let centers = [nodes[0].1, nodes[2].1].map(|node| {
            let bounds = node.bounds().expect("semantic bounds");
            Point::new(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            )
        });
        drop(nodes);

        for (point, expected) in centers.into_iter().zip([first, last]) {
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
            assert_eq!(snapshot(&mut ui, &renderer).update.focus, expected);
        }
    }

    #[test]
    fn scroll_translation_reaches_semantics_and_touch_focus() {
        let target = StableId::new("scrolled-control");
        let scroll_id: widget::Id = "scrolled-semantics".into();
        let spacer: TestElement<'static> = iced::widget::Space::new().height(100.0).into();
        let control: TestElement<'static> = accessible(
            iced::widget::Space::new().width(10.0).height(20.0),
            target,
            Role::Button,
        )
        .into();
        let content: TestElement<'static> =
            iced::widget::Column::with_children(vec![spacer, control]).into();
        let root: TestElement<'static> = iced::widget::scrollable(content)
            .id(scroll_id.clone())
            .width(20.0)
            .height(50.0)
            .into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(20.0, 50.0),
            user_interface::Cache::default(),
            &mut renderer,
        );

        let before = semantic_nodes(&snapshot(&mut ui, &renderer))[0]
            .1
            .bounds()
            .expect("semantic bounds");
        assert_eq!(before.y0, 100.0);

        let mut scroll = operation::scrollable::scroll_to::<()>(
            scroll_id,
            operation::scrollable::AbsoluteOffset {
                x: None,
                y: Some(100.0),
            },
        );
        ui.operate(&renderer, &mut operation::black_box(&mut scroll));

        let after = semantic_nodes(&snapshot(&mut ui, &renderer))[0]
            .1
            .bounds()
            .expect("semantic bounds");
        assert_eq!(after.y0, 30.0);

        let point = Point::new(5.0, 40.0);
        let mut messages = Vec::new();
        let _ = ui.update(
            &[Event::Touch(iced::touch::Event::FingerPressed {
                id: iced::touch::Finger(0),
                position: point,
            })],
            mouse::Cursor::Available(point),
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(snapshot(&mut ui, &renderer).update.focus, target.node_id());
    }

    #[test]
    fn keeps_exported_accessibility_bounds_finite() {
        let mut operation = SnapshotOperation::<Message>::named("Test application");
        operation.translation = Vector::new(-f32::MAX, -f32::MAX);
        let mut state: SemanticState<Message> = SemanticState {
            semantics: Semantics::new(StableId::new("extreme-bounds"), Role::Button),
            focus_visible: false,
        };
        let bounds = Rectangle::new(
            Point::new(f32::MAX, f32::MAX),
            Size::new(f32::MAX, f32::MAX),
        );
        operation.custom(None, bounds, &mut state.semantics.snapshot);
        operation.custom(None, bounds, &mut state);
        operation.custom(None, Rectangle::default(), &mut SemanticEnd);
        let Outcome::Some(snapshot) = operation.finish() else {
            panic!("snapshot operation did not finish");
        };
        let bounds = semantic_nodes(&snapshot)[0]
            .1
            .bounds()
            .expect("semantic bounds");

        assert!(
            [bounds.x0, bounds.y0, bounds.x1, bounds.y1]
                .into_iter()
                .all(f64::is_finite)
        );
    }

    #[derive(Default)]
    struct OperationCounts {
        focusable: usize,
        text_input: usize,
    }

    impl Operation for OperationCounts {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
            operate(self);
        }

        fn focusable(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _state: &mut dyn Focusable,
        ) {
            self.focusable += 1;
        }

        fn text_input(
            &mut self,
            _id: Option<&widget::Id>,
            _bounds: Rectangle,
            _state: &mut dyn TextInput,
        ) {
            self.text_input += 1;
        }
    }

    #[test]
    fn disabled_inputs_preserve_text_operations_but_filter_focus() {
        let id = StableId::new("disabled-input");
        let native: TestElement<'static> = iced::widget::text_input("", "value")
            .id(id.widget_id())
            .into();
        let root: TestElement<'static> = accessible(native, id, Role::TextInput)
            .disabled(true)
            .focus_id(id.widget_id())
            .into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(400.0, 80.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut counts = OperationCounts::default();

        ui.operate(&renderer, &mut operation::black_box(&mut counts));

        assert_eq!(counts.text_input, 1);
        assert_eq!(counts.focusable, 0);
        assert_eq!(snapshot(&mut ui, &renderer).update.focus, ROOT_ID);
    }

    struct CapturesTab;

    impl Widget<Message, Theme, TestRenderer> for CapturesTab {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fixed(80.0), Length::Fixed(30.0))
        }

        fn layout(
            &mut self,
            _tree: &mut WidgetTree,
            _renderer: &TestRenderer,
            _limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(Size::new(80.0, 30.0))
        }

        fn draw(
            &self,
            _tree: &WidgetTree,
            _renderer: &mut TestRenderer,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
        }

        fn update(
            &mut self,
            _tree: &mut WidgetTree,
            event: &Event,
            _layout: Layout<'_>,
            _cursor: mouse::Cursor,
            _renderer: &TestRenderer,
            _clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            _viewport: &Rectangle,
        ) {
            if matches!(
                event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(key::Named::Tab),
                    ..
                })
            ) {
                shell.publish(Message::First);
                shell.capture_event();
            }
        }
    }

    #[test]
    fn navigation_defers_to_children_and_ignores_modified_tab() {
        let child: TestElement<'static> = Element::new(CapturesTab);
        let root: TestElement<'static> = navigation(child, Message::Next, Message::Previous).into();
        let mut renderer = renderer();
        let mut ui = UserInterface::build(
            root,
            Size::new(400.0, 80.0),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let mut messages = Vec::new();
        let event = iced_test::simulator::press_key(key::Named::Tab, None);
        let _ = ui.update(
            &[event],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert_eq!(messages, [Message::First]);

        let passive: TestElement<'static> = iced::widget::Space::new().into();
        let root: TestElement<'static> =
            navigation(passive, Message::Next, Message::Previous).into();
        let cache = ui.into_cache();
        let mut ui = UserInterface::build(root, Size::new(400.0, 80.0), cache, &mut renderer);
        messages.clear();
        let Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key,
            location,
            repeat,
            text,
            ..
        }) = iced_test::simulator::press_key(key::Named::Tab, None)
        else {
            unreachable!()
        };
        let event = Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key,
            location,
            modifiers: keyboard::Modifiers::CTRL,
            repeat,
            text,
        });
        let _ = ui.update(
            &[event],
            mouse::Cursor::Unavailable,
            &mut renderer,
            &mut iced::advanced::clipboard::Null,
            &mut messages,
        );
        assert!(messages.is_empty());
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
            panic!("test leaf never allocates images");
        }
    }

    struct Leaf;

    impl Widget<Message, (), RecordingRenderer> for Leaf {
        fn size(&self) -> Size<Length> {
            Size::new(Length::Fixed(80.0), Length::Fixed(30.0))
        }

        fn layout(
            &mut self,
            _tree: &mut WidgetTree,
            _renderer: &RecordingRenderer,
            _limits: &layout::Limits,
        ) -> layout::Node {
            layout::Node::new(Size::new(80.0, 30.0))
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
    fn keyboard_focused_wrapper_draws_a_visible_outline() {
        let id = StableId::new("focus-ring");
        let leaf: Element<'_, Message, (), RecordingRenderer> = Element::new(Leaf);
        let mut element: Element<'_, Message, (), RecordingRenderer> =
            accessible(leaf, id, Role::Button).label("Focusable").into();
        let mut tree = WidgetTree::new(&element);
        let mut renderer = RecordingRenderer::default();
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
        );
        let mut focus = operation::focusable::focus::<()>(id.widget_id());
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
            &Rectangle::with_size(Size::new(100.0, 100.0)),
        );

        assert_eq!(renderer.quads.len(), 1);
        assert_eq!(renderer.quads[0].border.width, 2.0);
        assert_eq!(renderer.quads[0].border.color, iced::Color::WHITE);
    }

    #[test]
    fn pointer_focused_wrapper_does_not_draw_an_outline() {
        let id = StableId::new("pointer-focus");
        let leaf: Element<'_, Message, (), RecordingRenderer> = Element::new(Leaf);
        let mut element: Element<'_, Message, (), RecordingRenderer> =
            accessible(leaf, id, Role::Button).label("Focusable").into();
        let mut tree = WidgetTree::new(&element);
        let mut renderer = RecordingRenderer::default();
        let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);

        element.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(40.0, 15.0)),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        drop(shell);

        let state = tree.state.downcast_ref::<SemanticState<Message>>();
        assert!(state.semantics.focused);
        assert!(!state.focus_visible);

        element.as_widget().draw(
            &tree,
            &mut renderer,
            &(),
            &renderer::Style {
                text_color: iced::Color::WHITE,
            },
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );

        assert!(renderer.quads.is_empty());
    }

    #[test]
    fn styled_focus_ring_follows_the_focus_origin() {
        let id = StableId::new("styled-focus-ring");
        let ring_color = iced::Color::from_rgb(0.2, 0.4, 1.0);
        let build = || -> Element<'_, Message, (), RecordingRenderer> {
            let leaf: Element<'_, Message, (), RecordingRenderer> = Element::new(Leaf);
            accessible(leaf, id, Role::Button)
                .label("Styled")
                .focus_ring(ring_color, 8.0)
                .into()
        };
        let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
        let style = renderer::Style {
            text_color: iced::Color::WHITE,
        };

        // Pointer-acquired focus paints no ring at all.
        let mut element = build();
        let mut tree = WidgetTree::new(&element);
        let mut renderer = RecordingRenderer::default();
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        element.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(40.0, 15.0)),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        drop(shell);
        assert!(
            tree.state
                .downcast_ref::<SemanticState<Message>>()
                .semantics
                .focused
        );
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &(),
            &style,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        assert!(renderer.quads.is_empty());

        // Keyboard-acquired focus paints the recipe's ring, not the default.
        let mut element = build();
        let mut tree = WidgetTree::new(&element);
        let mut renderer = RecordingRenderer::default();
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut focus = operation::focusable::focus::<()>(id.widget_id());
        element
            .as_widget_mut()
            .operate(&mut tree, Layout::new(&node), &renderer, &mut focus);
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &(),
            &style,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        assert_eq!(renderer.quads.len(), 1);
        assert_eq!(renderer.quads[0].border.color, ring_color);
        assert_eq!(renderer.quads[0].border.width, 2.0);
        assert_eq!(renderer.quads[0].border.radius, 8.0.into());
    }

    #[test]
    fn key_press_after_pointer_focus_restores_the_outline() {
        let id = StableId::new("pointer-then-key");
        let leaf: Element<'_, Message, (), RecordingRenderer> = Element::new(Leaf);
        let mut element: Element<'_, Message, (), RecordingRenderer> =
            accessible(leaf, id, Role::Button).label("Focusable").into();
        let mut tree = WidgetTree::new(&element);
        let mut renderer = RecordingRenderer::default();
        let viewport = Rectangle::with_size(Size::new(100.0, 100.0));
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut messages = Vec::new();
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            iced_test::simulator::press_key(key::Named::Enter, None),
        ] {
            let mut shell = Shell::new(&mut messages);
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(40.0, 15.0)),
                &renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
        }

        let state = tree.state.downcast_ref::<SemanticState<Message>>();
        assert!(state.semantics.focused);
        assert!(state.focus_visible);

        element.as_widget().draw(
            &tree,
            &mut renderer,
            &(),
            &renderer::Style {
                text_color: iced::Color::WHITE,
            },
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );

        assert_eq!(renderer.quads.len(), 1);
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn native_adapter_action_handler_routes_requests_to_iced() {
        let (sender, mut receiver) =
            iced::futures::channel::mpsc::channel(ACCESSIBILITY_ACTION_BUFFER);
        let mut handler = Actions { sender };
        let request = ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: StableId::new("native-action").node_id(),
            data: None,
        };

        accesskit::ActionHandler::do_action(&mut handler, request.clone());

        let routed = iced_test::futures::futures::executor::block_on(receiver.next());
        assert_eq!(routed, Some(request));
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn native_adapter_action_handler_bounds_pending_requests() {
        let (sender, mut receiver) =
            iced::futures::channel::mpsc::channel(ACCESSIBILITY_ACTION_BUFFER);
        let mut handler = Actions { sender };

        for node in 1..=ACCESSIBILITY_ACTION_BUFFER + 2 {
            accesskit::ActionHandler::do_action(
                &mut handler,
                ActionRequest {
                    action: Action::Click,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(node as u64),
                    data: None,
                },
            );
        }

        let routed = (0..=ACCESSIBILITY_ACTION_BUFFER)
            .map(|_| receiver.try_recv().expect("buffered accessibility action"))
            .map(|request| request.target_node)
            .collect::<Vec<_>>();
        assert_eq!(
            routed,
            (1..=ACCESSIBILITY_ACTION_BUFFER + 1)
                .map(|node| NodeId(node as u64))
                .collect::<Vec<_>>(),
            "accepted accessibility actions must keep FIFO order"
        );
        assert!(
            receiver.try_recv().is_err(),
            "the native callback must not retain more than the configured buffer plus its sender slot"
        );

        drop(receiver);
        accesskit::ActionHandler::do_action(
            &mut handler,
            ActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(u64::MAX),
                data: None,
            },
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_bridge_defers_adapter_until_a_window_handle_arrives() {
        let bridge = Bridge::<Message>::new();
        assert!(bridge.adapter.is_none());
        assert!(bridge.sender.is_some());
        assert!(!bridge.is_attached());

        let disabled = Bridge::<Message>::without_native_adapter();
        assert!(disabled.adapter.is_none());
        assert!(disabled.sender.is_none());
        assert!(!disabled.is_attached());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_bridge_activation_uses_latest_tree_and_one_window() {
        let (mut ui, renderer) = interface();
        let snapshot = snapshot(&mut ui, &renderer);
        let mut bridge = Bridge::<Message>::new();
        bridge.update(snapshot.clone());
        let mut activation = Activation {
            latest_tree: Arc::clone(&bridge.latest_tree),
        };

        let initial = accesskit::ActivationHandler::request_initial_tree(&mut activation)
            .expect("latest tree");
        assert_eq!(initial.nodes, snapshot.update.nodes);
        assert_eq!(initial.focus, snapshot.update.focus);

        // Activation is also what opens the generated per-update snapshot
        // gate, and deactivation is what closes it. Asserted here — in the
        // one test that calls the activation handler — so no parallel test
        // races the process-wide flag.
        assert!(accessibility_active());
        accesskit::DeactivationHandler::deactivate_accessibility(&mut Deactivation);
        assert!(!accessibility_active());

        let first = iced::window::Id::unique();
        let second = iced::window::Id::unique();
        bridge.window_event(first, iced::window::Event::Focused);
        bridge.window_event(second, iced::window::Event::Unfocused);
        assert_eq!(bridge.window, Some(first));

        let disabled = Bridge::<Message>::without_native_adapter();
        assert!(disabled.adapter.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires an isolated Linux AT-SPI bus; run scripts/a11y-smoke.sh"]
    fn linux_native_atspi_exports_tree_and_routes_action() {
        use std::process::Command;
        use std::thread;
        use std::time::Duration;

        fn gdbus(args: &[&str]) -> Result<String, String> {
            let output = Command::new("gdbus")
                .args(args)
                .output()
                .map_err(|error| format!("failed to run gdbus: {error}"))?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).into_owned())
            }
        }

        fn quoted_values(output: &str) -> Vec<&str> {
            output.split('\'').skip(1).step_by(2).collect()
        }

        fn set_enabled(enabled: bool) -> Result<(), String> {
            gdbus(&[
                "call",
                "--session",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.freedesktop.DBus.Properties.Set",
                "org.a11y.Status",
                "IsEnabled",
                if enabled { "<true>" } else { "<false>" },
            ])
            .map(|_| ())
        }

        struct StatusGuard(bool);
        impl Drop for StatusGuard {
            fn drop(&mut self) {
                let _ = set_enabled(self.0);
            }
        }

        let address = std::env::var("AT_SPI_BUS_ADDRESS")
            .expect("run this gate through scripts/a11y-smoke.sh");

        let status = gdbus(&[
            "call",
            "--session",
            "--dest",
            "org.a11y.Bus",
            "--object-path",
            "/org/a11y/bus",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.a11y.Status",
            "IsEnabled",
        ])
        .expect("query org.a11y.Status.IsEnabled");
        let initially_enabled = status.contains("true");
        let _guard = StatusGuard(initially_enabled);

        let label = format!("ui-lang-native-smoke-{}", std::process::id());
        let id = StableId::new(&label).node_id();
        let mut root = Node::new(Role::Window);
        root.set_label(label.clone());
        root.set_children(vec![id]);
        let mut button = Node::new(Role::Button);
        button.set_label(label.clone());
        button.add_action(Action::Click);
        let snapshot = Snapshot {
            update: TreeUpdate {
                nodes: vec![(ROOT_ID, root), (id, button)],
                tree: Some(Tree {
                    root: ROOT_ID,
                    toolkit_name: Some("Ice native smoke".into()),
                    toolkit_version: Some(env!("CARGO_PKG_VERSION").into()),
                }),
                tree_id: TreeId::ROOT,
                focus: ROOT_ID,
            },
            actions: HashMap::from([(
                id,
                ActionTarget {
                    activate: Some(Message::First),
                    focus: None,
                },
            )]),
        };
        let mut bridge = Bridge::new();
        bridge.update(snapshot);
        bridge.window_event(iced::window::Id::unique(), iced::window::Event::Focused);
        let mut receiver = bridge
            .receiver
            .lock()
            .expect("native action receiver")
            .take()
            .expect("native action receiver owner");

        thread::sleep(Duration::from_millis(250));
        if initially_enabled {
            set_enabled(false).expect("temporarily disable accessibility");
            thread::sleep(Duration::from_millis(100));
        }
        set_enabled(true).expect("enable accessibility for native smoke");
        let mut exported = None;
        let mut diagnostic = String::new();
        for _ in 0..50 {
            let Ok(applications) = gdbus(&[
                "call",
                "--address",
                &address,
                "--dest",
                "org.a11y.atspi.Registry",
                "--object-path",
                "/org/a11y/atspi/accessible/root",
                "--method",
                "org.a11y.atspi.Accessible.GetChildren",
            ]) else {
                thread::sleep(Duration::from_millis(100));
                continue;
            };
            diagnostic = format!("applications={applications}");
            for bus in quoted_values(&applications)
                .into_iter()
                .filter(|value| value.starts_with(':'))
            {
                let Ok(roots) = gdbus(&[
                    "call",
                    "--address",
                    &address,
                    "--dest",
                    bus,
                    "--object-path",
                    "/org/a11y/atspi/accessible/root",
                    "--method",
                    "org.a11y.atspi.Accessible.GetChildren",
                ]) else {
                    continue;
                };
                diagnostic.push_str(&format!(" bus={bus} roots={roots}"));
                for root_path in quoted_values(&roots)
                    .into_iter()
                    .filter(|value| value.starts_with('/'))
                {
                    let Ok(name) = gdbus(&[
                        "call",
                        "--address",
                        &address,
                        "--dest",
                        bus,
                        "--object-path",
                        root_path,
                        "--method",
                        "org.freedesktop.DBus.Properties.Get",
                        "org.a11y.atspi.Accessible",
                        "Name",
                    ]) else {
                        continue;
                    };
                    diagnostic.push_str(&format!(" path={root_path} name={name}"));
                    if !name.contains(&label) {
                        continue;
                    }
                    let Ok(children) = gdbus(&[
                        "call",
                        "--address",
                        &address,
                        "--dest",
                        bus,
                        "--object-path",
                        root_path,
                        "--method",
                        "org.a11y.atspi.Accessible.GetChildren",
                    ]) else {
                        continue;
                    };
                    let Some(path) = quoted_values(&children)
                        .into_iter()
                        .find(|value| value.starts_with('/'))
                    else {
                        continue;
                    };
                    exported = Some((bus.to_owned(), path.to_owned()));
                    break;
                }
                if exported.is_some() {
                    break;
                }
            }
            if exported.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let (bus, path) = exported.unwrap_or_else(|| {
            panic!("AccessKit tree was not exported through AT-SPI; {diagnostic}")
        });
        gdbus(&[
            "call",
            "--address",
            &address,
            "--dest",
            &bus,
            "--object-path",
            &path,
            "--method",
            "org.a11y.atspi.Action.DoAction",
            "0",
        ])
        .expect("invoke exported AT-SPI action");

        let mut routed = None;
        for _ in 0..20 {
            if let Ok(request) = receiver.try_recv() {
                routed = Some(request);
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let request = routed.expect("native AT-SPI action was not routed to Iced");
        assert_eq!(request.action, Action::Click);
        assert_eq!(request.target_node, id);
    }
}
