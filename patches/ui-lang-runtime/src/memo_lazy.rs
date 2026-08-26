//! [`iced::widget::Lazy`] with LAYOUT memoization and unmount PARKING.
//!
//! iced's `Lazy` caches the built element while its dependency hash is
//! unchanged — but every layout pass still re-walks the cached subtree. In a
//! deep list (a chat stream of `lazy` rows) that walk dominates: profiling the
//! ducktape console showed ~150µs of layout per cached row per pass, so one
//! keystroke anywhere in the window re-laid a 150-row stream for ~23ms while
//! `view` itself cost ~1ms. This fork memoizes the layout node beside the
//! cached element: while the dependency hash AND the incoming `Limits` are
//! unchanged, `layout()` answers without touching the subtree.
//!
//! The memoized node never leaves this widget. `layout()` hands the parent one
//! childless node — this widget's own box — and every later phase re-roots the
//! stored subtree at wherever the parent put that box, which is the same
//! `Layout` the subtree would have produced had it been returned. Handing back
//! a copy instead cost a deep clone of every node under the boundary on every
//! frame, ~42ns a node, which on a dense screen was most of what the memo had
//! just saved.
//!
//! Both caches used to die with the widget tree: a `match` arm switch tears
//! down the inactive screen, and re-entering rebuilt and re-shaped every lazy
//! row from nothing (~10ms per chat row, on the UI thread, in one frame). To
//! survive unmounting, the child's widget [`Tree`] lives inside this widget's
//! state rather than in `Tree::children`, and dropping that state parks the
//! whole subtree — element, memoized layout, widget state — in a
//! thread-local lot keyed by `(codegen site, reconciliation scope, dependency
//! hash)`. The lot keeps only the latest dependency revision at each concrete
//! site/scope. A remount with that key reclaims it wholesale: no `view` call,
//! no layout, no re-shaping. A row whose content changed while unmounted
//! simply misses and cold-builds; a 64-bit hash collision is the same accepted
//! risk as `Lazy`'s rebuild skip, scoped per mounted expression so distinct
//! rows never share entries.
//!
//! Soundness rides on the contract `Lazy` already imposes: the content is a
//! pure function of the dependency, so anything that changes what the subtree
//! would lay out must change the dependency hash. Widget-internal state that
//! affects layout (a text editor's wrapped lines, say) would already be stale
//! under `Lazy`'s ELEMENT caching — such widgets don't belong under a lazy
//! boundary, memoized or not. Bounds changes arrive as different `Limits` and
//! recompute.
//!
//! Everything else is a verbatim fork of `iced_widget::lazy` (0.14.2),
//! ouroboros overlay machinery included.
#![allow(clippy::type_complexity)]

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::{self, Widget};
use iced::advanced::{self, Clipboard, Shell, overlay};
use iced::{Element, Event, Length, Rectangle, Size, Vector, mouse};

use ouroboros::self_referencing;
use rustc_hash::{FxHashMap, FxHasher};
use std::any::Any;
use std::cell::RefCell;
use std::hash::{Hash, Hasher as _};
use std::rc::Rc;

/// One unmounted lazy subtree, waiting under `(site, dependency hash)` for a
/// same-content remount to reclaim it.
struct Parked<Message: 'static, Theme: 'static, Renderer: 'static> {
    element: Element<'static, Message, Theme, Renderer>,
    layout: MemoLayout,
    tree: Tree,
}

/// The memoized layout as its own non-generic type, so an [`Operation`] can
/// reach it through `custom` and drop it: a virtual window below this memo
/// re-aims from the scrollable's real translation, and the relayout that
/// follows must recompute THROUGH the memo — a `(dependency, limits)` key
/// cannot see that the translation moved.
pub(crate) struct MemoLayout(pub(crate) Option<(layout::Limits, layout::Node)>);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MemoSite {
    expression: u64,
    scope: u64,
}

impl MemoSite {
    fn new(expression: u64, scope: &impl Hash) -> Self {
        let mut hasher = FxHasher::default();
        scope.hash(&mut hasher);
        Self {
            expression,
            scope: hasher.finish(),
        }
    }
}

struct Parking {
    entries: FxHashMap<MemoSite, (u64, Box<dyn Any>)>,
    /// Park order, oldest first, kept in exact parity with `entries`.
    order: std::collections::VecDeque<MemoSite>,
}

const PARKING_CAP: usize = 1024;

thread_local! {
    static PARKING: RefCell<Parking> = RefCell::new(Parking {
        entries: FxHashMap::default(),
        order: std::collections::VecDeque::new(),
    });
}

/// `try_with` because thread teardown drops parked subtrees whose nested lazy
/// state parks again; both here and in [`park`] any dropping of foreign
/// subtrees happens OUTSIDE the borrow, since those drops re-enter the lot.
fn reclaim(site: MemoSite, hash: u64) -> Option<Box<dyn Any>> {
    let taken = PARKING
        .try_with(|parking| {
            let mut parking = parking.borrow_mut();
            let Parking { entries, order } = &mut *parking;
            let parked = entries.remove(&site);
            if let Some(position) = order.iter().position(|parked_site| *parked_site == site) {
                let _ = order.remove(position);
            }
            match parked {
                Some((parked_hash, subtree)) if parked_hash == hash => (Some(subtree), None),
                Some((_, subtree)) => (None, Some(subtree)),
                None => (None, None),
            }
        })
        .ok();
    let (matched, stale) = taken?;
    drop(stale);
    matched
}

fn park(site: MemoSite, hash: u64, subtree: Box<dyn Any>) {
    let displaced = PARKING
        .try_with(|parking| {
            let mut parking = parking.borrow_mut();
            let Parking { entries, order } = &mut *parking;
            let stale = entries.remove(&site).map(|(_, subtree)| subtree);
            if let Some(position) = order.iter().position(|parked_site| *parked_site == site) {
                let _ = order.remove(position);
            }
            let evicted = if entries.len() >= PARKING_CAP {
                order
                    .pop_front()
                    .and_then(|oldest| entries.remove(&oldest))
                    .map(|(_, subtree)| subtree)
            } else {
                None
            };
            let replaced = entries.insert(site, (hash, subtree));
            debug_assert!(replaced.is_none());
            order.push_back(site);
            (stale, evicted, replaced)
        })
        .ok();
    drop(displaced);
}

/// How many unmounted subtrees the lot is holding, for probes that price it.
/// The lot is per-thread and capped at [`PARKING_CAP`].
pub fn parked_subtrees() -> usize {
    PARKING
        .try_with(|parking| parking.borrow().entries.len())
        .unwrap_or(0)
}

/// A widget that only rebuilds — and only re-lays — its contents when
/// necessary, and whose subtree survives unmounting via the parking lot.
pub struct MemoLazy<'a, Message, Theme, Renderer, Dependency, View> {
    dependency: Dependency,
    site: MemoSite,
    view: Box<dyn Fn(&Dependency) -> View + 'a>,
    element: RefCell<Option<Rc<RefCell<Option<Element<'static, Message, Theme, Renderer>>>>>>,
}

/// Creates a [`MemoLazy`] widget with the given dependency and view builder.
///
/// `site` identifies the `lazy` expression that produced this widget and
/// `scope` identifies its concrete reconciled mount. Parked subtrees use both,
/// so separate rows stay independent while stale revisions of one row replace
/// each other.
pub fn memo_lazy<'a, Message, Theme, Renderer, Dependency, View>(
    dependency: Dependency,
    view: impl Fn(&Dependency) -> View + 'a,
    site: u64,
    scope: impl Hash,
) -> MemoLazy<'a, Message, Theme, Renderer, Dependency, View>
where
    Dependency: Hash + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    MemoLazy {
        dependency,
        site: MemoSite::new(site, &scope),
        view: Box::new(view),
        element: RefCell::new(None),
    }
}

impl<'a, Message, Theme, Renderer, Dependency, View>
    MemoLazy<'a, Message, Theme, Renderer, Dependency, View>
where
    Dependency: Hash + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    fn with_element<T>(&self, f: impl FnOnce(&Element<'_, Message, Theme, Renderer>) -> T) -> T {
        f(self
            .element
            .borrow()
            .as_ref()
            .unwrap()
            .borrow()
            .as_ref()
            .unwrap())
    }

    fn with_element_mut<T>(
        &self,
        f: impl FnOnce(&mut Element<'_, Message, Theme, Renderer>) -> T,
    ) -> T {
        f(self
            .element
            .borrow()
            .as_ref()
            .unwrap()
            .borrow_mut()
            .as_mut()
            .unwrap())
    }
}

struct Internal<Message: 'static, Theme: 'static, Renderer: 'static> {
    element: Rc<RefCell<Option<Element<'static, Message, Theme, Renderer>>>>,
    hash: u64,
    site: MemoSite,
    /// The memoized layout for the current `hash`: the `Limits` the node was
    /// computed under and the node itself. `None` after a rebuild.
    layout: MemoLayout,
    /// The child's widget state. Owned here — not in `Tree::children` — so an
    /// unmount can park it wholesale and a remount can bring it back.
    tree: Tree,
}

/// What the parent is handed: this widget's own box, with nothing under it.
/// The subtree stays in the memo and is lent out by [`child_layout`], which is
/// what makes a cache hit cost one node instead of a deep clone of every node
/// under this `lazy` — and spares the parent freeing them all again.
fn shallow(node: &layout::Node) -> layout::Node {
    layout::Node::new(node.size()).move_to(node.bounds().position())
}

/// The memoized subtree, rooted where the parent put the box [`shallow`] gave
/// it. Every phase after `layout` receives the parent's `Layout`, which
/// describes only that box; the child needs its own nodes, positioned exactly
/// as if they had been returned along with it.
fn child_layout<'a>(
    memo: &'a Option<(layout::Limits, layout::Node)>,
    layout: Layout<'_>,
) -> Layout<'a> {
    let node = &memo
        .as_ref()
        .expect("`layout` runs before any phase that walks the tree")
        .1;
    let root = node.bounds();
    let position = layout.position();

    Layout::with_offset(Vector::new(position.x - root.x, position.y - root.y), node)
}

impl<Message, Theme, Renderer> Drop for Internal<Message, Theme, Renderer> {
    fn drop(&mut self) {
        // Dropping the state IS unmounting: park the subtree so a
        // same-content remount rehydrates instead of re-shaping. An element
        // already taken (overlay teardown edge) just skips parking.
        let Some(element) = self.element.borrow_mut().take() else {
            return;
        };
        park(
            self.site,
            self.hash,
            Box::new(Parked {
                element,
                layout: MemoLayout(self.layout.0.take()),
                tree: std::mem::replace(&mut self.tree, Tree::empty()),
            }),
        );
    }
}

impl<'a, Message, Theme, Renderer, Dependency, View> Widget<Message, Theme, Renderer>
    for MemoLazy<'a, Message, Theme, Renderer, Dependency, View>
where
    View: Into<Element<'static, Message, Theme, Renderer>> + 'static,
    Dependency: Hash + 'a,
    Message: 'static,
    Theme: 'static,
    Renderer: advanced::Renderer + 'static,
{
    fn tag(&self) -> tree::Tag {
        struct Tag<T>(T);
        tree::Tag::of::<Tag<View>>()
    }

    fn state(&self) -> tree::State {
        let hash = {
            let mut hasher = FxHasher::default();
            self.dependency.hash(&mut hasher);

            hasher.finish()
        };

        if let Some(parked) = reclaim(self.site, hash)
            && let Ok(parked) = parked.downcast::<Parked<Message, Theme, Renderer>>()
        {
            let Parked {
                element,
                layout,
                tree,
            } = *parked;
            let element = Rc::new(RefCell::new(Some(element)));
            (*self.element.borrow_mut()) = Some(element.clone());

            return tree::State::new(Internal::<Message, Theme, Renderer> {
                element,
                hash,
                site: self.site,
                layout,
                tree,
            });
        }

        let element = Rc::new(RefCell::new(Some((self.view)(&self.dependency).into())));
        let tree = {
            let element = element.borrow();
            Tree::new(element.as_ref().unwrap().as_widget())
        };

        (*self.element.borrow_mut()) = Some(element.clone());

        tree::State::new(Internal::<Message, Theme, Renderer> {
            element,
            hash,
            site: self.site,
            layout: MemoLayout(None),
            tree,
        })
    }

    fn children(&self) -> Vec<Tree> {
        Vec::new()
    }

    fn diff(&self, tree: &mut Tree) {
        let current = tree
            .state
            .downcast_mut::<Internal<Message, Theme, Renderer>>();

        let new_hash = {
            let mut hasher = FxHasher::default();
            self.dependency.hash(&mut hasher);

            hasher.finish()
        };

        if current.hash != new_hash {
            current.hash = new_hash;
            current.layout = MemoLayout(None);

            let element: Element<'static, Message, Theme, Renderer> =
                (self.view)(&self.dependency).into();
            current.tree.diff(element.as_widget());
            current.element = Rc::new(RefCell::new(Some(element)));
        }

        (*self.element.borrow_mut()) = Some(current.element.clone());
    }

    fn size(&self) -> Size<Length> {
        self.with_element(|element| element.as_widget().size())
    }

    fn size_hint(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree
            .state
            .downcast_mut::<Internal<Message, Theme, Renderer>>();

        if let Some((cached_limits, node)) = &state.layout.0
            && cached_limits == limits
        {
            return shallow(node);
        }

        let node = self.with_element_mut(|element| {
            element
                .as_widget_mut()
                .layout(&mut state.tree, renderer, limits)
        });
        let handed_up = shallow(&node);
        state.layout = MemoLayout(Some((*limits, node)));
        handed_up
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let Internal {
            layout: memo,
            tree: child,
            ..
        } = tree
            .state
            .downcast_mut::<Internal<Message, Theme, Renderer>>();
        // The memoized layout is state an operation may need to drop: a
        // virtual window below this memo re-aims from the scrollable's real
        // translation, and the relayout that follows must recompute through
        // here — the `(dependency, limits)` key cannot see a moved
        // translation. Unknown operations ignore the downcast.
        operation.custom(None, layout.bounds(), memo);
        // An operation that just dropped the memoized layout left nothing
        // coherent to walk below; the next layout pass rebuilds it, and any
        // nested memo keeps its own still-valid cache for that pass to reuse.
        if memo.0.is_none() {
            return;
        }
        let layout = child_layout(&memo.0, layout);
        self.with_element_mut(|element| {
            element
                .as_widget_mut()
                .operate(child, layout, renderer, operation);
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
        let Internal {
            layout: memo,
            tree: child,
            ..
        } = tree
            .state
            .downcast_mut::<Internal<Message, Theme, Renderer>>();
        let layout = child_layout(&memo.0, layout);
        self.with_element_mut(|element| {
            element.as_widget_mut().update(
                child, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let Internal {
            layout: memo,
            tree: child,
            ..
        } = tree
            .state
            .downcast_ref::<Internal<Message, Theme, Renderer>>();
        let layout = child_layout(&memo.0, layout);
        self.with_element(|element| {
            element
                .as_widget()
                .mouse_interaction(child, layout, cursor, viewport, renderer)
        })
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
        let Internal {
            layout: memo,
            tree: child,
            ..
        } = tree
            .state
            .downcast_ref::<Internal<Message, Theme, Renderer>>();
        let layout = child_layout(&memo.0, layout);
        self.with_element(|element| {
            element
                .as_widget()
                .draw(child, renderer, theme, style, layout, cursor, viewport);
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let Internal {
            layout: memo,
            tree: child,
            ..
        } = tree
            .state
            .downcast_mut::<Internal<Message, Theme, Renderer>>();
        let layout = child_layout(&memo.0, layout);
        let overlay = InnerBuilder {
            cell: self.element.borrow().as_ref().unwrap().clone(),
            element: self
                .element
                .borrow()
                .as_ref()
                .unwrap()
                .borrow_mut()
                .take()
                .unwrap(),
            tree: child,
            layout,
            overlay_builder: |element, tree, layout| {
                element
                    .as_widget_mut()
                    .overlay(tree, *layout, renderer, viewport, translation)
                    .map(|overlay| RefCell::new(overlay::Nested::new(overlay)))
            },
        }
        .build();

        #[allow(clippy::redundant_closure_for_method_calls)]
        if overlay.with_overlay(|overlay| overlay.is_some()) {
            Some(overlay::Element::new(Box::new(Overlay(Some(overlay)))))
        } else {
            let heads = overlay.into_heads();

            *self.element.borrow().as_ref().unwrap().borrow_mut() = Some(heads.element);

            None
        }
    }
}

#[self_referencing]
struct Inner<'a, Message: 'a, Theme: 'a, Renderer: 'a> {
    cell: Rc<RefCell<Option<Element<'static, Message, Theme, Renderer>>>>,
    element: Element<'static, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    layout: Layout<'a>,

    #[borrows(mut element, mut tree, layout)]
    #[not_covariant]
    overlay: Option<RefCell<overlay::Nested<'this, Message, Theme, Renderer>>>,
}

struct Overlay<'a, Message, Theme, Renderer>(Option<Inner<'a, Message, Theme, Renderer>>);

impl<Message, Theme, Renderer> Drop for Overlay<'_, Message, Theme, Renderer> {
    fn drop(&mut self) {
        let heads = self.0.take().unwrap().into_heads();
        (*heads.cell.borrow_mut()) = Some(heads.element);
    }
}

impl<Message, Theme, Renderer> Overlay<'_, Message, Theme, Renderer> {
    fn with_overlay_maybe<T>(
        &self,
        f: impl FnOnce(&mut overlay::Nested<'_, Message, Theme, Renderer>) -> T,
    ) -> Option<T> {
        self.0
            .as_ref()
            .unwrap()
            .with_overlay(|overlay| overlay.as_ref().map(|nested| (f)(&mut nested.borrow_mut())))
    }

    fn with_overlay_mut_maybe<T>(
        &mut self,
        f: impl FnOnce(&mut overlay::Nested<'_, Message, Theme, Renderer>) -> T,
    ) -> Option<T> {
        self.0
            .as_mut()
            .unwrap()
            .with_overlay_mut(|overlay| overlay.as_mut().map(|nested| (f)(nested.get_mut())))
    }
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        self.with_overlay_maybe(|overlay| overlay.layout(renderer, bounds))
            .unwrap_or_default()
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let _ = self.with_overlay_maybe(|overlay| {
            overlay.draw(renderer, theme, style, layout, cursor);
        });
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.with_overlay_maybe(|overlay| overlay.mouse_interaction(layout, cursor, renderer))
            .unwrap_or_default()
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let _ = self.with_overlay_mut_maybe(|overlay| {
            overlay.update(event, layout, cursor, renderer, clipboard, shell);
        });
    }
}

impl<'a, Message, Theme, Renderer, Dependency, View>
    From<MemoLazy<'a, Message, Theme, Renderer, Dependency, View>>
    for Element<'a, Message, Theme, Renderer>
where
    View: Into<Element<'static, Message, Theme, Renderer>> + 'static,
    Renderer: advanced::Renderer + 'static,
    Message: 'static,
    Theme: 'static,
    Dependency: Hash + 'a,
{
    fn from(lazy: MemoLazy<'a, Message, Theme, Renderer, Dependency, View>) -> Self {
        Self::new(lazy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    type TestLazy<'a> = MemoLazy<
        'a,
        (),
        iced::Theme,
        iced::Renderer,
        i32,
        Element<'static, (), iced::Theme, iced::Renderer>,
    >;

    fn widget(dependency: i32, site: u64) -> TestLazy<'static> {
        memo_lazy(
            dependency,
            |value: &i32| Element::from(iced::widget::text(value.to_string())),
            site,
            dependency,
        )
    }

    fn counting_widget(dependency: i32, site: u64, builds: Rc<Cell<u32>>) -> TestLazy<'static> {
        counting_widget_in(dependency, site, dependency, builds)
    }

    fn counting_widget_in(
        dependency: i32,
        site: u64,
        scope: i32,
        builds: Rc<Cell<u32>>,
    ) -> TestLazy<'static> {
        memo_lazy(
            dependency,
            move |value: &i32| {
                builds.set(builds.get() + 1);
                Element::from(iced::widget::text(value.to_string()))
            },
            site,
            scope,
        )
    }

    fn internal(tree: &mut Tree) -> &mut Internal<(), iced::Theme, iced::Renderer> {
        tree.state.downcast_mut()
    }

    /// The memo's whole contract: a same-dependency diff keeps the cached
    /// layout, a changed dependency drops it (the element rebuild would make
    /// any kept node stale).
    #[test]
    fn diff_keeps_the_layout_memo_only_while_the_dependency_holds() {
        let same = widget(7, 0);
        let mut tree = Tree::new(&same as &dyn Widget<(), iced::Theme, iced::Renderer>);
        assert!(internal(&mut tree).layout.0.is_none());

        let memoized = layout::Node::new(Size::new(10.0, 10.0));
        internal(&mut tree).layout = MemoLayout(Some((layout::Limits::NONE, memoized.clone())));

        same.diff(&mut tree);
        assert!(
            internal(&mut tree).layout.0.is_some(),
            "an unchanged dependency must keep the memoized layout"
        );

        let changed = widget(8, 0);
        changed.diff(&mut tree);
        assert!(
            internal(&mut tree).layout.0.is_none(),
            "a changed dependency rebuilds the element — a kept node would be stale"
        );
    }

    /// The parking contract: unmounting (dropping the state tree) parks the
    /// subtree, and a remount with the same site and dependency reclaims it —
    /// no `view` call, memoized layout intact.
    #[test]
    fn a_matching_remount_reclaims_the_parked_subtree_without_rebuilding() {
        let builds = Rc::new(Cell::new(0));

        let first = counting_widget(7, 800, builds.clone());
        let mut tree = Tree::new(&first as &dyn Widget<(), iced::Theme, iced::Renderer>);
        assert_eq!(builds.get(), 1);
        internal(&mut tree).layout = MemoLayout(Some((
            layout::Limits::NONE,
            layout::Node::new(Size::new(10.0, 10.0)),
        )));
        drop(tree);

        let second = counting_widget(7, 800, builds.clone());
        let mut tree = Tree::new(&second as &dyn Widget<(), iced::Theme, iced::Renderer>);
        assert_eq!(
            builds.get(),
            1,
            "a same-content remount must reclaim the parked element, not rebuild it"
        );
        assert!(
            internal(&mut tree).layout.0.is_some(),
            "the memoized layout must survive the unmount"
        );
    }

    /// A row whose content changed while unmounted misses the lot and
    /// cold-builds — a reclaimed subtree would show stale content.
    #[test]
    fn a_changed_dependency_remount_cold_builds() {
        let builds = Rc::new(Cell::new(0));

        let first = counting_widget_in(7, 810, 0, builds.clone());
        let mut tree = Tree::new(&first as &dyn Widget<(), iced::Theme, iced::Renderer>);
        internal(&mut tree).layout = MemoLayout(Some((
            layout::Limits::NONE,
            layout::Node::new(Size::new(10.0, 10.0)),
        )));
        drop(tree);

        let second = counting_widget_in(8, 810, 0, builds.clone());
        let mut tree = Tree::new(&second as &dyn Widget<(), iced::Theme, iced::Renderer>);
        assert_eq!(
            builds.get(),
            2,
            "changed content must miss the lot and rebuild"
        );
        assert!(internal(&mut tree).layout.0.is_none());
    }

    /// Evicting a parked subtree drops nested lazy state, which parks itself
    /// — that drop must happen outside the lot's borrow or it panics on
    /// re-entry.
    #[test]
    fn evicting_a_parked_subtree_reparks_its_nested_lazy_state() {
        let outer: TestLazy<'static> = memo_lazy(
            1,
            |value: &i32| {
                let inner: TestLazy<'static> = memo_lazy(
                    *value,
                    |value: &i32| Element::from(iced::widget::text(value.to_string())),
                    901,
                    *value,
                );
                Element::from(inner)
            },
            900,
            1,
        );
        drop(Tree::new(
            &outer as &dyn Widget<(), iced::Theme, iced::Renderer>,
        ));

        for index in 0..(PARKING_CAP as i32 + 8) {
            let filler = widget(index, 902);
            drop(Tree::new(
                &filler as &dyn Widget<(), iced::Theme, iced::Renderer>,
            ));
        }
    }

    /// Rows that are not distinct hash alike, so a list of them unmounts into
    /// one key and every park after the first displaces a live subtree. That
    /// displaced subtree parks its own nested lazy state, so it has the same
    /// re-entry hazard eviction has — and unlike eviction it needs no full lot
    /// to reach: two rows are enough.
    #[test]
    fn parking_a_key_twice_reparks_the_subtree_it_displaces() {
        let nested = || -> TestLazy<'static> {
            memo_lazy(
                1,
                |value: &i32| {
                    let inner: TestLazy<'static> = memo_lazy(
                        *value,
                        |value: &i32| Element::from(iced::widget::text(value.to_string())),
                        911,
                        *value,
                    );
                    Element::from(inner)
                },
                910,
                1,
            )
        };

        // Both mounted before either unmounts, so the second park displaces
        // the first rather than reclaiming it.
        let (first, second) = (nested(), nested());
        let first = Tree::new(&first as &dyn Widget<(), iced::Theme, iced::Renderer>);
        let second = Tree::new(&second as &dyn Widget<(), iced::Theme, iced::Renderer>);
        drop(first);
        drop(second);
    }

    #[test]
    fn parking_a_new_revision_replaces_the_old_revision_at_the_same_site() {
        let site = MemoSite::new(990, &"timeline");
        park(site, 1, Box::new(1_u8));
        park(site, 2, Box::new(2_u8));

        assert!(reclaim(site, 2).is_some());
        assert!(
            reclaim(site, 1).is_none(),
            "a concrete memo site must not retain stale dependency revisions"
        );
    }

    #[test]
    fn parking_keeps_different_scopes_of_one_expression_distinct() {
        let first_row = MemoSite::new(991, &"row/1");
        let second_row = MemoSite::new(991, &"row/2");
        park(first_row, 1, Box::new(1_u8));
        park(second_row, 2, Box::new(2_u8));

        assert!(reclaim(first_row, 1).is_some());
        assert!(reclaim(second_row, 2).is_some());
    }

    /// Past the cap the lot drops the park that has been waiting longest, so
    /// the rows a user just left keep their place while the screen they left
    /// two screens ago loses its.
    #[test]
    fn parking_past_the_cap_evicts_the_oldest_park() {
        let builds = Rc::new(Cell::new(0));

        // The two oldest parks, in order.
        for value in 0..2 {
            let row = counting_widget(value, 930, builds.clone());
            drop(Tree::new(
                &row as &dyn Widget<(), iced::Theme, iced::Renderer>,
            ));
        }
        // Fill the lot to the brim behind them.
        for value in 2..(PARKING_CAP as i32) {
            let filler = widget(value, 930);
            drop(Tree::new(
                &filler as &dyn Widget<(), iced::Theme, iced::Renderer>,
            ));
        }
        assert_eq!(builds.get(), 2);

        // One park too many, which is one eviction.
        let overflow = widget(-1, 930);
        drop(Tree::new(
            &overflow as &dyn Widget<(), iced::Theme, iced::Renderer>,
        ));

        let second = counting_widget(1, 930, builds.clone());
        drop(Tree::new(
            &second as &dyn Widget<(), iced::Theme, iced::Renderer>,
        ));
        assert_eq!(
            builds.get(),
            2,
            "the second-oldest park was not the one evicted, so it reclaims"
        );

        let first = counting_widget(0, 930, builds.clone());
        drop(Tree::new(
            &first as &dyn Widget<(), iced::Theme, iced::Renderer>,
        ));
        assert_eq!(
            builds.get(),
            3,
            "the oldest park is the one the cap dropped, so it cold-builds"
        );
    }

    /// The lot holds one entry per concrete `(site, scope)` that has unmounted.
    #[test]
    fn a_list_of_repeated_rows_parks_one_entry_per_distinct_row() {
        let before = parked_subtrees();

        let rows: Vec<TestLazy<'static>> = (0..20).map(|index| widget(index % 3, 920)).collect();
        let trees: Vec<Tree> = rows
            .iter()
            .map(|row| Tree::new(row as &dyn Widget<(), iced::Theme, iced::Renderer>))
            .collect();
        drop(trees);

        assert_eq!(
            parked_subtrees() - before,
            3,
            "twenty rows over three distinct scopes park three subtrees, not twenty"
        );
    }

    /// The memoized subtree never travels back to the parent, so the `Layout`
    /// the later phases build over it has to land exactly where a returned
    /// subtree would have: a row the parent placed at (37, 11) draws its cells
    /// there, each still offset by its own place inside the row.
    #[test]
    fn a_cache_hit_lays_the_memoized_subtree_over_the_box_the_parent_placed() {
        use iced::advanced::renderer::Headless as _;

        type Probed = MemoLazy<
            'static,
            (),
            iced::Theme,
            iced_test::renderer::Renderer,
            i32,
            Element<'static, (), iced::Theme, iced_test::renderer::Renderer>,
        >;

        /// A fixed-size cell that records the bounds it is handed to draw in.
        struct Drawn {
            size: Size,
            seen: Rc<RefCell<Vec<Rectangle>>>,
        }

        impl Widget<(), iced::Theme, iced_test::renderer::Renderer> for Drawn {
            fn size(&self) -> Size<Length> {
                Size::new(
                    Length::Fixed(self.size.width),
                    Length::Fixed(self.size.height),
                )
            }

            fn layout(
                &mut self,
                _tree: &mut Tree,
                _renderer: &iced_test::renderer::Renderer,
                _limits: &layout::Limits,
            ) -> layout::Node {
                layout::Node::new(self.size)
            }

            fn draw(
                &self,
                _tree: &Tree,
                _renderer: &mut iced_test::renderer::Renderer,
                _theme: &iced::Theme,
                _style: &renderer::Style,
                layout: Layout<'_>,
                _cursor: mouse::Cursor,
                _viewport: &Rectangle,
            ) {
                self.seen.borrow_mut().push(layout.bounds());
            }
        }

        let seen: Rc<RefCell<Vec<Rectangle>>> = Rc::new(RefCell::new(Vec::new()));
        let cells = seen.clone();
        let mut lazy: Probed = memo_lazy(
            1,
            move |_: &i32| {
                Element::from(iced::widget::Column::with_children(vec![
                    Element::new(Drawn {
                        size: Size::new(40.0, 10.0),
                        seen: cells.clone(),
                    }),
                    Element::new(Drawn {
                        size: Size::new(40.0, 12.0),
                        seen: cells.clone(),
                    }),
                ]))
            },
            940,
            "draw-probe",
        );

        let mut renderer = iced_test::futures::futures::executor::block_on(
            iced_test::renderer::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), None),
        )
        .expect("headless renderer");
        let mut tree =
            Tree::new(&lazy as &dyn Widget<(), iced::Theme, iced_test::renderer::Renderer>);
        let limits = layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));

        let cold = lazy.layout(&mut tree, &renderer, &limits);
        assert!(
            cold.children().is_empty(),
            "the parent is handed this widget's box, never the subtree under it"
        );

        // The same limits again is the memo hit — the path that used to answer
        // with a deep clone of the subtree.
        let hit = lazy.layout(&mut tree, &renderer, &limits);
        assert_eq!(hit.size(), cold.size(), "a hit measures what a miss did");

        let placed = hit.move_to(iced::Point::new(37.0, 11.0));
        lazy.draw(
            &tree,
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style {
                text_color: iced::Color::BLACK,
            },
            Layout::new(&placed),
            mouse::Cursor::Unavailable,
            &Rectangle::with_size(Size::new(200.0, 200.0)),
        );

        assert_eq!(
            *seen.borrow(),
            vec![
                Rectangle {
                    x: 37.0,
                    y: 11.0,
                    width: 40.0,
                    height: 10.0,
                },
                Rectangle {
                    x: 37.0,
                    y: 21.0,
                    width: 40.0,
                    height: 12.0,
                },
            ],
            "every cell draws where the parent put the row, plus its own offset"
        );
    }
}
